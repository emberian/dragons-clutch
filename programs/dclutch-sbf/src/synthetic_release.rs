//! Feature-gated checks around the shared non-production provider releases.

use dclutch_pyth_svm::{
    LOCAL_VALIDATOR_LABEL_V1, PythReleaseV1, SYNTHETIC_LOCAL_LABEL_V1, local_validator_release_v1,
    synthetic_local_release_v1,
};
use solana_program::{hash::hashv, program_error::ProgramError, pubkey::Pubkey};

use crate::AdapterError;

const ADAPTER_ID: [u8; 32] = [
    0x3f, 0xdf, 0xc9, 0x45, 0x89, 0xc6, 0x9b, 0x13, 0x38, 0x64, 0x46, 0x83, 0x20, 0x97, 0x6f, 0x8e,
    0x79, 0x0e, 0x7f, 0xe0, 0xf1, 0x45, 0x89, 0x7b, 0x6e, 0xab, 0xc2, 0x2b, 0xd7, 0xc8, 0x71, 0x1b,
];

/// Return every feature-gated non-production release after independently
/// checking its environment identity and the shared adapter boundary.
pub(crate) fn releases() -> Result<[PythReleaseV1; 2], ProgramError> {
    let expected_label = hashv(&[
        b"dclutch/synthetic-local-release/v1",
        &[0],
        b"local-upgraded-2026-08-22",
    ])
    .to_bytes();
    let expected_validator_label = hashv(&[
        b"dclutch/local-validator-release/v1",
        &[0],
        b"solana-test-validator-4.0.2-upgradeable-program-slot-zero",
    ])
    .to_bytes();
    let expected_adapter = hashv(&[
        b"dclutch/pyth-adapter/v1",
        &[0],
        b"resolve-categorical-pyth-v1",
        &[0],
        b"internal-post-update",
        &[0],
        b"inline-terminal-receipt",
    ])
    .to_bytes();
    if expected_label != SYNTHETIC_LOCAL_LABEL_V1
        || expected_validator_label != LOCAL_VALIDATOR_LABEL_V1
        || expected_adapter != ADAPTER_ID
    {
        return Err(AdapterError::ProviderAuthentication.into());
    }

    let captured = synthetic_local_release_v1()
        .map_err(|_| ProgramError::from(AdapterError::ProviderAuthentication))?;
    let validator = local_validator_release_v1()
        .map_err(|_| ProgramError::from(AdapterError::ProviderAuthentication))?;
    let releases = [*captured.release(), *validator.release()];
    for release in releases {
        if release.adapter_id() != ADAPTER_ID {
            return Err(AdapterError::ProviderAuthentication.into());
        }
        let receiver = Pubkey::new_from_array(release.receiver_program());
        let (expected_config, _) = Pubkey::find_program_address(&[b"config"], &receiver);
        if expected_config.to_bytes() != release.receiver_config() {
            return Err(AdapterError::ProviderAuthentication.into());
        }
    }
    Ok(releases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_manifest_is_rechecked_at_the_adapter_boundary() {
        let [captured, validator] = releases().expect("pinned fixture releases");
        assert_eq!(captured.cluster_id(), SYNTHETIC_LOCAL_LABEL_V1);
        assert_eq!(validator.cluster_id(), LOCAL_VALIDATOR_LABEL_V1);
        assert_eq!(captured.adapter_id(), ADAPTER_ID);
        assert_eq!(validator.adapter_id(), ADAPTER_ID);
        assert_eq!(captured.receiver_deployment_slot(), 460_336_311);
        assert_eq!(captured.router_deployment_slot(), 460_336_290);
        assert_eq!(validator.receiver_deployment_slot(), 0);
        assert_eq!(validator.router_deployment_slot(), 0);
        assert_ne!(hash(&captured.to_bytes()), hash(&validator.to_bytes()));
    }
}
