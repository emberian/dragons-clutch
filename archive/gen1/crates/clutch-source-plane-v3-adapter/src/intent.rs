use clutch_source_plane_v3::ContentId;
use sha2::{Digest, Sha256};

use crate::transition::{TransitionActionV3, TransitionPlanV3};
use crate::{Error, Result};

const INTENT_MAGIC: [u8; 8] = *b"DCSP3INT";
const INTENT_VERSION: u16 = 1;
const INTENT_DOMAIN: &[u8] = b"dragons-clutch/source-plane-v3/intent/v1";

/// Exact canonical intent-preimage width.
pub const INTENT_PREIMAGE_BYTES: usize = 160;

/// Canonical request preimage committing a complete pure transition plan.
///
/// The preimage carries no free nonce: replay convergence comes from the
/// expected before-state in the transition plan. Series ordinals are monotone
/// state facts, and deterministic Instances do not acquire caller entropy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentPreimageV3 {
    action: TransitionActionV3,
    adapter_program_id: ContentId,
    transition_id: ContentId,
    submitter: ContentId,
    current_bucket: u64,
    requested_ordinal: u32,
    valid_before_slot: u64,
}

impl IntentPreimageV3 {
    /// Bind one exact transition plan. Series clock and ordinal context are
    /// derived from the plan rather than accepted as parallel caller truths.
    pub fn new(
        plan: TransitionPlanV3,
        adapter_program_id: ContentId,
        submitter: ContentId,
        valid_before_slot: u64,
    ) -> Result<Self> {
        plan.validate()?;
        let value = Self {
            action: plan.action(),
            adapter_program_id,
            transition_id: plan.id()?,
            submitter,
            current_bucket: plan.current_bucket(),
            requested_ordinal: plan.requested_ordinal(),
            valid_before_slot,
        };
        value.validate()?;
        value.validate_for_program(adapter_program_id, plan)?;
        Ok(value)
    }

    /// Exact projected action.
    pub const fn action(self) -> TransitionActionV3 {
        self.action
    }

    /// Exact deployed adapter program, preventing cross-deployment replay.
    pub const fn adapter_program_id(self) -> ContentId {
        self.adapter_program_id
    }

    /// Complete transition plan digest.
    pub const fn transition_id(self) -> ContentId {
        self.transition_id
    }

    /// Permissionless transaction submitter bound by the request.
    ///
    /// This is deliberately not a rent-principal owner or Series funder.
    pub const fn submitter(self) -> ContentId {
        self.submitter
    }

    /// Adapter-authenticated clock bucket for Series eligibility.
    pub const fn current_bucket(self) -> u64 {
        self.current_bucket
    }

    /// Expected durable Series ordinal, zero outside Series cursor actions.
    pub const fn requested_ordinal(self) -> u32 {
        self.requested_ordinal
    }

    /// Exclusive Clock slot expiry. The live adapter must enforce it.
    pub const fn valid_before_slot(self) -> u64 {
        self.valid_before_slot
    }

    /// Validate live identities, expiry, and action-specific canonical fields.
    pub fn validate(&self) -> Result<()> {
        if self.adapter_program_id.is_zero()
            || self.transition_id.is_zero()
            || self.submitter.is_zero()
        {
            return Err(Error::ZeroIdentity);
        }
        if self.valid_before_slot == 0 {
            return Err(Error::InvalidParameter);
        }
        let is_series_cursor = matches!(
            self.action,
            TransitionActionV3::CreateSeriesInstance
                | TransitionActionV3::LapseSeriesOrdinal
                | TransitionActionV3::AdvanceExistingInstance
        );
        if !is_series_cursor && (self.current_bucket != 0 || self.requested_ordinal != 0) {
            return Err(Error::NonCanonicalPadding);
        }
        Ok(())
    }

    /// Rejoin hostile-decoded intent bytes to the recomputed pure plan. A live
    /// adapter must call this rather than checking only the plan digest or only
    /// the action discriminator.
    pub fn validate_for_program(
        &self,
        executing_program_id: ContentId,
        plan: TransitionPlanV3,
    ) -> Result<()> {
        self.validate()?;
        plan.validate()?;
        if executing_program_id.is_zero() {
            return Err(Error::ZeroIdentity);
        }
        if self.adapter_program_id != executing_program_id
            || self.action != plan.action()
            || self.transition_id != plan.id()?
            || self.current_bucket != plan.current_bucket()
            || self.requested_ordinal != plan.requested_ordinal()
        {
            return Err(Error::MismatchedState);
        }
        Ok(())
    }

    /// Encode exact fixed bytes with a zero reserved tail.
    pub fn encode(self) -> Result<[u8; INTENT_PREIMAGE_BYTES]> {
        self.validate()?;
        let mut output = [0; INTENT_PREIMAGE_BYTES];
        output[..8].copy_from_slice(&INTENT_MAGIC);
        output[8..10].copy_from_slice(&INTENT_VERSION.to_le_bytes());
        output[10..12].copy_from_slice(&(self.action as u16).to_le_bytes());
        output[12..44].copy_from_slice(&self.adapter_program_id.bytes());
        output[44..76].copy_from_slice(&self.transition_id.bytes());
        output[76..108].copy_from_slice(&self.submitter.bytes());
        output[108..116].copy_from_slice(&self.current_bucket.to_le_bytes());
        output[116..120].copy_from_slice(&self.requested_ordinal.to_le_bytes());
        output[120..128].copy_from_slice(&self.valid_before_slot.to_le_bytes());
        Ok(output)
    }

    /// Hostile-decode exact bytes. Unknown future versions and nonzero tail
    /// bytes refuse rather than negotiate.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != INTENT_PREIMAGE_BYTES {
            return Err(Error::WrongLength);
        }
        if input[..8] != INTENT_MAGIC {
            return Err(Error::WrongMagic);
        }
        if le_u16(input, 8) != INTENT_VERSION {
            return Err(Error::BadVersion);
        }
        if input[128..].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalPadding);
        }
        let value = Self {
            action: TransitionActionV3::decode(le_u16(input, 10))?,
            adapter_program_id: id_at(input, 12),
            transition_id: id_at(input, 44),
            submitter: id_at(input, 76),
            current_bucket: le_u64(input, 108),
            requested_ordinal: le_u32(input, 116),
            valid_before_slot: le_u64(input, 120),
        };
        value.validate()?;
        Ok(value)
    }

    /// Content identity of exact request bytes.
    pub fn id(self) -> Result<ContentId> {
        let bytes = self.encode()?;
        let mut hasher = Sha256::new();
        hasher.update(INTENT_DOMAIN);
        hasher.update(bytes);
        Ok(ContentId::from_bytes(hasher.finalize().into()))
    }
}

fn id_at(input: &[u8], offset: usize) -> ContentId {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(&input[offset..offset + 32]);
    ContentId::from_bytes(bytes)
}

fn le_u16(input: &[u8], offset: usize) -> u16 {
    let mut bytes = [0; 2];
    bytes.copy_from_slice(&input[offset..offset + 2]);
    u16::from_le_bytes(bytes)
}

fn le_u32(input: &[u8], offset: usize) -> u32 {
    let mut bytes = [0; 4];
    bytes.copy_from_slice(&input[offset..offset + 4]);
    u32::from_le_bytes(bytes)
}

fn le_u64(input: &[u8], offset: usize) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&input[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}
