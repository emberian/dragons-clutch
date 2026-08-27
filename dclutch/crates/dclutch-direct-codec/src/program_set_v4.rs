//! Canonical multi-action ProgramSetV2 construction for Direct V4 bundles.
//!
//! A set entry can be obtained only by successfully validating one complete
//! action bundle. This prevents release tooling from pairing a selector with
//! an arbitrary CapabilityProgramV4 digest while preserving the generic
//! family-neutral ProgramSetV2 wire and Hot selection path.

use dclutch_capability_program_contract::{
    set_v2::{
        CAPABILITY_PROGRAM_SET_MAX_ENTRIES_V2, CapabilityDescriptorReferenceV2,
        CapabilityProgramSetEntryV2, CapabilityProgramSetV2, SelectorWidthV2,
        encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
};
use dclutch_core_contract::ContentId;
use sha2::{Digest, Sha256};

use crate::{
    execution_v3::{DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3, DirectExecutionActionV3},
    ordinary_bundle_v4::{
        DirectInlineOrdinaryHotBundleV4, validate_direct_inline_ordinary_hot_bundle_v4,
    },
    registered_bundle_v4::{
        DirectRegisterBuyHotBundleV4, validate_direct_register_buy_hot_bundle_v4,
    },
};

/// One action/descriptor coordinate proven by its complete Direct bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedDirectCapabilityV4 {
    action: DirectExecutionActionV3,
    descriptor: [u8; 32],
}

impl ValidatedDirectCapabilityV4 {
    /// Selected Direct request action.
    pub const fn action(self) -> DirectExecutionActionV3 {
        self.action
    }

    /// SHA-256 identity of the exact validated CapabilityProgramV4 bytes.
    pub const fn descriptor(self) -> [u8; 32] {
        self.descriptor
    }
}

/// Stable Direct ProgramSet construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectProgramSetErrorV4 {
    /// A complete action bundle did not authenticate under its semantic owner.
    Bundle,
    /// Set or scratch width was not exact for the supplied entry count.
    InvalidLength,
    /// The entry set was empty or exceeded the generic fixed capacity.
    InvalidEntryCount,
    /// Direct actions were not strictly ascending and unique.
    NonCanonicalActionOrder,
    /// A descriptor schema or content identity was zero.
    InvalidIdentity,
    /// The generic ProgramSet encoder or hostile decoder refused.
    ProgramSet,
}

/// Validate one complete InlineOrdinary bundle before admitting its set entry.
pub fn validate_direct_inline_ordinary_capability_v4(
    bundle: &DirectInlineOrdinaryHotBundleV4,
    capacity_profile: [u8; 32],
) -> Result<ValidatedDirectCapabilityV4, DirectProgramSetErrorV4> {
    validate_direct_inline_ordinary_hot_bundle_v4(bundle, capacity_profile)
        .map_err(|_| DirectProgramSetErrorV4::Bundle)?;
    Ok(validated(
        DirectExecutionActionV3::InlineOrdinary,
        &bundle.descriptor,
    ))
}

/// Validate one complete RegisterBuy bundle before admitting its set entry.
pub fn validate_direct_register_buy_capability_v4(
    bundle: &DirectRegisterBuyHotBundleV4,
    capacity_profile: [u8; 32],
) -> Result<ValidatedDirectCapabilityV4, DirectProgramSetErrorV4> {
    validate_direct_register_buy_hot_bundle_v4(bundle, capacity_profile)
        .map_err(|_| DirectProgramSetErrorV4::Bundle)?;
    Ok(validated(
        DirectExecutionActionV3::RegisterBuy,
        &bundle.descriptor,
    ))
}

/// Exact ProgramSetV2 width for a bounded nonempty Direct action set.
pub fn encoded_direct_program_set_bytes_v4(
    entry_count: usize,
) -> Result<usize, DirectProgramSetErrorV4> {
    encoded_program_set_bytes_v2(entry_count).map_err(|_| {
        if entry_count == 0 || entry_count > CAPABILITY_PROGRAM_SET_MAX_ENTRIES_V2 {
            DirectProgramSetErrorV4::InvalidEntryCount
        } else {
            DirectProgramSetErrorV4::InvalidLength
        }
    })
}

/// Encode one canonical Direct ProgramSetV2 without changing output on refusal.
///
/// Entries must be produced by the action-specific bundle validators above.
/// The selector is the exact little-endian `u32` at request offset 12.
pub fn encode_direct_program_set_v2_atomic(
    entries: &[ValidatedDirectCapabilityV4],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DirectProgramSetErrorV4> {
    let expected = encoded_direct_program_set_bytes_v4(entries.len())?;
    if scratch.len() != expected || output.len() != expected {
        return Err(DirectProgramSetErrorV4::InvalidLength);
    }
    let first = entries
        .first()
        .copied()
        .ok_or(DirectProgramSetErrorV4::InvalidEntryCount)?;
    let first_entry = program_set_entry(first)?;
    let mut encoded = [first_entry; CAPABILITY_PROGRAM_SET_MAX_ENTRIES_V2];
    let mut prior = None;
    for (index, entry) in entries.iter().copied().enumerate() {
        let selector = entry.action as u32;
        if prior.is_some_and(|value| value >= selector) {
            return Err(DirectProgramSetErrorV4::NonCanonicalActionOrder);
        }
        prior = Some(selector);
        *encoded
            .get_mut(index)
            .ok_or(DirectProgramSetErrorV4::InvalidEntryCount)? = program_set_entry(entry)?;
    }
    encode_program_set_v2(
        DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3,
        SelectorWidthV2::U32,
        encoded
            .get(..entries.len())
            .ok_or(DirectProgramSetErrorV4::InvalidEntryCount)?,
        scratch,
    )
    .map_err(|_| DirectProgramSetErrorV4::ProgramSet)?;
    let decoded =
        CapabilityProgramSetV2::decode(scratch).map_err(|_| DirectProgramSetErrorV4::ProgramSet)?;
    if decoded.selector_offset() != DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3
        || decoded.selector_width() != SelectorWidthV2::U32
        || usize::from(decoded.entry_count()) != entries.len()
    {
        return Err(DirectProgramSetErrorV4::ProgramSet);
    }
    output.copy_from_slice(scratch);
    Ok(())
}

fn validated(action: DirectExecutionActionV3, descriptor: &[u8]) -> ValidatedDirectCapabilityV4 {
    ValidatedDirectCapabilityV4 {
        action,
        descriptor: digest(descriptor),
    }
}

fn program_set_entry(
    entry: ValidatedDirectCapabilityV4,
) -> Result<CapabilityProgramSetEntryV2, DirectProgramSetErrorV4> {
    let schema = ContentId::new(CAPABILITY_PROGRAM_SCHEMA_ID_V4)
        .map_err(|_| DirectProgramSetErrorV4::InvalidIdentity)?;
    let descriptor =
        ContentId::new(entry.descriptor).map_err(|_| DirectProgramSetErrorV4::InvalidIdentity)?;
    Ok(CapabilityProgramSetEntryV2::new(
        entry.action as u32,
        CapabilityDescriptorReferenceV2::new(schema, descriptor),
    ))
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;

    fn entry(action: DirectExecutionActionV3, byte: u8) -> ValidatedDirectCapabilityV4 {
        ValidatedDirectCapabilityV4 {
            action,
            descriptor: [byte; 32],
        }
    }

    #[test]
    fn multi_action_set_selects_exact_v4_descriptor() {
        let entries = [
            entry(DirectExecutionActionV3::InlineOrdinary, 1),
            entry(DirectExecutionActionV3::RegisterSell, 2),
            entry(DirectExecutionActionV3::RegisterBuy, 3),
        ];
        let bytes = encoded_direct_program_set_bytes_v4(entries.len()).expect("width");
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0x55_u8; bytes];
        encode_direct_program_set_v2_atomic(&entries, &mut scratch, &mut output).expect("set");
        let set = CapabilityProgramSetV2::decode(&output).expect("decode");
        assert_eq!(set.selector_offset(), 12);
        assert_eq!(set.selector_width(), SelectorWidthV2::U32);
        for value in entries {
            let mut request = [0_u8; 16];
            request[12..16].copy_from_slice(&(value.action as u32).to_le_bytes());
            let selected = set.select_descriptor(&request).expect("selected");
            assert_eq!(
                selected.schema().to_bytes(),
                CAPABILITY_PROGRAM_SCHEMA_ID_V4
            );
            assert_eq!(selected.program().to_bytes(), value.descriptor);
        }
    }

    #[test]
    fn reordered_zero_and_bad_width_refuse_without_changing_output() {
        let reordered = [
            entry(DirectExecutionActionV3::RegisterBuy, 3),
            entry(DirectExecutionActionV3::RegisterSell, 2),
        ];
        let bytes = encoded_direct_program_set_bytes_v4(reordered.len()).expect("width");
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0x55_u8; bytes];
        let unchanged = output.clone();
        assert_eq!(
            encode_direct_program_set_v2_atomic(&reordered, &mut scratch, &mut output),
            Err(DirectProgramSetErrorV4::NonCanonicalActionOrder)
        );
        assert_eq!(output, unchanged);

        let zero = [ValidatedDirectCapabilityV4 {
            action: DirectExecutionActionV3::RegisterBuy,
            descriptor: [0; 32],
        }];
        let bytes = encoded_direct_program_set_bytes_v4(1).expect("width");
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0x55_u8; bytes];
        let unchanged = output.clone();
        assert_eq!(
            encode_direct_program_set_v2_atomic(&zero, &mut scratch, &mut output),
            Err(DirectProgramSetErrorV4::InvalidIdentity)
        );
        assert_eq!(output, unchanged);

        assert_eq!(
            encode_direct_program_set_v2_atomic(&[], &mut [], &mut []),
            Err(DirectProgramSetErrorV4::InvalidEntryCount)
        );
    }
}
