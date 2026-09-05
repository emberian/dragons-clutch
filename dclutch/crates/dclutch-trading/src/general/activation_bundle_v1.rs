//! Canonical General capability-activation artifacts.
//!
//! Activation is the one Core-signed action that CREATES the General capability
//! root every General action executes against. Its V1 descriptor sits in the
//! same `CapabilityProgramSetV2` as the seven settlement actions, at
//! [`GENERAL_ACTIVATION_SELECTOR_V4`], carrying the one schema the activation
//! seam accepts and that no action carries — which is the whole of wall #22:
//! `programs/dclutch-trading-sbf/src/outer.rs::authenticate_set_descriptor`
//! refuses any selected descriptor not stamped
//! `CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1`, and every action entry is stamped
//! `v4::SCHEMA_RELEASE_ID`.
//!
//! [`crate::general::release_v3::authenticate_general_program_set_v3`] has admitted that
//! coordinate since the profile relaxation landed. What did not exist until now
//! is the artifact it names: General's activation triple lived only as fixture
//! functions inside `programs/dclutch-trading-sbf/program-test/tests/
//! activation.rs`, so a release could not carry it.
//!
//! # What makes General's tail different from Direct's
//!
//! Direct's 24-byte root tail is entirely constant. General's 128 bytes are not:
//! the Market, the config identity and the Market generation are only known at
//! activation time, and they reach the artifact through registers the seam seeds
//! before anything runs (`activation_registers_v2`). So this bundle declares
//! three [`ActivationTailFieldV1`]s and lets
//! `dclutch-market::capability_activation` compose everything else out of the
//! constants.
//!
//! # Nothing about the root layout is restated here
//!
//! The constant part of the tail is not written down. It is DERIVED: this module
//! asks [`general_root_creation_tail_v2`] — the family's own creation oracle,
//! which is required elsewhere to be byte-identical to `GeneralRootV2::active` —
//! for a complete tail and blanks the three regions it has declared as
//! seam-supplied. It does that twice, with two unrelated Markets, configs and
//! generations, and refuses if the two results differ: any byte that varies with
//! those inputs and is NOT inside a declared field is a runtime-varying byte
//! nobody declared, and composing it as a constant would create roots that
//! disagree with `GeneralRootV2::active` on every market but one.

extern crate alloc;

use alloc::vec::Vec;

use crate::general_config::{
    GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_BYTES_V2, GENERAL_ROOT_SCHEMA_ID_V2,
    root::{
        GENERAL_ROOT_CONFIG_ID_OFFSET_V2, GENERAL_ROOT_GENERATION_OFFSET_V2,
        GENERAL_ROOT_MARKET_OFFSET_V2, general_root_creation_tail_v2,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_market::capability_activation::{
    ActivationBundleErrorV1, ActivationBundleInputV1, ActivationBundleV1, ActivationSeamImageV1,
    ActivationTailFieldV1, activation_descriptor_schema_v1, build_activation_bundle_v1,
    project_activation_root_tail_v1, validate_activation_bundle_v1,
};
use dclutch_market::capability_program::{
    CapabilityProgramV1,
    activation_registers_v2::{
        ACTIVATION_CONFIG_IDENTITY_V2, ACTIVATION_GENERATION_SCALAR_V2,
        ACTIVATION_MARKET_IDENTITY_V2,
    },
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID},
};
use dclutch_sha256_adapter::digest;

use crate::general::{
    artifacts_v3::GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3,
    release_v3::{GENERAL_ACTIONS_V5, GENERAL_ACTIVATION_SELECTOR_V4, GeneralReleaseProfileV1},
};

/// Exact activation selector-request width; the selector byte is at 10.
pub const GENERAL_ACTIVATION_REQUEST_BYTES_V1: usize = 16;
/// Domain-separating activation selector-request magic.
pub const GENERAL_ACTIVATION_REQUEST_MAGIC_V1: [u8; 8] = *b"DCGNACT1";
/// Activation selector-request schema version.
pub const GENERAL_ACTIVATION_REQUEST_VERSION_V1: u16 = 1;
/// Finalized schema label for the General activation selector request.
pub const GENERAL_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/general-activation-request-v1";
/// SHA-256 of [`GENERAL_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1`].
pub const GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V1: [u8; 32] = [
    0x24, 0x15, 0xb9, 0x84, 0xec, 0x84, 0xba, 0xeb, 0x36, 0x14, 0xae, 0xf8, 0xa0, 0xe2, 0xae, 0x03,
    0x80, 0xa6, 0xbd, 0xae, 0xa0, 0x56, 0x8f, 0xae, 0x72, 0x74, 0x90, 0x81, 0xd6, 0x09, 0xd2, 0xcc,
];

const _: () = {
    // What makes the three narrowings below total rather than trusted: the
    // offsets are compile-time coordinates inside a 128-byte tail.
    assert!(GENERAL_ROOT_MARKET_OFFSET_V2 < GENERAL_ROOT_BYTES_V2);
    assert!(GENERAL_ROOT_CONFIG_ID_OFFSET_V2 < GENERAL_ROOT_BYTES_V2);
    assert!(GENERAL_ROOT_GENERATION_OFFSET_V2 < GENERAL_ROOT_BYTES_V2);
    assert!(GENERAL_ROOT_BYTES_V2 < 0x1_0000);
};

/// The three regions of a General root tail the seam supplies at activation.
///
/// Ascending and disjoint, which the template requires so that reading the
/// effect top to bottom reads the tail left to right.
#[allow(clippy::cast_possible_truncation)]
pub const GENERAL_ACTIVATION_TAIL_FIELDS_V1: [ActivationTailFieldV1; 3] = [
    ActivationTailFieldV1::SeamIdentity {
        offset: GENERAL_ROOT_MARKET_OFFSET_V2 as u32,
        register: ACTIVATION_MARKET_IDENTITY_V2,
    },
    ActivationTailFieldV1::SeamIdentity {
        offset: GENERAL_ROOT_CONFIG_ID_OFFSET_V2 as u32,
        register: ACTIVATION_CONFIG_IDENTITY_V2,
    },
    ActivationTailFieldV1::SeamScalar {
        offset: GENERAL_ROOT_GENERATION_OFFSET_V2 as u32,
        register: ACTIVATION_GENERATION_SCALAR_V2,
    },
];

/// Chain-selected General release facts the activation descriptor inherits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralActivationBundleInputV1<'a> {
    /// Exact bytes of any one of the release's action descriptors.
    ///
    /// The manifest-selected coordinates are read off it rather than restated;
    /// the release admission has already required every action to agree on them.
    pub action_descriptor: &'a [u8],
    /// Compartment rows the founding provisions in the selected funding ledger.
    pub funding_ledger_slot_count: u16,
}

/// Stable General activation construction or validation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralActivationBundleErrorV1 {
    /// The supplied action descriptor did not decode as a `CapabilityProgramV4`.
    ActionDescriptor,
    /// The action descriptor did not name General's kind, root schema, or the
    /// exact `GENERAL_ROOT_BYTES_V2` root width.
    ForeignRoot,
    /// The family's own creation oracle refused.
    CreationOracle,
    /// **The completeness gate.** Two unrelated `(market, config, generation)`
    /// triples produced tails differing outside every declared field, so the
    /// root carries a runtime-varying byte no activation field declares.
    RuntimeVaryingByteNotDeclared,
    /// Activation selector request bytes were noncanonical.
    Request,
    /// ProgramSet construction, count, order, or self-selection refused.
    ProgramSet,
    /// The family-neutral activation template refused.
    Template(ActivationBundleErrorV1),
}

/// Result alias for General activation construction.
pub type GeneralActivationResultV1<T> = core::result::Result<T, GeneralActivationBundleErrorV1>;

/// Encode the sole canonical General activation selector request.
///
/// The set reads one byte at
/// [`crate::general::artifacts_v3::GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3`], which
/// is the authority for where the selector lives; the rest is domain separation
/// so this request can never be mistaken for a `ControllerRequestV2`.
pub fn general_activation_request_v1()
-> GeneralActivationResultV1<[u8; GENERAL_ACTIVATION_REQUEST_BYTES_V1]> {
    let mut output = [0_u8; GENERAL_ACTIVATION_REQUEST_BYTES_V1];
    put(&mut output, 0, &GENERAL_ACTIVATION_REQUEST_MAGIC_V1)?;
    put(
        &mut output,
        GENERAL_ACTIVATION_REQUEST_MAGIC_V1.len(),
        &GENERAL_ACTIVATION_REQUEST_VERSION_V1.to_le_bytes(),
    )?;
    let selector_offset = usize::try_from(GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3)
        .map_err(|_| GeneralActivationBundleErrorV1::Request)?;
    let selector = u8::try_from(GENERAL_ACTIVATION_SELECTOR_V4)
        .map_err(|_| GeneralActivationBundleErrorV1::Request)?;
    put(&mut output, selector_offset, &[selector])?;
    Ok(output)
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) -> GeneralActivationResultV1<()> {
    let end = offset
        .checked_add(source.len())
        .ok_or(GeneralActivationBundleErrorV1::Request)?;
    output
        .get_mut(offset..end)
        .ok_or(GeneralActivationBundleErrorV1::Request)?
        .copy_from_slice(source);
    Ok(())
}

/// Hostile-check one exact General activation selector request.
pub fn validate_general_activation_request_v1(bytes: &[u8]) -> GeneralActivationResultV1<()> {
    if bytes != general_activation_request_v1()? {
        return Err(GeneralActivationBundleErrorV1::Request);
    }
    Ok(())
}

/// Schema a General activation descriptor record is finalized under, and the
/// value its `CapabilityProgramSetV2` entry must carry.
#[must_use]
pub const fn general_activation_descriptor_schema_v1() -> [u8; 32] {
    activation_descriptor_schema_v1()
}

/// Build General's canonical activation descriptor/profile/effect bundle.
pub fn build_general_activation_bundle_v1(
    input: GeneralActivationBundleInputV1<'_>,
) -> GeneralActivationResultV1<ActivationBundleV1> {
    let (template, constant_tail) = template_input(input)?;
    let _ = &constant_tail;
    build_activation_bundle_v1(template.as_input(&constant_tail))
        .map_err(GeneralActivationBundleErrorV1::Template)
}

/// Rejoin one General activation bundle to the release it inherits from.
pub fn validate_general_activation_bundle_v1(
    bundle: &ActivationBundleV1,
    input: GeneralActivationBundleInputV1<'_>,
) -> GeneralActivationResultV1<()> {
    let (template, constant_tail) = template_input(input)?;
    validate_activation_bundle_v1(bundle, template.as_input(&constant_tail))
        .map_err(GeneralActivationBundleErrorV1::Template)?;
    let descriptor = CapabilityProgramV1::decode(&bundle.descriptor)
        .map_err(|_| GeneralActivationBundleErrorV1::ActionDescriptor)?;
    if descriptor.kind().to_bytes() != GENERAL_CAPABILITY_KIND_ID_V1
        || descriptor.root_schema().to_bytes() != GENERAL_ROOT_SCHEMA_ID_V2
        || usize::try_from(descriptor.root_state_bytes())
            .map_err(|_| GeneralActivationBundleErrorV1::ForeignRoot)?
            != GENERAL_ROOT_BYTES_V2
        || descriptor.request_schema().to_bytes() != GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V1
    {
        return Err(GeneralActivationBundleErrorV1::ForeignRoot);
    }
    Ok(())
}

/// Run the real activation artifacts and return the General root tail the outer
/// would write for these exact chain coordinates.
///
/// This is how a General activation is reviewed: the answer is decoded with
/// `GeneralRootV2::decode` and compared to `GeneralRootV2::active`, not read off
/// the effect's instruction list.
pub fn project_general_root_tail_v1(
    bundle: &ActivationBundleV1,
    market: [u8; 32],
    config_id: [u8; 32],
    generation: u64,
    rent_quote: u64,
) -> GeneralActivationResultV1<Vec<u8>> {
    let mut scalars = [0_u64; ACTIVATION_COMMON_SCALARS];
    if let Some(slot) = scalars.get_mut(usize::from(ACTIVATION_GENERATION_SCALAR_V2)) {
        *slot = generation;
    }
    let mut identities = [[0_u8; 32]; ACTIVATION_COMMON_IDENTITIES];
    if let Some(slot) = identities.get_mut(usize::from(ACTIVATION_MARKET_IDENTITY_V2)) {
        *slot = market;
    }
    if let Some(slot) = identities.get_mut(usize::from(ACTIVATION_CONFIG_IDENTITY_V2)) {
        *slot = config_id;
    }
    let (tail, _) = project_activation_root_tail_v1(
        bundle,
        ActivationSeamImageV1 {
            scalars: &scalars,
            identities: &identities,
            rent_quote,
        },
    )
    .map_err(GeneralActivationBundleErrorV1::Template)?;
    Ok(tail)
}

const ACTIVATION_COMMON_SCALARS: usize =
    dclutch_market::capability_program::activation_registers_v2::ACTIVATION_COMMON_SCALARS_V2;
const ACTIVATION_COMMON_IDENTITIES: usize =
    dclutch_market::capability_program::activation_registers_v2::ACTIVATION_COMMON_IDENTITIES_V2;

/// The inherited coordinates, separated from the borrowed constant tail so both
/// can be produced by one fallible pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InheritedV1 {
    kind: ContentId,
    config_schema: ContentId,
    root_schema: ContentId,
    derivation_policy: ContentId,
    capacity_profile: ContentId,
    root_state_bytes: u32,
    funding_ledger_slot_count: u16,
    request_schema: ContentId,
}

impl InheritedV1 {
    fn as_input<'a>(self, constant_tail: &'a [u8]) -> ActivationBundleInputV1<'a> {
        ActivationBundleInputV1 {
            kind: self.kind,
            config_schema: self.config_schema,
            request_schema: self.request_schema,
            root_schema: self.root_schema,
            derivation_policy: self.derivation_policy,
            capacity_profile: self.capacity_profile,
            root_state_bytes: self.root_state_bytes,
            constant_root_tail: constant_tail,
            seam_fields: &GENERAL_ACTIVATION_TAIL_FIELDS_V1,
            funding_ledger_slot_count: self.funding_ledger_slot_count,
            // This family funds its root with its exact Rent reserve alone.
            delivers_creation_principal: false,
        }
    }
}

fn template_input(
    input: GeneralActivationBundleInputV1<'_>,
) -> GeneralActivationResultV1<(InheritedV1, [u8; GENERAL_ROOT_BYTES_V2])> {
    let action = CapabilityProgramV4::decode(input.action_descriptor)
        .map_err(|_| GeneralActivationBundleErrorV1::ActionDescriptor)?;
    if action.kind().to_bytes() != GENERAL_CAPABILITY_KIND_ID_V1
        || action.root_schema().to_bytes() != GENERAL_ROOT_SCHEMA_ID_V2
        || usize::try_from(action.root_state_bytes())
            .map_err(|_| GeneralActivationBundleErrorV1::ForeignRoot)?
            != GENERAL_ROOT_BYTES_V2
    {
        return Err(GeneralActivationBundleErrorV1::ForeignRoot);
    }
    let constant_tail = constant_creation_tail()?;
    Ok((
        InheritedV1 {
            kind: action.kind(),
            config_schema: action.config_schema(),
            root_schema: action.root_schema(),
            derivation_policy: action.derivation_policy(),
            capacity_profile: action.capacity_profile(),
            root_state_bytes: action.root_state_bytes(),
            funding_ledger_slot_count: input.funding_ledger_slot_count,
            request_schema: ContentId::new(GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V1)
                .map_err(|_| GeneralActivationBundleErrorV1::ForeignRoot)?,
        },
        constant_tail,
    ))
}

/// Derive the constant part of an initial General root tail from the family's
/// own creation oracle, and prove the declared fields cover everything else.
fn constant_creation_tail() -> GeneralActivationResultV1<[u8; GENERAL_ROOT_BYTES_V2]> {
    // Two unrelated probes. Every byte outside the declared fields must agree,
    // or a runtime-varying byte is being composed as a constant.
    let first = masked_creation_tail([0x31; 32], [0x53; 32], 7)?;
    let second = masked_creation_tail([0xc4; 32], [0x9e; 32], 0x0123_4567_89ab_cdef)?;
    if first != second {
        return Err(GeneralActivationBundleErrorV1::RuntimeVaryingByteNotDeclared);
    }
    Ok(first)
}

fn masked_creation_tail(
    market: [u8; 32],
    config_id: [u8; 32],
    generation: u64,
) -> GeneralActivationResultV1<[u8; GENERAL_ROOT_BYTES_V2]> {
    let mut tail = general_root_creation_tail_v2(market, config_id, generation)
        .map_err(|_| GeneralActivationBundleErrorV1::CreationOracle)?;
    for field in GENERAL_ACTIVATION_TAIL_FIELDS_V1 {
        let start = usize::try_from(field.offset())
            .map_err(|_| GeneralActivationBundleErrorV1::ForeignRoot)?;
        let end = start
            .checked_add(field.width())
            .ok_or(GeneralActivationBundleErrorV1::ForeignRoot)?;
        tail.get_mut(start..end)
            .ok_or(GeneralActivationBundleErrorV1::ForeignRoot)?
            .fill(0);
    }
    Ok(tail)
}

/// SHA-256 identity of exact bytes, for callers naming published records.
#[must_use]
pub fn general_activation_record_id_v1(bytes: &[u8]) -> [u8; 32] {
    digest(bytes)
}

/// The exact eight-entry General release set: seven V4 actions and the V1
/// activation coordinate that makes the release activatable at all.
///
/// This is the second half of wall #22 for General. The first half is the
/// artifact; this is the entry that names it. Without this the release is seven
/// V4 descriptors, `authenticate_set_descriptor` refuses every one of them with
/// `UnsupportedContent`, and no General market founded from it can ever create
/// its root.
///
/// The builder is total and self-refuting: it re-authenticates the bytes it just
/// wrote with [`crate::general::release_v3::authenticate_general_program_set_v3`],
/// requires the profile to be `SettlementWithActivation`, and requires the
/// canonical activation request to select exactly the descriptor it was handed.
/// A caller cannot obtain a set that does not activate.
pub fn build_general_activation_capable_program_set_v1(
    action_descriptor_ids: &[[u8; 32]],
    activation_descriptor_id: [u8; 32],
) -> GeneralActivationResultV1<Vec<u8>> {
    build_general_activation_capable_program_set_for_profile_v1(
        GeneralReleaseProfileV1::SettlementWithActivation,
        action_descriptor_ids,
        activation_descriptor_id,
    )
}

/// The exact sixteen-entry current General release set: all fifteen first-class
/// V5 actions followed by the activation coordinate.
///
/// This is append-only beside [`build_general_activation_capable_program_set_v1`]:
/// the historical seven-action constructor keeps its exact profile and bytes.
/// The current selected-release compiler uses this constructor so its published
/// action count, descriptors, selectable set, and family admission all name the
/// same complete catalogue.
pub fn build_general_activation_capable_program_set_v2(
    action_descriptor_ids: &[[u8; 32]],
    activation_descriptor_id: [u8; 32],
) -> GeneralActivationResultV1<Vec<u8>> {
    build_general_activation_capable_program_set_for_profile_v1(
        GeneralReleaseProfileV1::CompleteV2WithActivation,
        action_descriptor_ids,
        activation_descriptor_id,
    )
}

fn build_general_activation_capable_program_set_for_profile_v1(
    profile: GeneralReleaseProfileV1,
    action_descriptor_ids: &[[u8; 32]],
    activation_descriptor_id: [u8; 32],
) -> GeneralActivationResultV1<Vec<u8>> {
    use dclutch_market::capability_program::set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, CapabilityProgramSetV2,
        SelectorWidthV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    };

    if !matches!(
        profile,
        GeneralReleaseProfileV1::SettlementWithActivation
            | GeneralReleaseProfileV1::CompleteV2WithActivation
    ) {
        return Err(GeneralActivationBundleErrorV1::ProgramSet);
    }
    if action_descriptor_ids.len() != profile.action_count() {
        return Err(GeneralActivationBundleErrorV1::ProgramSet);
    }
    let mut entries = Vec::with_capacity(profile.entry_count());
    for (action, descriptor) in GENERAL_ACTIONS_V5
        .into_iter()
        .take(profile.action_count())
        .zip(action_descriptor_ids.iter().copied())
    {
        entries.push(CapabilityProgramSetEntryV2::new(
            u32::from(action as u8),
            CapabilityDescriptorReferenceV2::new(
                content(CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID)?,
                content(descriptor)?,
            ),
        ));
    }
    // The activation coordinate: the one entry carrying a schema no action
    // carries, at a selector no controller request can produce.
    entries.push(CapabilityProgramSetEntryV2::new(
        GENERAL_ACTIVATION_SELECTOR_V4,
        CapabilityDescriptorReferenceV2::new(
            content(general_activation_descriptor_schema_v1())?,
            content(activation_descriptor_id)?,
        ),
    ));
    let width = encoded_program_set_bytes_v2(entries.len())
        .map_err(|_| GeneralActivationBundleErrorV1::ProgramSet)?;
    let mut output = alloc::vec![0_u8; width];
    encode_program_set_v2(
        GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3,
        SelectorWidthV2::U8,
        &entries,
        &mut output,
    )
    .map_err(|_| GeneralActivationBundleErrorV1::ProgramSet)?;

    let digest_of_output = digest(&output);
    let (set, authenticated) = crate::general::release_v3::authenticate_general_program_set_v3(
        digest_of_output,
        digest_of_output,
        &output,
    )
    .map_err(|_| GeneralActivationBundleErrorV1::ProgramSet)?;
    if authenticated != profile {
        return Err(GeneralActivationBundleErrorV1::ProgramSet);
    }
    let request = general_activation_request_v1()?;
    let selected = set
        .select_descriptor(&request)
        .map_err(|_| GeneralActivationBundleErrorV1::ProgramSet)?;
    if selected.schema().to_bytes() != general_activation_descriptor_schema_v1()
        || selected.program().to_bytes() != activation_descriptor_id
    {
        return Err(GeneralActivationBundleErrorV1::ProgramSet);
    }
    // And no action request may reach it: every action selects its own V4.
    for (index, action) in GENERAL_ACTIONS_V5
        .into_iter()
        .take(profile.action_count())
        .enumerate()
    {
        let mut probe = alloc::vec![0_u8; GENERAL_ACTIVATION_REQUEST_BYTES_V1];
        put(
            &mut probe,
            usize::try_from(GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3)
                .map_err(|_| GeneralActivationBundleErrorV1::ProgramSet)?,
            &[action as u8],
        )?;
        let action_selected = set
            .select_descriptor(&probe)
            .map_err(|_| GeneralActivationBundleErrorV1::ProgramSet)?;
        let expected = action_descriptor_ids
            .get(index)
            .copied()
            .ok_or(GeneralActivationBundleErrorV1::ProgramSet)?;
        if action_selected.schema().to_bytes() != CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID
            || action_selected.program().to_bytes() != expected
        {
            return Err(GeneralActivationBundleErrorV1::ProgramSet);
        }
    }
    let _ = CapabilityProgramSetV2::decode(&output)
        .map_err(|_| GeneralActivationBundleErrorV1::ProgramSet)?;
    Ok(output)
}

fn content(bytes: [u8; 32]) -> GeneralActivationResultV1<ContentId> {
    ContentId::new(bytes).map_err(|_| GeneralActivationBundleErrorV1::ProgramSet)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use crate::general_config::root::GeneralRootV2;
    use dclutch_market::capability_program::v4::{
        ArtifactReferenceV4, CapabilityArtifactsV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
        SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    };

    use super::*;

    fn id(value: [u8; 32]) -> ContentId {
        ContentId::new(value).expect("content identity")
    }

    fn reference(schema: [u8; 32], program: u8) -> ArtifactReferenceV4 {
        ArtifactReferenceV4::new(id(schema), id([program; 32]))
    }

    /// A minimal General action descriptor: only the six coordinates the
    /// activation inherits are meaningful, and the release admission has already
    /// required every action to agree on them.
    fn action_descriptor(root_state_bytes: u32) -> Vec<u8> {
        CapabilityProgramV4::new(
            id(GENERAL_CAPABILITY_KIND_ID_V1),
            id([0x51; 32]),
            id([0x52; 32]),
            id(GENERAL_ROOT_SCHEMA_ID_V2),
            id([0x53; 32]),
            id([0x54; 32]),
            CapabilityArtifactsV4 {
                account_profile: reference([0x61; 32], 0x71),
                request_profile: reference([0x62; 32], 0x72),
                lifecycle: reference(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, 0x73),
                strategy: reference([0x64; 32], 0x74),
                transition: reference([0x65; 32], 0x75),
                effect: reference([0x66; 32], 0x76),
            },
            root_state_bytes,
        )
        .expect("action descriptor")
        .encode()
        .to_vec()
    }

    fn built() -> (Vec<u8>, ActivationBundleV1) {
        let descriptor =
            action_descriptor(u32::try_from(GENERAL_ROOT_BYTES_V2).expect("General root width"));
        let bundle = build_general_activation_bundle_v1(GeneralActivationBundleInputV1 {
            action_descriptor: &descriptor,
            funding_ledger_slot_count: 1,
        })
        .expect("General activation bundle");
        (descriptor, bundle)
    }

    #[test]
    fn the_request_schema_and_selector_are_frozen() {
        assert_eq!(
            digest(GENERAL_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1),
            GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V1
        );
        let request = general_activation_request_v1().expect("request");
        validate_general_activation_request_v1(&request).expect("canonical request");
        let offset = usize::try_from(GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3).expect("offset");
        assert_eq!(
            u32::from(*request.get(offset).expect("selector byte")),
            GENERAL_ACTIVATION_SELECTOR_V4
        );
        // No General action can produce it: the seven actions occupy the low
        // namespace and this is the reserved 255.
        for action in crate::general::release_v3::GENERAL_ACTIONS_V4 {
            assert_ne!(u32::from(action as u8), GENERAL_ACTIVATION_SELECTOR_V4);
        }
        for position in [0_usize, 8, offset] {
            let mut hostile = request;
            *hostile.get_mut(position).expect("hostile byte") ^= 1;
            assert_eq!(
                validate_general_activation_request_v1(&hostile),
                Err(GeneralActivationBundleErrorV1::Request)
            );
        }
    }

    /// **The gate that makes a General activation safe to publish.**
    ///
    /// The bundle's real artifacts, run through the real effect kernel, must
    /// produce exactly what `GeneralRootV2::active` produces — for arbitrary
    /// Markets, config identities and generations, not one fixture. A tail that
    /// is the right width and the wrong content would brick the root forever,
    /// and no General action could ever authenticate it.
    #[test]
    fn the_real_artifacts_compose_exactly_what_general_root_active_composes() {
        let (_, bundle) = built();
        for (market, config_id, generation) in [
            ([0x11_u8; 32], [0x22_u8; 32], 7_u64),
            ([0xfe; 32], [0x01; 32], 0),
            ([0x7c; 32], [0xb3; 32], u64::MAX),
        ] {
            let projected =
                project_general_root_tail_v1(&bundle, market, config_id, generation, 2_672_640)
                    .expect("projection");
            let expected = general_root_creation_tail_v2(market, config_id, generation)
                .expect("creation oracle");
            assert_eq!(projected.as_slice(), expected.as_slice());
            assert_eq!(
                GeneralRootV2::decode(&projected).expect("decodes as a General root"),
                GeneralRootV2::active(market, config_id, generation).expect("active root")
            );
        }
    }

    #[test]
    fn the_descriptor_carries_the_one_schema_the_activation_seam_accepts() {
        let (descriptor_bytes, bundle) = built();
        validate_general_activation_bundle_v1(
            &bundle,
            GeneralActivationBundleInputV1 {
                action_descriptor: &descriptor_bytes,
                funding_ledger_slot_count: 1,
            },
        )
        .expect("rejoin");
        let descriptor = CapabilityProgramV1::decode(&bundle.descriptor).expect("V1 descriptor");
        assert_eq!(
            general_activation_descriptor_schema_v1(),
            dclutch_market::capability_program::CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
        );
        assert_ne!(
            general_activation_descriptor_schema_v1(),
            CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID
        );
        let action = CapabilityProgramV4::decode(&descriptor_bytes).expect("V4");
        assert_eq!(descriptor.kind(), action.kind());
        assert_eq!(descriptor.config_schema(), action.config_schema());
        assert_eq!(descriptor.root_schema(), action.root_schema());
        assert_eq!(descriptor.derivation_policy(), action.derivation_policy());
        assert_eq!(descriptor.capacity_profile(), action.capacity_profile());
        assert_eq!(descriptor.root_state_bytes(), action.root_state_bytes());
        // The one coordinate activation does NOT inherit.
        assert_ne!(descriptor.request_schema(), action.request_schema());
        assert_eq!(
            descriptor.request_schema().to_bytes(),
            GENERAL_ACTIVATION_REQUEST_SCHEMA_ID_V1
        );
    }

    /// A foreign root width, schema or kind refuses BEFORE any artifact exists.
    #[test]
    fn an_action_descriptor_that_is_not_generals_refuses() {
        let narrow = action_descriptor(u32::try_from(GENERAL_ROOT_BYTES_V2).expect("width") - 8);
        assert_eq!(
            build_general_activation_bundle_v1(GeneralActivationBundleInputV1 {
                action_descriptor: &narrow,
                funding_ledger_slot_count: 1,
            })
            .err(),
            Some(GeneralActivationBundleErrorV1::ForeignRoot)
        );
        let foreign = CapabilityProgramV4::new(
            id([0x99; 32]),
            id([0x51; 32]),
            id([0x52; 32]),
            id(GENERAL_ROOT_SCHEMA_ID_V2),
            id([0x53; 32]),
            id([0x54; 32]),
            CapabilityArtifactsV4 {
                account_profile: reference([0x61; 32], 0x71),
                request_profile: reference([0x62; 32], 0x72),
                lifecycle: reference(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, 0x73),
                strategy: reference([0x64; 32], 0x74),
                transition: reference([0x65; 32], 0x75),
                effect: reference([0x66; 32], 0x76),
            },
            u32::try_from(GENERAL_ROOT_BYTES_V2).expect("width"),
        )
        .expect("foreign descriptor")
        .encode()
        .to_vec();
        assert_eq!(
            build_general_activation_bundle_v1(GeneralActivationBundleInputV1 {
                action_descriptor: &foreign,
                funding_ledger_slot_count: 1,
            })
            .err(),
            Some(GeneralActivationBundleErrorV1::ForeignRoot)
        );
        assert_eq!(
            build_general_activation_bundle_v1(GeneralActivationBundleInputV1 {
                action_descriptor: &[0_u8; 8],
                funding_ledger_slot_count: 1,
            })
            .err(),
            Some(GeneralActivationBundleErrorV1::ActionDescriptor)
        );
    }

    /// Substituting any of the three records refuses on rejoin.
    #[test]
    fn a_substituted_activation_record_refuses() {
        let (descriptor_bytes, bundle) = built();
        let input = GeneralActivationBundleInputV1 {
            action_descriptor: &descriptor_bytes,
            funding_ledger_slot_count: 1,
        };
        for mutate in [0_usize, 1, 2] {
            let mut hostile = bundle.clone();
            let target = match mutate {
                0 => &mut hostile.account_profile,
                1 => &mut hostile.effect,
                _ => &mut hostile.descriptor,
            };
            *target.last_mut().expect("record byte") ^= 1;
            assert!(validate_general_activation_bundle_v1(&hostile, input).is_err());
        }
        // A different provisioned ledger shape is a different profile.
        assert!(
            validate_general_activation_bundle_v1(
                &bundle,
                GeneralActivationBundleInputV1 {
                    action_descriptor: &descriptor_bytes,
                    funding_ledger_slot_count: 2,
                },
            )
            .is_err()
        );
    }

    /// The declared fields are the ONLY runtime-varying bytes, and the constant
    /// tail is derived from the family's oracle rather than restated.
    #[test]
    fn the_constant_tail_is_the_oracles_tail_with_exactly_the_declared_fields_blanked() {
        let constant = constant_creation_tail().expect("constant tail");
        let (market, config_id, generation) = ([0x11_u8; 32], [0x22_u8; 32], 7_u64);
        let mut expected =
            general_root_creation_tail_v2(market, config_id, generation).expect("oracle");
        for field in GENERAL_ACTIVATION_TAIL_FIELDS_V1 {
            let start = usize::try_from(field.offset()).expect("offset");
            expected
                .get_mut(start..start + field.width())
                .expect("field region")
                .fill(0);
        }
        assert_eq!(constant, expected);
        // Every declared field's region is zero in the constant tail, so no byte
        // has two sources.
        for field in GENERAL_ACTIVATION_TAIL_FIELDS_V1 {
            let start = usize::try_from(field.offset()).expect("offset");
            assert!(
                constant
                    .get(start..start + field.width())
                    .expect("field region")
                    .iter()
                    .all(|byte| *byte == 0)
            );
        }
        // And the constant tail is NOT a decodable root on its own: the Market
        // and config it lacks are exactly what the seam supplies.
        assert!(GeneralRootV2::decode(&constant).is_err());
    }

    /// The set the wall was about: seven V4 actions and the one V1 coordinate
    /// the activation seam will accept, selecting the bundle this module built.
    #[test]
    fn the_activation_capable_set_selects_the_bundles_own_descriptor() {
        use dclutch_market::capability_program::set_v2::{
            CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2, CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2,
            CapabilityProgramSetV2,
        };

        let (_, bundle) = built();
        let actions: Vec<[u8; 32]> = (0..GeneralReleaseProfileV1::SettlementWithActivation
            .action_count())
            .map(|index| [u8::try_from(index).expect("action index") + 1; 32])
            .collect();
        let set_bytes =
            build_general_activation_capable_program_set_v1(&actions, bundle.descriptor_id)
                .expect("activation-capable set");
        let set = CapabilityProgramSetV2::decode(&set_bytes).expect("set");
        assert_eq!(usize::from(set.entry_count()), 8);

        // The seven action coordinates keep the schema the hot path selects.
        for index in 0..7_u16 {
            assert_eq!(
                set.entry(index)
                    .expect("entry")
                    .descriptor()
                    .schema()
                    .to_bytes(),
                CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID
            );
        }
        // The eighth carries the schema NO action carries, and it is the exact
        // constant `authenticate_set_descriptor` compares against.
        let activation = set.entry(7).expect("activation entry").descriptor();
        assert_eq!(
            activation.schema().to_bytes(),
            dclutch_market::capability_program::CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(activation.program().to_bytes(), bundle.descriptor_id);

        // A wrong action count, and a set whose activation entry names nothing,
        // both refuse rather than producing a release that cannot activate.
        assert_eq!(
            build_general_activation_capable_program_set_v1(
                actions.get(..6).expect("short"),
                bundle.descriptor_id
            )
            .err(),
            Some(GeneralActivationBundleErrorV1::ProgramSet)
        );
        assert_eq!(
            build_general_activation_capable_program_set_v1(&actions, [0_u8; 32]).err(),
            Some(GeneralActivationBundleErrorV1::ProgramSet)
        );
        // An activation descriptor aliasing an action's descriptor is a
        // duplicate coordinate and refuses at the authenticator.
        assert_eq!(
            build_general_activation_capable_program_set_v1(
                &actions,
                actions.first().copied().expect("first action")
            )
            .err(),
            Some(GeneralActivationBundleErrorV1::ProgramSet)
        );

        // The current complete constructor is a strict append-only extension:
        // its first seven encoded entries are byte-for-byte the historical
        // set, and its activation entry is the same exact reference moved to
        // the coordinate after all fifteen actions.
        let complete_actions: Vec<[u8; 32]> = (0
            ..GeneralReleaseProfileV1::CompleteV2WithActivation.action_count())
            .map(|index| [u8::try_from(index).expect("action index") + 1; 32])
            .collect();
        assert_eq!(
            complete_actions
                .get(..actions.len())
                .expect("legacy prefix"),
            actions
        );
        let complete_bytes = build_general_activation_capable_program_set_v2(
            &complete_actions,
            bundle.descriptor_id,
        )
        .expect("complete activation-capable set");
        let complete = CapabilityProgramSetV2::decode(&complete_bytes).expect("complete set");
        assert_eq!(usize::from(complete.entry_count()), 16);
        for index in 0..actions.len() {
            let start = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
                + index * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2;
            let end = start + CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2;
            assert_eq!(
                complete_bytes
                    .get(start..end)
                    .expect("complete prefix entry"),
                set_bytes.get(start..end).expect("legacy prefix entry"),
                "historical action entry {index} moved"
            );
        }
        let legacy_activation_start = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
            + actions.len() * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2;
        let complete_activation_start = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2
            + complete_actions.len() * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2;
        assert_eq!(
            complete_bytes
                .get(
                    complete_activation_start
                        ..complete_activation_start + CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2
                )
                .expect("complete activation entry"),
            set_bytes
                .get(
                    legacy_activation_start
                        ..legacy_activation_start + CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2
                )
                .expect("legacy activation entry")
        );
        assert_eq!(
            build_general_activation_capable_program_set_v2(
                complete_actions.get(..14).expect("short complete"),
                bundle.descriptor_id,
            )
            .err(),
            Some(GeneralActivationBundleErrorV1::ProgramSet)
        );
    }
}
