//! Immutable release facts for one sponsored Solana Pyth push account.
//!
//! A sponsored push account is not a caller-owned Receiver update. It is
//! latest-value storage written by a separate push-oracle program through the
//! Receiver. This release therefore binds both upgradeable programs and the
//! exact feed PDA. It deliberately carries no Router, VAA, submitter, or
//! `PostUpdate` fact.

/// Canonical release preimage magic.
pub const PYTH_SPONSORED_PUSH_RELEASE_V1_MAGIC: [u8; 8] = *b"DCLTPSP1";
/// Canonical release schema version.
pub const PYTH_SPONSORED_PUSH_RELEASE_V1_SCHEMA_VERSION: u16 = 1;
/// Exact canonical release preimage width.
pub const PYTH_SPONSORED_PUSH_RELEASE_V1_ENCODED_LEN: usize = 592;

/// `sha256("dclutch/schema/pyth-sponsored-push-release-v1")`.
pub const PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1: [u8; 32] = [
    0xc8, 0x43, 0xf5, 0x34, 0x61, 0x6a, 0x9b, 0xca, 0xd0, 0x9c, 0x58, 0x9e, 0xbf, 0xa8, 0x0a, 0x31,
    0x63, 0x58, 0x4e, 0x5e, 0xf1, 0xcb, 0xf3, 0xfc, 0xbd, 0x6b, 0x13, 0x56, 0x8c, 0x7a, 0xe1, 0x82,
];

/// `sha256("dclutch/pyth-sponsored-push-provider-family/v1")`.
pub const PYTH_SPONSORED_PUSH_PROVIDER_FAMILY_ID_V1: [u8; 32] = [
    0x3f, 0x2c, 0x4b, 0x6d, 0x26, 0x16, 0x4b, 0xc9, 0x7e, 0x4d, 0x84, 0x49, 0x98, 0xea, 0x36, 0x48,
    0x4b, 0xaa, 0x93, 0xd6, 0x35, 0x57, 0x3b, 0x5f, 0x17, 0x65, 0x9b, 0x4b, 0xdf, 0x88, 0x28, 0x07,
];

/// `sha256("dclutch/pyth-sponsored-push-transport/v1")`.
pub const PYTH_SPONSORED_PUSH_TRANSPORT_PROFILE_ID_V1: [u8; 32] = [
    0xe4, 0x76, 0x03, 0xfd, 0x1b, 0x4d, 0x73, 0x1c, 0xb4, 0x5b, 0xfb, 0xc5, 0xe7, 0xc0, 0xa1, 0x01,
    0x20, 0x32, 0x5f, 0x9a, 0xf7, 0xc5, 0xa2, 0x16, 0xff, 0xfd, 0x90, 0xca, 0x4a, 0x4a, 0x16, 0x65,
];

/// `sha256("dclutch/pyth-sponsored-push-adapter/v1")`.
pub const PYTH_SPONSORED_PUSH_ADAPTER_ID_V1: [u8; 32] = [
    0x83, 0xc3, 0x50, 0xdb, 0x08, 0x39, 0xea, 0x25, 0x7c, 0x3c, 0x04, 0xfd, 0x84, 0xbe, 0x3e, 0x6f,
    0x47, 0xbb, 0x7e, 0xea, 0x8d, 0x29, 0xf2, 0x84, 0x99, 0x39, 0x7c, 0xec, 0x26, 0x62, 0xbf, 0x77,
];

/// A required nonzero field in a sponsored-push release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SponsoredPushReleaseFieldV1 {
    /// Solana genesis hash.
    ClusterId,
    /// Receiver program.
    ReceiverProgram,
    /// Receiver ProgramData.
    ReceiverProgramData,
    /// Receiver ELF digest.
    ReceiverAbiId,
    /// Receiver Loader-v3 upgrade authority.
    ReceiverUpgradeAuthority,
    /// Push-oracle program.
    PushOracleProgram,
    /// Push-oracle ProgramData.
    PushOracleProgramData,
    /// Push-oracle ELF digest.
    PushOracleAbiId,
    /// Push-oracle Loader-v3 upgrade authority.
    PushOracleUpgradeAuthority,
    /// Exact legacy Receiver Config PDA.
    ReceiverConfig,
    /// SHA-256 of the exact legacy Receiver Config body.
    ReceiverConfigDigest,
    /// Exact sponsored price account.
    PriceAccount,
    /// Exact feed identifier.
    FeedId,
    /// Exact price-account codec.
    PriceUpdateCodecId,
    /// dClutch adapter identity.
    AdapterId,
    /// Provider family identity.
    ProviderFamilyId,
    /// Transport profile identity.
    TransportProfileId,
}

/// Sponsored-push release refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PythSponsoredPushReleaseV1Error {
    /// Input had the wrong exact width.
    InvalidEncodedLength {
        /// Observed width.
        actual: usize,
    },
    /// Magic differed.
    InvalidMagic,
    /// Schema version differed.
    UnsupportedSchemaVersion {
        /// Observed version.
        actual: u16,
    },
    /// A required identity was zero.
    ZeroField {
        /// Rejected field.
        field: SponsoredPushReleaseFieldV1,
    },
    /// A deployment slot was zero.
    ZeroDeploymentSlot,
    /// Canonical reserved bytes were nonzero.
    NonzeroReserved,
}

/// Result alias for sponsored-push release operations.
pub type PythSponsoredPushReleaseV1Result<T> =
    core::result::Result<T, PythSponsoredPushReleaseV1Error>;

/// Construction input for one immutable sponsored-push release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythSponsoredPushReleaseV1Input {
    /// Solana genesis hash.
    pub cluster_id: [u8; 32],
    /// Receiver program.
    pub receiver_program: [u8; 32],
    /// Receiver ProgramData.
    pub receiver_programdata: [u8; 32],
    /// Receiver ELF digest.
    pub receiver_abi_id: [u8; 32],
    /// Exact current Receiver Loader-v3 upgrade authority.
    pub receiver_upgrade_authority: [u8; 32],
    /// Push-oracle program.
    pub push_oracle_program: [u8; 32],
    /// Push-oracle ProgramData.
    pub push_oracle_programdata: [u8; 32],
    /// Push-oracle ELF digest.
    pub push_oracle_abi_id: [u8; 32],
    /// Exact current push-oracle Loader-v3 upgrade authority.
    pub push_oracle_upgrade_authority: [u8; 32],
    /// Exact legacy Receiver Config PDA.
    pub receiver_config: [u8; 32],
    /// SHA-256 of the exact legacy Receiver Config body.
    pub receiver_config_digest: [u8; 32],
    /// Exact sponsored price account.
    pub price_account: [u8; 32],
    /// Exact feed identifier.
    pub feed_id: [u8; 32],
    /// Exact price-account codec identity.
    pub price_update_codec_id: [u8; 32],
    /// dClutch adapter identity.
    pub adapter_id: [u8; 32],
    /// Provider family identity.
    pub provider_family_id: [u8; 32],
    /// Transport profile identity.
    pub transport_profile_id: [u8; 32],
    /// Receiver ProgramData deployment slot.
    pub receiver_deployment_slot: u64,
    /// Push-oracle ProgramData deployment slot.
    pub push_oracle_deployment_slot: u64,
    /// Push PDA shard.
    pub shard: u16,
    /// Push PDA bump.
    pub feed_account_bump: u8,
    /// Unix activation time.
    pub activation_time: i64,
}

/// Fixed-layout release for a sponsored, latest-value Pyth push account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythSponsoredPushReleaseV1 {
    input: PythSponsoredPushReleaseV1Input,
}

impl PythSponsoredPushReleaseV1 {
    /// Validate and construct a release.
    pub fn new(input: PythSponsoredPushReleaseV1Input) -> PythSponsoredPushReleaseV1Result<Self> {
        for (value, field) in [
            (input.cluster_id, SponsoredPushReleaseFieldV1::ClusterId),
            (
                input.receiver_program,
                SponsoredPushReleaseFieldV1::ReceiverProgram,
            ),
            (
                input.receiver_programdata,
                SponsoredPushReleaseFieldV1::ReceiverProgramData,
            ),
            (
                input.receiver_abi_id,
                SponsoredPushReleaseFieldV1::ReceiverAbiId,
            ),
            (
                input.receiver_upgrade_authority,
                SponsoredPushReleaseFieldV1::ReceiverUpgradeAuthority,
            ),
            (
                input.push_oracle_program,
                SponsoredPushReleaseFieldV1::PushOracleProgram,
            ),
            (
                input.push_oracle_programdata,
                SponsoredPushReleaseFieldV1::PushOracleProgramData,
            ),
            (
                input.push_oracle_abi_id,
                SponsoredPushReleaseFieldV1::PushOracleAbiId,
            ),
            (
                input.push_oracle_upgrade_authority,
                SponsoredPushReleaseFieldV1::PushOracleUpgradeAuthority,
            ),
            (
                input.receiver_config,
                SponsoredPushReleaseFieldV1::ReceiverConfig,
            ),
            (
                input.receiver_config_digest,
                SponsoredPushReleaseFieldV1::ReceiverConfigDigest,
            ),
            (
                input.price_account,
                SponsoredPushReleaseFieldV1::PriceAccount,
            ),
            (input.feed_id, SponsoredPushReleaseFieldV1::FeedId),
            (
                input.price_update_codec_id,
                SponsoredPushReleaseFieldV1::PriceUpdateCodecId,
            ),
            (input.adapter_id, SponsoredPushReleaseFieldV1::AdapterId),
            (
                input.provider_family_id,
                SponsoredPushReleaseFieldV1::ProviderFamilyId,
            ),
            (
                input.transport_profile_id,
                SponsoredPushReleaseFieldV1::TransportProfileId,
            ),
        ] {
            if value == [0; 32] {
                return Err(PythSponsoredPushReleaseV1Error::ZeroField { field });
            }
        }
        if input.receiver_deployment_slot == 0 || input.push_oracle_deployment_slot == 0 {
            return Err(PythSponsoredPushReleaseV1Error::ZeroDeploymentSlot);
        }
        Ok(Self { input })
    }

    /// Decode one exact canonical release.
    pub fn decode(bytes: &[u8]) -> PythSponsoredPushReleaseV1Result<Self> {
        if bytes.len() != PYTH_SPONSORED_PUSH_RELEASE_V1_ENCODED_LEN {
            return Err(PythSponsoredPushReleaseV1Error::InvalidEncodedLength {
                actual: bytes.len(),
            });
        }
        if bytes.get(..8) != Some(&PYTH_SPONSORED_PUSH_RELEASE_V1_MAGIC) {
            return Err(PythSponsoredPushReleaseV1Error::InvalidMagic);
        }
        let version = u16_at(bytes, 8)?;
        if version != PYTH_SPONSORED_PUSH_RELEASE_V1_SCHEMA_VERSION {
            return Err(PythSponsoredPushReleaseV1Error::UnsupportedSchemaVersion {
                actual: version,
            });
        }
        if bytes.get(10..16).is_none_or(|reserved| reserved != [0; 6])
            || bytes
                .get(579..584)
                .is_none_or(|reserved| reserved != [0; 5])
        {
            return Err(PythSponsoredPushReleaseV1Error::NonzeroReserved);
        }
        Self::new(PythSponsoredPushReleaseV1Input {
            cluster_id: array_at(bytes, 16)?,
            receiver_program: array_at(bytes, 48)?,
            receiver_programdata: array_at(bytes, 80)?,
            receiver_abi_id: array_at(bytes, 112)?,
            push_oracle_program: array_at(bytes, 144)?,
            push_oracle_programdata: array_at(bytes, 176)?,
            push_oracle_abi_id: array_at(bytes, 208)?,
            price_account: array_at(bytes, 240)?,
            feed_id: array_at(bytes, 272)?,
            price_update_codec_id: array_at(bytes, 304)?,
            adapter_id: array_at(bytes, 336)?,
            provider_family_id: array_at(bytes, 368)?,
            transport_profile_id: array_at(bytes, 400)?,
            receiver_upgrade_authority: array_at(bytes, 432)?,
            push_oracle_upgrade_authority: array_at(bytes, 464)?,
            receiver_config: array_at(bytes, 496)?,
            receiver_config_digest: array_at(bytes, 528)?,
            receiver_deployment_slot: u64_at(bytes, 560)?,
            push_oracle_deployment_slot: u64_at(bytes, 568)?,
            shard: u16_at(bytes, 576)?,
            feed_account_bump: byte_at(bytes, 578)?,
            activation_time: i64_at(bytes, 584)?,
        })
    }

    /// Encode the exact canonical release preimage.
    pub fn to_bytes(self) -> [u8; PYTH_SPONSORED_PUSH_RELEASE_V1_ENCODED_LEN] {
        let mut out = [0_u8; PYTH_SPONSORED_PUSH_RELEASE_V1_ENCODED_LEN];
        out[..8].copy_from_slice(&PYTH_SPONSORED_PUSH_RELEASE_V1_MAGIC);
        out[8..10].copy_from_slice(&PYTH_SPONSORED_PUSH_RELEASE_V1_SCHEMA_VERSION.to_le_bytes());
        for (offset, value) in [
            (16, self.input.cluster_id),
            (48, self.input.receiver_program),
            (80, self.input.receiver_programdata),
            (112, self.input.receiver_abi_id),
            (144, self.input.push_oracle_program),
            (176, self.input.push_oracle_programdata),
            (208, self.input.push_oracle_abi_id),
            (240, self.input.price_account),
            (272, self.input.feed_id),
            (304, self.input.price_update_codec_id),
            (336, self.input.adapter_id),
            (368, self.input.provider_family_id),
            (400, self.input.transport_profile_id),
            (432, self.input.receiver_upgrade_authority),
            (464, self.input.push_oracle_upgrade_authority),
            (496, self.input.receiver_config),
            (528, self.input.receiver_config_digest),
        ] {
            out[offset..offset + 32].copy_from_slice(&value);
        }
        out[560..568].copy_from_slice(&self.input.receiver_deployment_slot.to_le_bytes());
        out[568..576].copy_from_slice(&self.input.push_oracle_deployment_slot.to_le_bytes());
        out[576..578].copy_from_slice(&self.input.shard.to_le_bytes());
        out[578] = self.input.feed_account_bump;
        out[584..592].copy_from_slice(&self.input.activation_time.to_le_bytes());
        out
    }

    /// Cluster genesis hash.
    pub const fn cluster_id(self) -> [u8; 32] {
        self.input.cluster_id
    }
    /// Receiver program.
    pub const fn receiver_program(self) -> [u8; 32] {
        self.input.receiver_program
    }
    /// Receiver ProgramData.
    pub const fn receiver_programdata(self) -> [u8; 32] {
        self.input.receiver_programdata
    }
    /// Receiver ABI identity.
    pub const fn receiver_abi_id(self) -> [u8; 32] {
        self.input.receiver_abi_id
    }
    /// Receiver Loader-v3 upgrade authority.
    pub const fn receiver_upgrade_authority(self) -> [u8; 32] {
        self.input.receiver_upgrade_authority
    }
    /// Push-oracle program.
    pub const fn push_oracle_program(self) -> [u8; 32] {
        self.input.push_oracle_program
    }
    /// Push-oracle ProgramData.
    pub const fn push_oracle_programdata(self) -> [u8; 32] {
        self.input.push_oracle_programdata
    }
    /// Push-oracle ABI identity.
    pub const fn push_oracle_abi_id(self) -> [u8; 32] {
        self.input.push_oracle_abi_id
    }
    /// Push-oracle Loader-v3 upgrade authority.
    pub const fn push_oracle_upgrade_authority(self) -> [u8; 32] {
        self.input.push_oracle_upgrade_authority
    }
    /// Exact legacy Receiver Config PDA.
    pub const fn receiver_config(self) -> [u8; 32] {
        self.input.receiver_config
    }
    /// SHA-256 of the exact legacy Receiver Config body.
    pub const fn receiver_config_digest(self) -> [u8; 32] {
        self.input.receiver_config_digest
    }
    /// Exact sponsored account.
    pub const fn price_account(self) -> [u8; 32] {
        self.input.price_account
    }
    /// Exact feed identifier.
    pub const fn feed_id(self) -> [u8; 32] {
        self.input.feed_id
    }
    /// Price-update codec identity.
    pub const fn price_update_codec_id(self) -> [u8; 32] {
        self.input.price_update_codec_id
    }
    /// dClutch adapter identity.
    pub const fn adapter_id(self) -> [u8; 32] {
        self.input.adapter_id
    }
    /// Provider family identity.
    pub const fn provider_family_id(self) -> [u8; 32] {
        self.input.provider_family_id
    }
    /// Transport profile identity.
    pub const fn transport_profile_id(self) -> [u8; 32] {
        self.input.transport_profile_id
    }
    /// Receiver deployment slot.
    pub const fn receiver_deployment_slot(self) -> u64 {
        self.input.receiver_deployment_slot
    }
    /// Push-oracle deployment slot.
    pub const fn push_oracle_deployment_slot(self) -> u64 {
        self.input.push_oracle_deployment_slot
    }
    /// Push PDA shard.
    pub const fn shard(self) -> u16 {
        self.input.shard
    }
    /// Push PDA bump.
    pub const fn feed_account_bump(self) -> u8 {
        self.input.feed_account_bump
    }
    /// Activation timestamp.
    pub const fn activation_time(self) -> i64 {
        self.input.activation_time
    }
}

/// Current official sponsored SOL/USD shard-zero release on Solana devnet.
///
/// Program and ProgramData facts were read with finalized commitment at slots
/// 489486600..489486972 on 2026-08-28. The account address and 1-minute/0.5%
/// policy are independently named by Pyth's official sponsored-feed list.
pub fn devnet_sponsored_sol_usd_release_v1()
-> PythSponsoredPushReleaseV1Result<PythSponsoredPushReleaseV1> {
    PythSponsoredPushReleaseV1::new(PythSponsoredPushReleaseV1Input {
        cluster_id: crate::DEVNET_CLUSTER_ID_V1,
        receiver_program: [
            12, 183, 250, 187, 82, 247, 166, 72, 187, 91, 49, 125, 154, 1, 139, 144, 87, 203, 2,
            71, 116, 250, 254, 1, 230, 196, 223, 152, 204, 56, 88, 129,
        ],
        receiver_programdata: [
            120, 64, 94, 54, 62, 193, 65, 198, 75, 116, 31, 129, 189, 80, 76, 61, 157, 156, 249,
            235, 174, 218, 235, 38, 160, 175, 216, 39, 146, 113, 45, 238,
        ],
        receiver_abi_id: [
            0x0d, 0x6b, 0xf9, 0x14, 0x2c, 0x2e, 0xb1, 0xfd, 0xb8, 0xea, 0xd4, 0x80, 0x54, 0x49,
            0x13, 0x69, 0xbe, 0xd3, 0xa8, 0xd6, 0x84, 0x6c, 0x58, 0x87, 0xe3, 0x62, 0x84, 0x41,
            0x55, 0x59, 0x9a, 0xb2,
        ],
        receiver_upgrade_authority: [
            13, 136, 27, 159, 103, 200, 203, 61, 82, 253, 46, 178, 125, 19, 194, 9, 81, 209, 153,
            33, 43, 117, 2, 29, 85, 236, 191, 94, 24, 59, 140, 219,
        ],
        push_oracle_program: [
            12, 74, 160, 18, 142, 149, 211, 225, 98, 42, 165, 1, 197, 133, 169, 235, 7, 179, 115,
            84, 193, 8, 234, 11, 121, 27, 69, 109, 199, 238, 163, 54,
        ],
        push_oracle_programdata: [
            118, 35, 168, 235, 146, 187, 137, 70, 69, 111, 164, 238, 174, 157, 7, 85, 205, 34, 96,
            92, 221, 11, 239, 206, 186, 22, 166, 39, 172, 163, 7, 157,
        ],
        push_oracle_abi_id: [
            0x84, 0x5d, 0x84, 0x60, 0x3c, 0xde, 0xf6, 0x88, 0xe9, 0x27, 0x52, 0x1e, 0xb0, 0x5b,
            0xe7, 0x09, 0x96, 0xa0, 0x43, 0x2c, 0x21, 0xf4, 0x0f, 0x4c, 0xde, 0xb3, 0xb7, 0xa5,
            0x9f, 0x44, 0xc4, 0xec,
        ],
        push_oracle_upgrade_authority: [
            136, 203, 115, 185, 154, 115, 170, 90, 12, 210, 228, 155, 20, 27, 49, 153, 106, 168,
            45, 10, 90, 31, 213, 3, 183, 127, 206, 114, 159, 184, 105, 26,
        ],
        receiver_config: [
            186, 225, 187, 153, 68, 243, 75, 231, 133, 193, 52, 245, 80, 148, 76, 230, 150, 67,
            230, 128, 167, 235, 141, 81, 111, 221, 130, 111, 253, 58, 56, 98,
        ],
        receiver_config_digest: [
            0xbb, 0xbc, 0x32, 0x4e, 0x70, 0xa4, 0x36, 0xd7, 0x05, 0x22, 0x59, 0x5f, 0x47, 0x7a,
            0x31, 0x04, 0x48, 0x8f, 0x2b, 0x02, 0x41, 0x7e, 0x20, 0x74, 0x83, 0x88, 0x03, 0x37,
            0xcd, 0x38, 0x35, 0x92,
        ],
        price_account: [
            96, 49, 71, 4, 52, 13, 237, 223, 55, 31, 212, 36, 114, 20, 143, 36, 142, 157, 26, 109,
            26, 94, 178, 172, 58, 205, 139, 127, 213, 214, 178, 67,
        ],
        feed_id: [
            0xef, 0x0d, 0x8b, 0x6f, 0xda, 0x2c, 0xeb, 0xa4, 0x1d, 0xa1, 0x5d, 0x40, 0x95, 0xd1,
            0xda, 0x39, 0x2a, 0x0d, 0x2f, 0x8e, 0xd0, 0xc6, 0xc7, 0xbc, 0x0f, 0x4c, 0xfa, 0xc8,
            0xc2, 0x80, 0xb5, 0x6d,
        ],
        // Same exact SDK 2.0.0 Full PriceUpdateV2 codec already measured and
        // admitted by the upgraded Receiver release.
        price_update_codec_id: [
            0x12, 0xd0, 0xce, 0x8b, 0xc3, 0x90, 0x7a, 0xe2, 0x94, 0x90, 0x43, 0x39, 0x7e, 0xaf,
            0x3d, 0x5b, 0xd2, 0x5d, 0xee, 0xd9, 0x84, 0x50, 0xc6, 0x96, 0x9d, 0x95, 0x7b, 0xe4,
            0x02, 0xc8, 0x07, 0xae,
        ],
        adapter_id: PYTH_SPONSORED_PUSH_ADAPTER_ID_V1,
        provider_family_id: PYTH_SPONSORED_PUSH_PROVIDER_FAMILY_ID_V1,
        transport_profile_id: PYTH_SPONSORED_PUSH_TRANSPORT_PROFILE_ID_V1,
        receiver_deployment_slot: 487_855_452,
        push_oracle_deployment_slot: 293_898_740,
        shard: 0,
        feed_account_bump: 252,
        activation_time: 0,
    })
}

fn byte_at(bytes: &[u8], offset: usize) -> PythSponsoredPushReleaseV1Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(PythSponsoredPushReleaseV1Error::InvalidEncodedLength {
            actual: bytes.len(),
        })
}

fn array_at(bytes: &[u8], offset: usize) -> PythSponsoredPushReleaseV1Result<[u8; 32]> {
    bytes
        .get(offset..offset + 32)
        .and_then(|value| value.try_into().ok())
        .ok_or(PythSponsoredPushReleaseV1Error::InvalidEncodedLength {
            actual: bytes.len(),
        })
}

fn u16_at(bytes: &[u8], offset: usize) -> PythSponsoredPushReleaseV1Result<u16> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(PythSponsoredPushReleaseV1Error::InvalidEncodedLength {
            actual: bytes.len(),
        })
}

fn u64_at(bytes: &[u8], offset: usize) -> PythSponsoredPushReleaseV1Result<u64> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(PythSponsoredPushReleaseV1Error::InvalidEncodedLength {
            actual: bytes.len(),
        })
}

fn i64_at(bytes: &[u8], offset: usize) -> PythSponsoredPushReleaseV1Result<i64> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(i64::from_le_bytes)
        .ok_or(PythSponsoredPushReleaseV1Error::InvalidEncodedLength {
            actual: bytes.len(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devnet_release_round_trips_and_pins_distinct_transport() {
        let release = devnet_sponsored_sol_usd_release_v1().expect("release");
        let bytes = release.to_bytes();
        assert_eq!(PythSponsoredPushReleaseV1::decode(&bytes), Ok(release));
        assert_eq!(release.feed_account_bump(), 252);
        assert_eq!(release.shard(), 0);
        assert_eq!(
            release.provider_family_id(),
            PYTH_SPONSORED_PUSH_PROVIDER_FAMILY_ID_V1
        );
        assert_eq!(
            release.transport_profile_id(),
            PYTH_SPONSORED_PUSH_TRANSPORT_PROFILE_ID_V1
        );
        assert_ne!(
            release.receiver_program(),
            crate::devnet_release_v1()
                .expect("pull release")
                .receiver_program()
        );
    }

    #[test]
    fn hostile_release_bytes_refuse() {
        let release = devnet_sponsored_sol_usd_release_v1().expect("release");
        let bytes = release.to_bytes();
        for length in 0..bytes.len() {
            assert!(matches!(
                PythSponsoredPushReleaseV1::decode(&bytes[..length]),
                Err(PythSponsoredPushReleaseV1Error::InvalidEncodedLength { actual }) if actual == length
            ));
        }
        let mut wrong = bytes;
        wrong[0] ^= 1;
        assert_eq!(
            PythSponsoredPushReleaseV1::decode(&wrong),
            Err(PythSponsoredPushReleaseV1Error::InvalidMagic)
        );
        let mut reserved = bytes;
        reserved[579] = 1;
        assert_eq!(
            PythSponsoredPushReleaseV1::decode(&reserved),
            Err(PythSponsoredPushReleaseV1Error::NonzeroReserved)
        );
        let mut zero = bytes;
        zero[240..272].fill(0);
        assert_eq!(
            PythSponsoredPushReleaseV1::decode(&zero),
            Err(PythSponsoredPushReleaseV1Error::ZeroField {
                field: SponsoredPushReleaseFieldV1::PriceAccount
            })
        );
    }
}
