//! Runtime-width General successor request with canonical-PDA bump witnesses.
//!
//! The two bump bytes are untrusted witnesses. Generic Trading recomputes the
//! canonical PDA and requires exact bump/key equality before any lifecycle
//! create, assign, or signer derivation. Keeping them in the exact request
//! permits one reusable lifecycle artifact across every Market.

use super::{Action, Error, Result};

/// Exact successor controller request width.
pub const CONTROLLER_REQUEST_BYTES_V2: usize = 64;
/// Action selector offset consumed by CapabilityProgramSet.
pub const CONTROLLER_REQUEST_ACTION_OFFSET_V2: usize = 10;
/// Primary action-state PDA bump offset.
pub const CONTROLLER_REQUEST_STATE_BUMP_OFFSET_V2: usize = 61;
/// Optional terminal-record PDA bump offset.
pub const CONTROLLER_REQUEST_TERMINAL_BUMP_OFFSET_V2: usize = 62;

const MAGIC: [u8; 8] = *b"DCGREQ02";
const VERSION: u16 = 2;

/// Exact runtime-width General successor request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerRequestV2 {
    /// Requested data-defined action.
    pub action: Action,
    /// Exact cursor revision observed by the caller.
    pub expected_revision: u64,
    /// Candidate identity, absent only for selection Freeze.
    pub candidate_id: Option<[u8; 32]>,
    /// Action-specific page or manifest-chunk coordinate.
    pub page_index: u32,
    /// Action-specific row coordinate inside the selected physical chunk.
    pub execution_index: u8,
    /// Untrusted canonical bump witness for the primary action state.
    pub state_bump: u8,
    /// Untrusted canonical bump witness for Close's terminal record.
    pub terminal_record_bump: u8,
}

impl ControllerRequestV2 {
    /// Hostile-decode one exact successor request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != CONTROLLER_REQUEST_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if input.get(..8) != Some(MAGIC.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_u16(input, 8)? != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        if !zero_range(input, 11, 5)? || byte(input, 63)? != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        let raw_candidate = read_array32(input, 24)?;
        let value = Self {
            action: Action::decode(byte(input, CONTROLLER_REQUEST_ACTION_OFFSET_V2)?)?,
            expected_revision: read_u64(input, 16)?,
            candidate_id: if raw_candidate == [0; 32] {
                None
            } else {
                Some(raw_candidate)
            },
            page_index: read_u32(input, 56)?,
            execution_index: byte(input, 60)?,
            state_bump: byte(input, CONTROLLER_REQUEST_STATE_BUMP_OFFSET_V2)?,
            terminal_record_bump: byte(input, CONTROLLER_REQUEST_TERMINAL_BUMP_OFFSET_V2)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical successor request.
    pub fn to_bytes(self) -> Result<[u8; CONTROLLER_REQUEST_BYTES_V2]> {
        self.validate()?;
        let mut output = [0_u8; CONTROLLER_REQUEST_BYTES_V2];
        put(&mut output, 0, &MAGIC)?;
        put(&mut output, 8, &VERSION.to_le_bytes())?;
        put_byte(
            &mut output,
            CONTROLLER_REQUEST_ACTION_OFFSET_V2,
            self.action as u8,
        )?;
        put(&mut output, 16, &self.expected_revision.to_le_bytes())?;
        if let Some(candidate) = self.candidate_id {
            put(&mut output, 24, &candidate)?;
        }
        put(&mut output, 56, &self.page_index.to_le_bytes())?;
        put_byte(&mut output, 60, self.execution_index)?;
        put_byte(
            &mut output,
            CONTROLLER_REQUEST_STATE_BUMP_OFFSET_V2,
            self.state_bump,
        )?;
        put_byte(
            &mut output,
            CONTROLLER_REQUEST_TERMINAL_BUMP_OFFSET_V2,
            self.terminal_record_bump,
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if self.candidate_id.as_ref().is_some_and(is_zero) {
            return Err(Error::ZeroCoordinate);
        }
        let nonterminal_bump = self.action != Action::Close && self.terminal_record_bump != 0;
        if nonterminal_bump {
            return Err(Error::NonCanonicalPadding);
        }
        match self.action {
            Action::Freeze
                if self.candidate_id.is_none()
                    && self.page_index == 0
                    && self.execution_index == 0 =>
            {
                Ok(())
            }
            Action::Consider if self.candidate_id.is_some() && self.execution_index == 0 => Ok(()),
            Action::Collect | Action::Distribute if self.candidate_id.is_some() => Ok(()),
            Action::InitializeSettlement | Action::Materialize | Action::Close
                if self.candidate_id.is_some()
                    && self.page_index == 0
                    && self.execution_index == 0 =>
            {
                Ok(())
            }
            _ => Err(Error::InvalidCursor),
        }
    }
}

fn byte(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array32(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    read_array(input, offset)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    input
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn zero_range(input: &[u8], offset: usize, width: usize) -> Result<bool> {
    Ok(input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|value| *value == 0))
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(action: Action) -> ControllerRequestV2 {
        ControllerRequestV2 {
            action,
            expected_revision: 7,
            candidate_id: (action != Action::Freeze).then_some([1; 32]),
            page_index: if matches!(
                action,
                Action::Consider | Action::Collect | Action::Distribute
            ) {
                258
            } else {
                0
            },
            execution_index: if matches!(action, Action::Collect | Action::Distribute) {
                17
            } else {
                0
            },
            state_bump: 0,
            terminal_record_bump: if action == Action::Close { 255 } else { 0 },
        }
    }

    #[test]
    fn all_actions_roundtrip_exact_bump_witnesses() {
        for action in [
            Action::Consider,
            Action::Freeze,
            Action::InitializeSettlement,
            Action::Collect,
            Action::Materialize,
            Action::Distribute,
            Action::Close,
        ] {
            let value = request(action);
            let bytes = value.to_bytes().expect("encode");
            assert_eq!(ControllerRequestV2::decode(&bytes), Ok(value));
            assert_eq!(bytes.len(), CONTROLLER_REQUEST_BYTES_V2);
        }
    }

    #[test]
    fn substituted_version_reserved_and_terminal_bump_refuse() {
        let canonical = request(Action::Collect)
            .to_bytes()
            .expect("canonical request");
        for offset in [8_usize, 11, 63] {
            let mut hostile = canonical;
            *hostile.get_mut(offset).expect("hostile offset") ^= 1;
            assert!(ControllerRequestV2::decode(&hostile).is_err());
        }
        let mut terminal = canonical;
        terminal[CONTROLLER_REQUEST_TERMINAL_BUMP_OFFSET_V2] = 1;
        assert_eq!(
            ControllerRequestV2::decode(&terminal),
            Err(Error::NonCanonicalPadding)
        );
    }
}
