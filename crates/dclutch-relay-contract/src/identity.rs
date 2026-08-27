//! Cross-cluster program identity — the Loopscale defense, second face.
//!
//! The Loopscale failure was trusting an account's *shape* without binding the
//! program that owns it.  Across a cluster boundary it has a second face that
//! was measured rather than reasoned about: the Meteora DBC program's `Program`
//! account is **byte-identical on mainnet-beta and devnet** — same 36 bytes,
//! same Loader V3 tag, same `programdata_address`.  A devnet observation of the
//! venue's program is therefore indistinguishable from a mainnet one, and
//! program identity alone does not identify a cluster.
//!
//! That is why `observed_cluster_id` is a **signed field** rather than an
//! adapter assumption.  It converts an otherwise-invisible substitution into an
//! explicit, non-repudiable false statement by a named key, and it is why
//! [`require_observed_cluster`] refuses with [`Error::ObservedClusterMismatch`]
//! specifically: on the byte-identical twin, nothing else can refuse.
//!
//! Everything below reconstructs the *existing* `DeploymentObservationV1` from
//! attested bytes and hands it to the *existing*
//! `ArtifactReleaseV1::authenticate_deployment`, unchanged.  No new
//! authentication primitive is introduced, and none is needed: for a
//! `ProgramData` account observed with `inline_len = 45`, the observation's
//! `tail_digest` is by construction SHA-256 over `data[45..]`, which is exactly
//! what the registry already calls `elf_digest`.

use dclutch_registry_contract::DeploymentObservationV1;
use dclutch_registry_svm::{
    LOADER_V3_PROGRAM_BYTES, LOADER_V3_PROGRAMDATA_METADATA_BYTES, ProgramDataMetadataV3View,
    ProgramV3View,
};

use crate::{ADDRESS_BYTES, Error, Result, wire::AccountObservationV1};

/// Loader V3 (`BPFLoaderUpgradeab1e11111111111111111111111`) program bytes.
pub const LOADER_V3_PROGRAM_ID: [u8; ADDRESS_BYTES] = [
    0x02, 0xa8, 0xf6, 0x91, 0x4e, 0x88, 0xa1, 0xb0, 0xe2, 0x10, 0x15, 0x3e, 0xf7, 0x63, 0xae, 0x2b,
    0x00, 0xc2, 0xb9, 0x3d, 0x16, 0xc1, 0x24, 0xd2, 0xc0, 0x53, 0x7a, 0x10, 0x04, 0x80, 0x00, 0x00,
];

/// Require the attested cluster to be the release-pinned one.
///
/// This is layer one of the defense stack and the only layer that can tell the
/// byte-identical twins apart.
pub fn require_observed_cluster(
    observed_cluster_id: [u8; 32],
    pinned_cluster_id: [u8; 32],
) -> Result<()> {
    if observed_cluster_id != pinned_cluster_id {
        return Err(Error::ObservedClusterMismatch);
    }
    Ok(())
}

/// Rebuild a Loader V3 deployment observation from two attested bodies.
///
/// `program` must be carried fully inline (36 bytes) and `programdata` must
/// carry exactly the 45-byte metadata prefix, so that its `tail_digest` *is* the
/// deployed ELF digest.  Any other pinned inline width for those two positions
/// is refused here rather than silently producing a digest of the wrong span.
pub fn reconstruct_deployment_observation_v1(
    program: AccountObservationV1<'_>,
    programdata: AccountObservationV1<'_>,
) -> Result<DeploymentObservationV1> {
    if program.inline().len() != LOADER_V3_PROGRAM_BYTES
        || usize::try_from(program.data_len()).map_err(|_| Error::ArithmeticOverflow)?
            != LOADER_V3_PROGRAM_BYTES
    {
        return Err(Error::InvalidInlineWidth);
    }
    if programdata.inline().len() != LOADER_V3_PROGRAMDATA_METADATA_BYTES {
        return Err(Error::InvalidInlineWidth);
    }
    if usize::try_from(programdata.data_len()).map_err(|_| Error::ArithmeticOverflow)?
        <= LOADER_V3_PROGRAMDATA_METADATA_BYTES
    {
        // A ProgramData account with no ELF tail is not a deployment; the tail
        // digest would be the empty-string digest and would match any program.
        return Err(Error::InvalidInlineWidth);
    }

    let program_view =
        ProgramV3View::parse(program.inline()).map_err(|_| Error::InvalidLoaderVariant)?;
    let metadata =
        ProgramDataMetadataV3View::parse(programdata.inline()).map_err(|error| match error {
            dclutch_registry_svm::Error::InvalidUpgradeAuthorityTag => {
                Error::InvalidUpgradeAuthorityTag
            }
            _ => Error::InvalidLoaderVariant,
        })?;

    DeploymentObservationV1::new(
        program.key(),
        program.owner(),
        program.executable(),
        programdata.key(),
        programdata.owner(),
        programdata.executable(),
        program_view.programdata(),
        LOADER_V3_PROGRAM_ID,
        metadata.deployment_slot(),
        programdata.tail_digest(),
        metadata.upgrade_authority(),
    )
    .map_err(|_| Error::RecordBindingMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SOLANA_DEVNET_GENESIS_HASH_V1, SOLANA_MAINNET_GENESIS_HASH_V1, put};
    use dclutch_core_contract::ContentId;
    use dclutch_registry_contract::{ArtifactReleaseV1, ArtifactUpgradePolicyV1};
    use dclutch_release_set_contract::ProgramIdentityV1;

    const PROGRAMDATA_KEY: [u8; 32] = [0xf4; 32];

    fn program_account_bytes() -> [u8; LOADER_V3_PROGRAM_BYTES] {
        let mut data = [0u8; LOADER_V3_PROGRAM_BYTES];
        put(&mut data, 0, &2u32.to_le_bytes()).expect("variant");
        put(&mut data, 4, &PROGRAMDATA_KEY).expect("link");
        data
    }

    fn programdata_metadata_bytes(
        slot: u64,
        authority: Option<[u8; 32]>,
    ) -> [u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES] {
        let mut data = [0u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES];
        put(&mut data, 0, &3u32.to_le_bytes()).expect("variant");
        put(&mut data, 4, &slot.to_le_bytes()).expect("slot");
        match authority {
            None => put(&mut data, 12, &[0]).expect("tag"),
            Some(key) => {
                put(&mut data, 12, &[1]).expect("tag");
                put(&mut data, 13, &key).expect("authority");
            }
        }
        data
    }

    #[test]
    fn the_devnet_twin_refuses_on_the_cluster_identity_and_nothing_else() {
        // The venue Program account is byte-identical on both clusters, so an
        // observation of the devnet twin reconstructs to the same deployment
        // identity.  The only thing that separates them is the signed genesis
        // hash, and it must be the thing that refuses.
        assert_eq!(
            require_observed_cluster(
                SOLANA_DEVNET_GENESIS_HASH_V1,
                SOLANA_MAINNET_GENESIS_HASH_V1
            ),
            Err(Error::ObservedClusterMismatch)
        );
        assert_eq!(
            require_observed_cluster(
                SOLANA_MAINNET_GENESIS_HASH_V1,
                SOLANA_MAINNET_GENESIS_HASH_V1
            ),
            Ok(())
        );

        let program_bytes = program_account_bytes();
        let metadata = programdata_metadata_bytes(423_941_138, Some([0x5a; 32]));
        let program = AccountObservationV1::new(
            [0x09; 32],
            LOADER_V3_PROGRAM_ID,
            1,
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("fits"),
            &program_bytes,
            true,
            crate::SHA256_EMPTY_DIGEST,
        )
        .expect("program body");
        let programdata = AccountObservationV1::new(
            PROGRAMDATA_KEY,
            LOADER_V3_PROGRAM_ID,
            1,
            2_326_622,
            &metadata,
            false,
            [0xee; 32],
        )
        .expect("programdata body");
        let observed =
            reconstruct_deployment_observation_v1(program, programdata).expect("reconstructs");

        // The reconstruction is handed to the *existing* release authenticator
        // unchanged.  That is the whole claim of this module: no new
        // authentication primitive, and the ELF digest of a 2.3 MB mainnet
        // program costs 157 wire bytes.
        let release = pinned_release(423_941_138, [0xee; 32], Some([0x5a; 32]));
        assert!(release.authenticate_deployment(observed).is_ok());

        // P-B, executed: a venue redeploy moves the digest and the pinned
        // release refuses, which is what drives the Source to the Product's
        // named failure outcome rather than to a wrong resolution.
        let upgraded = pinned_release(423_941_139, [0xef; 32], Some([0x5a; 32]));
        assert!(upgraded.authenticate_deployment(observed).is_err());

        // A substituted upgrade authority refuses too, on its own field.
        let rotated = pinned_release(423_941_138, [0xee; 32], Some([0x5b; 32]));
        assert!(rotated.authenticate_deployment(observed).is_err());
    }

    fn pinned_release(
        deployment_slot: u64,
        elf_digest: [u8; 32],
        upgrade_authority: Option<[u8; 32]>,
    ) -> ArtifactReleaseV1 {
        ArtifactReleaseV1::new(
            ProgramIdentityV1::new([0x09; 32]).expect("program"),
            ProgramIdentityV1::new(LOADER_V3_PROGRAM_ID).expect("loader"),
            PROGRAMDATA_KEY,
            ContentId::new([0x77; 32]).expect("semantic release"),
            elf_digest,
            deployment_slot,
            match upgrade_authority {
                None => ArtifactUpgradePolicyV1::Immutable,
                Some(_) => ArtifactUpgradePolicyV1::ExactAuthority,
            },
            upgrade_authority,
        )
        .expect("artifact release")
    }

    #[test]
    fn a_programdata_inline_window_other_than_the_metadata_prefix_refuses() {
        let mut wide = [0u8; 46];
        put(&mut wide, 0, &3u32.to_le_bytes()).expect("variant");
        let program_bytes = program_account_bytes();
        let program = AccountObservationV1::new(
            [0x09; 32],
            LOADER_V3_PROGRAM_ID,
            1,
            36,
            &program_bytes,
            true,
            crate::SHA256_EMPTY_DIGEST,
        )
        .expect("program body");
        let programdata = AccountObservationV1::new(
            PROGRAMDATA_KEY,
            LOADER_V3_PROGRAM_ID,
            1,
            2_326_622,
            &wide,
            false,
            [0xee; 32],
        )
        .expect("programdata body");
        assert_eq!(
            reconstruct_deployment_observation_v1(program, programdata),
            Err(Error::InvalidInlineWidth)
        );
    }

    #[test]
    fn a_programdata_account_with_no_elf_tail_refuses() {
        let metadata = programdata_metadata_bytes(1, None);
        let program_bytes = program_account_bytes();
        let program = AccountObservationV1::new(
            [0x09; 32],
            LOADER_V3_PROGRAM_ID,
            1,
            36,
            &program_bytes,
            true,
            crate::SHA256_EMPTY_DIGEST,
        )
        .expect("program body");
        let programdata = AccountObservationV1::new(
            PROGRAMDATA_KEY,
            LOADER_V3_PROGRAM_ID,
            1,
            u32::try_from(LOADER_V3_PROGRAMDATA_METADATA_BYTES).expect("fits"),
            &metadata,
            false,
            crate::SHA256_EMPTY_DIGEST,
        )
        .expect("programdata body");
        assert_eq!(
            reconstruct_deployment_observation_v1(program, programdata),
            Err(Error::InvalidInlineWidth)
        );
    }

    #[test]
    fn a_non_loader_variant_refuses() {
        let mut wrong = [0u8; LOADER_V3_PROGRAM_BYTES];
        put(&mut wrong, 0, &1u32.to_le_bytes()).expect("variant");
        let metadata = programdata_metadata_bytes(1, None);
        let program = AccountObservationV1::new(
            [0x09; 32],
            LOADER_V3_PROGRAM_ID,
            1,
            36,
            &wrong,
            true,
            crate::SHA256_EMPTY_DIGEST,
        )
        .expect("program body");
        let programdata = AccountObservationV1::new(
            PROGRAMDATA_KEY,
            LOADER_V3_PROGRAM_ID,
            1,
            2_326_622,
            &metadata,
            false,
            [0xee; 32],
        )
        .expect("programdata body");
        assert_eq!(
            reconstruct_deployment_observation_v1(program, programdata),
            Err(Error::InvalidLoaderVariant)
        );
    }
}
