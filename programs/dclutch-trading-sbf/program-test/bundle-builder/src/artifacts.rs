//! The emitted artifact set and everything derivable from it alone.
//!
//! One family campaign hands this module ten byte strings — the seven-artifact
//! capability bundle, the ProgramSet, the CapabilityManifest, and the family
//! config record — plus the waist facts. Everything else here is derivation:
//! record digests, raw/staging record addresses, the selected action, and the
//! validated-artifact seal's key and exact body.

use dclutch_capability_program_contract::{
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4},
};
use dclutch_capability_seal_contract::{
    CAPABILITY_SEAL_BYTES_V1, CAPABILITY_SEAL_ROW_COUNT_V1, CapabilitySealKeyV1,
    SealedDescriptorClosureV1, SealedRecordRowV1, SealedRoleV1,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use sha2::{Digest, Sha256};
use solana_program::pubkey::Pubkey;

use crate::{BuilderError, WaistFactsV1};

/// The ten emitted byte strings one family campaign supplies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSetV1<'a> {
    /// CapabilityProgramV4 descriptor record.
    pub descriptor: &'a [u8],
    /// AccountProfileV2 record.
    pub account_profile: &'a [u8],
    /// RequestProfile record (any schema the descriptor names).
    pub request_profile: &'a [u8],
    /// Transition program record.
    pub transition: &'a [u8],
    /// Effect program record.
    pub effect: &'a [u8],
    /// State lifecycle policy record.
    pub lifecycle: &'a [u8],
    /// Execution strategy record.
    pub strategy: &'a [u8],
    /// CapabilityProgramSetV2 record.
    pub program_set: &'a [u8],
    /// CapabilityManifestV1 record.
    pub manifest: &'a [u8],
    /// Family execution-config record.
    pub config: &'a [u8],
}

/// One finalized Registry record: content plus its two derived addresses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedRecordV1 {
    /// Raw finalized record account.
    pub raw: Pubkey,
    /// Vacant staging cursor account.
    pub staging: Pubkey,
    /// Exact record bytes.
    pub bytes: Vec<u8>,
    /// SHA-256 of the record bytes.
    pub digest: [u8; 32],
    /// Schema release identity the record is filed under.
    pub schema: [u8; 32],
    /// Record-owning program (the Registry).
    pub owner: Pubkey,
}

/// Every artifact-set fact derivable without touching the corpus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedArtifactsV1 {
    /// The ten finalized records, addressable by role.
    pub descriptor: DerivedRecordV1,
    /// AccountProfile record.
    pub account_profile: DerivedRecordV1,
    /// RequestProfile record.
    pub request_profile: DerivedRecordV1,
    /// Transition record.
    pub transition: DerivedRecordV1,
    /// Effect record.
    pub effect: DerivedRecordV1,
    /// Lifecycle record.
    pub lifecycle: DerivedRecordV1,
    /// Strategy record.
    pub strategy: DerivedRecordV1,
    /// ProgramSet record.
    pub program_set: DerivedRecordV1,
    /// Manifest record.
    pub manifest: DerivedRecordV1,
    /// Config record.
    pub config: DerivedRecordV1,
    /// The action selector the ProgramSet chose for the family request.
    pub action: u32,
    /// The validated-artifact seal account.
    pub seal: Pubkey,
    /// Exact canonical seal body.
    pub seal_bytes: Vec<u8>,
}

/// Compute SHA-256 of one record body.
pub fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Derive one finalized record's raw and staging-cursor addresses.
pub fn derive_record(owner: Pubkey, schema: [u8; 32], bytes: &[u8]) -> DerivedRecordV1 {
    let content = digest(bytes);
    let raw = Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &content], &owner).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &content], &owner).0;
    DerivedRecordV1 {
        raw,
        staging,
        bytes: bytes.to_vec(),
        digest: content,
        schema,
        owner,
    }
}

/// Derive every artifact-set fact: identities, record addresses, the selected
/// action, and the seal.
///
/// `family_request` is consumed only for action selection — the ProgramSet is
/// the authority for which descriptor a request selects, so the builder asks
/// it rather than taking an action number from the campaign.
pub fn derive_artifact_facts(
    set: ArtifactSetV1<'_>,
    waist: WaistFactsV1,
    family_request: &[u8],
) -> Result<DerivedArtifactsV1, BuilderError> {
    let descriptor_digest = digest(set.descriptor);
    let descriptor =
        CapabilityProgramV4::decode(set.descriptor).map_err(|_| BuilderError::Artifact)?;

    let program_set_digest = digest(set.program_set);
    let program_set = CapabilityProgramSetV2::decode_selected(
        program_set_digest,
        program_set_digest,
        set.program_set,
    )
    .map_err(|_| BuilderError::Artifact)?;
    let entry = program_set
        .select_entry(family_request)
        .map_err(|_| BuilderError::Artifact)?;
    if entry.descriptor().schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_ID_V4
        || entry.descriptor().program().to_bytes() != descriptor_digest
    {
        return Err(BuilderError::Artifact);
    }
    let action = entry.selector();

    let registry = waist.registry_program;
    let records = DerivedArtifactsV1 {
        descriptor: derive_record(registry, CAPABILITY_PROGRAM_SCHEMA_ID_V4, set.descriptor),
        account_profile: derive_record(
            registry,
            descriptor.account_profile().schema().to_bytes(),
            set.account_profile,
        ),
        request_profile: derive_record(
            registry,
            descriptor.request_profile().schema().to_bytes(),
            set.request_profile,
        ),
        transition: derive_record(
            registry,
            descriptor.transition().schema().to_bytes(),
            set.transition,
        ),
        effect: derive_record(
            registry,
            descriptor.effect().schema().to_bytes(),
            set.effect,
        ),
        lifecycle: derive_record(
            registry,
            descriptor.lifecycle().schema().to_bytes(),
            set.lifecycle,
        ),
        strategy: derive_record(
            registry,
            descriptor.strategy().schema().to_bytes(),
            set.strategy,
        ),
        program_set: derive_record(
            registry,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            set.program_set,
        ),
        manifest: derive_record(
            registry,
            dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            set.manifest,
        ),
        config: derive_record(registry, descriptor.config_schema().to_bytes(), set.config),
        action,
        seal: Pubkey::default(),
        seal_bytes: Vec::new(),
    };
    let (seal, seal_bytes) = derive_seal(&records, waist, action)?;
    Ok(DerivedArtifactsV1 {
        seal,
        seal_bytes,
        ..records
    })
}

/// Derive the validated-artifact seal key and exact body from the records.
///
/// Decision 0005: the seal is a Trading PDA of (descriptor schema, descriptor
/// digest, action, Trading interpreter semantic release, Registry). Its six
/// rows restate the six sealed records exactly as the on-chain seal outer must
/// write them.
fn derive_seal(
    records: &DerivedArtifactsV1,
    waist: WaistFactsV1,
    action: u32,
) -> Result<(Pubkey, Vec<u8>), BuilderError> {
    let key = CapabilitySealKeyV1::new(
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        records.descriptor.digest,
        action,
        waist.trading_semantic_release,
        waist.registry_program.to_bytes(),
    )
    .map_err(|_| BuilderError::Artifact)?;
    let (seal, seal_bump) =
        Pubkey::find_program_address(&key.seeds().as_slices(), &waist.trading_program);
    let rows = [
        (SealedRoleV1::Descriptor, &records.descriptor),
        (SealedRoleV1::LifecyclePolicy, &records.lifecycle),
        (SealedRoleV1::AccountProfile, &records.account_profile),
        (SealedRoleV1::RequestProfile, &records.request_profile),
        (SealedRoleV1::TransitionProgram, &records.transition),
        (SealedRoleV1::EffectProgram, &records.effect),
    ]
    .into_iter()
    .map(|(role, record)| {
        SealedRecordRowV1::new(
            role,
            u32::try_from(record.bytes.len()).map_err(|_| BuilderError::Arithmetic)?,
            record.schema,
            record.digest,
            record.raw.to_bytes(),
            record.staging.to_bytes(),
        )
        .map_err(|_| BuilderError::Artifact)
    })
    .collect::<Result<Vec<_>, _>>()?;
    let rows: [SealedRecordRowV1; CAPABILITY_SEAL_ROW_COUNT_V1] =
        rows.try_into().map_err(|_| BuilderError::Artifact)?;
    let mut bytes = vec![0_u8; CAPABILITY_SEAL_BYTES_V1];
    SealedDescriptorClosureV1::encode(key, rows, seal_bump, &mut bytes)
        .map_err(|_| BuilderError::Artifact)?;
    Ok((seal, bytes))
}
