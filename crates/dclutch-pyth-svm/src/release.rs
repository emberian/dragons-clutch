//! Immutable semantic contracts for Pyth adapter releases.
//!
//! The production catalog deliberately starts empty. No address, digest, or
//! release claim is inferred from this source file.

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

/// Error returned while validating a production Pyth release contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PythReleaseV1Error {
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
