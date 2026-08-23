//! Structured-claim purpose extension for the canonical Replay V3 envelope.

use crate::{put, take, Error, Result, StructuredClaimActionV1};

/// Exact Replay V3 extension schema owned by StructuredClaim `75/v1`.
pub const STRUCTURED_CLAIM_REPLAY_EXTENSION_SCHEMA_V1: u32 = u32::from_le_bytes(*b"SCV1");
/// Exact fixed width of [`StructuredClaimReplayExtensionV1`].
pub const STRUCTURED_CLAIM_REPLAY_EXTENSION_BYTES_V1: usize = 208;
/// Exact structured-custody delta preimage width, excluding its hash domain.
pub const STRUCTURED_CLAIM_REPLAY_DELTA_BYTES_V1: usize = 241;
/// Domain for the exact transfer delta accepted by both Position Replay envelopes.
pub const STRUCTURED_CLAIM_REPLAY_DELTA_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/replay-delta/v1\0";

const EXTENSION_MAGIC: [u8; 8] = *b"DCSCRPV1";
const EXTENSION_VERSION: u16 = 1;

const _: () = assert!(STRUCTURED_CLAIM_REPLAY_EXTENSION_BYTES_V1 == 16 + (6 * 32));
const _: () = assert!(STRUCTURED_CLAIM_REPLAY_DELTA_BYTES_V1 == 1 + (2 * 8) + (7 * 32));

/// Lifecycle of the structured-claim purpose extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StructuredClaimReplayExtensionStateV1 {
    /// No structured-custody transition has yet been accepted.
    Founding = 0,
    /// At least one exact transition has advanced the common Replay envelope.
    Advanced = 1,
}

impl StructuredClaimReplayExtensionStateV1 {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Founding),
            1 => Ok(Self::Advanced),
            _ => Err(Error::InvalidReplayExtension),
        }
    }

    const fn encode(self) -> u8 {
        match self {
            Self::Founding => 0,
            Self::Advanced => 1,
        }
    }
}

/// Canonical fixed-width extension for one structured-claim vault Replay V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredClaimReplayExtensionV1 {
    /// Immutable canonical `0x88/1` descriptor account.
    pub descriptor_account: [u8; 32],
    /// Immutable deployment-bound wrapper-product identity.
    pub wrapper_product_id: [u8; 32],
    /// Immutable wrapper vault-owner PDA.
    pub vault_authority: [u8; 32],
    /// Semantic identity of the Position V3 body paired with this Replay state.
    pub current_position_semantic_id: [u8; 32],
    /// Last complete authenticated custody-call digest, or zero at founding.
    pub last_transition_id: [u8; 32],
    /// Last exact action/delta digest, or zero at founding.
    pub last_delta_id: [u8; 32],
    /// Founding or advanced shape.
    pub state: StructuredClaimReplayExtensionStateV1,
    /// Last accepted StructuredClaim family-local action, or zero at founding.
    pub last_action: u8,
}

impl StructuredClaimReplayExtensionV1 {
    /// Construct the unique founding extension for an authenticated vault Position.
    pub fn founding(
        descriptor_account: [u8; 32],
        wrapper_product_id: [u8; 32],
        vault_authority: [u8; 32],
        current_position_semantic_id: [u8; 32],
    ) -> Result<Self> {
        let value = Self {
            descriptor_account,
            wrapper_product_id,
            vault_authority,
            current_position_semantic_id,
            last_transition_id: [0; 32],
            last_delta_id: [0; 32],
            state: StructuredClaimReplayExtensionStateV1::Founding,
            last_action: 0,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate immutable identities and the exact founding/advanced partition.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.descriptor_account,
            self.wrapper_product_id,
            self.vault_authority,
            self.current_position_semantic_id,
        ] {
            if identity == [0; 32] {
                return Err(Error::InvalidReplayExtension);
            }
        }
        match self.state {
            StructuredClaimReplayExtensionStateV1::Founding => {
                if self.last_action != 0
                    || self.last_transition_id != [0; 32]
                    || self.last_delta_id != [0; 32]
                {
                    return Err(Error::InvalidReplayExtension);
                }
            }
            StructuredClaimReplayExtensionStateV1::Advanced => {
                if !is_custody_action(self.last_action)
                    || self.last_transition_id == [0; 32]
                    || self.last_delta_id == [0; 32]
                {
                    return Err(Error::InvalidReplayExtension);
                }
            }
        }
        Ok(())
    }

    /// Encode the sole canonical 208-byte structured-purpose extension.
    pub fn encode(&self) -> Result<[u8; STRUCTURED_CLAIM_REPLAY_EXTENSION_BYTES_V1]> {
        self.validate()?;
        let mut output = [0_u8; STRUCTURED_CLAIM_REPLAY_EXTENSION_BYTES_V1];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &EXTENSION_MAGIC)?;
        put(&mut output, &mut cursor, &EXTENSION_VERSION.to_le_bytes())?;
        put(
            &mut output,
            &mut cursor,
            &[self.state.encode(), self.last_action],
        )?;
        put(&mut output, &mut cursor, &[0; 4])?;
        for identity in [
            self.descriptor_account,
            self.wrapper_product_id,
            self.vault_authority,
            self.current_position_semantic_id,
            self.last_transition_id,
            self.last_delta_id,
        ] {
            put(&mut output, &mut cursor, &identity)?;
        }
        if cursor != output.len() {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    /// Decode and validate an exact hostile structured-purpose extension.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != STRUCTURED_CLAIM_REPLAY_EXTENSION_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        let mut cursor = 0_usize;
        if take(input, &mut cursor, 8)? != EXTENSION_MAGIC
            || read_u16(input, &mut cursor)? != EXTENSION_VERSION
        {
            return Err(Error::InvalidReplayExtension);
        }
        let state = StructuredClaimReplayExtensionStateV1::decode(take(input, &mut cursor, 1)?[0])?;
        let last_action = take(input, &mut cursor, 1)?[0];
        if take(input, &mut cursor, 4)? != [0; 4] {
            return Err(Error::NonCanonicalPadding);
        }
        let value = Self {
            descriptor_account: read_key(input, &mut cursor)?,
            wrapper_product_id: read_key(input, &mut cursor)?,
            vault_authority: read_key(input, &mut cursor)?,
            current_position_semantic_id: read_key(input, &mut cursor)?,
            last_transition_id: read_key(input, &mut cursor)?,
            last_delta_id: read_key(input, &mut cursor)?,
            state,
            last_action,
        };
        if cursor != input.len() {
            return Err(Error::InvalidLength);
        }
        value.validate()?;
        Ok(value)
    }

    /// Advance the purpose extension after one completely staged custody call.
    pub fn advanced(self, transition: StructuredClaimReplayTransitionV1) -> Result<Self> {
        self.validate()?;
        transition.validate()?;
        if self.descriptor_account != transition.descriptor_account
            || self.wrapper_product_id != transition.wrapper_product_id
            || self.vault_authority != transition.vault_authority
            || self.current_position_semantic_id != transition.position_pre_semantic_id
        {
            return Err(Error::InvalidReplayExtension);
        }
        let value = Self {
            current_position_semantic_id: transition.position_post_semantic_id,
            last_transition_id: transition.transition_id,
            last_delta_id: transition.delta_id,
            state: StructuredClaimReplayExtensionStateV1::Advanced,
            last_action: transition.action.tag(),
            ..self
        };
        value.validate()?;
        Ok(value)
    }
}

/// Exact structured-purpose update inputs after both Position poststates exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredClaimReplayTransitionV1 {
    /// Canonical descriptor account retained by the extension.
    pub descriptor_account: [u8; 32],
    /// Deployment-bound wrapper product retained by the extension.
    pub wrapper_product_id: [u8; 32],
    /// Vault authority retained by the extension.
    pub vault_authority: [u8; 32],
    /// Exact local wrap or unwind action.
    pub action: StructuredClaimActionV1,
    /// Complete authenticated custody-call digest.
    pub transition_id: [u8; 32],
    /// Digest of the exact action, ordinals, accounts, and Position prestates/poststates.
    pub delta_id: [u8; 32],
    /// Vault Position semantic identity before mutation.
    pub position_pre_semantic_id: [u8; 32],
    /// Vault Position semantic identity after mutation.
    pub position_post_semantic_id: [u8; 32],
}

impl StructuredClaimReplayTransitionV1 {
    fn validate(self) -> Result<()> {
        if !is_custody_action(self.action.tag()) {
            return Err(Error::InvalidReplayExtension);
        }
        for identity in [
            self.descriptor_account,
            self.wrapper_product_id,
            self.vault_authority,
            self.transition_id,
            self.delta_id,
            self.position_pre_semantic_id,
            self.position_post_semantic_id,
        ] {
            if identity == [0; 32] {
                return Err(Error::InvalidReplayExtension);
            }
        }
        if self.position_pre_semantic_id == self.position_post_semantic_id {
            return Err(Error::InvalidReplayExtension);
        }
        Ok(())
    }
}

/// Exact action/delta body shared by both purpose-owned Replay advances.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredClaimReplayDeltaV1 {
    /// Exact local wrap or unwind action.
    pub action: StructuredClaimActionV1,
    /// Source Replay V3 ordinal consumed by this mutation.
    pub source_sequence: u64,
    /// Destination Replay V3 ordinal consumed by this mutation.
    pub destination_sequence: u64,
    /// Complete authenticated custody-call digest.
    pub transition_id: [u8; 32],
    /// Exact source Position V3 account.
    pub source_position_account: [u8; 32],
    /// Exact source Position semantic identity before mutation.
    pub source_position_pre_semantic_id: [u8; 32],
    /// Exact source Position semantic identity after mutation.
    pub source_position_post_semantic_id: [u8; 32],
    /// Exact destination Position V3 account.
    pub destination_position_account: [u8; 32],
    /// Exact destination Position semantic identity before mutation.
    pub destination_position_pre_semantic_id: [u8; 32],
    /// Exact destination Position semantic identity after mutation.
    pub destination_position_post_semantic_id: [u8; 32],
}

impl StructuredClaimReplayDeltaV1 {
    /// Encode the exact digest body; adapters hash `domain || body`.
    pub fn encode(&self) -> Result<[u8; STRUCTURED_CLAIM_REPLAY_DELTA_BYTES_V1]> {
        if !is_custody_action(self.action.tag())
            || self.source_sequence == u64::MAX
            || self.destination_sequence == u64::MAX
            || self.source_position_account == self.destination_position_account
        {
            return Err(Error::InvalidReplayExtension);
        }
        for identity in [
            self.transition_id,
            self.source_position_account,
            self.source_position_pre_semantic_id,
            self.source_position_post_semantic_id,
            self.destination_position_account,
            self.destination_position_pre_semantic_id,
            self.destination_position_post_semantic_id,
        ] {
            if identity == [0; 32] {
                return Err(Error::InvalidReplayExtension);
            }
        }
        if self.source_position_pre_semantic_id == self.source_position_post_semantic_id
            || self.destination_position_pre_semantic_id
                == self.destination_position_post_semantic_id
        {
            return Err(Error::InvalidReplayExtension);
        }
        let mut output = [0_u8; STRUCTURED_CLAIM_REPLAY_DELTA_BYTES_V1];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &[self.action.tag()])?;
        put(
            &mut output,
            &mut cursor,
            &self.source_sequence.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.destination_sequence.to_le_bytes(),
        )?;
        for identity in [
            self.transition_id,
            self.source_position_account,
            self.source_position_pre_semantic_id,
            self.source_position_post_semantic_id,
            self.destination_position_account,
            self.destination_position_pre_semantic_id,
            self.destination_position_post_semantic_id,
        ] {
            put(&mut output, &mut cursor, &identity)?;
        }
        if cursor != output.len() {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }
}

const fn is_custody_action(action: u8) -> bool {
    action == StructuredClaimActionV1::WrapCanonical.tag()
        || action == StructuredClaimActionV1::UnwrapCanonical.tag()
}

fn read_key(input: &[u8], cursor: &mut usize) -> Result<[u8; 32]> {
    let mut value = [0_u8; 32];
    value.copy_from_slice(take(input, cursor, 32)?);
    Ok(value)
}

fn read_u16(input: &[u8], cursor: &mut usize) -> Result<u16> {
    let mut value = [0_u8; 2];
    value.copy_from_slice(take(input, cursor, 2)?);
    Ok(u16::from_le_bytes(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn founding_and_advanced_shapes_are_disjoint() {
        let founding =
            StructuredClaimReplayExtensionV1::founding([1; 32], [2; 32], [3; 32], [4; 32]).unwrap();
        assert_eq!(
            StructuredClaimReplayExtensionV1::decode(&founding.encode().unwrap()),
            Ok(founding)
        );
        let advanced = founding
            .advanced(StructuredClaimReplayTransitionV1 {
                descriptor_account: [1; 32],
                wrapper_product_id: [2; 32],
                vault_authority: [3; 32],
                action: StructuredClaimActionV1::WrapCanonical,
                transition_id: [5; 32],
                delta_id: [6; 32],
                position_pre_semantic_id: [4; 32],
                position_post_semantic_id: [7; 32],
            })
            .unwrap();
        assert_eq!(advanced.current_position_semantic_id, [7; 32]);
        assert_eq!(advanced.last_action, 2);
    }
}
