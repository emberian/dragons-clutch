//! Reusable manifest templates with exact occurrence-specific configuration projection.

use crate::funding::{FUNDING_QUOTE_BYTES, FundingQuoteV1};
use crate::{
    ARTIFACT_PROFILE_V1, ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1,
    CapabilityManifestV1, ContentId, Error, MANIFEST_HEADER_BYTES, MANIFEST_MAGIC,
    MANIFEST_SCHEMA_VERSION, MAX_CAPABILITIES, MAX_DEPENDENCIES_PER_CAPABILITY, Result,
};

/// Exact profile-1 template-entry width.
///
/// This equals the manifest entry width: byte 194, reserved in a realized
/// manifest, owns the template-only configuration-projection selector.
pub const CAPABILITY_TEMPLATE_ENTRY_BYTES: usize = CAPABILITY_ENTRY_BYTES;
/// Maximum profile-1 capability-template byte width.
pub const MAX_CAPABILITY_TEMPLATE_BYTES: usize =
    MANIFEST_HEADER_BYTES + MAX_CAPABILITIES * CAPABILITY_TEMPLATE_ENTRY_BYTES;
/// Canonical capability-template magic.
pub const CAPABILITY_TEMPLATE_MAGIC_V1: [u8; 8] = *b"DCLTCTP1";
/// Implemented capability-template schema version.
pub const CAPABILITY_TEMPLATE_SCHEMA_VERSION_V1: u16 = 1;
/// Canonical finalized-record schema label for [`CapabilityTemplateV1`].
pub const CAPABILITY_TEMPLATE_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/capability-template-v1";
/// SHA-256 identity of [`CAPABILITY_TEMPLATE_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0xb0, 0x61, 0x6a, 0x35, 0xa5, 0x83, 0x74, 0x98, 0x71, 0x87, 0x56, 0xdc, 0x10, 0xb2, 0xa3, 0x2e,
    0x90, 0x33, 0xb4, 0xb7, 0x80, 0x60, 0x47, 0xda, 0x57, 0x56, 0xe1, 0xc2, 0x6d, 0xd4, 0xf5, 0x87,
];

const TEMPLATE_SCHEMA_OFFSET: usize = 8;
const TEMPLATE_PROFILE_OFFSET: usize = 10;
const TEMPLATE_COUNT_OFFSET: usize = 12;
const TEMPLATE_RESERVED_OFFSET: usize = 14;
const TEMPLATE_RESERVED_BYTES: usize = 2;

const KIND_ID_OFFSET: usize = 0;
const RELEASE_ID_OFFSET: usize = 32;
const CONFIG_ID_OFFSET: usize = 64;
const CAPACITY_PROFILE_ID_OFFSET: usize = 96;
const CHILD_SCHEMA_ID_OFFSET: usize = 128;
const CHILD_DERIVATION_ID_OFFSET: usize = 160;
const ACTIVATION_POLICY_OFFSET: usize = 192;
const DEPENDENCY_COUNT_OFFSET: usize = 193;
const CONFIG_PROJECTION_OFFSET: usize = 194;
const ENTRY_RESERVED_OFFSET: usize = 195;
const ENTRY_RESERVED_BYTES: usize = 5;
const ACTIVATION_DEADLINE_OFFSET: usize = 200;
const DEPENDENCIES_OFFSET: usize = 208;
const QUOTE_OFFSET: usize = 224;

/// How one reusable template obtains the realized manifest configuration ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityConfigProjectionV1 {
    /// Preserve one exact immutable configuration identity in every occurrence.
    Static(ContentId),
    /// Substitute the authenticated occurrence Source-material identity.
    OccurrenceResolutionMaterial,
}

impl CapabilityConfigProjectionV1 {
    const fn selector(self) -> u8 {
        match self {
            Self::Static(_) => 0,
            Self::OccurrenceResolutionMaterial => 1,
        }
    }

    const fn resolve(self, occurrence_resolution_material_id: ContentId) -> ContentId {
        match self {
            Self::Static(config_id) => config_id,
            Self::OccurrenceResolutionMaterial => occurrence_resolution_material_id,
        }
    }
}

/// One canonical reusable capability-template entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityTemplateEntryV1 {
    kind_id: ContentId,
    release_id: ContentId,
    config_projection: CapabilityConfigProjectionV1,
    capacity_profile_id: ContentId,
    child_schema_id: ContentId,
    child_derivation_id: ContentId,
    activation_policy: ActivationPolicy,
    activation_deadline_slot: u64,
    dependency_count: u8,
    dependencies: [u8; MAX_DEPENDENCIES_PER_CAPABILITY],
    funding_quote: FundingQuoteV1,
}

impl CapabilityTemplateEntryV1 {
    /// Construct and validate one reusable template entry.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind_id: ContentId,
        release_id: ContentId,
        config_projection: CapabilityConfigProjectionV1,
        capacity_profile_id: ContentId,
        child_schema_id: ContentId,
        child_derivation_id: ContentId,
        activation_policy: ActivationPolicy,
        activation_deadline_slot: u64,
        dependency_count: u8,
        dependencies: [u8; MAX_DEPENDENCIES_PER_CAPABILITY],
        funding_quote: FundingQuoteV1,
    ) -> Result<Self> {
        let value = Self {
            kind_id,
            release_id,
            config_projection,
            capacity_profile_id,
            child_schema_id,
            child_derivation_id,
            activation_policy,
            activation_deadline_slot,
            dependency_count,
            dependencies,
            funding_quote,
        };
        let placeholder = ContentId::new([1; 32]).map_err(|_| Error::ZeroContentId)?;
        value.project(placeholder)?;
        Ok(value)
    }

    /// Decode one exact canonical profile-1 template entry.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CAPABILITY_TEMPLATE_ENTRY_BYTES {
            return Err(Error::InvalidLength);
        }
        require_zero(bytes, ENTRY_RESERVED_OFFSET, ENTRY_RESERVED_BYTES)?;
        let config_bytes = read_array::<32>(bytes, CONFIG_ID_OFFSET)?;
        let config_projection = match read_byte(bytes, CONFIG_PROJECTION_OFFSET)? {
            0 => CapabilityConfigProjectionV1::Static(
                ContentId::new(config_bytes).map_err(|_| Error::NonCanonicalConfigProjection)?,
            ),
            1 => {
                if config_bytes.iter().any(|byte| *byte != 0) {
                    return Err(Error::NonCanonicalConfigProjection);
                }
                CapabilityConfigProjectionV1::OccurrenceResolutionMaterial
            }
            _ => return Err(Error::UnknownConfigProjection),
        };
        let mut dependencies = [0u8; MAX_DEPENDENCIES_PER_CAPABILITY];
        copy_exact(
            &mut dependencies,
            0,
            subslice(bytes, DEPENDENCIES_OFFSET, MAX_DEPENDENCIES_PER_CAPABILITY)?,
        )?;
        Self::new(
            read_content_id(bytes, KIND_ID_OFFSET)?,
            read_content_id(bytes, RELEASE_ID_OFFSET)?,
            config_projection,
            read_content_id(bytes, CAPACITY_PROFILE_ID_OFFSET)?,
            read_content_id(bytes, CHILD_SCHEMA_ID_OFFSET)?,
            read_content_id(bytes, CHILD_DERIVATION_ID_OFFSET)?,
            ActivationPolicy::decode(read_byte(bytes, ACTIVATION_POLICY_OFFSET)?)?,
            read_u64(bytes, ACTIVATION_DEADLINE_OFFSET)?,
            read_byte(bytes, DEPENDENCY_COUNT_OFFSET)?,
            dependencies,
            FundingQuoteV1::decode(subslice(bytes, QUOTE_OFFSET, FUNDING_QUOTE_BYTES)?)?,
        )
    }

    /// Return the exact canonical template-entry bytes.
    pub fn to_bytes(self) -> [u8; CAPABILITY_TEMPLATE_ENTRY_BYTES] {
        let mut output = [0u8; CAPABILITY_TEMPLATE_ENTRY_BYTES];
        put(&mut output, KIND_ID_OFFSET, self.kind_id.as_bytes());
        put(&mut output, RELEASE_ID_OFFSET, self.release_id.as_bytes());
        if let CapabilityConfigProjectionV1::Static(config_id) = self.config_projection {
            put(&mut output, CONFIG_ID_OFFSET, config_id.as_bytes());
        }
        put(
            &mut output,
            CAPACITY_PROFILE_ID_OFFSET,
            self.capacity_profile_id.as_bytes(),
        );
        put(
            &mut output,
            CHILD_SCHEMA_ID_OFFSET,
            self.child_schema_id.as_bytes(),
        );
        put(
            &mut output,
            CHILD_DERIVATION_ID_OFFSET,
            self.child_derivation_id.as_bytes(),
        );
        put_byte(
            &mut output,
            ACTIVATION_POLICY_OFFSET,
            activation_policy_byte(self.activation_policy),
        );
        put_byte(&mut output, DEPENDENCY_COUNT_OFFSET, self.dependency_count);
        put_byte(
            &mut output,
            CONFIG_PROJECTION_OFFSET,
            self.config_projection.selector(),
        );
        put_u64(
            &mut output,
            ACTIVATION_DEADLINE_OFFSET,
            self.activation_deadline_slot,
        );
        put(&mut output, DEPENDENCIES_OFFSET, &self.dependencies);
        put(&mut output, QUOTE_OFFSET, &self.funding_quote.to_bytes());
        output
    }

    /// Materialize the exact occurrence-specific manifest entry.
    pub fn project(
        self,
        occurrence_resolution_material_id: ContentId,
    ) -> Result<CapabilityEntryV1> {
        CapabilityEntryV1::new(
            self.kind_id,
            self.release_id,
            self.config_projection
                .resolve(occurrence_resolution_material_id),
            self.capacity_profile_id,
            self.child_schema_id,
            self.child_derivation_id,
            self.activation_policy,
            self.activation_deadline_slot,
            self.dependency_count,
            self.dependencies,
            self.funding_quote,
        )
    }

    /// Return the capability-kind identity used for canonical ordering.
    pub const fn kind_id(self) -> ContentId {
        self.kind_id
    }

    /// Return the configuration projection selected by this entry.
    pub const fn config_projection(self) -> CapabilityConfigProjectionV1 {
        self.config_projection
    }

    /// Return the immutable activation policy.
    pub const fn activation_policy(self) -> ActivationPolicy {
        self.activation_policy
    }

    /// Return the number of active dependency indices.
    pub const fn dependency_count(self) -> u8 {
        self.dependency_count
    }

    /// Return one dependency index, refusing inactive positions.
    pub fn dependency(self, position: usize) -> Result<u8> {
        if position >= usize::from(self.dependency_count) {
            return Err(Error::InvalidDependency);
        }
        self.dependencies
            .get(position)
            .copied()
            .ok_or(Error::InvalidDependency)
    }
}

/// Borrowed, validated reusable capability-template preimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityTemplateV1<'a> {
    bytes: &'a [u8],
    entry_count: u16,
}

impl<'a> CapabilityTemplateV1<'a> {
    /// Decode and fully validate one canonical profile-1 template.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < MANIFEST_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != CAPABILITY_TEMPLATE_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, TEMPLATE_SCHEMA_OFFSET)? != CAPABILITY_TEMPLATE_SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, TEMPLATE_PROFILE_OFFSET)? != ARTIFACT_PROFILE_V1 {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(bytes, TEMPLATE_RESERVED_OFFSET, TEMPLATE_RESERVED_BYTES)?;
        let entry_count = read_u16(bytes, TEMPLATE_COUNT_OFFSET)?;
        if usize::from(entry_count) > MAX_CAPABILITIES
            || bytes.len() != template_bytes_for_count(usize::from(entry_count))?
        {
            return Err(Error::InvalidLength);
        }
        let value = Self { bytes, entry_count };
        validate_template(value)?;
        Ok(value)
    }

    /// Encode entries into caller-owned storage and return its validated view.
    pub fn encode_into(
        entries: &[CapabilityTemplateEntryV1],
        output: &'a mut [u8],
    ) -> Result<Self> {
        if entries.len() > MAX_CAPABILITIES
            || output.len() != template_bytes_for_count(entries.len())?
        {
            return Err(Error::InvalidLength);
        }
        validate_template_entries(entries)?;
        output.fill(0);
        put(output, 0, &CAPABILITY_TEMPLATE_MAGIC_V1);
        put_u16(
            output,
            TEMPLATE_SCHEMA_OFFSET,
            CAPABILITY_TEMPLATE_SCHEMA_VERSION_V1,
        );
        put_u16(output, TEMPLATE_PROFILE_OFFSET, ARTIFACT_PROFILE_V1);
        put_u16(
            output,
            TEMPLATE_COUNT_OFFSET,
            u16::try_from(entries.len()).map_err(|_| Error::TooManyCapabilities)?,
        );
        for (index, entry) in entries.iter().enumerate() {
            put(output, template_entry_offset(index)?, &entry.to_bytes());
        }
        Self::decode(output)
    }

    /// Borrow the exact template content-hash preimage.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Return the number of template entries.
    pub const fn entry_count(self) -> u16 {
        self.entry_count
    }

    /// Decode one selected template entry.
    pub fn entry(self, index: u16) -> Result<CapabilityTemplateEntryV1> {
        if index >= self.entry_count {
            return Err(Error::InvalidDependency);
        }
        CapabilityTemplateEntryV1::decode(subslice(
            self.bytes,
            template_entry_offset(usize::from(index))?,
            CAPABILITY_TEMPLATE_ENTRY_BYTES,
        )?)
    }

    /// Validate and expose the exact occurrence-specific manifest projection.
    pub fn project_for_resolution_material(
        self,
        occurrence_resolution_material_id: ContentId,
    ) -> Result<CapabilityManifestProjectionV1<'a>> {
        let projected_required = self.required_resolution_material_projection_index()?;
        let mut selected_required: Option<u16> = None;
        let mut index = 0u16;
        while index < self.entry_count {
            let template_entry = self.entry(index)?;
            let realized = template_entry.project(occurrence_resolution_material_id)?;
            if realized.activation_policy() == ActivationPolicy::RequiredAtFounding
                && realized.config_id() == occurrence_resolution_material_id
            {
                if selected_required.is_some() {
                    return Err(Error::RequiredFoundingConfigAmbiguous);
                }
                selected_required = Some(index);
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if selected_required != Some(projected_required) {
            return Err(Error::RequiredOccurrenceProjectionMissing);
        }
        Ok(CapabilityManifestProjectionV1 {
            template: self,
            occurrence_resolution_material_id,
        })
    }

    /// Require exactly one founding entry to project occurrence resolution material.
    ///
    /// This shape check is independent of any occurrence ID, so a Series may
    /// reject an unusable template before accepting finite capitalization.
    pub fn required_resolution_material_projection_index(self) -> Result<u16> {
        let mut projected: Option<u16> = None;
        let mut index = 0u16;
        while index < self.entry_count {
            let entry = self.entry(index)?;
            if entry.config_projection == CapabilityConfigProjectionV1::OccurrenceResolutionMaterial
            {
                if projected.is_some() {
                    return Err(Error::RequiredOccurrenceProjectionAmbiguous);
                }
                if entry.activation_policy != ActivationPolicy::RequiredAtFounding {
                    return Err(Error::RequiredOccurrenceProjectionMissing);
                }
                projected = Some(index);
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        projected.ok_or(Error::RequiredOccurrenceProjectionMissing)
    }
}

/// Exact, allocation-free projection of one template into one manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityManifestProjectionV1<'a> {
    template: CapabilityTemplateV1<'a>,
    occurrence_resolution_material_id: ContentId,
}

impl<'a> CapabilityManifestProjectionV1<'a> {
    /// Return the exact realized manifest width.
    pub fn manifest_bytes(self) -> Result<usize> {
        manifest_bytes_for_count(usize::from(self.template.entry_count))
    }

    /// Return the exact realized manifest header for streaming content hashing.
    pub fn manifest_header_bytes(self) -> [u8; MANIFEST_HEADER_BYTES] {
        let mut output = [0u8; MANIFEST_HEADER_BYTES];
        put(&mut output, 0, &MANIFEST_MAGIC);
        put_u16(&mut output, 8, MANIFEST_SCHEMA_VERSION);
        put_u16(&mut output, 10, ARTIFACT_PROFILE_V1);
        put_u16(&mut output, 12, self.template.entry_count);
        output
    }

    /// Return the exact realized manifest entry count.
    pub const fn entry_count(self) -> u16 {
        self.template.entry_count
    }

    /// Project one exact realized manifest entry.
    pub fn entry(self, index: u16) -> Result<CapabilityEntryV1> {
        self.template
            .entry(index)?
            .project(self.occurrence_resolution_material_id)
    }

    /// Encode the exact realized manifest into caller-owned storage.
    pub fn encode_into<'out>(self, output: &'out mut [u8]) -> Result<CapabilityManifestV1<'out>> {
        if output.len() != self.manifest_bytes()? {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        put(output, 0, &self.manifest_header_bytes());
        let mut index = 0u16;
        while index < self.entry_count() {
            put(
                output,
                manifest_entry_offset(usize::from(index))?,
                &self.entry(index)?.to_bytes(),
            );
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let manifest = CapabilityManifestV1::decode(output)?;
        self.validate_manifest(manifest)?;
        Ok(manifest)
    }

    /// Require a supplied manifest to equal this projection entry-for-entry.
    pub fn validate_manifest(self, manifest: CapabilityManifestV1<'_>) -> Result<()> {
        if manifest.entry_count() != self.entry_count() {
            return Err(Error::ProjectedManifestMismatch);
        }
        let mut index = 0u16;
        while index < self.entry_count() {
            if manifest.entry(index)? != self.entry(index)? {
                return Err(Error::ProjectedManifestMismatch);
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let selected =
            manifest.required_founding_entry_for_config(self.occurrence_resolution_material_id)?;
        let template_entry = self.template.entry(selected.index())?;
        if template_entry.config_projection
            != CapabilityConfigProjectionV1::OccurrenceResolutionMaterial
        {
            return Err(Error::ProjectedManifestMismatch);
        }
        Ok(())
    }
}

fn validate_template(template: CapabilityTemplateV1<'_>) -> Result<()> {
    let count = usize::from(template.entry_count);
    let mut prior_kind: Option<[u8; 32]> = None;
    let mut index = 0usize;
    while index < count {
        let entry =
            template.entry(u16::try_from(index).map_err(|_| Error::TooManyCapabilities)?)?;
        let kind = entry.kind_id.to_bytes();
        if prior_kind.is_some_and(|prior| prior >= kind) {
            return Err(Error::NonCanonicalEntryOrder);
        }
        prior_kind = Some(kind);
        let mut position = 0usize;
        while position < usize::from(entry.dependency_count) {
            let dependency = usize::from(entry.dependency(position)?);
            if dependency >= count || dependency == index {
                return Err(Error::InvalidDependency);
            }
            position = position.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    validate_template_acyclic(template)
}

fn validate_template_entries(entries: &[CapabilityTemplateEntryV1]) -> Result<()> {
    let count = entries.len();
    let mut prior_kind: Option<[u8; 32]> = None;
    for (index, entry) in entries.iter().enumerate() {
        let kind = entry.kind_id.to_bytes();
        if prior_kind.is_some_and(|prior| prior >= kind) {
            return Err(Error::NonCanonicalEntryOrder);
        }
        prior_kind = Some(kind);
        let mut position = 0usize;
        while position < usize::from(entry.dependency_count) {
            let dependency = usize::from(entry.dependency(position)?);
            if dependency >= count || dependency == index {
                return Err(Error::InvalidDependency);
            }
            position = position.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
    }
    validate_template_entries_acyclic(entries)
}

fn validate_template_entries_acyclic(entries: &[CapabilityTemplateEntryV1]) -> Result<()> {
    let count = entries.len();
    let mut resolved = [false; MAX_CAPABILITIES];
    let mut resolved_count = 0usize;
    while resolved_count < count {
        let mut progressed = false;
        for (index, entry) in entries.iter().enumerate() {
            if !read_bool(&resolved, index)? {
                let mut ready = true;
                let mut position = 0usize;
                while position < usize::from(entry.dependency_count) {
                    if !read_bool(&resolved, usize::from(entry.dependency(position)?))? {
                        ready = false;
                    }
                    position = position.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
                }
                if ready {
                    write_bool(&mut resolved, index, true)?;
                    resolved_count = resolved_count
                        .checked_add(1)
                        .ok_or(Error::ArithmeticOverflow)?;
                    progressed = true;
                }
            }
        }
        if !progressed {
            return Err(Error::CyclicDependencies);
        }
    }
    Ok(())
}

fn validate_template_acyclic(template: CapabilityTemplateV1<'_>) -> Result<()> {
    let count = usize::from(template.entry_count);
    let mut resolved = [false; MAX_CAPABILITIES];
    let mut resolved_count = 0usize;
    while resolved_count < count {
        let mut progressed = false;
        let mut index = 0usize;
        while index < count {
            if !read_bool(&resolved, index)? {
                let entry = template
                    .entry(u16::try_from(index).map_err(|_| Error::TooManyCapabilities)?)?;
                let mut ready = true;
                let mut position = 0usize;
                while position < usize::from(entry.dependency_count) {
                    if !read_bool(&resolved, usize::from(entry.dependency(position)?))? {
                        ready = false;
                    }
                    position = position.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
                }
                if ready {
                    write_bool(&mut resolved, index, true)?;
                    resolved_count = resolved_count
                        .checked_add(1)
                        .ok_or(Error::ArithmeticOverflow)?;
                    progressed = true;
                }
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if !progressed {
            return Err(Error::CyclicDependencies);
        }
    }
    Ok(())
}

fn template_bytes_for_count(count: usize) -> Result<usize> {
    MANIFEST_HEADER_BYTES
        .checked_add(
            count
                .checked_mul(CAPABILITY_TEMPLATE_ENTRY_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

fn manifest_bytes_for_count(count: usize) -> Result<usize> {
    MANIFEST_HEADER_BYTES
        .checked_add(
            count
                .checked_mul(CAPABILITY_ENTRY_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

fn template_entry_offset(index: usize) -> Result<usize> {
    MANIFEST_HEADER_BYTES
        .checked_add(
            index
                .checked_mul(CAPABILITY_TEMPLATE_ENTRY_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

fn manifest_entry_offset(index: usize) -> Result<usize> {
    MANIFEST_HEADER_BYTES
        .checked_add(
            index
                .checked_mul(CAPABILITY_ENTRY_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

const fn activation_policy_byte(policy: ActivationPolicy) -> u8 {
    match policy {
        ActivationPolicy::RequiredAtFounding => 0,
        ActivationPolicy::PrepaidLazy => 1,
    }
}

fn read_bool(values: &[bool], index: usize) -> Result<bool> {
    values.get(index).copied().ok_or(Error::InvalidDependency)
}

fn write_bool(values: &mut [bool], index: usize, value: bool) -> Result<()> {
    let target = values.get_mut(index).ok_or(Error::InvalidDependency)?;
    *target = value;
    Ok(())
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    subslice(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_content_id(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(read_array(bytes, offset)?).map_err(|_| Error::ZeroContentId)
}

fn require_zero(bytes: &[u8], offset: usize, len: usize) -> Result<()> {
    if subslice(bytes, offset, len)?.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn subslice(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    bytes
        .get(offset..offset.checked_add(len).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)
}

fn copy_exact(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let target = output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?;
    target.copy_from_slice(value);
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(end) = offset.checked_add(value.len())
        && let Some(target) = output.get_mut(offset..end)
    {
        target.copy_from_slice(value);
    }
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) {
    if let Some(target) = output.get_mut(offset) {
        *target = value;
    }
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::funding::{FundingAmountsV1, FundingQuoteV1};

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero content ID")
    }

    fn quote() -> FundingQuoteV1 {
        FundingQuoteV1::new(FundingAmountsV1::default(), None).expect("empty exact quote")
    }

    fn entry(
        kind: u8,
        config_projection: CapabilityConfigProjectionV1,
        dependency: Option<u8>,
    ) -> CapabilityTemplateEntryV1 {
        let mut dependencies = [0u8; MAX_DEPENDENCIES_PER_CAPABILITY];
        let dependency_count = if let Some(value) = dependency {
            *dependencies.get_mut(0).expect("first dependency") = value;
            1
        } else {
            0
        };
        CapabilityTemplateEntryV1::new(
            id(kind),
            id(kind.saturating_add(20)),
            config_projection,
            id(kind.saturating_add(40)),
            id(kind.saturating_add(60)),
            id(kind.saturating_add(80)),
            ActivationPolicy::RequiredAtFounding,
            0,
            dependency_count,
            dependencies,
            quote(),
        )
        .expect("canonical template entry")
    }

    #[test]
    fn template_round_trips_and_projects_exact_manifest() {
        let entries = [
            entry(1, CapabilityConfigProjectionV1::Static(id(90)), None),
            entry(
                2,
                CapabilityConfigProjectionV1::OccurrenceResolutionMaterial,
                Some(0),
            ),
        ];
        let mut template_bytes =
            [0u8; MANIFEST_HEADER_BYTES + (2 * CAPABILITY_TEMPLATE_ENTRY_BYTES)];
        CapabilityTemplateV1::encode_into(&entries, &mut template_bytes)
            .expect("canonical template");
        let template = CapabilityTemplateV1::decode(&template_bytes).expect("decoded template");
        assert_eq!(
            CapabilityTemplateV1::decode(template.as_bytes()),
            Ok(template)
        );
        let dynamic_offset = template_entry_offset(1).expect("dynamic offset");
        assert!(
            template_bytes
                .get(dynamic_offset + CONFIG_ID_OFFSET..dynamic_offset + CONFIG_ID_OFFSET + 32)
                .expect("dynamic config slot")
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(
            template_bytes.get(dynamic_offset + CONFIG_PROJECTION_OFFSET),
            Some(&1)
        );

        let material_id = id(120);
        let projection = template
            .project_for_resolution_material(material_id)
            .expect("unique occurrence projection");
        let mut manifest_bytes = [0u8; MANIFEST_HEADER_BYTES + (2 * CAPABILITY_ENTRY_BYTES)];
        let manifest = projection
            .encode_into(&mut manifest_bytes)
            .expect("exact realized manifest");
        projection
            .validate_manifest(manifest)
            .expect("projection equality");
        assert_eq!(
            projection.manifest_header_bytes(),
            MANIFEST_MAGIC_HEADER_TWO
        );
        assert_eq!(manifest.entry(0).expect("static entry").config_id(), id(90));
        assert_eq!(
            manifest.entry(1).expect("projected entry").config_id(),
            material_id
        );
        assert_eq!(
            manifest
                .required_founding_entry_for_config(material_id)
                .expect("unique Found selector")
                .index(),
            1
        );
        assert_eq!(
            manifest_bytes.get(dynamic_offset + CONFIG_PROJECTION_OFFSET),
            Some(&0)
        );
    }

    const MANIFEST_MAGIC_HEADER_TWO: [u8; MANIFEST_HEADER_BYTES] = [
        b'D', b'C', b'L', b'T', b'C', b'A', b'P', b'1', 1, 0, 1, 0, 2, 0, 0, 0,
    ];

    #[test]
    fn projection_refuses_missing_ambiguous_and_static_collision() {
        let static_entries = [entry(1, CapabilityConfigProjectionV1::Static(id(90)), None)];
        let mut static_bytes = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_TEMPLATE_ENTRY_BYTES];
        let static_template = CapabilityTemplateV1::encode_into(&static_entries, &mut static_bytes)
            .expect("static template");
        assert_eq!(
            static_template.project_for_resolution_material(id(91)),
            Err(Error::RequiredOccurrenceProjectionMissing)
        );

        let ambiguous_entries = [
            entry(
                1,
                CapabilityConfigProjectionV1::OccurrenceResolutionMaterial,
                None,
            ),
            entry(
                2,
                CapabilityConfigProjectionV1::OccurrenceResolutionMaterial,
                None,
            ),
        ];
        let mut ambiguous_bytes =
            [0u8; MANIFEST_HEADER_BYTES + (2 * CAPABILITY_TEMPLATE_ENTRY_BYTES)];
        let ambiguous = CapabilityTemplateV1::encode_into(&ambiguous_entries, &mut ambiguous_bytes)
            .expect("structurally canonical template");
        assert_eq!(
            ambiguous.project_for_resolution_material(id(92)),
            Err(Error::RequiredOccurrenceProjectionAmbiguous)
        );

        let material_id = id(93);
        let collision_entries = [
            entry(1, CapabilityConfigProjectionV1::Static(material_id), None),
            entry(
                2,
                CapabilityConfigProjectionV1::OccurrenceResolutionMaterial,
                None,
            ),
        ];
        let mut collision_bytes =
            [0u8; MANIFEST_HEADER_BYTES + (2 * CAPABILITY_TEMPLATE_ENTRY_BYTES)];
        let collision = CapabilityTemplateV1::encode_into(&collision_entries, &mut collision_bytes)
            .expect("structurally canonical collision template");
        assert_eq!(
            collision.project_for_resolution_material(material_id),
            Err(Error::RequiredFoundingConfigAmbiguous)
        );
    }

    #[test]
    fn hostile_projection_selector_config_and_manifest_substitution_refuse() {
        let entries = [entry(
            1,
            CapabilityConfigProjectionV1::OccurrenceResolutionMaterial,
            None,
        )];
        let mut template_bytes = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_TEMPLATE_ENTRY_BYTES];
        CapabilityTemplateV1::encode_into(&entries, &mut template_bytes)
            .expect("canonical template");
        let entry_offset = template_entry_offset(0).expect("entry offset");

        let mut nonzero_dynamic = template_bytes;
        *nonzero_dynamic
            .get_mut(entry_offset + CONFIG_ID_OFFSET)
            .expect("config byte") = 1;
        assert_eq!(
            CapabilityTemplateV1::decode(&nonzero_dynamic),
            Err(Error::NonCanonicalConfigProjection)
        );
        let mut unknown = template_bytes;
        *unknown
            .get_mut(entry_offset + CONFIG_PROJECTION_OFFSET)
            .expect("projection selector") = 2;
        assert_eq!(
            CapabilityTemplateV1::decode(&unknown),
            Err(Error::UnknownConfigProjection)
        );
        assert_eq!(
            CapabilityTemplateV1::decode(
                template_bytes
                    .get(..template_bytes.len() - 1)
                    .expect("short template")
            ),
            Err(Error::InvalidLength)
        );

        let material_id = id(120);
        let template = CapabilityTemplateV1::decode(&template_bytes).expect("decoded template");
        let projection = template
            .project_for_resolution_material(material_id)
            .expect("projection");
        let mut manifest_bytes = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        projection
            .encode_into(&mut manifest_bytes)
            .expect("manifest");
        let static_substitute = entry(1, CapabilityConfigProjectionV1::Static(id(121)), None)
            .project(material_id)
            .expect("substituted entry");
        put(
            &mut manifest_bytes,
            manifest_entry_offset(0).expect("entry offset"),
            &static_substitute.to_bytes(),
        );
        let substituted = CapabilityManifestV1::decode(&manifest_bytes).expect("valid manifest");
        assert_eq!(
            projection.validate_manifest(substituted),
            Err(Error::ProjectedManifestMismatch)
        );
    }

    #[test]
    fn template_preserves_manifest_order_and_dependency_invariants() {
        let descending = [
            entry(
                2,
                CapabilityConfigProjectionV1::OccurrenceResolutionMaterial,
                None,
            ),
            entry(1, CapabilityConfigProjectionV1::Static(id(90)), None),
        ];
        let mut descending_bytes =
            [0u8; MANIFEST_HEADER_BYTES + (2 * CAPABILITY_TEMPLATE_ENTRY_BYTES)];
        descending_bytes.fill(0xa5);
        let descending_before = descending_bytes;
        assert_eq!(
            CapabilityTemplateV1::encode_into(&descending, &mut descending_bytes),
            Err(Error::NonCanonicalEntryOrder)
        );
        assert_eq!(descending_bytes, descending_before);

        let cycle = [
            entry(
                1,
                CapabilityConfigProjectionV1::OccurrenceResolutionMaterial,
                Some(1),
            ),
            entry(2, CapabilityConfigProjectionV1::Static(id(90)), Some(0)),
        ];
        let mut cycle_bytes = [0u8; MANIFEST_HEADER_BYTES + (2 * CAPABILITY_TEMPLATE_ENTRY_BYTES)];
        cycle_bytes.fill(0x5a);
        let cycle_before = cycle_bytes;
        assert_eq!(
            CapabilityTemplateV1::encode_into(&cycle, &mut cycle_bytes),
            Err(Error::CyclicDependencies)
        );
        assert_eq!(cycle_bytes, cycle_before);
    }
}
