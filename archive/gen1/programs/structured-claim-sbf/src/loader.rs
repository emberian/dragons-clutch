//! Exact locus-aware upgradeable-loader release authentication.

use clutch_product_series::{
    ContentId, FixedCodec, RegistryProgramReleaseV2, RegistryReleaseLocusV2,
};
use clutch_solana_layout::artifact::ArtifactKind;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;
use solana_sha256_hasher::hashv;

use crate::error::{Result, WrapperError};

/// Upgradeable Loader v3 executable.
pub const UPGRADEABLE_LOADER_ID: [u8; 32] = [
    2, 168, 246, 145, 78, 136, 161, 176, 226, 16, 21, 62, 247, 99, 174, 43, 0, 194, 185, 61,
    22, 193, 36, 210, 192, 83, 122, 16, 4, 128, 0, 0,
];

const PROGRAM_METADATA_BYTES: usize = 36;
const PROGRAMDATA_METADATA_BYTES: usize = 45;
const PROGRAM_TAG: [u8; 4] = 2_u32.to_le_bytes();
const PROGRAMDATA_TAG: [u8; 4] = 3_u32.to_le_bytes();

/// Exact authenticated deployment and immutable release-artifact identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedReleaseV2 {
    /// ProgramData address linked by the executable account.
    pub program_data: [u8; 32],
    /// Deployment or last-upgrade slot in ProgramData.
    pub slot: u64,
    /// Semantic identity of the exact `RegistryProgramReleaseV2` body.
    pub release_id: ContentId,
}

/// Authenticate one read-only Program/ProgramData pair through its exact
/// base-owned `RegistryProgramReleaseV2` artifact.
pub fn authenticate_release_v2(
    artifact_owner: &Pubkey,
    program: &AccountInfo<'_>,
    data: &AccountInfo<'_>,
    artifact: &AccountInfo<'_>,
    expected_manifest_id: ContentId,
) -> Result<AuthenticatedReleaseV2> {
    if program.key == data.key
        || program.key == artifact.key
        || data.key == artifact.key
        || program.owner.to_bytes() != UPGRADEABLE_LOADER_ID
        || data.owner.to_bytes() != UPGRADEABLE_LOADER_ID
        || artifact.owner != artifact_owner
        || !program.executable
        || data.executable
        || artifact.executable
        || program.is_writable
        || data.is_writable
        || artifact.is_writable
        || program.is_signer
        || data.is_signer
        || artifact.is_signer
    {
        return Err(WrapperError::Deployment);
    }
    let artifact_body = artifact
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let release = RegistryProgramReleaseV2::decode(&artifact_body)
        .map_err(|_| WrapperError::Deployment)?;
    drop(artifact_body);
    let release_id = release
        .id()
        .map_err(|_| WrapperError::Deployment)?
        .content_id();
    let expected_artifact = product_artifact_pda(
        artifact_owner,
        ArtifactKind::RegistryProgramReleaseV2.byte(),
        release_id.bytes(),
    );
    if *artifact.key != expected_artifact {
        return Err(WrapperError::Deployment);
    }
    let program_body = program
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let data_body = data.try_borrow_data().map_err(|_| WrapperError::Borrow)?;
    if program_body.len() < PROGRAM_METADATA_BYTES
        || data_body.len() < PROGRAMDATA_METADATA_BYTES
        || program_body[0..4] != PROGRAM_TAG
        || data_body[0..4] != PROGRAMDATA_TAG
        || program_body[4..36] != data.key.to_bytes()
        || !matches!(data_body[12], 0 | 1)
        || (data_body[12] == 1 && data_body[13..45] == [0_u8; 32])
    {
        return Err(WrapperError::Deployment);
    }
    let mut slot_bytes = [0_u8; 8];
    slot_bytes.copy_from_slice(&data_body[4..12]);
    let slot = u64::from_le_bytes(slot_bytes);
    let locus_matches = release_locus_matches_slot(release.locus, slot);
    if release.program.bytes() != program.key.to_bytes()
        || release.programdata.bytes() != data.key.to_bytes()
        || release.programdata_sha256.bytes() != hashv(&[&data_body]).to_bytes()
        || release.capability_manifest_id != expected_manifest_id
        || release.deployment_slot != slot
        || !locus_matches
    {
        return Err(WrapperError::Deployment);
    }
    Ok(AuthenticatedReleaseV2 {
        program_data: data.key.to_bytes(),
        slot,
        release_id,
    })
}

fn release_locus_matches_slot(locus: RegistryReleaseLocusV2, slot: u64) -> bool {
    matches!(
        (locus, slot),
        (RegistryReleaseLocusV2::SynthesizedGenesisZero, 0)
    ) || matches!(
        (locus, slot),
        (RegistryReleaseLocusV2::ObservedPositive, observed) if observed != 0
    )
}

fn product_artifact_pda(program_id: &Pubkey, kind: u8, id: [u8; 32]) -> Pubkey {
    let kind_seed = [kind];
    Pubkey::find_program_address(
        &[b"dc:product-artifact:v1", &kind_seed, &id],
        program_id,
    )
    .0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_loci_are_disjoint_at_slot_zero() {
        assert!(release_locus_matches_slot(
            RegistryReleaseLocusV2::SynthesizedGenesisZero,
            0,
        ));
        assert!(!release_locus_matches_slot(
            RegistryReleaseLocusV2::SynthesizedGenesisZero,
            1,
        ));
        assert!(!release_locus_matches_slot(
            RegistryReleaseLocusV2::ObservedPositive,
            0,
        ));
        assert!(release_locus_matches_slot(
            RegistryReleaseLocusV2::ObservedPositive,
            u64::MAX,
        ));
    }
}
