//! RequestProfile V2 and generic native-Ed25519 evidence projection.
//!
//! V2 wraps one exact V1 request program and appends a canonical, record-bounded
//! list of signature requirements.  Each requirement selects an absolute byte
//! range in the complete current Trading instruction and one initially-zero
//! identity register.  The SBF adapter owns Instructions-sysvar observation;
//! this module owns the SDK-free fixed wire and failure-atomic projection.

use core::convert::TryFrom;

use super::{ProjectionRegistersV1, ProjectionTargetV1, RequestProfileV1, project_atomic};

/// RequestProfile V2 magic.
pub const REQUEST_PROFILE_V2_MAGIC: [u8; 8] = *b"DCLTRP02";
/// RequestProfile V2 schema version.
pub const REQUEST_PROFILE_V2_SCHEMA_VERSION: u16 = 2;
/// RequestProfile V2 physical artifact profile.
pub const REQUEST_PROFILE_V2_ARTIFACT_PROFILE: u16 = 2;
/// Exact RequestProfile V2 wrapper-header width.
pub const REQUEST_PROFILE_V2_HEADER_BYTES: usize = 24;
/// Exact width of one native-signature evidence requirement.
pub const NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1: usize = 8;
/// Finalized-record schema label for RequestProfile V2.
pub const REQUEST_PROFILE_V2_SCHEMA_RELEASE_PREIMAGE: &[u8] = b"dclutch/schema/request-profile-v2";
/// SHA-256 of [`REQUEST_PROFILE_V2_SCHEMA_RELEASE_PREIMAGE`].
pub const REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID: [u8; 32] = [
    0x1d, 0x51, 0x51, 0x75, 0x92, 0x0b, 0x5c, 0x8a, 0x93, 0xb3, 0x00, 0x4c, 0x6a, 0x67, 0x1b, 0x33,
    0xb5, 0xfb, 0xcc, 0x69, 0x82, 0xf4, 0x81, 0xc9, 0x90, 0x0e, 0xe7, 0x16, 0x68, 0xce, 0x75, 0x99,
];

/// Official `solana-ed25519-program = 3.0.0` descriptor width.
pub const ED25519_SIGNATURE_OFFSETS_BYTES: usize = 14;
/// Official native-Ed25519 descriptor-table start.
pub const ED25519_SIGNATURE_OFFSETS_START: usize = 2;
/// Official serialized Ed25519 public-key width.
pub const ED25519_PUBLIC_KEY_BYTES: usize = 32;
/// Official serialized Ed25519 signature width.
pub const ED25519_SIGNATURE_BYTES: usize = 64;
/// Official sentinel selecting bytes in the Ed25519 instruction itself.
pub const ED25519_SELF_INSTRUCTION_INDEX: u16 = u16::MAX;

const EMBEDDED_V1_BYTES_OFFSET: usize = 12;
const REQUIREMENT_COUNT_OFFSET: usize = 16;
const HEADER_RESERVED_OFFSET: usize = 20;

/// Stable RequestProfile V2 or native-signature evidence refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Selected and authenticated profile identities differed or were zero.
    ProgramIdentityMismatch,
    /// A profile, instruction, register bank, or selected range had another width.
    InvalidLength,
    /// Profile magic, schema, artifact profile, or reserved bytes were noncanonical.
    InvalidHeader,
    /// Embedded RequestProfile V1 refused.
    InvalidEmbeddedProfile,
    /// Signature requirements were empty, unordered, overlapping, or aliased.
    InvalidRequirement,
    /// Native Ed25519 descriptor or canonical payload placement differed.
    InvalidNativeInstruction,
    /// Native message differed from the exact selected current-instruction slice.
    MessageMismatch,
    /// A signer identity was zero or its destination register was not vacant.
    InvalidSigner,
    /// Checked offset, length, count, or register arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for RequestProfile V2 operations.
pub type Result<T> = core::result::Result<T, Error>;

/// One exact current-instruction message slice and signer-register destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeSignatureRequirementV1 {
    message_offset: u16,
    message_bytes: u16,
    destination_identity_register: u32,
}

impl NativeSignatureRequirementV1 {
    /// Construct one exact current-instruction range and identity destination.
    ///
    /// Complete ordering, nonoverlap, and destination uniqueness are checked by
    /// [`encode_request_profile_v2_atomic`] and hostile decoding.
    pub const fn new(
        message_offset: u16,
        message_bytes: u16,
        destination_identity_register: u32,
    ) -> Self {
        Self {
            message_offset,
            message_bytes,
            destination_identity_register,
        }
    }

    /// Absolute byte offset in the complete current Trading instruction.
    pub const fn message_offset(self) -> u16 {
        self.message_offset
    }

    /// Exact nonzero signed message width.
    pub const fn message_bytes(self) -> u16 {
        self.message_bytes
    }

    /// Flat destination index in the RequestProfile identity bank.
    pub const fn destination_identity_register(self) -> u32 {
        self.destination_identity_register
    }
}

/// Encode one complete signed RequestProfile V2 into caller-owned buffers.
///
/// The embedded V1 bytes are hostile-decoded first. The candidate wrapper is
/// then built in `scratch`, hostile-decoded as a complete V2 profile, and
/// copied to `output` only after every signature requirement accepts.
pub fn encode_request_profile_v2_atomic(
    embedded_v1: &[u8],
    requirements: &[NativeSignatureRequirementV1],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    RequestProfileV1::decode(embedded_v1).map_err(|_| Error::InvalidEmbeddedProfile)?;
    let count = u32::try_from(requirements.len()).map_err(|_| Error::ArithmeticOverflow)?;
    let embedded_bytes = u32::try_from(embedded_v1.len()).map_err(|_| Error::ArithmeticOverflow)?;
    let tail_bytes = requirements
        .len()
        .checked_mul(NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1)
        .ok_or(Error::ArithmeticOverflow)?;
    let expected = REQUEST_PROFILE_V2_HEADER_BYTES
        .checked_add(embedded_v1.len())
        .and_then(|prefix| prefix.checked_add(tail_bytes))
        .ok_or(Error::ArithmeticOverflow)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    write(scratch, 0, &REQUEST_PROFILE_V2_MAGIC)?;
    write(scratch, 8, &REQUEST_PROFILE_V2_SCHEMA_VERSION.to_le_bytes())?;
    write(
        scratch,
        10,
        &REQUEST_PROFILE_V2_ARTIFACT_PROFILE.to_le_bytes(),
    )?;
    write(
        scratch,
        EMBEDDED_V1_BYTES_OFFSET,
        &embedded_bytes.to_le_bytes(),
    )?;
    write(scratch, REQUIREMENT_COUNT_OFFSET, &count.to_le_bytes())?;
    write(scratch, REQUEST_PROFILE_V2_HEADER_BYTES, embedded_v1)?;
    let mut cursor = REQUEST_PROFILE_V2_HEADER_BYTES
        .checked_add(embedded_v1.len())
        .ok_or(Error::ArithmeticOverflow)?;
    for requirement in requirements {
        write(scratch, cursor, &requirement.message_offset.to_le_bytes())?;
        write(
            scratch,
            add(cursor, 2)?,
            &requirement.message_bytes.to_le_bytes(),
        )?;
        write(
            scratch,
            add(cursor, 4)?,
            &requirement.destination_identity_register.to_le_bytes(),
        )?;
        cursor = add(cursor, NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1)?;
    }
    if cursor != expected {
        return Err(Error::InvalidLength);
    }
    RequestProfileV2::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

/// Borrowed fixed-entry native-signature evidence tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeSignatureEvidenceProfileV1<'a> {
    count: u32,
    bytes: &'a [u8],
}

impl<'a> NativeSignatureEvidenceProfileV1<'a> {
    /// Exact nonzero number of required native signatures.
    pub const fn requirement_count(self) -> u32 {
        self.count
    }

    /// Return one checked requirement by canonical ordinal.
    pub fn requirement(self, index: u32) -> Result<NativeSignatureRequirementV1> {
        if index >= self.count {
            return Err(Error::InvalidRequirement);
        }
        let offset = usize::try_from(index)
            .map_err(|_| Error::ArithmeticOverflow)?
            .checked_mul(NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(NativeSignatureRequirementV1 {
            message_offset: read_u16(self.bytes, offset)?,
            message_bytes: read_u16(self.bytes, add(offset, 2)?)?,
            destination_identity_register: read_u32(self.bytes, add(offset, 4)?)?,
        })
    }

    /// Borrow the complete canonical fixed-entry tail.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Hostile-decoded RequestProfile V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestProfileV2<'a> {
    request: RequestProfileV1<'a>,
    signatures: NativeSignatureEvidenceProfileV1<'a>,
    bytes: &'a [u8],
}

impl<'a> RequestProfileV2<'a> {
    /// Decode only after descriptor selection joins authenticated raw bytes.
    pub fn decode_selected(
        selected_program_id: [u8; 32],
        authenticated_program_id: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<Self> {
        if selected_program_id == [0; 32]
            || authenticated_program_id == [0; 32]
            || selected_program_id != authenticated_program_id
        {
            return Err(Error::ProgramIdentityMismatch);
        }
        Self::decode(bytes)
    }

    /// Hostile-decode one exact V2 wrapper, V1 program, and evidence tail.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < REQUEST_PROFILE_V2_HEADER_BYTES
            || bytes.get(..8) != Some(REQUEST_PROFILE_V2_MAGIC.as_slice())
            || read_u16(bytes, 8)? != REQUEST_PROFILE_V2_SCHEMA_VERSION
            || read_u16(bytes, 10)? != REQUEST_PROFILE_V2_ARTIFACT_PROFILE
            || read_u32(bytes, HEADER_RESERVED_OFFSET)? != 0
        {
            return Err(Error::InvalidHeader);
        }
        let embedded_bytes = usize::try_from(read_u32(bytes, EMBEDDED_V1_BYTES_OFFSET)?)
            .map_err(|_| Error::ArithmeticOverflow)?;
        let count = read_u32(bytes, REQUIREMENT_COUNT_OFFSET)?;
        if count == 0 || count > u32::from(u8::MAX) {
            return Err(Error::InvalidRequirement);
        }
        let tail_bytes = usize::try_from(count)
            .map_err(|_| Error::ArithmeticOverflow)?
            .checked_mul(NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1)
            .ok_or(Error::ArithmeticOverflow)?;
        let embedded_end = REQUEST_PROFILE_V2_HEADER_BYTES
            .checked_add(embedded_bytes)
            .ok_or(Error::ArithmeticOverflow)?;
        let expected = embedded_end
            .checked_add(tail_bytes)
            .ok_or(Error::ArithmeticOverflow)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let request = RequestProfileV1::decode(
            bytes
                .get(REQUEST_PROFILE_V2_HEADER_BYTES..embedded_end)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|_| Error::InvalidEmbeddedProfile)?;
        let signatures = NativeSignatureEvidenceProfileV1 {
            count,
            bytes: bytes.get(embedded_end..).ok_or(Error::InvalidLength)?,
        };
        validate_requirements(signatures)?;
        Ok(Self {
            request,
            signatures,
            bytes,
        })
    }

    /// Embedded exact RequestProfile V1 program.
    pub const fn request_profile(self) -> RequestProfileV1<'a> {
        self.request
    }

    /// Authenticated native-signature evidence requirements.
    pub const fn native_signatures(self) -> NativeSignatureEvidenceProfileV1<'a> {
        self.signatures
    }

    /// Borrow the complete canonical V2 content preimage.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Whether the embedded V1 projector writes `target`.
    pub fn writes_register(self, target: ProjectionTargetV1) -> Result<bool> {
        self.request
            .writes_register(target)
            .map_err(|_| Error::InvalidEmbeddedProfile)
    }

    /// Delegate exact request validation and projection to the embedded V1 owner.
    pub fn project_request_atomic(
        self,
        tail_count: u32,
        request: &[u8],
        registers: ProjectionRegistersV1<'_>,
    ) -> Result<()> {
        project_atomic(self.request, tail_count, request, registers)
            .map_err(|_| Error::InvalidEmbeddedProfile)
    }
}

/// Immediately preceding canonical native-Ed25519 data and the exact message
/// slice authenticated by the enclosing adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeEd25519InstructionViewV1<'a> {
    /// Exact preceding native instruction data.
    pub ed25519_data: &'a [u8],
    /// Exact complete nested message bytes owned by the selected request
    /// profile (the Trading instruction in direct mode and the byte-identical
    /// nested Trading instruction in continuation mode).
    pub authenticated_message_data: &'a [u8],
    /// Authenticated top-level instruction index containing every message.
    pub message_instruction_index: u16,
    /// Checked top-level byte offset at which `authenticated_message_data`
    /// begins. Direct execution uses zero; a typed continuation supplies its
    /// exact canonical header width.
    pub message_offset_bias: u16,
}

/// Failure-atomic signer-identity register banks.
pub struct NativeSignatureRegistersV1<'a> {
    /// Immutable identities after common seeding and before signature evidence.
    pub input_identities: &'a [[u8; 32]],
    /// Scratch candidate; may change on refusal.
    pub scratch_identities: &'a mut [[u8; 32]],
    /// Output changed only after every signature requirement authenticates.
    pub output_identities: &'a mut [[u8; 32]],
}

/// Authenticate the complete canonical native instruction and seed every signer
/// into its distinct initially-zero identity register atomically.
pub fn seed_authenticated_signers_atomic(
    profile: RequestProfileV2<'_>,
    tail_count: u32,
    view: NativeEd25519InstructionViewV1<'_>,
    registers: NativeSignatureRegistersV1<'_>,
) -> Result<()> {
    let identity_count = profile
        .request
        .identity_count(tail_count)
        .map_err(|_| Error::ArithmeticOverflow)?;
    if registers.input_identities.len() != identity_count
        || registers.scratch_identities.len() != identity_count
        || registers.output_identities.len() != identity_count
    {
        return Err(Error::InvalidLength);
    }
    registers
        .scratch_identities
        .copy_from_slice(registers.input_identities);

    let signatures = profile.signatures;
    let count = usize::try_from(signatures.count).map_err(|_| Error::ArithmeticOverflow)?;
    let payload_start = ED25519_SIGNATURE_OFFSETS_START
        .checked_add(
            count
                .checked_mul(ED25519_SIGNATURE_OFFSETS_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)?;
    if view.ed25519_data.first().copied() != u8::try_from(count).ok()
        || view.ed25519_data.get(1).copied() != Some(0)
        || view.ed25519_data.len() < payload_start
    {
        return Err(Error::InvalidNativeInstruction);
    }

    let mut payload = payload_start;
    let mut index = 0_u32;
    while index < signatures.count {
        let requirement = signatures.requirement(index)?;
        let descriptor = ED25519_SIGNATURE_OFFSETS_START
            .checked_add(
                usize::try_from(index)
                    .map_err(|_| Error::ArithmeticOverflow)?
                    .checked_mul(ED25519_SIGNATURE_OFFSETS_BYTES)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        let public_key_offset = payload;
        let signature_offset = public_key_offset
            .checked_add(ED25519_PUBLIC_KEY_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let message_offset = requirement
            .message_offset
            .checked_add(view.message_offset_bias)
            .ok_or(Error::ArithmeticOverflow)?;
        if read_u16(view.ed25519_data, descriptor)? != u16_from(signature_offset)?
            || read_u16(view.ed25519_data, add(descriptor, 2)?)? != ED25519_SELF_INSTRUCTION_INDEX
            || read_u16(view.ed25519_data, add(descriptor, 4)?)? != u16_from(public_key_offset)?
            || read_u16(view.ed25519_data, add(descriptor, 6)?)? != ED25519_SELF_INSTRUCTION_INDEX
            || read_u16(view.ed25519_data, add(descriptor, 8)?)? != message_offset
            || read_u16(view.ed25519_data, add(descriptor, 10)?)? != requirement.message_bytes
            || read_u16(view.ed25519_data, add(descriptor, 12)?)? != view.message_instruction_index
            || view.message_instruction_index == ED25519_SELF_INSTRUCTION_INDEX
        {
            return Err(Error::InvalidNativeInstruction);
        }
        let signer: [u8; 32] = array(
            view.ed25519_data
                .get(public_key_offset..signature_offset)
                .ok_or(Error::InvalidNativeInstruction)?,
        )?;
        if signer == [0; 32]
            || view
                .ed25519_data
                .get(
                    signature_offset
                        ..signature_offset
                            .checked_add(ED25519_SIGNATURE_BYTES)
                            .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::InvalidNativeInstruction)?
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(Error::InvalidSigner);
        }
        let current_start = usize::from(requirement.message_offset);
        let current_end = current_start
            .checked_add(usize::from(requirement.message_bytes))
            .ok_or(Error::ArithmeticOverflow)?;
        if view
            .authenticated_message_data
            .get(current_start..current_end)
            .is_none()
        {
            return Err(Error::MessageMismatch);
        }
        let destination = usize::try_from(requirement.destination_identity_register)
            .map_err(|_| Error::ArithmeticOverflow)?;
        let slot = registers
            .scratch_identities
            .get_mut(destination)
            .ok_or(Error::InvalidRequirement)?;
        if *slot != [0; 32] {
            return Err(Error::InvalidSigner);
        }
        *slot = signer;
        payload = signature_offset
            .checked_add(ED25519_SIGNATURE_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    if payload != view.ed25519_data.len() {
        return Err(Error::InvalidNativeInstruction);
    }
    registers
        .output_identities
        .copy_from_slice(registers.scratch_identities);
    Ok(())
}

fn validate_requirements(profile: NativeSignatureEvidenceProfileV1<'_>) -> Result<()> {
    let mut index = 0_u32;
    let mut prior_end = 0_u32;
    while index < profile.count {
        let requirement = profile.requirement(index)?;
        if requirement.message_bytes == 0 {
            return Err(Error::InvalidRequirement);
        }
        let start = u32::from(requirement.message_offset);
        let end = start
            .checked_add(u32::from(requirement.message_bytes))
            .ok_or(Error::ArithmeticOverflow)?;
        if end > u32::from(u16::MAX) || (index != 0 && start < prior_end) {
            return Err(Error::InvalidRequirement);
        }
        let mut prior = 0_u32;
        while prior < index {
            if profile.requirement(prior)?.destination_identity_register
                == requirement.destination_identity_register
            {
                return Err(Error::InvalidRequirement);
            }
            prior = prior.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        prior_end = end;
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Error::ArithmeticOverflow)
}

fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N]> {
    bytes.try_into().map_err(|_| Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let end = add(offset, 2)?;
    Ok(u16::from_le_bytes(array(
        bytes.get(offset..end).ok_or(Error::InvalidLength)?,
    )?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = add(offset, 4)?;
    Ok(u32::from_le_bytes(array(
        bytes.get(offset..end).ok_or(Error::InvalidLength)?,
    )?))
}

fn u16_from(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::ArithmeticOverflow)
}

fn write(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<()> {
    let end = add(offset, bytes.len())?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;
    use std::vec::Vec;

    use crate::generated::AGREEMENT_PROFILE_V1;

    use super::*;

    #[test]
    fn native_wire_constants_match_pinned_official_solana_definitions() {
        assert_eq!(
            ED25519_SIGNATURE_OFFSETS_BYTES,
            solana_ed25519_program::SIGNATURE_OFFSETS_SERIALIZED_SIZE,
        );
        assert_eq!(
            ED25519_SIGNATURE_OFFSETS_START,
            solana_ed25519_program::SIGNATURE_OFFSETS_START,
        );
        assert_eq!(
            ED25519_PUBLIC_KEY_BYTES,
            solana_ed25519_program::PUBKEY_SERIALIZED_SIZE,
        );
        assert_eq!(
            ED25519_SIGNATURE_BYTES,
            solana_ed25519_program::SIGNATURE_SERIALIZED_SIZE,
        );
        assert_eq!(
            core::mem::size_of::<solana_ed25519_program::Ed25519SignatureOffsets>(),
            ED25519_SIGNATURE_OFFSETS_BYTES,
        );
    }

    fn profile_bytes(requirements: &[(u16, u16, u32)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&REQUEST_PROFILE_V2_MAGIC);
        bytes.extend_from_slice(&REQUEST_PROFILE_V2_SCHEMA_VERSION.to_le_bytes());
        bytes.extend_from_slice(&REQUEST_PROFILE_V2_ARTIFACT_PROFILE.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(AGREEMENT_PROFILE_V1.len())
                .expect("fixture width")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(
            &u32::try_from(requirements.len())
                .expect("fixture count")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&AGREEMENT_PROFILE_V1);
        for (offset, width, destination) in requirements {
            bytes.extend_from_slice(&offset.to_le_bytes());
            bytes.extend_from_slice(&width.to_le_bytes());
            bytes.extend_from_slice(&destination.to_le_bytes());
        }
        bytes
    }

    fn native_instruction(
        requirements: &[(u16, u16, u32)],
        signers: &[[u8; 32]],
        message_instruction_index: u16,
        message_offset_bias: u16,
    ) -> Vec<u8> {
        let header =
            ED25519_SIGNATURE_OFFSETS_START + requirements.len() * ED25519_SIGNATURE_OFFSETS_BYTES;
        let mut bytes = vec![0_u8; header];
        *bytes.first_mut().expect("count byte") =
            u8::try_from(requirements.len()).expect("fixture count");
        let mut payload = header;
        for (index, ((offset, width, _), signer)) in requirements.iter().zip(signers).enumerate() {
            let descriptor =
                ED25519_SIGNATURE_OFFSETS_START + index * ED25519_SIGNATURE_OFFSETS_BYTES;
            let public_key = payload;
            let signature = public_key + ED25519_PUBLIC_KEY_BYTES;
            for (field, value) in [
                (descriptor, u16::try_from(signature).expect("offset")),
                (descriptor + 2, ED25519_SELF_INSTRUCTION_INDEX),
                (descriptor + 4, u16::try_from(public_key).expect("offset")),
                (descriptor + 6, ED25519_SELF_INSTRUCTION_INDEX),
                (
                    descriptor + 8,
                    offset
                        .checked_add(message_offset_bias)
                        .expect("message offset"),
                ),
                (descriptor + 10, *width),
                (descriptor + 12, message_instruction_index),
            ] {
                bytes
                    .get_mut(field..field + 2)
                    .expect("descriptor field")
                    .copy_from_slice(&value.to_le_bytes());
            }
            bytes.extend_from_slice(signer);
            bytes.extend_from_slice(&[0x55; ED25519_SIGNATURE_BYTES]);
            payload = signature + ED25519_SIGNATURE_BYTES;
        }
        assert_eq!(payload, bytes.len());
        bytes
    }

    #[test]
    fn exact_record_bounded_profile_and_native_batch_seed_atomically() {
        let requirements = [(20, 3, 0), (30, 4, 1)];
        let bytes = profile_bytes(&requirements);
        let profile = RequestProfileV2::decode(&bytes).expect("profile");
        assert_eq!(profile.request_profile().bytes(), AGREEMENT_PROFILE_V1);
        assert_eq!(profile.native_signatures().requirement_count(), 2);
        let mut current = [0_u8; 40];
        current
            .get_mut(20..23)
            .expect("message one")
            .copy_from_slice(b"one");
        current
            .get_mut(30..34)
            .expect("message two")
            .copy_from_slice(b"two!");
        let native = native_instruction(&requirements, &[[7; 32], [8; 32]], 4, 128);
        let input = [[0_u8; 32]; 2];
        let mut scratch = [[9_u8; 32]; 2];
        let mut output = [[6_u8; 32]; 2];
        seed_authenticated_signers_atomic(
            profile,
            2,
            NativeEd25519InstructionViewV1 {
                ed25519_data: &native,
                authenticated_message_data: &current,
                message_instruction_index: 4,
                message_offset_bias: 128,
            },
            NativeSignatureRegistersV1 {
                input_identities: &input,
                scratch_identities: &mut scratch,
                output_identities: &mut output,
            },
        )
        .expect("evidence");
        assert_eq!(output, [[7; 32], [8; 32]]);
    }

    #[test]
    fn typed_v2_encoder_round_trips_and_preserves_output_on_refusal() {
        let requirements = [
            NativeSignatureRequirementV1::new(20, 3, 0),
            NativeSignatureRequirementV1::new(30, 4, 1),
        ];
        let width = REQUEST_PROFILE_V2_HEADER_BYTES
            + AGREEMENT_PROFILE_V1.len()
            + requirements.len() * NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1;
        let mut scratch = vec![0_u8; width];
        let mut output = vec![9_u8; width];
        encode_request_profile_v2_atomic(
            &AGREEMENT_PROFILE_V1,
            &requirements,
            &mut scratch,
            &mut output,
        )
        .expect("typed v2 profile");
        let decoded = RequestProfileV2::decode(&output).expect("decode encoded v2 profile");
        assert_eq!(decoded.native_signatures().requirement_count(), 2);
        assert_eq!(
            decoded
                .native_signatures()
                .requirement(1)
                .expect("second requirement")
                .destination_identity_register(),
            1
        );

        let overlapping = [
            NativeSignatureRequirementV1::new(20, 3, 0),
            NativeSignatureRequirementV1::new(22, 4, 1),
        ];
        let mut hostile_scratch = vec![0_u8; width];
        let mut hostile_output = vec![7_u8; width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_request_profile_v2_atomic(
                &AGREEMENT_PROFILE_V1,
                &overlapping,
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::InvalidRequirement)
        );
        assert_eq!(hostile_output, before);
    }

    #[test]
    fn hostile_profiles_refuse_noncanonical_or_aliased_requirements() {
        for requirements in [
            Vec::new(),
            vec![(20, 0, 0)],
            vec![(20, 4, 0), (23, 4, 1)],
            vec![(30, 4, 0), (20, 4, 1)],
            vec![(20, 4, 0), (30, 4, 0)],
        ] {
            assert!(RequestProfileV2::decode(&profile_bytes(&requirements)).is_err());
        }
        let canonical = profile_bytes(&[(20, 4, 0)]);
        for mutate in [0_usize, 8, 10, 20] {
            let mut hostile = canonical.clone();
            *hostile.get_mut(mutate).expect("hostile coordinate") ^= 1;
            assert!(RequestProfileV2::decode(&hostile).is_err());
        }
        assert_eq!(
            RequestProfileV2::decode(canonical.get(..canonical.len() - 1).expect("short profile"),),
            Err(Error::InvalidLength)
        );
        assert_eq!(
            RequestProfileV2::decode_selected([1; 32], [2; 32], &canonical),
            Err(Error::ProgramIdentityMismatch)
        );
    }

    #[test]
    fn hostile_native_layout_message_and_registers_preserve_output() {
        let requirements = [(20, 3, 0), (30, 4, 1)];
        let profile_bytes = profile_bytes(&requirements);
        let profile = RequestProfileV2::decode(&profile_bytes).expect("profile");
        let mut current = [0_u8; 40];
        current
            .get_mut(20..23)
            .expect("message one")
            .copy_from_slice(b"one");
        current
            .get_mut(30..34)
            .expect("message two")
            .copy_from_slice(b"two!");
        let canonical = native_instruction(&requirements, &[[7; 32], [8; 32]], 4, 128);
        for case in 0..10 {
            let mut native = canonical.clone();
            let mut authenticated_message_data = current.as_slice();
            let mut message_instruction_index = 4;
            let mut message_offset_bias = 128;
            let mut input = [[0_u8; 32]; 2];
            match case {
                0 => *native.first_mut().expect("count") = 1,
                1 => *native.get_mut(1).expect("padding") = 1,
                2 => *native.get_mut(4).expect("descriptor") = 0,
                3 => native.push(0),
                4 => *native.get_mut(2 + 2).expect("descriptor index") = 0,
                5 => *native.get_mut(2 + 14 + 12).expect("descriptor index") = 0,
                6 => message_instruction_index = 5,
                7 => message_offset_bias = 127,
                8 => authenticated_message_data = &current[..32],
                _ => *input.get_mut(1).expect("identity register") = [4; 32],
            }
            let mut scratch = [[9_u8; 32]; 2];
            let mut output = [[6_u8; 32]; 2];
            let before = output;
            assert!(
                seed_authenticated_signers_atomic(
                    profile,
                    2,
                    NativeEd25519InstructionViewV1 {
                        ed25519_data: &native,
                        authenticated_message_data,
                        message_instruction_index,
                        message_offset_bias,
                    },
                    NativeSignatureRegistersV1 {
                        input_identities: &input,
                        scratch_identities: &mut scratch,
                        output_identities: &mut output,
                    },
                )
                .is_err()
            );
            assert_eq!(output, before);
        }
    }
}
