use clutch_source_plane_v3::{
    ContentId, FixedCodec, RawRecordV3, SourcePlaneProgramV3, MAX_SOURCE_VALUE,
    SOURCE_PLANE_PROGRAM_BYTES,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use sha2::{Digest, Sha256};

use crate::{Error, Result};

const ACCOUNT_DATA_DOMAIN: &[u8] = b"dragons-clutch/runtime-account-data/v1";
const DEPLOYMENT_DOMAIN: &[u8] = b"dragons-clutch/runtime-deployment-binding/v1";
const CLOCK_POLICY_DOMAIN: &[u8] = b"dragons-clutch/source-clock-policy/v1";
const SOURCE_RELEASE_DOMAIN: &[u8] = b"dragons-clutch/source-release-manifest/v1";
const SOURCE_RELEASE_AUTH_DOMAIN: &[u8] = b"dragons-clutch/authenticated-source-release/v1";
const SOURCE_ROUTE_AUTH_DOMAIN: &[u8] = b"dragons-clutch/authenticated-source-route/v1";
const PARSER_OUTPUT_DOMAIN: &[u8] = b"dragons-clutch/source-parser-output/v1";
const INVOCATION_DOMAIN: &[u8] = b"dragons-clutch/runtime-invocation/v1";
const BOUNDARY_DOMAIN: &[u8] = b"dragons-clutch/source-boundary-receipt/v1";
const CLOCK_BUCKET_DOMAIN: &[u8] = b"dragons-clutch/authenticated-clock-bucket/v1";

const SOURCE_RELEASE_MAGIC: [u8; 8] = *b"DCSREL01";
const CLOCK_POLICY_MAGIC: [u8; 8] = *b"DCCLOCK1";
const SCHEMA_V1: u16 = 1;

/// Exact canonical bytes in [`ClockPolicyV1`].
pub const CLOCK_POLICY_BYTES: usize = 64;
/// Exact canonical bytes in [`SourceReleaseManifestV1`].
pub const SOURCE_RELEASE_MANIFEST_BYTES: usize = 1_008;

/// A runtime account/program address, kept distinct from a content digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct RuntimeKey([u8; 32]);

impl RuntimeKey {
    /// Reserved absent/padding key.
    pub const ZERO: Self = Self([0; 32]);

    /// Construct from exact runtime key bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return exact runtime key bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    /// Whether this is the reserved all-zero key.
    pub fn is_zero(self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }

    pub(crate) fn validate(self) -> Result<()> {
        if self.is_zero() {
            Err(Error::ZeroIdentity)
        } else {
            Ok(())
        }
    }
}

/// Complete runtime-observed account view consumed by pure authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAccountViewV1<'a> {
    /// Account address supplied by the runtime.
    pub key: RuntimeKey,
    /// Account owner supplied by the runtime.
    pub owner: RuntimeKey,
    /// Native balance supplied by the runtime.
    pub lamports: u64,
    /// Runtime executable bit.
    pub executable: bool,
    /// Instruction writable privilege.
    pub writable: bool,
    /// Instruction signer privilege.
    pub signer: bool,
    /// Complete account data, not a prefix or host projection.
    pub data: &'a [u8],
}

/// Digest complete account data together with its runtime address.
pub fn account_data_id(key: RuntimeKey, data: &[u8]) -> Result<ContentId> {
    key.validate()?;
    let mut hasher = Sha256::new();
    hasher.update(ACCOUNT_DATA_DOMAIN);
    hasher.update(key.bytes());
    hasher.update(
        u64::try_from(data.len())
            .map_err(|_| Error::ArithmeticOverflow)?
            .to_le_bytes(),
    );
    hasher.update(data);
    Ok(ContentId::from_bytes(hasher.finalize().into()))
}

pub(crate) fn account_data_parts_id(
    key: RuntimeKey,
    total_len: usize,
    first: &[u8],
    second: &[u8],
) -> Result<ContentId> {
    key.validate()?;
    if first
        .len()
        .checked_add(second.len())
        .ok_or(Error::ArithmeticOverflow)?
        != total_len
    {
        return Err(Error::InvalidCodec);
    }
    let mut hasher = Sha256::new();
    hasher.update(ACCOUNT_DATA_DOMAIN);
    hasher.update(key.bytes());
    hasher.update(
        u64::try_from(total_len)
            .map_err(|_| Error::ArithmeticOverflow)?
            .to_le_bytes(),
    );
    hasher.update(first);
    hasher.update(second);
    Ok(ContentId::from_bytes(hasher.finalize().into()))
}

/// Exact reviewed program/ProgramData release binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeploymentBindingV1 {
    /// Executable program account.
    pub program: RuntimeKey,
    /// Frozen digest of the complete program account bytes.
    pub program_account_data_id: ContentId,
    /// ProgramData account linked inside the program account.
    pub programdata: RuntimeKey,
    /// Frozen digest of the complete ProgramData account bytes, including ELF.
    pub programdata_account_data_id: ContentId,
    /// Exact loader owning both accounts.
    pub loader: RuntimeKey,
    /// Byte offset of the 32-byte ProgramData key in the program account.
    pub programdata_link_offset: u16,
    /// Byte offset of the little-endian deployment slot in ProgramData.
    pub deployment_slot_offset: u16,
    /// Frozen deployment slot.
    pub deployment_slot: u64,
}

impl DeploymentBindingV1 {
    /// Validate closed deployment coordinates and content digests.
    pub fn validate(&self) -> Result<()> {
        self.program.validate()?;
        self.programdata.validate()?;
        self.loader.validate()?;
        live_id(self.program_account_data_id)?;
        live_id(self.programdata_account_data_id)?;
        if self.program == self.programdata
            || self.program == self.loader
            || self.programdata == self.loader
        {
            return Err(Error::IdentityAlias);
        }
        usize::from(self.programdata_link_offset)
            .checked_add(32)
            .ok_or(Error::ArithmeticOverflow)?;
        usize::from(self.deployment_slot_offset)
            .checked_add(8)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Content identity of the exact release coordinates.
    pub fn id(&self) -> Result<ContentId> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(DEPLOYMENT_DOMAIN);
        hasher.update(self.program.bytes());
        hasher.update(self.program_account_data_id.bytes());
        hasher.update(self.programdata.bytes());
        hasher.update(self.programdata_account_data_id.bytes());
        hasher.update(self.loader.bytes());
        hasher.update(self.programdata_link_offset.to_le_bytes());
        hasher.update(self.deployment_slot_offset.to_le_bytes());
        hasher.update(self.deployment_slot.to_le_bytes());
        Ok(ContentId::from_bytes(hasher.finalize().into()))
    }

    pub(crate) fn authenticate(
        &self,
        program: RuntimeAccountViewV1<'_>,
        programdata: RuntimeAccountViewV1<'_>,
    ) -> Result<ContentId> {
        self.validate()?;
        if program.key != self.program || programdata.key != self.programdata {
            return Err(Error::WrongAccount);
        }
        if program.owner != self.loader || programdata.owner != self.loader {
            return Err(Error::WrongOwner);
        }
        if !program.executable || programdata.executable {
            return Err(Error::WrongExecutableState);
        }
        if program.signer || program.writable || programdata.signer || programdata.writable {
            return Err(Error::WrongPrivilege);
        }
        if account_data_id(program.key, program.data)? != self.program_account_data_id
            || account_data_id(programdata.key, programdata.data)?
                != self.programdata_account_data_id
        {
            return Err(Error::WrongAccountData);
        }
        let link_at = usize::from(self.programdata_link_offset);
        let link_end = link_at.checked_add(32).ok_or(Error::ArithmeticOverflow)?;
        if link_end > program.data.len()
            || program.data[link_at..link_end] != self.programdata.bytes()
        {
            return Err(Error::WrongProgramDataLink);
        }
        let slot_at = usize::from(self.deployment_slot_offset);
        let slot_end = slot_at.checked_add(8).ok_or(Error::ArithmeticOverflow)?;
        if slot_end > programdata.data.len()
            || le_u64(&programdata.data[slot_at..slot_end]) != self.deployment_slot
        {
            return Err(Error::WrongDeploymentSlot);
        }
        self.id()
    }
}

/// Immutable mapping from Unix time to canonical source buckets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockPolicyV1 {
    /// Unix timestamp at the beginning of bucket zero.
    pub anchor_unix_timestamp: u64,
    /// Width of every canonical bucket in seconds.
    pub bucket_seconds: u32,
    /// Maximum delay after a boundary at which it can be archived.
    pub maximum_boundary_lateness_seconds: u32,
    /// Maximum age of a source publication at admission.
    pub maximum_source_age_seconds: u32,
    /// Maximum source publication slot lag at admission.
    pub maximum_source_slot_lag: u32,
}

impl ClockPolicyV1 {
    /// Validate finite nonzero timing bounds.
    pub fn validate(&self) -> Result<()> {
        if self.bucket_seconds == 0
            || self.maximum_boundary_lateness_seconds == 0
            || self.maximum_source_age_seconds == 0
            || self.maximum_source_slot_lag == 0
        {
            return Err(Error::OutsideClockWindow);
        }
        Ok(())
    }

    /// Canonical fixed bytes.
    pub fn encode(&self) -> Result<[u8; CLOCK_POLICY_BYTES]> {
        self.validate()?;
        let mut out = [0; CLOCK_POLICY_BYTES];
        out[..8].copy_from_slice(&CLOCK_POLICY_MAGIC);
        out[8..10].copy_from_slice(&SCHEMA_V1.to_le_bytes());
        out[16..24].copy_from_slice(&self.anchor_unix_timestamp.to_le_bytes());
        out[24..28].copy_from_slice(&self.bucket_seconds.to_le_bytes());
        out[28..32].copy_from_slice(&self.maximum_boundary_lateness_seconds.to_le_bytes());
        out[32..36].copy_from_slice(&self.maximum_source_age_seconds.to_le_bytes());
        out[36..40].copy_from_slice(&self.maximum_source_slot_lag.to_le_bytes());
        Ok(out)
    }

    /// Content identity of the exact time/bucket policy.
    pub fn id(&self) -> Result<ContentId> {
        let bytes = self.encode()?;
        Ok(domain_id(CLOCK_POLICY_DOMAIN, &bytes))
    }

    /// Exclusive Unix timestamp ending one bucket.
    pub fn boundary_timestamp(&self, bucket: u64) -> Result<u64> {
        self.validate()?;
        self.anchor_unix_timestamp
            .checked_add(
                bucket
                    .checked_add(1)
                    .and_then(|value| value.checked_mul(u64::from(self.bucket_seconds)))
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Unix timestamp at one bucket coordinate, including exclusive boundaries.
    pub fn bucket_timestamp(&self, bucket: u64) -> Result<u64> {
        self.validate()?;
        self.anchor_unix_timestamp
            .checked_add(
                bucket
                    .checked_mul(u64::from(self.bucket_seconds))
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Canonical bucket containing one nonnegative Unix timestamp.
    pub fn bucket_at_timestamp(&self, unix_timestamp: u64) -> Result<u64> {
        self.validate()?;
        let elapsed = unix_timestamp
            .checked_sub(self.anchor_unix_timestamp)
            .ok_or(Error::OutsideClockWindow)?;
        Ok(elapsed / u64::from(self.bucket_seconds))
    }
}

/// Adapter-authenticated Clock sysvar projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockSnapshotV1 {
    /// Current bank slot.
    pub slot: u64,
    /// Current nonnegative Unix timestamp.
    pub unix_timestamp: u64,
}

/// Policy-bound canonical bucket derived from one adapter-supplied Clock snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedClockBucketV1 {
    policy_id: ContentId,
    snapshot: ClockSnapshotV1,
    bucket: u64,
    receipt_id: ContentId,
}

impl AuthenticatedClockBucketV1 {
    /// Derive the sole canonical bucket without accepting a caller-provided coordinate.
    pub fn from_snapshot(policy: &ClockPolicyV1, snapshot: ClockSnapshotV1) -> Result<Self> {
        let policy_id = policy.id()?;
        let bucket = policy.bucket_at_timestamp(snapshot.unix_timestamp)?;
        let mut bytes = [0; 56];
        bytes[..32].copy_from_slice(&policy_id.bytes());
        bytes[32..40].copy_from_slice(&snapshot.slot.to_le_bytes());
        bytes[40..48].copy_from_slice(&snapshot.unix_timestamp.to_le_bytes());
        bytes[48..56].copy_from_slice(&bucket.to_le_bytes());
        Ok(Self {
            policy_id,
            snapshot,
            bucket,
            receipt_id: domain_id(CLOCK_BUCKET_DOMAIN, &bytes),
        })
    }

    /// Exact ClockPolicy identity used for floor mapping.
    pub const fn policy_id(self) -> ContentId {
        self.policy_id
    }

    /// Exact adapter-supplied Clock snapshot committed by this receipt.
    pub const fn snapshot(self) -> ClockSnapshotV1 {
        self.snapshot
    }

    /// Canonical containing bucket.
    pub const fn bucket(self) -> u64 {
        self.bucket
    }

    /// Complete policy/snapshot/bucket receipt identity.
    pub const fn id(self) -> ContentId {
        self.receipt_id
    }
}

/// Immutable complete source-release manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceReleaseManifestV1 {
    /// Sole exact semantic SourcePlane core compatibility contract body.
    pub source_plane: SourcePlaneProgramV3,
    /// Existing SourceSpec semantic identity.
    pub source_spec_id: ContentId,
    /// Existing immutable SourceSpec account address.
    pub source_spec_account: RuntimeKey,
    /// Exact deployed program owning the SourceSpec account.
    pub source_spec_owner: RuntimeKey,
    /// Digest of the complete canonical SourceSpec account bytes.
    pub source_spec_account_data_id: ContentId,
    /// Reviewed runtime adapter deployment.
    pub adapter: DeploymentBindingV1,
    /// Reviewed source parser deployment.
    pub parser: DeploymentBindingV1,
    /// Immutable parser/config account.
    pub parser_config: RuntimeKey,
    /// Exact config owner.
    pub parser_config_owner: RuntimeKey,
    /// Digest of complete config account bytes.
    pub parser_config_data_id: ContentId,
    /// Mutable source feed account address.
    pub feed: RuntimeKey,
    /// Exact source program owning the feed.
    pub feed_owner: RuntimeKey,
    /// Program owning immutable initial/repair generation requests.
    pub generation_authority_program: RuntimeKey,
    /// Exact System Program used to recognize an unallocated absent PDA.
    pub system_program: RuntimeKey,
    /// Sole immutable Clock/bucket policy body; its identity is derived.
    pub clock_policy: ClockPolicyV1,
    /// Immutable heterogeneous Source work schedule identity.
    pub source_work_schedule_id: ContentId,
    /// Runtime liveness policy funding Source calls.
    pub liveness_policy_id: ContentId,
    /// Source compartment account identity.
    pub source_compartment_account: RuntimeKey,
    /// Sole Source semantic owner used by liveness call receipts.
    pub source_compartment_owner: RuntimeKey,
    /// Frozen neutral sink for prefunds and unsolicited surplus.
    pub neutral_sink: RuntimeKey,
}

impl SourceReleaseManifestV1 {
    /// Validate all immutable identities and role separation.
    pub fn validate(&self) -> Result<()> {
        self.source_plane.validate()?;
        live_id(self.source_spec_id)?;
        self.source_spec_account.validate()?;
        self.source_spec_owner.validate()?;
        live_id(self.source_spec_account_data_id)?;
        self.adapter.validate()?;
        self.parser.validate()?;
        self.parser_config.validate()?;
        self.parser_config_owner.validate()?;
        live_id(self.parser_config_data_id)?;
        self.feed.validate()?;
        self.feed_owner.validate()?;
        self.generation_authority_program.validate()?;
        self.system_program.validate()?;
        self.clock_policy.validate()?;
        live_id(self.source_work_schedule_id)?;
        live_id(self.liveness_policy_id)?;
        self.source_compartment_account.validate()?;
        self.source_compartment_owner.validate()?;
        self.neutral_sink.validate()?;
        if self.adapter.program == self.parser.program
            || self.parser_config == self.feed
            || self.source_spec_account == self.feed
            || self.source_spec_account == self.parser_config
            || self.generation_authority_program == self.neutral_sink
            || self.source_compartment_account == self.neutral_sink
            || self.source_compartment_owner == self.neutral_sink
        {
            return Err(Error::IdentityAlias);
        }
        Ok(())
    }

    /// Exact fixed manifest bytes.
    pub fn encode(&self) -> Result<[u8; SOURCE_RELEASE_MANIFEST_BYTES]> {
        self.validate()?;
        let mut out = [0; SOURCE_RELEASE_MANIFEST_BYTES];
        out[..8].copy_from_slice(&SOURCE_RELEASE_MAGIC);
        out[8..10].copy_from_slice(&SCHEMA_V1.to_le_bytes());
        let mut at = 16;
        let source_plane_end = at
            .checked_add(SOURCE_PLANE_PROGRAM_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        if source_plane_end > out.len() {
            return Err(Error::InvalidCodec);
        }
        self.source_plane
            .encode_into(&mut out[at..source_plane_end])?;
        at = source_plane_end;
        put_id(&mut out, &mut at, self.source_spec_id);
        encode_deployment(&self.adapter, &mut out, &mut at);
        encode_deployment(&self.parser, &mut out, &mut at);
        for key in [
            self.parser_config,
            self.parser_config_owner,
            self.source_spec_account,
            self.source_spec_owner,
            self.feed,
            self.feed_owner,
            self.generation_authority_program,
            self.system_program,
            self.source_compartment_account,
            self.source_compartment_owner,
            self.neutral_sink,
        ] {
            put_key(&mut out, &mut at, key);
        }
        for id in [
            self.parser_config_data_id,
            self.source_spec_account_data_id,
            self.source_work_schedule_id,
            self.liveness_policy_id,
        ] {
            put_id(&mut out, &mut at, id);
        }
        let clock_policy_bytes = self.clock_policy.encode()?;
        let end = at
            .checked_add(CLOCK_POLICY_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        if end > out.len() {
            return Err(Error::InvalidCodec);
        }
        out[at..end].copy_from_slice(&clock_policy_bytes);
        at = end;
        if at > out.len() {
            return Err(Error::InvalidCodec);
        }
        Ok(out)
    }

    /// Hostile-decode one exact immutable Source release account body.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != SOURCE_RELEASE_MANIFEST_BYTES
            || input[..8] != SOURCE_RELEASE_MAGIC
            || le_u16(&input[8..10]) != SCHEMA_V1
            || input[10..16].iter().any(|byte| *byte != 0)
        {
            return Err(Error::InvalidCodec);
        }
        let mut at = 16_usize;
        let source_plane_end = at
            .checked_add(SOURCE_PLANE_PROGRAM_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        if source_plane_end > input.len() {
            return Err(Error::InvalidCodec);
        }
        let source_plane = SourcePlaneProgramV3::decode(&input[at..source_plane_end])?;
        at = source_plane_end;
        let source_spec_id = take_id(input, &mut at);
        let adapter = decode_deployment(input, &mut at)?;
        let parser = decode_deployment(input, &mut at)?;
        let parser_config = take_key(input, &mut at);
        let parser_config_owner = take_key(input, &mut at);
        let source_spec_account = take_key(input, &mut at);
        let source_spec_owner = take_key(input, &mut at);
        let feed = take_key(input, &mut at);
        let feed_owner = take_key(input, &mut at);
        let generation_authority_program = take_key(input, &mut at);
        let system_program = take_key(input, &mut at);
        let source_compartment_account = take_key(input, &mut at);
        let source_compartment_owner = take_key(input, &mut at);
        let neutral_sink = take_key(input, &mut at);
        let parser_config_data_id = take_id(input, &mut at);
        let source_spec_account_data_id = take_id(input, &mut at);
        let source_work_schedule_id = take_id(input, &mut at);
        let liveness_policy_id = take_id(input, &mut at);
        let clock_policy_end = at
            .checked_add(CLOCK_POLICY_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        if clock_policy_end > input.len() {
            return Err(Error::InvalidCodec);
        }
        let clock_policy = ClockPolicyV1::decode(&input[at..clock_policy_end])?;
        at = clock_policy_end;
        if at != input.len() {
            return Err(Error::InvalidCodec);
        }
        let value = Self {
            source_plane,
            source_spec_id,
            source_spec_account,
            source_spec_owner,
            source_spec_account_data_id,
            adapter,
            parser,
            parser_config,
            parser_config_owner,
            parser_config_data_id,
            feed,
            feed_owner,
            generation_authority_program,
            system_program,
            clock_policy,
            source_work_schedule_id,
            liveness_policy_id,
            source_compartment_account,
            source_compartment_owner,
            neutral_sink,
        };
        value.validate()?;
        Ok(value)
    }

    /// Content identity of the complete source route.
    pub fn id(&self) -> Result<ContentId> {
        Ok(domain_id(SOURCE_RELEASE_DOMAIN, &self.encode()?))
    }
}

/// Exact immutable release account authenticated under the executing adapter program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceReleaseV1 {
    account: RuntimeKey,
    account_data_id: ContentId,
    manifest: SourceReleaseManifestV1,
    manifest_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedSourceReleaseV1 {
    /// Physical immutable release account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of complete release-account bytes.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Canonical release body.
    pub const fn manifest(self) -> SourceReleaseManifestV1 {
        self.manifest
    }

    /// Content identity of the canonical release body.
    pub const fn manifest_id(self) -> ContentId {
        self.manifest_id
    }

    /// Sole SourcePlane compatibility contract embedded in the release.
    pub const fn source_plane(self) -> SourcePlaneProgramV3 {
        self.manifest.source_plane
    }

    /// Sole Clock policy embedded in the canonical release.
    pub const fn clock_policy(self) -> ClockPolicyV1 {
        self.manifest.clock_policy
    }

    /// Complete owner/PDA/body authentication receipt.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Authenticate one immutable content-addressed Source release account.
pub fn authenticate_source_release_account(
    expected_adapter_program: RuntimeKey,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
) -> Result<AuthenticatedSourceReleaseV1> {
    expected_adapter_program.validate()?;
    if account.owner != expected_adapter_program {
        return Err(Error::WrongOwner);
    }
    if account.executable || account.signer || account.writable {
        return Err(Error::WrongPrivilege);
    }
    let manifest = SourceReleaseManifestV1::decode(account.data)?;
    if manifest.adapter.program != expected_adapter_program {
        return Err(Error::MismatchedBinding);
    }
    let manifest_id = manifest.id()?;
    let recipe = PdaRecipeV3::source_release(manifest_id)?;
    derived_pda.validate_for(
        expected_adapter_program,
        recipe.id()?,
        account.key,
        derived_pda.bump,
    )?;
    let account_data_id = account_data_id(account.key, account.data)?;
    let mut bytes = [0; 104];
    bytes[..32].copy_from_slice(&expected_adapter_program.bytes());
    bytes[32..64].copy_from_slice(&account.key.bytes());
    bytes[64..96].copy_from_slice(&account_data_id.bytes());
    bytes[96] = derived_pda.bump;
    Ok(AuthenticatedSourceReleaseV1 {
        account: account.key,
        account_data_id,
        manifest,
        manifest_id,
        authentication_id: domain_id(SOURCE_RELEASE_AUTH_DOMAIN, &bytes),
    })
}

/// Authenticated immutable source route; fields are private to prevent partial joins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceRouteV1 {
    manifest: SourceReleaseManifestV1,
    release_account: RuntimeKey,
    release_manifest_id: ContentId,
    release_authentication_id: ContentId,
    route_id: ContentId,
    source_plane_contract_id: ContentId,
    adapter_deployment_id: ContentId,
    parser_deployment_id: ContentId,
    clock_policy_id: ContentId,
}

impl AuthenticatedSourceRouteV1 {
    /// Complete release/deployment/config authentication identity.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Physical immutable release account.
    pub const fn release_account(self) -> RuntimeKey {
        self.release_account
    }

    /// Semantic identity of its exact canonical release body.
    pub const fn release_manifest_id(self) -> ContentId {
        self.release_manifest_id
    }

    /// Exact release-account owner/PDA/body receipt.
    pub const fn release_authentication_id(self) -> ContentId {
        self.release_authentication_id
    }

    /// Exact semantic SourcePlane contract identity.
    pub const fn source_plane_contract_id(self) -> ContentId {
        self.source_plane_contract_id
    }

    /// Existing SourceSpec identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.manifest.source_spec_id
    }

    /// Runtime-authenticated existing SourceSpec account.
    pub const fn source_spec_account(self) -> RuntimeKey {
        self.manifest.source_spec_account
    }

    /// Reviewed runtime adapter program.
    pub const fn adapter_program(self) -> RuntimeKey {
        self.manifest.adapter.program
    }

    /// Reviewed parser program.
    pub const fn parser_program(self) -> RuntimeKey {
        self.manifest.parser.program
    }

    /// Immutable parser configuration account selected by the release.
    pub const fn parser_config(self) -> RuntimeKey {
        self.manifest.parser_config
    }

    /// Exact reviewed runtime adapter deployment identity.
    pub const fn adapter_deployment_id(self) -> ContentId {
        self.adapter_deployment_id
    }

    /// Exact reviewed parser deployment identity.
    pub const fn parser_deployment_id(self) -> ContentId {
        self.parser_deployment_id
    }

    /// Mutable feed address.
    pub const fn feed(self) -> RuntimeKey {
        self.manifest.feed
    }

    /// Immutable source work-schedule identity.
    pub const fn source_work_schedule_id(self) -> ContentId {
        self.manifest.source_work_schedule_id
    }

    /// Runtime liveness policy identity.
    pub const fn liveness_policy_id(self) -> ContentId {
        self.manifest.liveness_policy_id
    }

    /// Source compartment account.
    pub const fn source_compartment_account(self) -> RuntimeKey {
        self.manifest.source_compartment_account
    }

    /// Source compartment semantic owner.
    pub const fn source_compartment_owner(self) -> RuntimeKey {
        self.manifest.source_compartment_owner
    }

    /// Frozen neutral sink.
    pub const fn neutral_sink(self) -> RuntimeKey {
        self.manifest.neutral_sink
    }

    pub(crate) const fn feed_owner(self) -> RuntimeKey {
        self.manifest.feed_owner
    }

    pub(crate) const fn generation_authority_program(self) -> RuntimeKey {
        self.manifest.generation_authority_program
    }

    pub(crate) const fn system_program(self) -> RuntimeKey {
        self.manifest.system_program
    }

    /// Sole Clock policy embedded in the authenticated release account.
    pub const fn clock_policy(self) -> ClockPolicyV1 {
        self.manifest.clock_policy
    }

    /// Derived identity of the sole Clock policy embedded in the release.
    pub const fn clock_policy_id(self) -> ContentId {
        self.clock_policy_id
    }
}

/// Authenticate complete release, ProgramData, config, and core contract bytes.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_source_route(
    release: AuthenticatedSourceReleaseV1,
    adapter_program: RuntimeAccountViewV1<'_>,
    adapter_programdata: RuntimeAccountViewV1<'_>,
    parser_program: RuntimeAccountViewV1<'_>,
    parser_programdata: RuntimeAccountViewV1<'_>,
    parser_config: RuntimeAccountViewV1<'_>,
    source_spec_account: RuntimeAccountViewV1<'_>,
) -> Result<AuthenticatedSourceRouteV1> {
    let manifest = release.manifest();
    manifest.validate()?;
    let source_plane_contract_id = manifest.source_plane.id()?;
    let adapter_deployment_id = manifest
        .adapter
        .authenticate(adapter_program, adapter_programdata)?;
    let parser_deployment_id = manifest
        .parser
        .authenticate(parser_program, parser_programdata)?;
    if parser_config.key != manifest.parser_config {
        return Err(Error::WrongAccount);
    }
    if parser_config.owner != manifest.parser_config_owner {
        return Err(Error::WrongOwner);
    }
    if parser_config.executable {
        return Err(Error::WrongExecutableState);
    }
    if parser_config.signer || parser_config.writable {
        return Err(Error::WrongPrivilege);
    }
    if account_data_id(parser_config.key, parser_config.data)? != manifest.parser_config_data_id {
        return Err(Error::WrongAccountData);
    }
    if source_spec_account.key != manifest.source_spec_account {
        return Err(Error::WrongAccount);
    }
    if source_spec_account.owner != manifest.source_spec_owner {
        return Err(Error::WrongOwner);
    }
    if source_spec_account.executable || source_spec_account.signer || source_spec_account.writable
    {
        return Err(Error::WrongPrivilege);
    }
    if account_data_id(source_spec_account.key, source_spec_account.data)?
        != manifest.source_spec_account_data_id
    {
        return Err(Error::WrongAccountData);
    }
    let mut route_bytes = [0; 96];
    route_bytes[..32].copy_from_slice(&release.id().bytes());
    route_bytes[32..64].copy_from_slice(&adapter_deployment_id.bytes());
    route_bytes[64..96].copy_from_slice(&parser_deployment_id.bytes());
    Ok(AuthenticatedSourceRouteV1 {
        release_account: release.account(),
        release_manifest_id: release.manifest_id(),
        release_authentication_id: release.id(),
        route_id: domain_id(SOURCE_ROUTE_AUTH_DOMAIN, &route_bytes),
        source_plane_contract_id,
        clock_policy_id: manifest.clock_policy.id()?,
        manifest,
        adapter_deployment_id,
        parser_deployment_id,
    })
}

/// Canonical output returned by the reviewed parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParserOutputV1 {
    /// Existing SourceSpec identity selected by parser config.
    pub source_spec_id: ContentId,
    /// Conservative normalized low endpoint.
    pub low: u128,
    /// Conservative normalized high endpoint.
    pub high: u128,
    /// Source-native monotone-or-equal sequence.
    pub source_sequence: u64,
    /// Authenticated source publication slot.
    pub publish_slot: u64,
    /// Authenticated source publication time.
    pub publish_time: u64,
    /// Digest of complete feed bytes supplied to the parser.
    pub feed_account_data_id: ContentId,
}

impl ParserOutputV1 {
    /// Validate exact normalized representation.
    pub fn validate(&self) -> Result<()> {
        live_id(self.source_spec_id)?;
        live_id(self.feed_account_data_id)?;
        if self.low > self.high || self.high > MAX_SOURCE_VALUE {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Canonical return-data digest.
    pub fn id(&self) -> Result<ContentId> {
        self.validate()?;
        let mut bytes = [0; 120];
        bytes[..32].copy_from_slice(&self.source_spec_id.bytes());
        bytes[32..48].copy_from_slice(&self.low.to_le_bytes());
        bytes[48..64].copy_from_slice(&self.high.to_le_bytes());
        bytes[64..72].copy_from_slice(&self.source_sequence.to_le_bytes());
        bytes[72..80].copy_from_slice(&self.publish_slot.to_le_bytes());
        bytes[80..88].copy_from_slice(&self.publish_time.to_le_bytes());
        bytes[88..].copy_from_slice(&self.feed_account_data_id.bytes());
        Ok(domain_id(PARSER_OUTPUT_DOMAIN, &bytes))
    }
}

/// Runtime-attested CPI/invocation facts for one reviewed parser/evaluator call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterInvocationV1 {
    /// Exact program invoked by the runtime adapter.
    pub invoked_program: RuntimeKey,
    /// Program that wrote return data.
    pub return_data_program: RuntimeKey,
    /// Digest of canonical returned semantic bytes.
    pub return_data_id: ContentId,
    /// Digest of exact instruction data.
    pub instruction_data_id: ContentId,
    /// Digest of the ordered runtime account vector.
    pub account_vector_id: ContentId,
}

impl AdapterInvocationV1 {
    /// Validate live invocation coordinates.
    pub fn validate(&self) -> Result<()> {
        self.invoked_program.validate()?;
        self.return_data_program.validate()?;
        live_id(self.return_data_id)?;
        live_id(self.instruction_data_id)?;
        live_id(self.account_vector_id)?;
        if self.invoked_program != self.return_data_program {
            return Err(Error::WrongInvocation);
        }
        Ok(())
    }

    /// Content identity of the exact adapter-attested invocation.
    pub fn id(&self) -> Result<ContentId> {
        self.validate()?;
        let mut bytes = [0; 160];
        bytes[..32].copy_from_slice(&self.invoked_program.bytes());
        bytes[32..64].copy_from_slice(&self.return_data_program.bytes());
        bytes[64..96].copy_from_slice(&self.return_data_id.bytes());
        bytes[96..128].copy_from_slice(&self.instruction_data_id.bytes());
        bytes[128..].copy_from_slice(&self.account_vector_id.bytes());
        Ok(domain_id(INVOCATION_DOMAIN, &bytes))
    }
}

/// Fully joined one-boundary parser/Clock/feed receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedBoundaryV1 {
    route_id: ContentId,
    source_spec_id: ContentId,
    repair_generation: u64,
    bucket: u64,
    clock: ClockSnapshotV1,
    feed_account_data_id: ContentId,
    invocation_id: ContentId,
    record: RawRecordV3,
    receipt_id: ContentId,
}

impl AuthenticatedBoundaryV1 {
    pub(crate) const ZERO: Self = Self {
        route_id: ContentId::ZERO,
        source_spec_id: ContentId::ZERO,
        repair_generation: 0,
        bucket: 0,
        clock: ClockSnapshotV1 {
            slot: 0,
            unix_timestamp: 0,
        },
        feed_account_data_id: ContentId::ZERO,
        invocation_id: ContentId::ZERO,
        record: RawRecordV3::observation(0, 0, 0, 0, 0),
        receipt_id: ContentId::ZERO,
    };

    /// Exact source route used for authentication.
    pub const fn route_id(self) -> ContentId {
        self.route_id
    }

    /// Existing SourceSpec identity.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Exact repair generation being populated.
    pub const fn repair_generation(self) -> u64 {
        self.repair_generation
    }

    /// State-owned canonical bucket.
    pub const fn bucket(self) -> u64 {
        self.bucket
    }

    /// Adapter-authenticated Clock snapshot.
    pub const fn clock(self) -> ClockSnapshotV1 {
        self.clock
    }

    /// Canonical source record admitted by the core.
    pub const fn record(self) -> RawRecordV3 {
        self.record
    }

    /// Exact boundary authentication receipt identity.
    pub const fn id(self) -> ContentId {
        self.receipt_id
    }

    pub(crate) const fn feed_account_data_id(self) -> ContentId {
        self.feed_account_data_id
    }

    pub(crate) const fn invocation_id(self) -> ContentId {
        self.invocation_id
    }
}

/// Authenticate one real feed/parser/Clock boundary.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_boundary(
    route: AuthenticatedSourceRouteV1,
    clock_policy: &ClockPolicyV1,
    clock: ClockSnapshotV1,
    feed: RuntimeAccountViewV1<'_>,
    expected_bucket: u64,
    repair_generation: u64,
    parser_output: ParserOutputV1,
    invocation: AdapterInvocationV1,
) -> Result<AuthenticatedBoundaryV1> {
    if clock_policy.id()? != route.clock_policy_id() {
        return Err(Error::MismatchedBinding);
    }
    if feed.key != route.feed() {
        return Err(Error::WrongAccount);
    }
    if feed.owner != route.feed_owner() {
        return Err(Error::WrongOwner);
    }
    if feed.executable || feed.signer || feed.writable {
        return Err(Error::WrongPrivilege);
    }
    parser_output.validate()?;
    if parser_output.source_spec_id != route.source_spec_id()
        || parser_output.feed_account_data_id != account_data_id(feed.key, feed.data)?
    {
        return Err(Error::MismatchedBinding);
    }
    invocation.validate()?;
    if invocation.invoked_program != route.parser_program()
        || invocation.return_data_id != parser_output.id()?
    {
        return Err(Error::WrongInvocation);
    }

    let boundary = clock_policy.boundary_timestamp(expected_bucket)?;
    let closes = boundary
        .checked_add(u64::from(clock_policy.maximum_boundary_lateness_seconds))
        .ok_or(Error::ArithmeticOverflow)?;
    if clock.unix_timestamp < boundary || clock.unix_timestamp > closes {
        return Err(Error::OutsideClockWindow);
    }
    if parser_output.publish_time > clock.unix_timestamp
        || clock
            .unix_timestamp
            .checked_sub(parser_output.publish_time)
            .ok_or(Error::ArithmeticOverflow)?
            > u64::from(clock_policy.maximum_source_age_seconds)
        || parser_output.publish_slot > clock.slot
        || clock
            .slot
            .checked_sub(parser_output.publish_slot)
            .ok_or(Error::ArithmeticOverflow)?
            > u64::from(clock_policy.maximum_source_slot_lag)
    {
        return Err(Error::OutsideClockWindow);
    }
    let record = RawRecordV3::observation(
        parser_output.low,
        parser_output.high,
        parser_output.source_sequence,
        parser_output.publish_slot,
        parser_output.publish_time,
    );
    let invocation_id = invocation.id()?;
    let feed_account_data_id = parser_output.feed_account_data_id;
    let mut bytes = [0; 224];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&route.source_spec_id().bytes());
    bytes[64..72].copy_from_slice(&repair_generation.to_le_bytes());
    bytes[72..80].copy_from_slice(&expected_bucket.to_le_bytes());
    bytes[80..88].copy_from_slice(&clock.slot.to_le_bytes());
    bytes[88..96].copy_from_slice(&clock.unix_timestamp.to_le_bytes());
    bytes[96..128].copy_from_slice(&feed_account_data_id.bytes());
    bytes[128..160].copy_from_slice(&invocation_id.bytes());
    let mut record_bytes = [0; 64];
    record_bytes[0] = 1;
    record_bytes[8..24].copy_from_slice(&parser_output.low.to_le_bytes());
    record_bytes[24..40].copy_from_slice(&parser_output.high.to_le_bytes());
    record_bytes[40..48].copy_from_slice(&parser_output.source_sequence.to_le_bytes());
    record_bytes[48..56].copy_from_slice(&parser_output.publish_slot.to_le_bytes());
    record_bytes[56..64].copy_from_slice(&parser_output.publish_time.to_le_bytes());
    bytes[160..].copy_from_slice(&record_bytes);
    Ok(AuthenticatedBoundaryV1 {
        route_id: route.route_id(),
        source_spec_id: route.source_spec_id(),
        repair_generation,
        bucket: expected_bucket,
        clock,
        feed_account_data_id,
        invocation_id,
        record,
        receipt_id: domain_id(BOUNDARY_DOMAIN, &bytes),
    })
}

/// Runtime-derived PDA result. The small SBF adapter remains responsible for
/// calling the canonical PDA derivation routine under `program_id`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeDerivedPdaV1 {
    /// Program under which the address was derived.
    pub program_id: RuntimeKey,
    /// Digest of the exact ordered seed recipe.
    pub recipe_id: ContentId,
    /// Runtime-derived PDA address.
    pub address: RuntimeKey,
    /// Runtime-derived bump.
    pub bump: u8,
}

impl RuntimeDerivedPdaV1 {
    pub(crate) fn validate_for(
        self,
        program_id: RuntimeKey,
        recipe_id: ContentId,
        address: RuntimeKey,
        bump: u8,
    ) -> Result<()> {
        self.program_id.validate()?;
        live_id(self.recipe_id)?;
        self.address.validate()?;
        if self.program_id != program_id
            || self.recipe_id != recipe_id
            || self.address != address
            || self.bump != bump
        {
            return Err(Error::WrongPda);
        }
        Ok(())
    }
}

pub(crate) fn live_id(id: ContentId) -> Result<()> {
    if id.is_zero() {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

pub(crate) fn domain_id(domain: &[u8], bytes: &[u8]) -> ContentId {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    ContentId::from_bytes(hasher.finalize().into())
}

fn put_id(out: &mut [u8], at: &mut usize, id: ContentId) {
    let end = *at + 32;
    if end <= out.len() {
        out[*at..end].copy_from_slice(&id.bytes());
    }
    *at = end;
}

fn put_key(out: &mut [u8], at: &mut usize, key: RuntimeKey) {
    let end = *at + 32;
    if end <= out.len() {
        out[*at..end].copy_from_slice(&key.bytes());
    }
    *at = end;
}

fn encode_deployment(value: &DeploymentBindingV1, out: &mut [u8], at: &mut usize) {
    put_key(out, at, value.program);
    put_id(out, at, value.program_account_data_id);
    put_key(out, at, value.programdata);
    put_id(out, at, value.programdata_account_data_id);
    put_key(out, at, value.loader);
    let end = *at + 16;
    if end <= out.len() {
        out[*at..*at + 2].copy_from_slice(&value.programdata_link_offset.to_le_bytes());
        out[*at + 2..*at + 4].copy_from_slice(&value.deployment_slot_offset.to_le_bytes());
        out[*at + 8..end].copy_from_slice(&value.deployment_slot.to_le_bytes());
    }
    *at = end;
}

fn le_u64(input: &[u8]) -> u64 {
    let mut word = [0; 8];
    word.copy_from_slice(input);
    u64::from_le_bytes(word)
}

const _: () = assert!(SOURCE_PLANE_PROGRAM_BYTES == 64);
