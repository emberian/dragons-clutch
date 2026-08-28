//! Canonical multi-action ProgramSetV2 construction for Direct V4 bundles.
//!
//! A set entry can be obtained only by successfully validating one complete
//! action bundle. This prevents release tooling from pairing a selector with
//! an arbitrary CapabilityProgramV4 digest while preserving the generic
//! family-neutral ProgramSetV2 wire and Hot selection path.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_capability_program_contract::{
    set_v2::{
        CAPABILITY_PROGRAM_SET_MAX_ENTRIES_V2, CapabilityDescriptorReferenceV2,
        CapabilityProgramSetEntryV2, CapabilityProgramSetV2, SelectorWidthV2,
        encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
};
use dclutch_core_contract::ContentId;
use dclutch_sha256_adapter::digest;

use crate::{
    execution_v3::{DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3, DirectExecutionActionV3},
    native_close_bundle_v1::{
        DIRECT_NATIVE_CLOSE_SELECTOR_V1, DirectNativeCloseBundleInputV1, DirectNativeCloseBundleV1,
        build_direct_native_close_bundle_v1, direct_native_close_request_v1,
        validate_direct_native_close_bundle_v1,
    },
    ordinary_bundle_v4::{
        DirectInlineOrdinaryHotBundleV4, validate_direct_inline_ordinary_hot_bundle_v4,
    },
    registered_bundle_v4::{
        DirectRegisterBuyHotBundleV4, DirectRegisterSellHotBundleV4,
        validate_direct_register_buy_hot_bundle_v4, validate_direct_register_sell_hot_bundle_v4,
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
    /// The canonical lifecycle-native close bundle refused.
    NativeClose,
}

/// Canonical ordinary execution plus lifecycle-native close release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineOrdinaryNativeCloseProgramSetV1 {
    /// Complete selector-1 ordinary V4 artifacts.
    pub ordinary: DirectInlineOrdinaryHotBundleV4,
    /// Complete reserved-high-selector close V1 artifacts.
    pub native_close: DirectNativeCloseBundleV1,
    /// Exact heterogeneous two-entry CapabilityProgramSetV2 bytes.
    pub program_set: Vec<u8>,
    /// SHA-256 identity of `program_set` selected by the Market manifest.
    pub program_set_id: [u8; 32],
}

/// Build the exact two-entry ordinary/native-close ProgramSet.
pub fn build_direct_inline_ordinary_native_close_program_set_v1(
    ordinary: DirectInlineOrdinaryHotBundleV4,
    capacity_profile: [u8; 32],
) -> Result<DirectInlineOrdinaryNativeCloseProgramSetV1, DirectProgramSetErrorV4> {
    validate_direct_inline_ordinary_hot_bundle_v4(&ordinary, capacity_profile)
        .map_err(|_| DirectProgramSetErrorV4::Bundle)?;
    let native_close = build_direct_native_close_bundle_v1(DirectNativeCloseBundleInputV1 {
        ordinary: &ordinary,
        capacity_profile,
    })
    .map_err(|_| DirectProgramSetErrorV4::NativeClose)?;
    let entries = ordinary_native_close_entries(&ordinary, &native_close)?;
    let width = encoded_direct_program_set_bytes_v4(entries.len())?;
    let mut program_set = vec![0_u8; width];
    encode_program_set_v2(
        DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3,
        SelectorWidthV2::U32,
        &entries,
        &mut program_set,
    )
    .map_err(|_| DirectProgramSetErrorV4::ProgramSet)?;
    let output = DirectInlineOrdinaryNativeCloseProgramSetV1 {
        ordinary,
        native_close,
        program_set_id: digest(&program_set),
        program_set,
    };
    validate_direct_inline_ordinary_native_close_program_set_v1(&output, capacity_profile)?;
    Ok(output)
}

/// Hostile-decode and bind the exact heterogeneous two-entry Direct release.
pub fn validate_direct_inline_ordinary_native_close_program_set_v1(
    value: &DirectInlineOrdinaryNativeCloseProgramSetV1,
    capacity_profile: [u8; 32],
) -> Result<(), DirectProgramSetErrorV4> {
    validate_direct_inline_ordinary_hot_bundle_v4(&value.ordinary, capacity_profile)
        .map_err(|_| DirectProgramSetErrorV4::Bundle)?;
    validate_direct_native_close_bundle_v1(
        &value.native_close,
        DirectNativeCloseBundleInputV1 {
            ordinary: &value.ordinary,
            capacity_profile,
        },
    )
    .map_err(|_| DirectProgramSetErrorV4::NativeClose)?;
    if value.program_set_id != digest(&value.program_set) {
        return Err(DirectProgramSetErrorV4::ProgramSet);
    }
    let set = CapabilityProgramSetV2::decode(&value.program_set)
        .map_err(|_| DirectProgramSetErrorV4::ProgramSet)?;
    if set.selector_offset() != DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U32
        || set.entry_count() != 2
    {
        return Err(DirectProgramSetErrorV4::ProgramSet);
    }
    let expected = ordinary_native_close_entries(&value.ordinary, &value.native_close)?;
    for (index, entry) in expected.iter().copied().enumerate() {
        if set
            .entry(u16::try_from(index).map_err(|_| DirectProgramSetErrorV4::ProgramSet)?)
            .map_err(|_| DirectProgramSetErrorV4::ProgramSet)?
            != entry
        {
            return Err(DirectProgramSetErrorV4::ProgramSet);
        }
    }
    let ordinary_selected = set
        .select_descriptor(&ordinary_selector_request())
        .map_err(|_| DirectProgramSetErrorV4::ProgramSet)?;
    if ordinary_selected != expected[0].descriptor() {
        return Err(DirectProgramSetErrorV4::ProgramSet);
    }
    let close_selected = set
        .select_descriptor(&direct_native_close_request_v1())
        .map_err(|_| DirectProgramSetErrorV4::ProgramSet)?;
    if close_selected != expected[1].descriptor() {
        return Err(DirectProgramSetErrorV4::ProgramSet);
    }
    Ok(())
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

/// Validate one complete RegisterSell bundle before admitting its set entry.
///
/// A Sell's entry is obtained the same way every other entry is -- by its own
/// bundle validating -- and it is a DIFFERENT entry: distinct selector, distinct
/// descriptor, and not one artifact digest shared with the Buy.
pub fn validate_direct_register_sell_capability_v4(
    bundle: &DirectRegisterSellHotBundleV4,
    capacity_profile: [u8; 32],
) -> Result<ValidatedDirectCapabilityV4, DirectProgramSetErrorV4> {
    validate_direct_register_sell_hot_bundle_v4(bundle, capacity_profile)
        .map_err(|_| DirectProgramSetErrorV4::Bundle)?;
    Ok(validated(
        DirectExecutionActionV3::RegisterSell,
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

fn ordinary_native_close_entries(
    ordinary: &DirectInlineOrdinaryHotBundleV4,
    native_close: &DirectNativeCloseBundleV1,
) -> Result<[CapabilityProgramSetEntryV2; 2], DirectProgramSetErrorV4> {
    Ok([
        CapabilityProgramSetEntryV2::new(
            DirectExecutionActionV3::InlineOrdinary as u32,
            CapabilityDescriptorReferenceV2::new(
                content(CAPABILITY_PROGRAM_SCHEMA_ID_V4)?,
                content(digest(&ordinary.descriptor))?,
            ),
        ),
        CapabilityProgramSetEntryV2::new(
            DIRECT_NATIVE_CLOSE_SELECTOR_V1,
            CapabilityDescriptorReferenceV2::new(
                content(
                    dclutch_capability_program_contract::CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
                )?,
                content(native_close.descriptor_id)?,
            ),
        ),
    ])
}

fn ordinary_selector_request() -> [u8; 16] {
    let mut request = [0_u8; 16];
    request[12..16]
        .copy_from_slice(&(DirectExecutionActionV3::InlineOrdinary as u32).to_le_bytes());
    request
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DirectProgramSetErrorV4> {
    ContentId::new(bytes).map_err(|_| DirectProgramSetErrorV4::InvalidIdentity)
}

#[cfg(test)]
mod tests {
    extern crate std;

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

    #[test]
    fn ordinary_and_native_close_are_distinct_schema_bound_selectors() {
        let ordinary = crate::ordinary_bundle_v4::tests::canonical_bundle_for_cross_module_tests();
        let release =
            build_direct_inline_ordinary_native_close_program_set_v1(ordinary, [0x44; 32])
                .expect("release");
        validate_direct_inline_ordinary_native_close_program_set_v1(&release, [0x44; 32])
            .expect("validate");
        let set = CapabilityProgramSetV2::decode(&release.program_set).expect("set");
        let ordinary_entry = set.entry(0).expect("ordinary");
        let close_entry = set.entry(1).expect("close");
        assert_eq!(ordinary_entry.selector(), 1);
        assert_eq!(
            ordinary_entry.descriptor().schema().to_bytes(),
            CAPABILITY_PROGRAM_SCHEMA_ID_V4
        );
        assert_eq!(close_entry.selector(), DIRECT_NATIVE_CLOSE_SELECTOR_V1);
        assert_eq!(
            close_entry.descriptor().schema().to_bytes(),
            dclutch_capability_program_contract::CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(
            close_entry.descriptor().program().to_bytes(),
            release.native_close.descriptor_id
        );
        let mut register_sell = [0_u8; 16];
        register_sell[12..16]
            .copy_from_slice(&(DirectExecutionActionV3::RegisterSell as u32).to_le_bytes());
        assert!(set.select_descriptor(&register_sell).is_err());
    }

    #[test]
    fn selector_schema_content_order_and_identity_substitutions_refuse() {
        let ordinary = crate::ordinary_bundle_v4::tests::canonical_bundle_for_cross_module_tests();
        let release =
            build_direct_inline_ordinary_native_close_program_set_v1(ordinary, [0x44; 32])
                .expect("release");

        let mutate_entry = |selector: u32, schema: [u8; 32], program: [u8; 32]| {
            let ordinary_entry = CapabilityProgramSetEntryV2::new(
                1,
                CapabilityDescriptorReferenceV2::new(
                    ContentId::new(CAPABILITY_PROGRAM_SCHEMA_ID_V4).expect("schema"),
                    ContentId::new(digest(&release.ordinary.descriptor)).expect("descriptor"),
                ),
            );
            let hostile = CapabilityProgramSetEntryV2::new(
                selector,
                CapabilityDescriptorReferenceV2::new(
                    ContentId::new(schema).expect("schema"),
                    ContentId::new(program).expect("program"),
                ),
            );
            let entries = [ordinary_entry, hostile];
            let mut bytes =
                vec![0_u8; encoded_direct_program_set_bytes_v4(entries.len()).expect("width")];
            encode_program_set_v2(
                DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3,
                SelectorWidthV2::U32,
                &entries,
                &mut bytes,
            )
            .expect("hostile set still canonical");
            DirectInlineOrdinaryNativeCloseProgramSetV1 {
                ordinary: release.ordinary,
                native_close: release.native_close.clone(),
                program_set_id: digest(&bytes),
                program_set: bytes,
            }
        };

        let bad_selector = mutate_entry(
            DIRECT_NATIVE_CLOSE_SELECTOR_V1 - 1,
            dclutch_capability_program_contract::CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
            release.native_close.descriptor_id,
        );
        assert!(
            validate_direct_inline_ordinary_native_close_program_set_v1(&bad_selector, [0x44; 32])
                .is_err()
        );
        let bad_schema = mutate_entry(
            DIRECT_NATIVE_CLOSE_SELECTOR_V1,
            CAPABILITY_PROGRAM_SCHEMA_ID_V4,
            release.native_close.descriptor_id,
        );
        assert!(
            validate_direct_inline_ordinary_native_close_program_set_v1(&bad_schema, [0x44; 32])
                .is_err()
        );
        let bad_content = mutate_entry(
            DIRECT_NATIVE_CLOSE_SELECTOR_V1,
            dclutch_capability_program_contract::CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
            [0x66; 32],
        );
        assert!(
            validate_direct_inline_ordinary_native_close_program_set_v1(&bad_content, [0x44; 32])
                .is_err()
        );
        let mut bad_identity = release.clone();
        bad_identity.program_set_id[0] ^= 1;
        assert!(
            validate_direct_inline_ordinary_native_close_program_set_v1(&bad_identity, [0x44; 32])
                .is_err()
        );
    }
}
