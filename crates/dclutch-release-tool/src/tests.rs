use dclutch_pyth_svm::{PYTH_RELEASE_V1_ENCODED_LEN, PythReleaseV1Input};

use super::*;

struct Fixture {
    elf: Vec<u8>,
    semantic: Vec<u8>,
    program: Vec<u8>,
    programdata: Vec<u8>,
    metadata: BuildMetadataV1,
}

impl Fixture {
    fn capability() -> Result<Self> {
        Self::new(
            "capability",
            b"dclutch/test/capability-semantic-release/v1".to_vec(),
            Some([9; 32]),
        )
    }

    fn new(kind: &str, semantic: Vec<u8>, authority: Option<[u8; 32]>) -> Result<Self> {
        let elf = sbf_elf()?;
        let mut program = vec![0_u8; LOADER_V3_PROGRAM_BYTES];
        put(&mut program, 0, &2_u32.to_le_bytes())?;
        put(&mut program, 4, &[2; 32])?;
        let mut programdata = vec![0_u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES + elf.len() + 8];
        put(&mut programdata, 0, &3_u32.to_le_bytes())?;
        put(&mut programdata, 4, &77_u64.to_le_bytes())?;
        if let Some(value) = authority {
            put(&mut programdata, 12, &[1])?;
            put(&mut programdata, 13, &value)?;
        }
        put(&mut programdata, LOADER_V3_PROGRAMDATA_METADATA_BYTES, &elf)?;
        let metadata = BuildMetadataV1::parse(&metadata_text(kind, "source-commit-abc"))?;
        Ok(Self {
            elf,
            semantic,
            program,
            programdata,
            metadata,
        })
    }

    fn evidence(&self) -> ReleaseEvidenceV1<'_> {
        ReleaseEvidenceV1 {
            elf: &self.elf,
            semantic_preimage: &self.semantic,
            program_account_data: &self.program,
            programdata_account_data: &self.programdata,
            metadata: &self.metadata,
        }
    }
}

fn sbf_elf() -> Result<Vec<u8>> {
    let mut elf = vec![0_u8; ELF_HEADER_BYTES];
    put(&mut elf, 0, &[0x7f, b'E', b'L', b'F'])?;
    put(&mut elf, 4, &[ELF_CLASS_64])?;
    put(&mut elf, 5, &[ELF_DATA_LITTLE_ENDIAN])?;
    put(&mut elf, 6, &[ELF_CURRENT_VERSION])?;
    put(&mut elf, 16, &ELF_TYPE_SHARED_OBJECT.to_le_bytes())?;
    put(&mut elf, 18, &ELF_MACHINE_BPF.to_le_bytes())?;
    put(&mut elf, 20, &u32::from(ELF_CURRENT_VERSION).to_le_bytes())?;
    put(&mut elf, 52, &64_u16.to_le_bytes())?;
    Ok(elf)
}

fn metadata_text(kind: &str, revision: &str) -> String {
    format!(
        "{RELEASE_METADATA_HEADER_V1}\nsemantic_kind={kind}\nprogram_id={}\nprogramdata_id={}\nloader_program_id={}\nprogram_owner={}\nprogram_executable=true\nprogramdata_owner={}\nprogramdata_executable=false\nsource_digest={}\ncargo_lock_digest={}\nsource_revision={revision}\nrustc_version=rustc 1.89.0\nsolana_version=solana-cli 3.0.0\ncargo_build_sbf_version=cargo-build-sbf 3.0.0\ntarget_triple=sbf-solana-solana\nbuild_command=cargo build-sbf --manifest-path programs/dclutch-sbf/Cargo.toml\nassumption=account snapshots were captured after the named deployment slot\nassumption=source digest covers every first-party build input\n",
        repeated_hex(1),
        repeated_hex(2),
        repeated_hex(3),
        repeated_hex(3),
        repeated_hex(3),
        repeated_hex(4),
        repeated_hex(5),
    )
}

fn repeated_hex(value: u8) -> String {
    format!("{value:02x}").repeat(32)
}

fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::ArithmeticOverflow)?;
    bytes
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn pyth_semantic_preimage() -> Result<Vec<u8>> {
    let release = PythReleaseV1::new(PythReleaseV1Input {
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
        receiver_deployment_slot: 12,
        router_deployment_slot: 13,
        guardian_set_count: 5,
        required_guardian_count: 3,
        upstream_commit: [14; 20],
        sdk_crate_digest: [15; 32],
        activation_time: 16,
    })
    .map_err(Error::InvalidPythRelease)?;
    Ok(release.to_bytes().to_vec())
}

#[test]
fn checked_release_round_trips_and_text_surfaces_every_boundary() -> Result<()> {
    let fixture = Fixture::capability()?;
    let release = build_checked_release(fixture.evidence())?;
    let bytes = release.encode()?;
    assert_eq!(CheckedReleaseV1::decode(&bytes), Ok(release.clone()));
    assert_eq!(
        verify_checked_release(&bytes, fixture.evidence()),
        Ok(release.clone())
    );
    assert_eq!(
        build_checked_release(fixture.evidence()),
        Ok(release.clone())
    );
    assert_eq!(release.deployment_slot(), 77);
    assert_eq!(release.upgrade_authority(), Some([9; 32]));
    assert_eq!(bytes.len(), release.encoded_len()?);

    let text = release.render_text()?;
    for required in [
        "semantic_kind=capability\n",
        "elf_machine=BPF-SBF\n",
        "loader_profile=upgradeable-loader-v3\n",
        "deployment_slot=77\n",
        "source_revision=source-commit-abc\n",
        "rustc_version=rustc 1.89.0\n",
        "solana_version=solana-cli 3.0.0\n",
        "cargo_build_sbf_version=cargo-build-sbf 3.0.0\n",
        "assumption=account snapshots were captured after the named deployment slot\n",
    ] {
        assert!(text.contains(required));
    }
    assert_eq!(text, release.render_text()?);
    Ok(())
}

#[test]
fn canonical_metadata_refuses_aliases_reordering_and_ambiguous_text() -> Result<()> {
    let canonical = metadata_text("capability", "source-commit-abc");
    assert!(BuildMetadataV1::parse(&canonical).is_ok());
    assert_eq!(
        BuildMetadataV1::parse(canonical.trim_end()),
        Err(Error::InvalidMetadata)
    );
    assert_eq!(
        BuildMetadataV1::parse(&canonical.replacen(&repeated_hex(4), &"AA".repeat(32), 1)),
        Err(Error::InvalidHex)
    );
    assert_eq!(
        BuildMetadataV1::parse(&canonical.replacen(
            &format!("program_owner={}", repeated_hex(3)),
            &format!("program_owner={}", repeated_hex(8)),
            1,
        )),
        Err(Error::LoaderObservationMismatch)
    );
    let reversed = canonical.replace(
        "assumption=account snapshots were captured after the named deployment slot\nassumption=source digest covers every first-party build input\n",
        "assumption=source digest covers every first-party build input\nassumption=account snapshots were captured after the named deployment slot\n",
    );
    assert_eq!(
        BuildMetadataV1::parse(&reversed),
        Err(Error::NonCanonicalAssumptions)
    );
    let duplicate = canonical.replace(
        "assumption=source digest covers every first-party build input",
        "assumption=account snapshots were captured after the named deployment slot",
    );
    assert_eq!(
        BuildMetadataV1::parse(&duplicate),
        Err(Error::NonCanonicalAssumptions)
    );
    Ok(())
}

#[test]
fn hostile_binary_manifests_and_evidence_tampering_refuse() -> Result<()> {
    let fixture = Fixture::capability()?;
    let release = build_checked_release(fixture.evidence())?;
    let bytes = release.encode()?;
    for length in 0..bytes.len() {
        let truncated = bytes.get(..length).ok_or(Error::InvalidLength)?;
        assert!(CheckedReleaseV1::decode(truncated).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        CheckedReleaseV1::decode(&trailing),
        Err(Error::InvalidManifestLength)
    );
    for (offset, value, expected) in [
        (0, 0, Error::InvalidMagic),
        (8, 2, Error::UnsupportedSchema),
        (RESERVED_OFFSET, 1, Error::NonCanonicalReservedBytes),
        (SEMANTIC_KIND_OFFSET, 9, Error::UnknownSemanticKind),
        (LOADER_KIND_OFFSET, 9, Error::UnknownLoaderKind),
        (
            AUTHORITY_KIND_OFFSET,
            AUTHORITY_NONE,
            Error::NonCanonicalUpgradeAuthority,
        ),
    ] {
        let mut hostile = bytes.clone();
        *hostile.get_mut(offset).ok_or(Error::InvalidLength)? = value;
        assert_eq!(CheckedReleaseV1::decode(&hostile), Err(expected));
    }
    let mut false_digest = bytes.clone();
    *false_digest
        .get_mut(ARTIFACT_DIGEST_OFFSET)
        .ok_or(Error::InvalidLength)? ^= 1;
    let decoded = CheckedReleaseV1::decode(&false_digest)?;
    assert_ne!(decoded.artifact_digest(), release.artifact_digest());
    assert_eq!(
        verify_checked_release(&false_digest, fixture.evidence()),
        Err(Error::CheckedManifestMismatch)
    );

    let mut changed_revision = Fixture::capability()?;
    changed_revision.metadata =
        BuildMetadataV1::parse(&metadata_text("capability", "different-source-commit"))?;
    assert_eq!(
        verify_checked_release(&bytes, changed_revision.evidence()),
        Err(Error::CheckedManifestMismatch)
    );
    Ok(())
}

#[test]
fn loader_link_exact_elf_and_zero_padding_are_all_required() -> Result<()> {
    let fixture = Fixture::capability()?;

    let mut wrong_link = Fixture::capability()?;
    put(&mut wrong_link.program, 4, &[8; 32])?;
    assert_eq!(
        build_checked_release(wrong_link.evidence()),
        Err(Error::ProgramDataLinkMismatch)
    );

    let mut changed_elf = Fixture::capability()?;
    *changed_elf.elf.get_mut(63).ok_or(Error::InvalidLength)? = 1;
    assert_eq!(
        build_checked_release(changed_elf.evidence()),
        Err(Error::DeployedElfMismatch)
    );

    let mut changed_payload = Fixture::capability()?;
    let payload_byte = LOADER_V3_PROGRAMDATA_METADATA_BYTES
        .checked_add(63)
        .ok_or(Error::ArithmeticOverflow)?;
    *changed_payload
        .programdata
        .get_mut(payload_byte)
        .ok_or(Error::InvalidLength)? = 1;
    assert_eq!(
        build_checked_release(changed_payload.evidence()),
        Err(Error::DeployedElfMismatch)
    );

    let mut nonzero_padding = Fixture::capability()?;
    let last = nonzero_padding
        .programdata
        .len()
        .checked_sub(1)
        .ok_or(Error::ArithmeticOverflow)?;
    *nonzero_padding
        .programdata
        .get_mut(last)
        .ok_or(Error::InvalidLength)? = 1;
    assert_eq!(
        build_checked_release(nonzero_padding.evidence()),
        Err(Error::NonZeroProgramDataPadding)
    );

    let mut wrong_variant = Fixture::capability()?;
    put(&mut wrong_variant.programdata, 0, &2_u32.to_le_bytes())?;
    assert!(matches!(
        build_checked_release(wrong_variant.evidence()),
        Err(Error::LoaderV3(LoaderV3Error::InvalidProgramDataVariant {
            variant: 2
        }))
    ));

    let mut none = Fixture::new("capability", fixture.semantic.clone(), None)?;
    let immutable = build_checked_release(none.evidence())?;
    assert_eq!(immutable.upgrade_authority(), None);
    *none.programdata.get_mut(13).ok_or(Error::InvalidLength)? = 1;
    assert_eq!(
        build_checked_release(none.evidence()),
        Err(Error::NonCanonicalUpgradeAuthority)
    );
    Ok(())
}

#[test]
fn sbf_header_and_pyth_semantic_owner_are_not_bypassed() -> Result<()> {
    for offset in [0, 4, 5, 6, 16, 18, 20, 52] {
        let mut hostile = Fixture::capability()?;
        *hostile.elf.get_mut(offset).ok_or(Error::InvalidLength)? ^= 1;
        assert_eq!(
            build_checked_release(hostile.evidence()),
            Err(Error::InvalidSbfElf)
        );
    }

    let semantic = pyth_semantic_preimage()?;
    assert_eq!(semantic.len(), PYTH_RELEASE_V1_ENCODED_LEN);
    let pyth = Fixture::new("pyth-v1", semantic, Some([9; 32]))?;
    let checked = build_checked_release(pyth.evidence())?;
    assert_eq!(CheckedReleaseV1::decode(&checked.encode()?), Ok(checked));

    let mut hostile = Fixture::new("pyth-v1", pyth_semantic_preimage()?, Some([9; 32]))?;
    *hostile.semantic.get_mut(0).ok_or(Error::InvalidLength)? ^= 1;
    assert!(matches!(
        build_checked_release(hostile.evidence()),
        Err(Error::InvalidPythRelease(PythReleaseV1Error::InvalidMagic))
    ));
    Ok(())
}
