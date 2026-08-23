//! Versioned extension-envelope parsing and fail-closed runtime admission.

use crate::runtime_contract::{
    decode_structured_claim_payload_v1, StructuredClaimActionV1, StructuredClaimPayloadV1,
    CREATE_DESCRIPTOR_PAYLOAD_BYTES, STRUCTURED_CLAIM_FAMILY_TAG,
    STRUCTURED_CLAIM_FAMILY_VERSION,
};
use crate::{Error, Result};

/// All family-local actions allocated by the canonical runtime contract.
pub const RESERVED_STRUCTURED_CLAIM_ACTION_MASK: u16 =
    ((1_u16 << (StructuredClaimActionV1::LAST_TAG + 1)) - 1)
        & !((1_u16 << StructuredClaimActionV1::FIRST_TAG) - 1);

/// Runtime actions admitted by this adapter artifact.
///
/// The default is empty. The separately deployed wrapper feature admits only
/// create and the two canonical supply-neutral routes.
#[cfg(not(feature = "live-canonical-wrapper"))]
pub const ENABLED_STRUCTURED_CLAIM_ACTION_MASK: u16 = 0;
/// Exact create/canonical-wrap/canonical-unwind capability set of the
/// separately deployed wrapper artifact.
#[cfg(feature = "live-canonical-wrapper")]
pub const ENABLED_STRUCTURED_CLAIM_ACTION_MASK: u16 =
    (1_u16 << 1) | (1_u16 << 2) | (1_u16 << 4);

const _: () = assert!(StructuredClaimActionV1::LAST_TAG < 16);
const _: () = assert!(
    STRUCTURED_CLAIM_FAMILY_TAG == clutch_solana_layout::registry::STRUCTURED_CLAIM_FAMILY_TAG
);
const _: () = assert!(
    STRUCTURED_CLAIM_FAMILY_VERSION
        == clutch_solana_layout::registry::STRUCTURED_CLAIM_FAMILY_VERSION
);
#[cfg(not(feature = "live-canonical-wrapper"))]
const _: () = assert!(ENABLED_STRUCTURED_CLAIM_ACTION_MASK == 0);
#[cfg(feature = "live-canonical-wrapper")]
const _: () = assert!(ENABLED_STRUCTURED_CLAIM_ACTION_MASK == 0b1_0110);
const _: () =
    assert!(ENABLED_STRUCTURED_CLAIM_ACTION_MASK & !RESERVED_STRUCTURED_CLAIM_ACTION_MASK == 0);
const _: () = assert!(
    CREATE_DESCRIPTOR_PAYLOAD_BYTES
        <= clutch_solana_layout::registry::MAX_EXTENSION_PAYLOAD_BYTES
);

/// Borrowed exact structured-claim family envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredClaimEnvelopeV1<'a> {
    /// Canonical family-local action.
    pub action: StructuredClaimActionV1,
    /// Bytes owned by that action's canonical runtime-contract codec.
    pub payload: &'a [u8],
}

impl<'a> StructuredClaimEnvelopeV1<'a> {
    /// Parse only the family/version/action header.
    ///
    /// Runtime dispatch uses this before it reads payload or account data so a
    /// reserved-disabled action cannot gain parsing side effects.
    pub fn decode_header(input: &'a [u8]) -> Result<Self> {
        let family = *input.first().ok_or(Error::InvalidInstruction)?;
        let version = *input.get(1).ok_or(Error::InvalidInstruction)?;
        let action_tag = *input.get(2).ok_or(Error::InvalidInstruction)?;
        if family != STRUCTURED_CLAIM_FAMILY_TAG {
            return Err(Error::WrongFamily);
        }
        if version != STRUCTURED_CLAIM_FAMILY_VERSION {
            return Err(Error::WrongFamilyVersion);
        }
        let action = StructuredClaimActionV1::from_tag(action_tag)?;
        Ok(Self {
            action,
            payload: input.get(3..).ok_or(Error::InvalidInstruction)?,
        })
    }

    /// Decode the action-owned bytes through the canonical runtime contract.
    pub fn decode_payload(&self) -> Result<StructuredClaimPayloadV1> {
        decode_structured_claim_payload_v1(self.action.tag(), self.payload).map_err(Into::into)
    }
}

/// Decode a complete instruction through the canonical family-local codec.
///
/// This is a construction/client/parser API, not runtime capability.
pub fn decode_instruction_v1(input: &[u8]) -> Result<StructuredClaimPayloadV1> {
    StructuredClaimEnvelopeV1::decode_header(input)?.decode_payload()
}

/// Apply the current ELF's runtime capability gate.
///
/// Only the exact three-byte extension header is inspected. With the current
/// empty mask this always refuses an allocated structured-claim action before
/// payload or account data is read.
pub fn admit_runtime_envelope_v1(input: &[u8]) -> Result<StructuredClaimEnvelopeV1<'_>> {
    let envelope = StructuredClaimEnvelopeV1::decode_header(input)?;
    let bit = 1_u16
        .checked_shl(u32::from(envelope.action.tag()))
        .ok_or(Error::UnknownAction)?;
    if ENABLED_STRUCTURED_CLAIM_ACTION_MASK & bit == 0 {
        return Err(Error::CapabilityDisabled);
    }
    Ok(envelope)
}
