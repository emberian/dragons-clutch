//! Feature-gated checks around the one shared synthetic-local release row.

use dclutch_pyth_svm::{PythReleaseV1, SYNTHETIC_LOCAL_LABEL_V1, synthetic_local_release_v1};
use solana_program::{hash::hashv, program_error::ProgramError, pubkey::Pubkey};

use crate::AdapterError;

const ADAPTER_ID: [u8; 32] = [
    0x3f, 0xdf, 0xc9, 0x45, 0x89, 0xc6, 0x9b, 0x13, 0x38, 0x64, 0x46, 0x83, 0x20, 0x97, 0x6f, 0x8e,
    0x79, 0x0e, 0x7f, 0xe0, 0xf1, 0x45, 0x89, 0x7b, 0x6e, 0xab, 0xc2, 0x2b, 0xd7, 0xc8, 0x71, 0x1b,
];

/// Return the sole non-production release after independently checking the
/// adapter-domain identity and receiver Config PDA used by the SBF boundary.
pub(crate) fn release() -> Result<PythReleaseV1, ProgramError> {
    let expected_label = hashv(&[
        b"dclutch/synthetic-local-release/v1",
        &[0],
        b"local-upgraded-2026-08-22",
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
    if expected_label != SYNTHETIC_LOCAL_LABEL_V1 || expected_adapter != ADAPTER_ID {
        return Err(AdapterError::ProviderAuthentication.into());
    }

    let local = synthetic_local_release_v1()
        .map_err(|_| ProgramError::from(AdapterError::ProviderAuthentication))?;
    let release = *local.release();
    if release.adapter_id() != ADAPTER_ID {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    let receiver = Pubkey::new_from_array(release.receiver_program());
    let (expected_config, _) = Pubkey::find_program_address(&[b"config"], &receiver);
    if expected_config.to_bytes() != release.receiver_config() {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    Ok(release)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_manifest_is_rechecked_at_the_adapter_boundary() {
        let value = release().expect("pinned fixture release");
        assert_eq!(value.cluster_id(), SYNTHETIC_LOCAL_LABEL_V1);
        assert_eq!(value.adapter_id(), ADAPTER_ID);
        assert_eq!(value.guardian_set_count(), 19);
        assert_eq!(value.required_guardian_count(), 10);
    }
}
