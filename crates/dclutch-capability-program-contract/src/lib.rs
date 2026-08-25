#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Data-defined capability-program descriptor and Trading root-account header.
//!
//! This crate performs no hashing, finalized-record authentication, Solana
//! account access, PDA derivation, CPI, or effect application. Adapters must
//! establish those facts before constructing an admitted invocation.
//! In particular, the adapter proves
//! `entry.release_id == selection.capability_release == SHA256(descriptor)`;
//! no hard-coded family release constant may replace that descriptor identity.

use dclutch_capability_contract::CapabilityEntryV1;
use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1, CapabilityExecutionSelectionV1,
};
use dclutch_transition_vm::v2::{ProgramV2, RegisterInput, RegisterOutput, execute_atomic};

#[rustfmt::skip]
#[allow(missing_docs)]
mod generated;

pub use generated::*;

/// Schema label for finalized [`CapabilityProgramV1`] raw records.
pub const CAPABILITY_PROGRAM_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/capability-program-v1";
/// SHA-256 of [`CAPABILITY_PROGRAM_SCHEMA_RELEASE_PREIMAGE_V1`].
pub const CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x7f, 0xa4, 0xd3, 0x78, 0x16, 0x86, 0x44, 0x77, 0x02, 0xc3, 0xb9, 0x50, 0xe1, 0xaf, 0x75, 0x30,
    0x87, 0xa1, 0x4e, 0x40, 0x93, 0x02, 0x87, 0x61, 0x3b, 0xac, 0x6d, 0x59, 0x0a, 0xc1, 0xa4, 0x4c,
];
/// Schema label for the immutable common Trading root header.
pub const CAPABILITY_ROOT_HEADER_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/capability-root-v1";
/// SHA-256 of [`CAPABILITY_ROOT_HEADER_SCHEMA_RELEASE_PREIMAGE_V1`].
///
/// This is an artifact-implied physical header schema, not the manifest
/// entry's `child_schema_id`; that identity selects the mutable root tail.
pub const CAPABILITY_ROOT_HEADER_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x74, 0x75, 0xab, 0x4c, 0x6b, 0x39, 0x1c, 0x4a, 0x19, 0xae, 0xe6, 0x9f, 0xe9, 0xb7, 0x10, 0x4a,
    0x28, 0xe0, 0xff, 0x8e, 0x11, 0x12, 0x35, 0xa0, 0xc6, 0xd3, 0xf7, 0xa8, 0xec, 0x60, 0x44, 0x0c,
];
/// Derivation-policy label for one immutable common Trading child root.
pub const CAPABILITY_ROOT_DERIVATION_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/derivation/capability-root-v1";
/// SHA-256 of [`CAPABILITY_ROOT_DERIVATION_RELEASE_PREIMAGE_V1`].
pub const CAPABILITY_ROOT_DERIVATION_RELEASE_ID_V1: [u8; 32] = [
    0x4a, 0xbf, 0x3a, 0xed, 0xeb, 0x76, 0x84, 0x74, 0x10, 0x78, 0xe5, 0x8e, 0x09, 0x3f, 0xe1, 0xbd,
    0xda, 0x10, 0x41, 0xd9, 0xe2, 0x54, 0xf7, 0x9d, 0x05, 0x12, 0x88, 0x29, 0x1d, 0x8d, 0x36, 0x0e,
];
/// Semantic schema label for common activation effects.
pub const CAPABILITY_ACTIVATION_EFFECT_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/capability-activation-effect-v1";
/// SHA-256 of [`CAPABILITY_ACTIVATION_EFFECT_SCHEMA_PREIMAGE_V1`].
pub const CAPABILITY_ACTIVATION_EFFECT_SCHEMA_ID_V1: [u8; 32] = [
    0xcc, 0x46, 0x44, 0xcc, 0xa7, 0x3a, 0x09, 0xbd, 0x86, 0xae, 0x2d, 0x19, 0xc6, 0xfc, 0xff, 0x7e,
    0xec, 0x60, 0x8c, 0x6f, 0x73, 0x4f, 0x98, 0x01, 0xcd, 0x61, 0xb1, 0x04, 0xb4, 0x8d, 0xa5, 0x24,
];
/// Exact PDA domain for every immutable common Trading child root.
pub const CAPABILITY_ROOT_PDA_DOMAIN_V1: &[u8] = b"dclutch:capability-root:v1";
/// Exact maximum descriptor rent under pinned Solana `Rent::default()`.
pub const CAPABILITY_PROGRAM_MAX_RENT_LAMPORTS_V1: u64 = 9_966_720;

const _: () = assert!(CAPABILITY_ROOT_PDA_DOMAIN_V1.len() <= 32);

/// Stable refusal from the capability-program contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have its exact header- and program-derived width.
    InvalidLength,
    /// Magic did not identify the requested record.
    InvalidMagic,
    /// The record named an unsupported semantic schema version.
    UnsupportedSchema,
    /// The record named an unsupported artifact profile.
    UnsupportedArtifactProfile,
    /// Reserved bytes were not canonical zeros.
    NonCanonicalReservedBytes,
    /// A required content or account identity was zero.
    ZeroIdentity,
    /// The descriptor's transition program was empty or malformed.
    InvalidTransitionProgram,
    /// The mutable root tail width was zero or exceeded the fixed profile.
    InvalidRootStateBytes,
    /// The descriptor did not bind the exact selected kind.
    SelectionMismatch,
    /// The descriptor did not bind the exact selected manifest-entry profile.
    ManifestEntryMismatch,
    /// The adapter artifact does not implement an identified content schema.
    UnsupportedContent,
    /// The checked transition program refused the authenticated registers.
    TransitionRefused,
}

/// Result alias for capability-program operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Caller-owned runtime-width banks for one atomic ProgramV2 execution.
pub struct CapabilityRegistersV2<'a> {
    input: RegisterInput<'a>,
    scratch: RegisterOutput<'a>,
    output: RegisterOutput<'a>,
}

impl<'a> CapabilityRegistersV2<'a> {
    /// Construct one exact input/scratch/output projection.
    pub const fn new(
        input: RegisterInput<'a>,
        scratch: RegisterOutput<'a>,
        output: RegisterOutput<'a>,
    ) -> Self {
        Self {
            input,
            scratch,
            output,
        }
    }
}

/// Borrowed typed view of one finalized data-defined capability program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityProgramV1<'a> {
    kind: ContentId,
    config_schema: ContentId,
    request_schema: ContentId,
    root_schema: ContentId,
    account_profile: ContentId,
    derivation_policy: ContentId,
    capacity_profile: ContentId,
    effect_schema: ContentId,
    root_state_bytes: u32,
    transition_program: ProgramV2<'a>,
}

impl<'a> CapabilityProgramV1<'a> {
    /// Hostile-decode one exact descriptor and its canonical transition program.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() <= CAPABILITY_PROGRAM_HEADER_BYTES_V1
            || bytes.len() > CAPABILITY_PROGRAM_MAX_BYTES_V1
        {
            return Err(Error::InvalidLength);
        }
        if slice(
            bytes,
            CAPABILITY_PROGRAM_MAGIC_OFFSET,
            CAPABILITY_PROGRAM_MAGIC_V1.len(),
        )? != CAPABILITY_PROGRAM_MAGIC_V1
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, CAPABILITY_PROGRAM_SCHEMA_VERSION_OFFSET)?
            != CAPABILITY_PROGRAM_SCHEMA_VERSION_V1
        {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, CAPABILITY_PROGRAM_PROFILE_OFFSET)? != CAPABILITY_PROGRAM_PROFILE_V2 {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(bytes, CAPABILITY_PROGRAM_RESERVED_OFFSET, 4)?;
        require_zero(bytes, CAPABILITY_PROGRAM_BODY_RESERVED_OFFSET, 4)?;
        let root_state_bytes = read_u32(bytes, CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET)?;
        if root_state_bytes == 0
            || usize::try_from(root_state_bytes).map_err(|_| Error::InvalidRootStateBytes)?
                > CAPABILITY_ROOT_STATE_MAX_BYTES_V1
        {
            return Err(Error::InvalidRootStateBytes);
        }
        let transition_program = ProgramV2::decode(
            bytes
                .get(CAPABILITY_PROGRAM_HEADER_BYTES_V1..)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|_| Error::InvalidTransitionProgram)?;
        Ok(Self {
            kind: content(bytes, CAPABILITY_PROGRAM_KIND_OFFSET)?,
            config_schema: content(bytes, CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET)?,
            request_schema: content(bytes, CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET)?,
            root_schema: content(bytes, CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET)?,
            account_profile: content(bytes, CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET)?,
            derivation_policy: content(bytes, CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET)?,
            capacity_profile: content(bytes, CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET)?,
            effect_schema: content(bytes, CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET)?,
            root_state_bytes,
            transition_program,
        })
    }

    /// Require this descriptor to be the exact semantic projection of a selection and entry.
    pub fn validate_selection(
        self,
        selection: CapabilityExecutionSelectionV1,
        entry: CapabilityEntryV1,
    ) -> Result<()> {
        if self.kind != selection.kind() || entry.kind_id() != selection.kind() {
            return Err(Error::SelectionMismatch);
        }
        if entry.release_id() != selection.capability_release()
            || entry.config_id() != selection.config()
            || self.capacity_profile != entry.capacity_profile_id()
            || self.root_schema != entry.child_schema_id()
            || self.derivation_policy != entry.child_derivation_id()
        {
            return Err(Error::ManifestEntryMismatch);
        }
        Ok(())
    }

    /// Require the kind carried by an already authenticated persisted selector.
    ///
    /// Activation performs the stronger manifest-entry join once before
    /// persisting the immutable root header. Hot actions recheck the complete
    /// descriptor digest against `selection.capability_release` and need not
    /// carry the manifest account again.
    pub fn validate_persisted_selection(
        self,
        selection: CapabilityExecutionSelectionV1,
    ) -> Result<()> {
        if self.kind == selection.kind() {
            Ok(())
        } else {
            Err(Error::SelectionMismatch)
        }
    }

    /// Execute the descriptor's ProgramV2 transition over runtime-width banks.
    pub fn execute(self, registers: CapabilityRegistersV2<'_>) -> Result<()> {
        execute_atomic(
            self.transition_program,
            registers.input,
            registers.scratch,
            registers.output,
        )
        .map_err(|_| Error::TransitionRefused)
    }

    /// Return the selected capability kind.
    pub const fn kind(self) -> ContentId {
        self.kind
    }
    /// Return the config record's schema release.
    pub const fn config_schema(self) -> ContentId {
        self.config_schema
    }
    /// Return the request schema release understood by this program.
    pub const fn request_schema(self) -> ContentId {
        self.request_schema
    }
    /// Return the mutable root-tail schema release selected by the manifest.
    pub const fn root_schema(self) -> ContentId {
        self.root_schema
    }
    /// Return the authenticated account/register projection profile.
    pub const fn account_profile(self) -> ContentId {
        self.account_profile
    }
    /// Return the child PDA derivation-policy release.
    pub const fn derivation_policy(self) -> ContentId {
        self.derivation_policy
    }
    /// Return the exact admitted capacity-profile content identity.
    pub const fn capacity_profile(self) -> ContentId {
        self.capacity_profile
    }
    /// Return the allowed-effect schema release.
    pub const fn effect_schema(self) -> ContentId {
        self.effect_schema
    }
    /// Return the exact mutable root-state tail width.
    pub const fn root_state_bytes(self) -> u32 {
        self.root_state_bytes
    }
    /// Return the exact whole Trading root-account width.
    pub fn root_account_bytes(self) -> Result<usize> {
        root_account_bytes(self)
    }
    /// Borrow the canonical transition program.
    pub const fn transition_program(self) -> ProgramV2<'a> {
        self.transition_program
    }
}

/// Exact schema IDs implemented by one adapter projection/effect boundary.
///
/// This value must come from the checked Trading artifact, never instruction
/// bytes. Equality against all coordinates makes unknown content fail closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupportedContentV1 {
    /// Supported config schema.
    pub config_schema: ContentId,
    /// Supported request schema.
    pub request_schema: ContentId,
    /// Supported mutable root-tail schema.
    pub root_schema: ContentId,
    /// Supported account/register projection profile.
    pub account_profile: ContentId,
    /// Supported child derivation policy.
    pub derivation_policy: ContentId,
    /// Supported allowed-effect schema.
    pub effect_schema: ContentId,
}

impl SupportedContentV1 {
    /// Refuse a descriptor unless the current artifact implements every schema.
    pub fn require(self, program: CapabilityProgramV1<'_>) -> Result<()> {
        if self.config_schema != program.config_schema
            || self.request_schema != program.request_schema
            || self.root_schema != program.root_schema
            || self.account_profile != program.account_profile
            || self.derivation_policy != program.derivation_policy
            || self.effect_schema != program.effect_schema
        {
            return Err(Error::UnsupportedContent);
        }
        Ok(())
    }
}

/// Immutable activation projection at the front of one Trading root account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityRootHeaderV1 {
    release_set: ContentId,
    market: [u8; 32],
    generation: u64,
    selection: CapabilityExecutionSelectionV1,
}

impl CapabilityRootHeaderV1 {
    /// Construct one exact immutable activation projection.
    pub fn new(
        release_set: ContentId,
        market: [u8; 32],
        generation: u64,
        selection: CapabilityExecutionSelectionV1,
    ) -> Result<Self> {
        if market.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroIdentity);
        }
        Ok(Self {
            release_set,
            market,
            generation,
            selection,
        })
    }

    /// Hostile-decode one exact immutable root header.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CAPABILITY_ROOT_HEADER_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if slice(
            bytes,
            CAPABILITY_ROOT_MAGIC_OFFSET,
            CAPABILITY_ROOT_MAGIC_V1.len(),
        )? != CAPABILITY_ROOT_MAGIC_V1
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, CAPABILITY_ROOT_SCHEMA_VERSION_OFFSET)?
            != CAPABILITY_ROOT_SCHEMA_VERSION_V1
        {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, CAPABILITY_ROOT_PROFILE_OFFSET)? != CAPABILITY_ROOT_PROFILE_V1 {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(bytes, CAPABILITY_ROOT_RESERVED_OFFSET, 4)?;
        Self::new(
            content(bytes, CAPABILITY_ROOT_RELEASE_SET_OFFSET)?,
            read_array(bytes, CAPABILITY_ROOT_MARKET_OFFSET)?,
            read_u64(bytes, CAPABILITY_ROOT_GENERATION_OFFSET)?,
            CapabilityExecutionSelectionV1::decode(slice(
                bytes,
                CAPABILITY_ROOT_SELECTION_OFFSET,
                CAPABILITY_EXECUTION_SELECTION_BYTES_V1,
            )?)
            .map_err(|_| Error::SelectionMismatch)?,
        )
    }

    /// Encode the one exact immutable root-header projection.
    pub fn to_bytes(self) -> [u8; CAPABILITY_ROOT_HEADER_BYTES_V1] {
        let mut output = [0_u8; CAPABILITY_ROOT_HEADER_BYTES_V1];
        copy(
            &mut output,
            CAPABILITY_ROOT_MAGIC_OFFSET,
            &CAPABILITY_ROOT_MAGIC_V1,
        );
        put_u16(
            &mut output,
            CAPABILITY_ROOT_SCHEMA_VERSION_OFFSET,
            CAPABILITY_ROOT_SCHEMA_VERSION_V1,
        );
        put_u16(
            &mut output,
            CAPABILITY_ROOT_PROFILE_OFFSET,
            CAPABILITY_ROOT_PROFILE_V1,
        );
        copy(
            &mut output,
            CAPABILITY_ROOT_RELEASE_SET_OFFSET,
            self.release_set.as_bytes(),
        );
        copy(&mut output, CAPABILITY_ROOT_MARKET_OFFSET, &self.market);
        copy(
            &mut output,
            CAPABILITY_ROOT_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        copy(
            &mut output,
            CAPABILITY_ROOT_SELECTION_OFFSET,
            &self.selection.to_bytes(),
        );
        output
    }

    /// Return the immutable execution release-set identity.
    pub const fn release_set(self) -> ContentId {
        self.release_set
    }
    /// Return the exact Market account identity.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Return the exact Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Return the exact manifest-derived activation projection.
    pub const fn selection(self) -> CapabilityExecutionSelectionV1 {
        self.selection
    }
    /// Return the sole canonical root-account PDA seed projection.
    pub fn seeds(self) -> CapabilityRootSeedsV1 {
        CapabilityRootSeedsV1::new(self)
    }
}

/// Borrowed exact view of one composite Trading root account.
///
/// The immutable header is selected by the Trading artifact profile. The
/// descriptor's `root_schema` names only `state`, the mutable family tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityRootAccountV1<'a> {
    header: CapabilityRootHeaderV1,
    state: &'a [u8],
}

impl<'a> CapabilityRootAccountV1<'a> {
    /// Hostile-decode an exact `header || descriptor-sized state` account.
    pub fn decode(bytes: &'a [u8], program: CapabilityProgramV1<'_>) -> Result<Self> {
        let expected = root_account_bytes(program)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let (header, state) = bytes.split_at(CAPABILITY_ROOT_HEADER_BYTES_V1);
        Ok(Self {
            header: CapabilityRootHeaderV1::decode(header)?,
            state,
        })
    }

    /// Return the immutable common activation projection.
    pub const fn header(self) -> CapabilityRootHeaderV1 {
        self.header
    }

    /// Borrow the exact descriptor-schema-owned mutable state tail.
    pub const fn state(self) -> &'a [u8] {
        self.state
    }
}

/// Initialize one vacant composite root account atomically in caller memory.
pub fn initialize_root_account_v1(
    output: &mut [u8],
    header: CapabilityRootHeaderV1,
    program: CapabilityProgramV1<'_>,
    initial_state: &[u8],
) -> Result<()> {
    let expected = root_account_bytes(program)?;
    let state_bytes =
        usize::try_from(program.root_state_bytes()).map_err(|_| Error::InvalidRootStateBytes)?;
    if output.len() != expected || initial_state.len() != state_bytes {
        return Err(Error::InvalidLength);
    }
    let (header_output, state_output) = output.split_at_mut(CAPABILITY_ROOT_HEADER_BYTES_V1);
    header_output.copy_from_slice(&header.to_bytes());
    state_output.copy_from_slice(initial_state);
    Ok(())
}

/// Authenticate a composite account and borrow only its mutable family tail.
///
/// The returned mutable slice cannot alias the immutable header. This prevents
/// a family handler from mutating the activation projection by construction.
pub fn split_root_account_mut_v1<'a>(
    bytes: &'a mut [u8],
    program: CapabilityProgramV1<'_>,
) -> Result<(CapabilityRootHeaderV1, &'a mut [u8])> {
    let expected = root_account_bytes(program)?;
    if bytes.len() != expected {
        return Err(Error::InvalidLength);
    }
    let (header, state) = bytes.split_at_mut(CAPABILITY_ROOT_HEADER_BYTES_V1);
    Ok((CapabilityRootHeaderV1::decode(header)?, state))
}

fn root_account_bytes(program: CapabilityProgramV1<'_>) -> Result<usize> {
    let state =
        usize::try_from(program.root_state_bytes()).map_err(|_| Error::InvalidRootStateBytes)?;
    CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(state)
        .filter(|bytes| *bytes <= CAPABILITY_ROOT_ACCOUNT_MAX_BYTES_V1)
        .ok_or(Error::InvalidRootStateBytes)
}

/// Owned exact seed projection for one immutable Trading child root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityRootSeedsV1 {
    market: [u8; 32],
    generation: [u8; 8],
    manifest: [u8; 32],
    entry_index: [u8; 2],
    kind: [u8; 32],
    capability_release: [u8; 32],
    config: [u8; 32],
}

impl CapabilityRootSeedsV1 {
    fn new(root: CapabilityRootHeaderV1) -> Self {
        let selection = root.selection;
        Self {
            market: root.market,
            generation: root.generation.to_le_bytes(),
            manifest: selection.manifest().to_bytes(),
            entry_index: selection.entry_index().to_le_bytes(),
            kind: selection.kind().to_bytes(),
            capability_release: selection.capability_release().to_bytes(),
            config: selection.config().to_bytes(),
        }
    }

    /// Return the exact seed order interpreted under the Trading Program ID.
    pub fn as_slices(&self) -> [&[u8]; 8] {
        [
            CAPABILITY_ROOT_PDA_DOMAIN_V1,
            &self.market,
            &self.generation,
            &self.manifest,
            &self.entry_index,
            &self.kind,
            &self.capability_release,
            &self.config,
        ]
    }
}

fn content(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(read_array(bytes, offset)?).map_err(|_| Error::ZeroIdentity)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    bytes.get(offset..end).ok_or(Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if slice(bytes, offset, width)?.iter().any(|byte| *byte != 0) {
        Err(Error::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

fn copy(output: &mut [u8], offset: usize, source: &[u8]) {
    let Some(end) = offset.checked_add(source.len()) else {
        return;
    };
    let Some(destination) = output.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(source);
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    copy(output, offset, &value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{vec, vec::Vec};

    use dclutch_capability_contract::{
        ActivationPolicy, FundingAmountsV1, FundingQuoteV1, MAX_DEPENDENCIES_PER_CAPABILITY,
    };

    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("identity")
    }

    fn fixture_write(output: &mut [u8], offset: usize, source: &[u8]) {
        let end = offset.checked_add(source.len()).expect("fixture width");
        output
            .get_mut(offset..end)
            .expect("fixture destination")
            .copy_from_slice(source);
    }

    fn fixture_fill(output: &mut [u8], offset: usize, width: usize, value: u8) {
        let end = offset.checked_add(width).expect("fixture width");
        output
            .get_mut(offset..end)
            .expect("fixture destination")
            .fill(value);
    }

    fn fixture_set(output: &mut [u8], offset: usize, value: u8) {
        *output.get_mut(offset).expect("fixture byte") = value;
    }

    fn program_bytes(instruction_count: usize) -> Vec<u8> {
        let mut bytes = vec![0_u8; 16 + instruction_count * 24];
        fixture_write(&mut bytes, 0, b"DCTV");
        fixture_set(&mut bytes, 4, 2);
        fixture_write(
            &mut bytes,
            6,
            &u16::try_from(instruction_count)
                .expect("profile count")
                .to_le_bytes(),
        );
        fixture_write(&mut bytes, 8, &300_u16.to_le_bytes());
        fixture_write(&mut bytes, 10, &1_u16.to_le_bytes());
        for index in 0..instruction_count {
            let offset = 16 + index * 24;
            fixture_set(&mut bytes, offset, 0);
            fixture_write(
                &mut bytes,
                offset + 2,
                &u16::try_from(index % 300).expect("register").to_le_bytes(),
            );
            fixture_write(
                &mut bytes,
                offset + 16,
                &u64::try_from(index + 1).expect("immediate").to_le_bytes(),
            );
        }
        bytes
    }

    fn descriptor(instruction_count: usize) -> Vec<u8> {
        let transition = program_bytes(instruction_count);
        let mut bytes = vec![0_u8; CAPABILITY_PROGRAM_HEADER_BYTES_V1 + transition.len()];
        fixture_write(&mut bytes, 0, &CAPABILITY_PROGRAM_MAGIC_V1);
        fixture_write(&mut bytes, 8, &1_u16.to_le_bytes());
        fixture_write(
            &mut bytes,
            CAPABILITY_PROGRAM_PROFILE_OFFSET,
            &CAPABILITY_PROGRAM_PROFILE_V2.to_le_bytes(),
        );
        for (offset, byte) in [
            (CAPABILITY_PROGRAM_KIND_OFFSET, 1),
            (CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, 2),
            (CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET, 3),
            (CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, 4),
            (CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, 5),
            (CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET, 6),
            (CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET, 7),
            (CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, 8),
        ] {
            fixture_fill(&mut bytes, offset, 32, byte);
        }
        fixture_write(
            &mut bytes,
            CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
            &128_u32.to_le_bytes(),
        );
        fixture_write(&mut bytes, CAPABILITY_PROGRAM_HEADER_BYTES_V1, &transition);
        bytes
    }

    fn selection() -> CapabilityExecutionSelectionV1 {
        CapabilityExecutionSelectionV1::new(3, id(9), id(1), id(10), id(11)).expect("selection")
    }

    fn entry() -> CapabilityEntryV1 {
        CapabilityEntryV1::new(
            id(1),
            id(10),
            id(11),
            id(7),
            id(4),
            id(6),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(FundingAmountsV1::default(), None).expect("zero quote"),
        )
        .expect("entry")
    }

    #[test]
    fn descriptor_is_runtime_width_and_manifest_joined() {
        let bytes = descriptor(1);
        let decoded = CapabilityProgramV1::decode(&bytes).expect("descriptor");
        decoded
            .validate_selection(selection(), entry())
            .expect("join");
        assert_eq!(decoded.kind(), id(1));
        assert_eq!(decoded.root_state_bytes(), 128);
        assert_eq!(decoded.transition_program().bytes(), program_bytes(1));
        assert_eq!(decoded.transition_program().scalar_count(), 300);
        assert_eq!(decoded.transition_program().identity_count(), 1);
        let supported = SupportedContentV1 {
            config_schema: id(2),
            request_schema: id(3),
            root_schema: id(4),
            account_profile: id(5),
            derivation_policy: id(6),
            effect_schema: id(8),
        };
        supported.require(decoded).expect("supported content");
        let input_scalars = vec![0_u64; 300];
        let input_identities = vec![[0_u8; 32]; 1];
        let mut scratch_scalars = vec![0_u64; 300];
        let mut scratch_identities = vec![[0_u8; 32]; 1];
        let mut output_scalars = vec![0_u64; 300];
        let mut output_identities = vec![[0_u8; 32]; 1];
        decoded
            .execute(CapabilityRegistersV2::new(
                RegisterInput {
                    scalars: &input_scalars,
                    identities: &input_identities,
                },
                RegisterOutput {
                    scalars: &mut scratch_scalars,
                    identities: &mut scratch_identities,
                },
                RegisterOutput {
                    scalars: &mut output_scalars,
                    identities: &mut output_identities,
                },
            ))
            .expect("transition");
        assert_eq!(output_scalars.first(), Some(&1));
    }

    #[test]
    fn maximum_descriptor_width_is_exact_and_decodable() {
        let bytes = descriptor(CAPABILITY_PROGRAM_TRANSITION_MAX_INSTRUCTIONS_V2);
        assert_eq!(CAPABILITY_PROGRAM_MAX_BYTES_V1, 1_304);
        assert_eq!(bytes.len(), CAPABILITY_PROGRAM_MAX_BYTES_V1);
        assert!(CapabilityProgramV1::decode(&bytes).is_ok());

        let direct_shaped = descriptor(35);
        assert_eq!(
            direct_shaped.len() - CAPABILITY_PROGRAM_HEADER_BYTES_V1,
            856
        );
        assert_eq!(direct_shaped.len(), 1_136);
        assert!(CapabilityProgramV1::decode(&direct_shaped).is_ok());

        assert_eq!(
            CapabilityProgramV1::decode(&descriptor(
                CAPABILITY_PROGRAM_TRANSITION_MAX_INSTRUCTIONS_V2 + 1
            )),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn descriptor_hostile_encodings_and_unknown_content_refuse() {
        let canonical = descriptor(1);
        for (offset, expected) in [
            (0, Error::InvalidMagic),
            (8, Error::UnsupportedSchema),
            (10, Error::UnsupportedArtifactProfile),
            (12, Error::NonCanonicalReservedBytes),
            (CAPABILITY_PROGRAM_KIND_OFFSET, Error::ZeroIdentity),
        ] {
            let mut hostile = canonical.clone();
            if offset == CAPABILITY_PROGRAM_KIND_OFFSET {
                fixture_fill(&mut hostile, offset, 32, 0);
            } else {
                let value = hostile.get(offset).copied().expect("hostile byte") ^ 0xff;
                fixture_set(&mut hostile, offset, value);
            }
            assert_eq!(CapabilityProgramV1::decode(&hostile), Err(expected));
        }
        for hostile in [
            canonical
                .get(..canonical.len() - 1)
                .expect("truncated descriptor"),
            canonical
                .get(..CAPABILITY_PROGRAM_HEADER_BYTES_V1)
                .expect("header-only descriptor"),
        ] {
            assert!(CapabilityProgramV1::decode(hostile).is_err());
        }
        let decoded = CapabilityProgramV1::decode(&canonical).expect("descriptor");
        let unsupported = SupportedContentV1 {
            config_schema: id(99),
            request_schema: id(3),
            root_schema: id(4),
            account_profile: id(5),
            derivation_policy: id(6),
            effect_schema: id(8),
        };
        assert_eq!(unsupported.require(decoded), Err(Error::UnsupportedContent));

        let mut legacy_fixed_bank_body = canonical.clone();
        fixture_set(
            &mut legacy_fixed_bank_body,
            CAPABILITY_PROGRAM_HEADER_BYTES_V1 + 4,
            1,
        );
        assert_eq!(
            CapabilityProgramV1::decode(&legacy_fixed_bank_body),
            Err(Error::InvalidTransitionProgram)
        );

        let mut zero_state = canonical;
        fixture_fill(
            &mut zero_state,
            CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
            4,
            0,
        );
        assert_eq!(
            CapabilityProgramV1::decode(&zero_state),
            Err(Error::InvalidRootStateBytes)
        );
    }

    #[test]
    fn composite_root_round_trip_and_seed_order_bind_full_selection() {
        let header =
            CapabilityRootHeaderV1::new(id(12), [13; 32], 14, selection()).expect("header");
        let descriptor_bytes = descriptor(1);
        let program = CapabilityProgramV1::decode(&descriptor_bytes).expect("descriptor");
        let initial_state = [44_u8; 128];
        let mut bytes = [0_u8; CAPABILITY_ROOT_HEADER_BYTES_V1 + 128];
        initialize_root_account_v1(&mut bytes, header, program, &initial_state)
            .expect("composite root");
        let decoded = CapabilityRootAccountV1::decode(&bytes, program).expect("root account");
        assert_eq!(decoded.header(), header);
        assert_eq!(decoded.state(), initial_state);
        let seeds = header.seeds();
        let slices = seeds.as_slices();
        assert_eq!(slices[0], CAPABILITY_ROOT_PDA_DOMAIN_V1);
        assert_eq!(slices[1], [13; 32]);
        assert_eq!(slices[2], 14_u64.to_le_bytes());
        assert_eq!(slices[3], [9; 32]);
        assert_eq!(slices[4], 3_u16.to_le_bytes());
        assert_eq!(slices[5], [1; 32]);
        assert_eq!(slices[6], [10; 32]);
        assert_eq!(slices[7], [11; 32]);
    }

    #[test]
    fn root_hostile_decoding_refuses_without_alternate_projection() {
        let canonical = CapabilityRootHeaderV1::new(id(12), [13; 32], 14, selection())
            .expect("root")
            .to_bytes();
        for (offset, expected) in [
            (0, Error::InvalidMagic),
            (8, Error::UnsupportedSchema),
            (10, Error::UnsupportedArtifactProfile),
            (12, Error::NonCanonicalReservedBytes),
            (CAPABILITY_ROOT_SELECTION_OFFSET, Error::SelectionMismatch),
        ] {
            let mut hostile = canonical;
            let value = hostile.get(offset).copied().expect("hostile byte") ^ 0xff;
            fixture_set(&mut hostile, offset, value);
            assert_eq!(CapabilityRootHeaderV1::decode(&hostile), Err(expected));
        }
        assert_eq!(
            CapabilityRootHeaderV1::decode(&canonical[..231]),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn mutable_split_exposes_only_the_descriptor_sized_tail() {
        let header =
            CapabilityRootHeaderV1::new(id(12), [13; 32], 14, selection()).expect("header");
        let descriptor_bytes = descriptor(1);
        let program = CapabilityProgramV1::decode(&descriptor_bytes).expect("descriptor");
        let mut account = [0_u8; CAPABILITY_ROOT_HEADER_BYTES_V1 + 128];
        initialize_root_account_v1(&mut account, header, program, &[0; 128]).expect("root account");
        let immutable_before = header.to_bytes();
        let (decoded, state) =
            split_root_account_mut_v1(&mut account, program).expect("mutable state tail");
        assert_eq!(decoded, header);
        state.fill(77);
        assert_eq!(
            &account[..CAPABILITY_ROOT_HEADER_BYTES_V1],
            immutable_before.as_slice()
        );
        assert_eq!(&account[CAPABILITY_ROOT_HEADER_BYTES_V1..], &[77; 128]);
    }
}
