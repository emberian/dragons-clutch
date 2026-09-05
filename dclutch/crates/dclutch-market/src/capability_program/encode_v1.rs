//! Safe atomic construction of variable-width [`CapabilityProgramV1`](crate::capability_program::CapabilityProgramV1) records.
//!
//! The V1 descriptor embeds its TransitionVM V2 program. This module is the
//! sole writer of that layout: family artifact builders supply typed content
//! coordinates and already-encoded transition bytes, never header offsets.

use dclutch_core_contract::ContentId;
use dclutch_vm::v2::ProgramV2;

use crate::capability_program::{
    CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, CAPABILITY_PROGRAM_BODY_RESERVED_OFFSET,
    CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET, CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET,
    CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET, CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET,
    CAPABILITY_PROGRAM_HEADER_BYTES_V1, CAPABILITY_PROGRAM_KIND_OFFSET,
    CAPABILITY_PROGRAM_MAGIC_OFFSET, CAPABILITY_PROGRAM_MAGIC_V1, CAPABILITY_PROGRAM_MAX_BYTES_V1,
    CAPABILITY_PROGRAM_PROFILE_OFFSET, CAPABILITY_PROGRAM_PROFILE_V2,
    CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET, CAPABILITY_PROGRAM_RESERVED_OFFSET,
    CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
    CAPABILITY_PROGRAM_SCHEMA_VERSION_OFFSET, CAPABILITY_PROGRAM_SCHEMA_VERSION_V1,
    CapabilityProgramV1, Error,
};

/// Typed semantic inputs to one V1 capability descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityProgramInputV1<'a> {
    /// Capability kind selected by the manifest entry.
    pub kind: ContentId,
    /// Finalized config-record schema.
    pub config_schema: ContentId,
    /// Family request schema interpreted by this descriptor.
    pub request_schema: ContentId,
    /// Mutable child-root tail schema.
    pub root_schema: ContentId,
    /// Finalized AccountProfileV1 content identity.
    pub account_profile: ContentId,
    /// Manifest-selected child derivation policy.
    pub derivation_policy: ContentId,
    /// Manifest-selected capacity-profile content identity.
    pub capacity_profile: ContentId,
    /// Finalized EffectProgramV2 content identity.
    pub effect_schema: ContentId,
    /// Exact mutable child-root tail width.
    pub root_state_bytes: u32,
    /// Canonical TransitionVM V2 bytes embedded in the descriptor.
    pub transition_program: &'a [u8],
}

/// Exact encoded width for one V1 descriptor.
pub fn capability_program_v1_bytes(transition_bytes: usize) -> Result<usize, Error> {
    let width = CAPABILITY_PROGRAM_HEADER_BYTES_V1
        .checked_add(transition_bytes)
        .ok_or(Error::InvalidLength)?;
    if width > CAPABILITY_PROGRAM_MAX_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    Ok(width)
}

/// Encode one exact V1 descriptor atomically.
///
/// Both buffers must have the exact transition-derived width. The output is
/// copied only after the transition and complete descriptor hostile-decode.
pub fn encode_capability_program_v1_atomic(
    input: CapabilityProgramInputV1<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    ProgramV2::decode(input.transition_program).map_err(|_| Error::InvalidTransitionProgram)?;
    let expected = capability_program_v1_bytes(input.transition_program.len())?;
    if scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    write(
        scratch,
        CAPABILITY_PROGRAM_MAGIC_OFFSET,
        &CAPABILITY_PROGRAM_MAGIC_V1,
    )?;
    write_u16(
        scratch,
        CAPABILITY_PROGRAM_SCHEMA_VERSION_OFFSET,
        CAPABILITY_PROGRAM_SCHEMA_VERSION_V1,
    )?;
    write_u16(
        scratch,
        CAPABILITY_PROGRAM_PROFILE_OFFSET,
        CAPABILITY_PROGRAM_PROFILE_V2,
    )?;
    for (offset, value) in [
        (CAPABILITY_PROGRAM_KIND_OFFSET, input.kind),
        (CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, input.config_schema),
        (
            CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
            input.request_schema,
        ),
        (CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, input.root_schema),
        (
            CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET,
            input.account_profile,
        ),
        (
            CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
            input.derivation_policy,
        ),
        (
            CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
            input.capacity_profile,
        ),
        (CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, input.effect_schema),
    ] {
        write(scratch, offset, &value.to_bytes())?;
    }
    write(
        scratch,
        CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
        &input.root_state_bytes.to_le_bytes(),
    )?;
    write(
        scratch,
        CAPABILITY_PROGRAM_HEADER_BYTES_V1,
        input.transition_program,
    )?;
    debug_assert!(
        scratch
            .get(CAPABILITY_PROGRAM_RESERVED_OFFSET..CAPABILITY_PROGRAM_RESERVED_OFFSET + 4)
            .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0))
    );
    debug_assert!(
        scratch
            .get(
                CAPABILITY_PROGRAM_BODY_RESERVED_OFFSET
                    ..CAPABILITY_PROGRAM_BODY_RESERVED_OFFSET + 4
            )
            .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0))
    );
    CapabilityProgramV1::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn write(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), Error> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(bytes.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

fn write_u16(output: &mut [u8], offset: usize, value: u16) -> Result<(), Error> {
    write(output, offset, &value.to_le_bytes())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{vec, vec::Vec};

    use dclutch_vm::v2::encode::{
        RegisterGeometryV2, TransitionInstructionV2, encode_transition_program_v2_atomic,
        transition_program_v2_bytes,
    };

    use super::*;

    fn content(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("content")
    }

    fn transition() -> Vec<u8> {
        let width = transition_program_v2_bytes(1).expect("width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_transition_program_v2_atomic(
            RegisterGeometryV2 {
                scalars: 1,
                identities: 1,
            },
            &[TransitionInstructionV2::load_const(0, 1)],
            &mut scratch,
            &mut output,
        )
        .expect("transition");
        output
    }

    fn input(transition_program: &[u8]) -> CapabilityProgramInputV1<'_> {
        CapabilityProgramInputV1 {
            kind: content(1),
            config_schema: content(2),
            request_schema: content(3),
            root_schema: content(4),
            account_profile: content(5),
            derivation_policy: content(6),
            capacity_profile: content(7),
            effect_schema: content(8),
            root_state_bytes: 16,
            transition_program,
        }
    }

    #[test]
    fn typed_encoder_round_trips_every_coordinate() {
        let transition = transition();
        let width = capability_program_v1_bytes(transition.len()).expect("width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_capability_program_v1_atomic(input(&transition), &mut scratch, &mut output)
            .expect("descriptor");
        let decoded = CapabilityProgramV1::decode(&output).expect("decode");
        assert_eq!(decoded.kind(), content(1));
        assert_eq!(decoded.config_schema(), content(2));
        assert_eq!(decoded.request_schema(), content(3));
        assert_eq!(decoded.root_schema(), content(4));
        assert_eq!(decoded.account_profile(), content(5));
        assert_eq!(decoded.derivation_policy(), content(6));
        assert_eq!(decoded.capacity_profile(), content(7));
        assert_eq!(decoded.effect_schema(), content(8));
        assert_eq!(decoded.root_state_bytes(), 16);
        assert_eq!(decoded.transition_program().bytes(), transition);
    }

    #[test]
    fn malformed_transition_and_width_refuse_without_output_mutation() {
        let transition = transition();
        let width = capability_program_v1_bytes(transition.len()).expect("width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0x55_u8; width];
        let unchanged = output.clone();
        assert_eq!(
            encode_capability_program_v1_atomic(
                input(&transition),
                scratch.get_mut(..width - 1).expect("short scratch"),
                &mut output
            ),
            Err(Error::InvalidLength)
        );
        assert_eq!(output, unchanged);

        let malformed = [0_u8; 16];
        assert_eq!(
            encode_capability_program_v1_atomic(input(&malformed), &mut scratch, &mut output),
            Err(Error::InvalidTransitionProgram)
        );
        assert_eq!(output, unchanged);
    }
}
