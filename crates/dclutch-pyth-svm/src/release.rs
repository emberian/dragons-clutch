//! Immutable semantic contracts for Pyth adapter releases.
//!
//! The production catalog deliberately starts empty. No address, digest, or
//! release claim is inferred from this source file.

/// Canonical magic for a serialized [`PythReleaseV1`] preimage.
pub const PYTH_RELEASE_V1_MAGIC: [u8; 8] = *b"DCLTPR01";

/// Canonical schema version for a serialized [`PythReleaseV1`] preimage.
pub const PYTH_RELEASE_V1_SCHEMA_VERSION: u16 = 1;

/// Exact length of the canonical [`PythReleaseV1`] preimage.
///
/// The encoding has no Rust-layout padding and no variable-width fields.
pub const PYTH_RELEASE_V1_ENCODED_LEN: usize = 440;

/// A named nonzero field in a Pyth release contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseField {
    /// Solana cluster identity.
    ClusterId,
    /// Pyth receiver program public key.
    ReceiverProgram,
    /// Pyth receiver ProgramData public key.
    ReceiverProgramData,
    /// Pyth receiver configuration public key.
    ReceiverConfig,
    /// Provider/Wormhole router program public key.
    RouterProgram,
    /// Provider/Wormhole router ProgramData public key.
    RouterProgramData,
    /// Pyth receiver configuration digest.
    ConfigDigest,
    /// Pyth receiver ABI identity.
    ReceiverAbiId,
    /// Provider/Wormhole router ABI identity.
    RouterAbiId,
    /// Pyth price-update codec identity.
    PriceUpdateCodecId,
    /// dClutch adapter identity.
    AdapterId,
    /// Pinned upstream commit identifier.
    UpstreamCommit,
    /// Pinned SDK crate digest.
    SdkCrateDigest,
}

/// Error returned while validating or decoding a production Pyth release contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PythReleaseV1Error {
    /// A canonical preimage or output slice did not have the exact V1 length.
    InvalidEncodedLength {
        /// Observed number of bytes.
        actual: usize,
    },
    /// The canonical preimage did not start with [`PYTH_RELEASE_V1_MAGIC`].
    InvalidMagic,
    /// The canonical preimage named an unsupported schema version.
    UnsupportedSchemaVersion {
        /// Observed little-endian schema version.
        actual: u16,
    },
    /// A required opaque key, digest, ABI, codec, adapter, or commit was zero.
    ZeroField {
        /// The rejected zero-valued field.
        field: ReleaseField,
    },
    /// The claimed guardian-set size was zero.
    ZeroGuardianSetCount,
    /// The supplied guardian threshold was not the fixed strict-majority V1 threshold.
    InvalidStrictMajority {
        /// Guardian set cardinality used to derive the required count.
        guardian_set_count: u8,
        /// Submitted required guardian count.
        required_guardian_count: u8,
    },
}

/// Result alias for production Pyth release validation.
pub type PythReleaseV1Result<T> = core::result::Result<T, PythReleaseV1Error>;

/// All values required to construct a production Pyth release contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythReleaseV1Input {
    /// Nonzero opaque Solana cluster identifier.
    pub cluster_id: [u8; 32],
    /// Nonzero Pyth receiver program public key.
    pub receiver_program: [u8; 32],
    /// Nonzero Pyth receiver ProgramData public key.
    pub receiver_programdata: [u8; 32],
    /// Nonzero Pyth receiver configuration public key.
    pub receiver_config: [u8; 32],
    /// Nonzero provider/Wormhole router program public key.
    pub router_program: [u8; 32],
    /// Nonzero provider/Wormhole router ProgramData public key.
    pub router_programdata: [u8; 32],
    /// Nonzero digest of the receiver configuration bytes.
    pub config_digest: [u8; 32],
    /// Nonzero receiver ABI identifier.
    pub receiver_abi_id: [u8; 32],
    /// Nonzero provider/Wormhole router ABI identifier.
    pub router_abi_id: [u8; 32],
    /// Nonzero price-update codec identifier.
    pub price_update_codec_id: [u8; 32],
    /// Nonzero dClutch adapter identifier.
    pub adapter_id: [u8; 32],
    /// Slot recorded for the receiver program deployment.
    pub receiver_deployment_slot: u64,
    /// Slot recorded for the router program deployment.
    pub router_deployment_slot: u64,
    /// Cardinality of the guardian set used to derive strict majority.
    pub guardian_set_count: u8,
    /// Required guardian signatures, fixed to strict majority in V1.
    pub required_guardian_count: u8,
    /// Nonzero 20-byte upstream source commit identifier.
    pub upstream_commit: [u8; 20],
    /// Nonzero digest of the pinned SDK crate source or archive.
    pub sdk_crate_digest: [u8; 32],
    /// Unix timestamp at which this release becomes eligible for activation.
    pub activation_time: i64,
}

/// A private-field, fixed-layout immutable production Pyth release contract.
///
/// V1 fixes guardian quorum semantics to strict majority. It intentionally
/// offers no policy enum that could silently select a weaker quorum.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythReleaseV1 {
    cluster_id: [u8; 32],
    receiver_program: [u8; 32],
    receiver_programdata: [u8; 32],
    receiver_config: [u8; 32],
    router_program: [u8; 32],
    router_programdata: [u8; 32],
    config_digest: [u8; 32],
    receiver_abi_id: [u8; 32],
    router_abi_id: [u8; 32],
    price_update_codec_id: [u8; 32],
    adapter_id: [u8; 32],
    receiver_deployment_slot: u64,
    router_deployment_slot: u64,
    guardian_set_count: u8,
    required_guardian_count: u8,
    upstream_commit: [u8; 20],
    sdk_crate_digest: [u8; 32],
    activation_time: i64,
}

impl PythReleaseV1 {
    /// Validate and construct a production release contract.
    pub fn new(input: PythReleaseV1Input) -> PythReleaseV1Result<Self> {
        validate_nonzero(input.cluster_id, ReleaseField::ClusterId)?;
        validate_nonzero(input.receiver_program, ReleaseField::ReceiverProgram)?;
        validate_nonzero(
            input.receiver_programdata,
            ReleaseField::ReceiverProgramData,
        )?;
        validate_nonzero(input.receiver_config, ReleaseField::ReceiverConfig)?;
        validate_nonzero(input.router_program, ReleaseField::RouterProgram)?;
        validate_nonzero(input.router_programdata, ReleaseField::RouterProgramData)?;
        validate_nonzero(input.config_digest, ReleaseField::ConfigDigest)?;
        validate_nonzero(input.receiver_abi_id, ReleaseField::ReceiverAbiId)?;
        validate_nonzero(input.router_abi_id, ReleaseField::RouterAbiId)?;
        validate_nonzero(
            input.price_update_codec_id,
            ReleaseField::PriceUpdateCodecId,
        )?;
        validate_nonzero(input.adapter_id, ReleaseField::AdapterId)?;
        validate_nonzero(input.upstream_commit, ReleaseField::UpstreamCommit)?;
        validate_nonzero(input.sdk_crate_digest, ReleaseField::SdkCrateDigest)?;
        if input.guardian_set_count == 0 {
            return Err(PythReleaseV1Error::ZeroGuardianSetCount);
        }
        if input.required_guardian_count != strict_majority(input.guardian_set_count) {
            return Err(PythReleaseV1Error::InvalidStrictMajority {
                guardian_set_count: input.guardian_set_count,
                required_guardian_count: input.required_guardian_count,
            });
        }
        Ok(Self {
            cluster_id: input.cluster_id,
            receiver_program: input.receiver_program,
            receiver_programdata: input.receiver_programdata,
            receiver_config: input.receiver_config,
            router_program: input.router_program,
            router_programdata: input.router_programdata,
            config_digest: input.config_digest,
            receiver_abi_id: input.receiver_abi_id,
            router_abi_id: input.router_abi_id,
            price_update_codec_id: input.price_update_codec_id,
            adapter_id: input.adapter_id,
            receiver_deployment_slot: input.receiver_deployment_slot,
            router_deployment_slot: input.router_deployment_slot,
            guardian_set_count: input.guardian_set_count,
            required_guardian_count: input.required_guardian_count,
            upstream_commit: input.upstream_commit,
            sdk_crate_digest: input.sdk_crate_digest,
            activation_time: input.activation_time,
        })
    }

    /// Decode and semantically validate one exact canonical V1 preimage.
    ///
    /// This decoder never depends on `repr(C)`, native alignment, or a
    /// framework serializer. Integers are explicitly little-endian.
    pub fn decode(bytes: &[u8]) -> PythReleaseV1Result<Self> {
        if bytes.len() != PYTH_RELEASE_V1_ENCODED_LEN {
            return Err(PythReleaseV1Error::InvalidEncodedLength {
                actual: bytes.len(),
            });
        }
        if bytes.get(0..8) != Some(&PYTH_RELEASE_V1_MAGIC) {
            return Err(PythReleaseV1Error::InvalidMagic);
        }
        let schema_version = u16_at(bytes, 8)?;
        if schema_version != PYTH_RELEASE_V1_SCHEMA_VERSION {
            return Err(PythReleaseV1Error::UnsupportedSchemaVersion {
                actual: schema_version,
            });
        }
        Self::new(PythReleaseV1Input {
            cluster_id: array_at(bytes, 10)?,
            receiver_program: array_at(bytes, 42)?,
            receiver_programdata: array_at(bytes, 74)?,
            receiver_config: array_at(bytes, 106)?,
            router_program: array_at(bytes, 138)?,
            router_programdata: array_at(bytes, 170)?,
            config_digest: array_at(bytes, 202)?,
            receiver_abi_id: array_at(bytes, 234)?,
            router_abi_id: array_at(bytes, 266)?,
            price_update_codec_id: array_at(bytes, 298)?,
            adapter_id: array_at(bytes, 330)?,
            receiver_deployment_slot: u64_at(bytes, 362)?,
            router_deployment_slot: u64_at(bytes, 370)?,
            guardian_set_count: byte_at(bytes, 378)?,
            required_guardian_count: byte_at(bytes, 379)?,
            upstream_commit: array_at(bytes, 380)?,
            sdk_crate_digest: array_at(bytes, 400)?,
            activation_time: i64_at(bytes, 432)?,
        })
    }

    /// Return the exact canonical V1 preimage for hashing or persistence.
    pub fn to_bytes(&self) -> [u8; PYTH_RELEASE_V1_ENCODED_LEN] {
        let mut bytes = [0_u8; PYTH_RELEASE_V1_ENCODED_LEN];
        bytes[0..8].copy_from_slice(&PYTH_RELEASE_V1_MAGIC);
        bytes[8..10].copy_from_slice(&PYTH_RELEASE_V1_SCHEMA_VERSION.to_le_bytes());
        bytes[10..42].copy_from_slice(&self.cluster_id);
        bytes[42..74].copy_from_slice(&self.receiver_program);
        bytes[74..106].copy_from_slice(&self.receiver_programdata);
        bytes[106..138].copy_from_slice(&self.receiver_config);
        bytes[138..170].copy_from_slice(&self.router_program);
        bytes[170..202].copy_from_slice(&self.router_programdata);
        bytes[202..234].copy_from_slice(&self.config_digest);
        bytes[234..266].copy_from_slice(&self.receiver_abi_id);
        bytes[266..298].copy_from_slice(&self.router_abi_id);
        bytes[298..330].copy_from_slice(&self.price_update_codec_id);
        bytes[330..362].copy_from_slice(&self.adapter_id);
        bytes[362..370].copy_from_slice(&self.receiver_deployment_slot.to_le_bytes());
        bytes[370..378].copy_from_slice(&self.router_deployment_slot.to_le_bytes());
        bytes[378] = self.guardian_set_count;
        bytes[379] = self.required_guardian_count;
        bytes[380..400].copy_from_slice(&self.upstream_commit);
        bytes[400..432].copy_from_slice(&self.sdk_crate_digest);
        bytes[432..440].copy_from_slice(&self.activation_time.to_le_bytes());
        bytes
    }

    /// Encode into an exact-width output slice.
    ///
    /// An output-length refusal leaves `output` unchanged.
    pub fn encode(&self, output: &mut [u8]) -> PythReleaseV1Result<()> {
        if output.len() != PYTH_RELEASE_V1_ENCODED_LEN {
            return Err(PythReleaseV1Error::InvalidEncodedLength {
                actual: output.len(),
            });
        }
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Return the opaque Solana cluster identifier.
    pub const fn cluster_id(&self) -> [u8; 32] {
        self.cluster_id
    }
    /// Return the receiver program public key.
    pub const fn receiver_program(&self) -> [u8; 32] {
        self.receiver_program
    }
    /// Return the receiver ProgramData public key.
    pub const fn receiver_programdata(&self) -> [u8; 32] {
        self.receiver_programdata
    }
    /// Return the receiver configuration public key.
    pub const fn receiver_config(&self) -> [u8; 32] {
        self.receiver_config
    }
    /// Return the router program public key.
    pub const fn router_program(&self) -> [u8; 32] {
        self.router_program
    }
    /// Return the router ProgramData public key.
    pub const fn router_programdata(&self) -> [u8; 32] {
        self.router_programdata
    }
    /// Return the committed receiver configuration digest.
    pub const fn config_digest(&self) -> [u8; 32] {
        self.config_digest
    }
    /// Return the receiver ABI identifier.
    pub const fn receiver_abi_id(&self) -> [u8; 32] {
        self.receiver_abi_id
    }
    /// Return the provider/Wormhole router ABI identifier.
    pub const fn router_abi_id(&self) -> [u8; 32] {
        self.router_abi_id
    }
    /// Return the price-update codec identifier.
    pub const fn price_update_codec_id(&self) -> [u8; 32] {
        self.price_update_codec_id
    }
    /// Return the dClutch adapter identifier.
    pub const fn adapter_id(&self) -> [u8; 32] {
        self.adapter_id
    }
    /// Return the recorded receiver deployment slot.
    pub const fn receiver_deployment_slot(&self) -> u64 {
        self.receiver_deployment_slot
    }
    /// Return the recorded router deployment slot.
    pub const fn router_deployment_slot(&self) -> u64 {
        self.router_deployment_slot
    }
    /// Return the committed guardian set cardinality.
    pub const fn guardian_set_count(&self) -> u8 {
        self.guardian_set_count
    }
    /// Return the strict-majority V1 required guardian count.
    pub const fn required_guardian_count(&self) -> u8 {
        self.required_guardian_count
    }
    /// Return the pinned upstream source commit identifier.
    pub const fn upstream_commit(&self) -> [u8; 20] {
        self.upstream_commit
    }
    /// Return the pinned SDK crate digest.
    pub const fn sdk_crate_digest(&self) -> [u8; 32] {
        self.sdk_crate_digest
    }
    /// Return the activation Unix timestamp.
    pub const fn activation_time(&self) -> i64 {
        self.activation_time
    }
}

/// The deliberately empty typed production catalog.
pub const PRODUCTION_RELEASES: [PythReleaseV1; 0] = [];

/// Error returned while validating a synthetic local release marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntheticLocalReleaseV1Error {
    /// The local-only label was zero.
    ZeroLocalLabel,
    /// The embedded full release contract was invalid.
    InvalidRelease {
        /// The exact production-release validation error.
        error: PythReleaseV1Error,
    },
}

/// Input to the explicitly non-production synthetic local release marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntheticLocalReleaseV1Input {
    /// Nonzero local-only label, not a Solana cluster identity.
    pub local_label: [u8; 32],
    /// Complete release facts for the synthetic local environment.
    pub release: PythReleaseV1Input,
}

/// A private-field fixed-layout marker for synthetic local testing only.
///
/// This is intentionally a distinct type with a distinct constructor. It has
/// no conversion to [`PythReleaseV1`] and therefore cannot inhabit
/// [`PRODUCTION_RELEASES`].
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntheticLocalReleaseV1 {
    local_label: [u8; 32],
    release: PythReleaseV1,
}

impl SyntheticLocalReleaseV1 {
    /// Validate and construct a local-only synthetic release marker.
    pub fn new(input: SyntheticLocalReleaseV1Input) -> Result<Self, SyntheticLocalReleaseV1Error> {
        if is_zero(&input.local_label) {
            return Err(SyntheticLocalReleaseV1Error::ZeroLocalLabel);
        }
        Ok(Self {
            local_label: input.local_label,
            release: PythReleaseV1::new(input.release)
                .map_err(|error| SyntheticLocalReleaseV1Error::InvalidRelease { error })?,
        })
    }

    /// Return the local-only release label.
    pub const fn local_label(&self) -> [u8; 32] {
        self.local_label
    }
    /// Return the one semantic owner of this local release's full release facts.
    pub const fn release(&self) -> &PythReleaseV1 {
        &self.release
    }
}

fn strict_majority(guardian_set_count: u8) -> u8 {
    guardian_set_count / 2 + 1
}

fn validate_nonzero<const N: usize>(
    value: [u8; N],
    field: ReleaseField,
) -> PythReleaseV1Result<()> {
    if is_zero(&value) {
        return Err(PythReleaseV1Error::ZeroField { field });
    }
    Ok(())
}

fn is_zero<const N: usize>(value: &[u8; N]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn byte_at(bytes: &[u8], offset: usize) -> PythReleaseV1Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(PythReleaseV1Error::InvalidEncodedLength {
            actual: bytes.len(),
        })
}

fn array_at<const N: usize>(bytes: &[u8], offset: usize) -> PythReleaseV1Result<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or(PythReleaseV1Error::InvalidEncodedLength {
            actual: bytes.len(),
        })?;
    bytes
        .get(offset..end)
        .ok_or(PythReleaseV1Error::InvalidEncodedLength {
            actual: bytes.len(),
        })?
        .try_into()
        .map_err(|_| PythReleaseV1Error::InvalidEncodedLength {
            actual: bytes.len(),
        })
}

fn u16_at(bytes: &[u8], offset: usize) -> PythReleaseV1Result<u16> {
    Ok(u16::from_le_bytes(array_at(bytes, offset)?))
}

fn u64_at(bytes: &[u8], offset: usize) -> PythReleaseV1Result<u64> {
    Ok(u64::from_le_bytes(array_at(bytes, offset)?))
}

fn i64_at(bytes: &[u8], offset: usize) -> PythReleaseV1Result<i64> {
    Ok(i64::from_le_bytes(array_at(bytes, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> PythReleaseV1Input {
        PythReleaseV1Input {
            cluster_id: [1; 32],
            receiver_program: [2; 32],
            receiver_programdata: [3; 32],
            receiver_config: [4; 32],
            router_program: [5; 32],
            router_programdata: [6; 32],
            config_digest: [7; 32],
            receiver_abi_id: [8; 32],
            router_abi_id: [9; 32],
            price_update_codec_id: [10; 32],
            adapter_id: [11; 32],
            receiver_deployment_slot: 11,
            router_deployment_slot: 12,
            guardian_set_count: 5,
            required_guardian_count: 3,
            upstream_commit: [13; 20],
            sdk_crate_digest: [14; 32],
            activation_time: -15,
        }
    }

    #[test]
    fn production_record_round_trips_all_committed_facts() -> PythReleaseV1Result<()> {
        let release = PythReleaseV1::new(input())?;
        assert_eq!(release.cluster_id(), [1; 32]);
        assert_eq!(release.receiver_program(), [2; 32]);
        assert_eq!(release.receiver_programdata(), [3; 32]);
        assert_eq!(release.receiver_config(), [4; 32]);
        assert_eq!(release.router_program(), [5; 32]);
        assert_eq!(release.router_programdata(), [6; 32]);
        assert_eq!(release.config_digest(), [7; 32]);
        assert_eq!(release.receiver_abi_id(), [8; 32]);
        assert_eq!(release.router_abi_id(), [9; 32]);
        assert_eq!(release.price_update_codec_id(), [10; 32]);
        assert_eq!(release.adapter_id(), [11; 32]);
        assert_eq!(release.receiver_deployment_slot(), 11);
        assert_eq!(release.router_deployment_slot(), 12);
        assert_eq!(release.guardian_set_count(), 5);
        assert_eq!(release.required_guardian_count(), 3);
        assert_eq!(release.upstream_commit(), [13; 20]);
        assert_eq!(release.sdk_crate_digest(), [14; 32]);
        assert_eq!(release.activation_time(), -15);
        assert_eq!(PRODUCTION_RELEASES.len(), 0);
        Ok(())
    }

    #[test]
    fn canonical_preimage_is_exact_and_round_trips_every_field() -> PythReleaseV1Result<()> {
        let release = PythReleaseV1::new(input())?;
        let bytes = release.to_bytes();
        assert_eq!(bytes.len(), PYTH_RELEASE_V1_ENCODED_LEN);
        assert_eq!(bytes.get(0..8), Some(PYTH_RELEASE_V1_MAGIC.as_slice()));
        assert_eq!(bytes.get(8..10), Some([1_u8, 0].as_slice()));
        assert_eq!(bytes.get(10..42), Some([1_u8; 32].as_slice()));
        assert_eq!(bytes.get(330..362), Some([11_u8; 32].as_slice()));
        assert_eq!(bytes.get(362..370), Some(11_u64.to_le_bytes().as_slice()));
        assert_eq!(bytes.get(370..378), Some(12_u64.to_le_bytes().as_slice()));
        assert_eq!(bytes.get(378..380), Some([5_u8, 3].as_slice()));
        assert_eq!(bytes.get(380..400), Some([13_u8; 20].as_slice()));
        assert_eq!(bytes.get(400..432), Some([14_u8; 32].as_slice()));
        assert_eq!(
            bytes.get(432..440),
            Some((-15_i64).to_le_bytes().as_slice())
        );
        assert_eq!(PythReleaseV1::decode(&bytes), Ok(release));

        let mut output = [0xa5_u8; PYTH_RELEASE_V1_ENCODED_LEN];
        release.encode(&mut output)?;
        assert_eq!(output, bytes);
        Ok(())
    }

    #[test]
    fn canonical_preimage_refuses_hostile_envelopes_and_invalid_semantics()
    -> PythReleaseV1Result<()> {
        let release = PythReleaseV1::new(input())?;
        let bytes = release.to_bytes();
        for length in 0..PYTH_RELEASE_V1_ENCODED_LEN {
            let truncated = bytes
                .get(..length)
                .ok_or(PythReleaseV1Error::InvalidEncodedLength { actual: length })?;
            assert_eq!(
                PythReleaseV1::decode(truncated),
                Err(PythReleaseV1Error::InvalidEncodedLength { actual: length })
            );
        }
        let mut long = [0_u8; PYTH_RELEASE_V1_ENCODED_LEN + 1];
        long.get_mut(..PYTH_RELEASE_V1_ENCODED_LEN)
            .ok_or(PythReleaseV1Error::InvalidEncodedLength { actual: 0 })?
            .copy_from_slice(&bytes);
        assert_eq!(
            PythReleaseV1::decode(&long),
            Err(PythReleaseV1Error::InvalidEncodedLength {
                actual: PYTH_RELEASE_V1_ENCODED_LEN + 1
            })
        );

        let mut wrong_magic = bytes;
        wrong_magic[0] ^= 1;
        assert_eq!(
            PythReleaseV1::decode(&wrong_magic),
            Err(PythReleaseV1Error::InvalidMagic)
        );
        let mut wrong_schema = bytes;
        wrong_schema[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            PythReleaseV1::decode(&wrong_schema),
            Err(PythReleaseV1Error::UnsupportedSchemaVersion { actual: 2 })
        );
        let mut zero_identifier = bytes;
        zero_identifier[298..330].fill(0);
        assert_eq!(
            PythReleaseV1::decode(&zero_identifier),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::PriceUpdateCodecId
            })
        );

        let mut short_output = [0xa5_u8; PYTH_RELEASE_V1_ENCODED_LEN - 1];
        assert_eq!(
            release.encode(&mut short_output),
            Err(PythReleaseV1Error::InvalidEncodedLength {
                actual: PYTH_RELEASE_V1_ENCODED_LEN - 1
            })
        );
        assert!(short_output.iter().all(|byte| *byte == 0xa5));
        Ok(())
    }

    #[test]
    fn every_required_production_identifier_rejects_zero() {
        let mut candidate = input();
        candidate.cluster_id = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::ClusterId
            })
        );
        let mut candidate = input();
        candidate.receiver_program = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::ReceiverProgram
            })
        );
        let mut candidate = input();
        candidate.receiver_programdata = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::ReceiverProgramData
            })
        );
        let mut candidate = input();
        candidate.receiver_config = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::ReceiverConfig
            })
        );
        let mut candidate = input();
        candidate.router_program = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::RouterProgram
            })
        );
        let mut candidate = input();
        candidate.router_programdata = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::RouterProgramData
            })
        );
        let mut candidate = input();
        candidate.config_digest = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::ConfigDigest
            })
        );
        let mut candidate = input();
        candidate.receiver_abi_id = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::ReceiverAbiId
            })
        );
        let mut candidate = input();
        candidate.router_abi_id = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::RouterAbiId
            })
        );
        let mut candidate = input();
        candidate.price_update_codec_id = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::PriceUpdateCodecId
            })
        );
        let mut candidate = input();
        candidate.adapter_id = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::AdapterId
            })
        );
        let mut candidate = input();
        candidate.upstream_commit = [0; 20];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::UpstreamCommit
            })
        );
        let mut candidate = input();
        candidate.sdk_crate_digest = [0; 32];
        assert_eq!(
            PythReleaseV1::new(candidate),
            Err(PythReleaseV1Error::ZeroField {
                field: ReleaseField::SdkCrateDigest
            })
        );
    }

    #[test]
    fn strict_majority_and_synthetic_local_are_not_negotiable() {
        let mut zero_set = input();
        zero_set.guardian_set_count = 0;
        zero_set.required_guardian_count = 0;
        assert_eq!(
            PythReleaseV1::new(zero_set),
            Err(PythReleaseV1Error::ZeroGuardianSetCount)
        );
        let mut weak = input();
        weak.required_guardian_count = 2;
        assert_eq!(
            PythReleaseV1::new(weak),
            Err(PythReleaseV1Error::InvalidStrictMajority {
                guardian_set_count: 5,
                required_guardian_count: 2
            })
        );
        let local = SyntheticLocalReleaseV1::new(SyntheticLocalReleaseV1Input {
            local_label: [1; 32],
            release: input(),
        });
        assert_eq!(local.map(|value| value.local_label()), Ok([1; 32]));
        let local = SyntheticLocalReleaseV1::new(SyntheticLocalReleaseV1Input {
            local_label: [1; 32],
            release: input(),
        });
        assert_eq!(
            local.map(|value| value.release().adapter_id()),
            Ok([11; 32])
        );
        assert_eq!(
            SyntheticLocalReleaseV1::new(SyntheticLocalReleaseV1Input {
                local_label: [0; 32],
                release: input(),
            }),
            Err(SyntheticLocalReleaseV1Error::ZeroLocalLabel)
        );
        let mut invalid_release = input();
        invalid_release.router_abi_id = [0; 32];
        assert_eq!(
            SyntheticLocalReleaseV1::new(SyntheticLocalReleaseV1Input {
                local_label: [1; 32],
                release: invalid_release,
            }),
            Err(SyntheticLocalReleaseV1Error::InvalidRelease {
                error: PythReleaseV1Error::ZeroField {
                    field: ReleaseField::RouterAbiId
                }
            })
        );
    }
}
