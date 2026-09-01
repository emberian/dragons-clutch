//! Canonical Series capability-activation artifacts.
//!
//! Activation is the one Core-signed action that CREATES the Series composite
//! root every Series action executes against. Until this module existed, the
//! current Series V5 ProgramSet carried only the five Prepare/Consume/Expire/
//! Retire/Close `CapabilityProgramV4` action descriptors and no activation
//! coordinate at all — so
//! `programs/dclutch-trading-sbf/src/outer.rs::authenticate_set_descriptor`
//! refused every entry it could select with `TradingSbfError::UnsupportedContent`
//! (it requires the selected descriptor to be stamped
//! `CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1`, and every action entry is stamped
//! `v4::SCHEMA_RELEASE_ID`). A Series Market founded from such a release is
//! foundable, admittable, and permanently unable to create its root.
//!
//! # Nothing about the root layout is restated here
//!
//! The initial tail is not written down. It is asked for, from the kernel's own
//! creation oracle
//! [`dclutch_series_v3_kernel::activation::series_activation_root_tail_v3`],
//! which is `SeriesStateV3::new(template.close_rent())` under the Template's own
//! occurrence count and nothing else. Every byte of a Series tail is a function
//! of the Template, and the release binds one exact Template (see
//! [`SERIES_ACTIVATION_TAIL_FIELDS_V1`]), so this activation declares **no**
//! seam-supplied fields at all — unlike General, whose tail carries the Market,
//! the config identity and the generation.
//!
//! # The Template is the completeness gate
//!
//! Because the whole tail is Template-derived, the one thing that could go
//! wrong is composing it from a DIFFERENT Template than the release binds.
//! `super::release_v5::encode_descriptor` stores the exact Template content
//! identity in each action descriptor's `capacity_profile` coordinate, and
//! `CapabilityProgramV1::validate_selection` requires that same value to equal
//! the manifest entry's `capacity_profile_id`. So [`template_input`] requires
//! `template_content_id(input.template) == action.capacity_profile()` and
//! refuses [`SeriesActivationBundleErrorV1::TemplateSubstitution`] otherwise.
//! That is stronger than General's two-probe completeness check, not weaker: it
//! is an identity join rather than a variance sample.
//!
//! # The prepaid close principal, and where it currently cannot go
//!
//! A Template's `close_rent` is separately prepaid principal. The kernel's
//! terminal contract already reads it back out of the root and classifies it
//! apart from root Rent and donation
//! ([`dclutch_series_v3_kernel::terminal::plan_series_root_closure_v3`]), and
//! this module composes it into the root exactly as that contract expects, for
//! every Template, with no zero-principal rule imposed anywhere.
//!
//! What it cannot do is FUND it. The family-neutral activation seam moves the
//! selected funding ledger's parked Rent quote into the vacant root and then
//! requires, at `outer.rs:1644`, that the root's poststate balance equal
//! `rent.minimum_balance(descriptor.root_account_bytes())` exactly —
//! `TradingSbfError::Root` otherwise. That quantity is a pure function of the
//! declared root width and carries no family-varying term, so a Template with a
//! nonzero `close_rent` describes a root that activation may not fund and
//! terminal Close will therefore refuse with `SeriesTerminalErrorV3::Balance`
//! forever. [`series_activation_seam_funding_verdict_v1`] states that
//! arithmetic explicitly rather than letting a release discover it on a bricked
//! root; it convicts the seam, and refuses no Template.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_capability_activation_codec::{
    ActivationBundleErrorV1, ActivationBundleInputV1, ActivationBundleV1, ActivationSeamImageV1,
    ActivationTailFieldV1, activation_descriptor_schema_v1, build_activation_bundle_v1,
    project_activation_root_tail_v1, validate_activation_bundle_v1,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    activation_registers_v2::{ACTIVATION_COMMON_IDENTITIES_V2, ACTIVATION_COMMON_SCALARS_V2},
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, CapabilityProgramSetV2,
        SelectorWidthV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID},
};
use dclutch_core_contract::ContentId;
use dclutch_series_v3_kernel::{
    SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3, TemplateV3,
    activation::{SeriesActivationErrorV3, series_activation_root_tail_v3},
    replay::{SERIES_STATE_BYTES_V3, SeriesStateV3},
    template_content_id,
};
use dclutch_sha256_adapter::digest;

use super::{
    artifacts_v3::{
        SERIES_ACTION_SELECTOR_OFFSET_V3, SERIES_ROOT_SCHEMA_PREIMAGE_V3,
        SERIES_SUCCESSOR_KIND_PREIMAGE_V3, SERIES_TICKET_DERIVATION_PREIMAGE_V3,
    },
    release_v5::SERIES_RELEASE_ACTION_COUNT_V5,
};

/// Selector reserved for the Series activation descriptor's set coordinate.
///
/// Deliberately outside the action tag space: `SeriesActionV3::decode` refuses
/// every value above four, so no Series hot request can ever select this entry,
/// and the canonical activation request can select nothing else.
pub const SERIES_ACTIVATION_SELECTOR_V5: u32 = 255;

/// Exact entry count of an activation-capable current Series release.
pub const SERIES_RELEASE_ENTRY_COUNT_V5: usize = SERIES_RELEASE_ACTION_COUNT_V5 + 1;

/// Exact activation selector-request width; the selector byte is at twelve.
pub const SERIES_ACTIVATION_REQUEST_BYTES_V1: usize = 16;

/// Domain-separating activation selector-request magic.
///
/// Distinct from the Series action header magic, so this request can never be
/// mistaken for a [`dclutch_series_v3_kernel::request::SeriesActionRequestV3`].
pub const SERIES_ACTIVATION_REQUEST_MAGIC_V1: [u8; 8] = *b"DCSEACT1";

/// Activation selector-request schema version.
pub const SERIES_ACTIVATION_REQUEST_VERSION_V1: u16 = 1;

/// Finalized schema label for the Series activation selector request.
pub const SERIES_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/series-activation-request-v1";

/// The regions of a Series root tail the seam supplies at activation: none.
///
/// Every byte of [`SeriesStateV3`] is a function of the Template the release
/// binds — the close principal, the occurrence count, and constants. There is
/// no Market, config identity, generation, rent quote or PDA bump anywhere in
/// it, so declaring a seam field here would be declaring a source for a byte
/// that has one already.
pub const SERIES_ACTIVATION_TAIL_FIELDS_V1: [ActivationTailFieldV1; 0] = [];

/// Chain-selected Series release facts the activation descriptor inherits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesActivationBundleInputV1<'a> {
    /// Exact bytes of any one of the release's five action descriptors.
    ///
    /// The manifest-selected coordinates are read off it rather than restated;
    /// `compile_series_release_v5` has already required every action to agree.
    pub action_descriptor: &'a [u8],
    /// Exact finalized Series Template record the release binds as its config.
    pub template: &'a [u8],
    /// Compartment rows the founding provisions in the selected funding ledger.
    pub funding_ledger_slot_count: u16,
}

/// Stable Series activation construction or validation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesActivationBundleErrorV1 {
    /// The supplied action descriptor did not decode as a `CapabilityProgramV4`.
    ActionDescriptor,
    /// The action descriptor did not name Series' kind, config schema, root
    /// schema, derivation policy, or the exact `SERIES_STATE_BYTES_V3` width.
    ForeignRoot,
    /// The supplied Template did not decode under the current V3 record schema.
    TemplateRecord,
    /// **The completeness gate.** The supplied Template is not the one this
    /// release binds, so the composed tail would describe a different Series.
    TemplateSubstitution,
    /// The kernel's own creation oracle refused this Template.
    CreationOracle(SeriesActivationErrorV3),
    /// Activation selector request bytes were noncanonical.
    Request,
    /// ProgramSet construction, count, order, or self-selection refused.
    ProgramSet,
    /// The projected root tail was not the Template's canonical initial state.
    ProjectedState,
    /// The family-neutral activation template refused.
    Bundle(ActivationBundleErrorV1),
}

/// Result alias for Series activation construction.
pub type SeriesActivationResultV1<T> = core::result::Result<T, SeriesActivationBundleErrorV1>;

/// Whether the family-neutral activation seam can fund one Template's root.
///
/// This is a statement about `outer.rs`, not about the Template. It carries the
/// two quantities whose disagreement is the whole finding so a caller can report
/// the wall rather than rediscover it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesActivationSeamVerdictV1 {
    /// The seam's exact root quote is the whole opening balance the terminal
    /// contract will later demand, so this Template activates and closes.
    Fundable {
        /// Lamports the founding must park, which the seam will move whole.
        parked_quote: u64,
    },
    /// The Template prepays close principal the seam has no way to deliver.
    ///
    /// `outer.rs:1644` requires the activated root to hold exactly
    /// `seam_root_quote`, while
    /// [`dclutch_series_v3_kernel::terminal::plan_series_root_closure_v3`]
    /// requires it to hold `seam_root_quote + prepaid_principal` before it will
    /// classify anything. No release can satisfy both.
    PrepaidPrincipalUnfundable {
        /// Template-authenticated close principal the root must persist.
        prepaid_principal: u64,
        /// The only balance the activation seam will admit.
        seam_root_quote: u64,
    },
}

/// Exact Solana account width of an activated Series composite root.
#[must_use]
pub const fn series_root_account_bytes_v1() -> usize {
    CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3
}

/// State whether the current activation seam can fund this Template's root.
///
/// `exact_root_rent` is the caller's own
/// `Rent::minimum_balance(series_root_account_bytes_v1())`; this module reads no
/// sysvar. The verdict imposes nothing — a caller may build the bundle either
/// way — but a release publisher that ignores it publishes roots that cannot be
/// closed.
pub fn series_activation_seam_funding_verdict_v1(
    template: TemplateV3,
    exact_root_rent: u64,
) -> SeriesActivationSeamVerdictV1 {
    if template.close_rent() == 0 {
        SeriesActivationSeamVerdictV1::Fundable {
            parked_quote: exact_root_rent,
        }
    } else {
        SeriesActivationSeamVerdictV1::PrepaidPrincipalUnfundable {
            prepaid_principal: template.close_rent(),
            seam_root_quote: exact_root_rent,
        }
    }
}

/// Finalized schema of the Series activation selector request.
#[must_use]
pub fn series_activation_request_schema_id_v1() -> [u8; 32] {
    digest(SERIES_ACTIVATION_REQUEST_SCHEMA_PREIMAGE_V1)
}

/// Schema a Series activation descriptor record is finalized under, and the
/// value its `CapabilityProgramSetV2` entry must carry.
#[must_use]
pub const fn series_activation_descriptor_schema_v1() -> [u8; 32] {
    activation_descriptor_schema_v1()
}

/// Encode the sole canonical Series activation selector request.
pub fn series_activation_request_v1()
-> SeriesActivationResultV1<[u8; SERIES_ACTIVATION_REQUEST_BYTES_V1]> {
    let mut output = [0_u8; SERIES_ACTIVATION_REQUEST_BYTES_V1];
    put(&mut output, 0, &SERIES_ACTIVATION_REQUEST_MAGIC_V1)?;
    put(
        &mut output,
        SERIES_ACTIVATION_REQUEST_MAGIC_V1.len(),
        &SERIES_ACTIVATION_REQUEST_VERSION_V1.to_le_bytes(),
    )?;
    let selector_offset = usize::try_from(SERIES_ACTION_SELECTOR_OFFSET_V3)
        .map_err(|_| SeriesActivationBundleErrorV1::Request)?;
    let selector = u8::try_from(SERIES_ACTIVATION_SELECTOR_V5)
        .map_err(|_| SeriesActivationBundleErrorV1::Request)?;
    put(&mut output, selector_offset, &[selector])?;
    Ok(output)
}

/// Hostile-check one exact Series activation selector request.
pub fn validate_series_activation_request_v1(bytes: &[u8]) -> SeriesActivationResultV1<()> {
    if bytes != series_activation_request_v1()? {
        return Err(SeriesActivationBundleErrorV1::Request);
    }
    Ok(())
}

/// Build the canonical Series activation descriptor/profile/effect bundle.
///
/// Returns only after the family-neutral constructor has run the real effect
/// kernel over the effect it built and agreed its projected request buffer is
/// this Template's exact initial state, byte for byte.
pub fn build_series_activation_bundle_v1(
    input: SeriesActivationBundleInputV1<'_>,
) -> SeriesActivationResultV1<ActivationBundleV1> {
    let (inherited, constant_tail) = template_input(input)?;
    let bundle = build_activation_bundle_v1(inherited.as_input(&constant_tail))
        .map_err(SeriesActivationBundleErrorV1::Bundle)?;
    validate_series_activation_bundle_v1(&bundle, input)?;
    Ok(bundle)
}

/// Rejoin one Series activation bundle to the release it inherits from.
pub fn validate_series_activation_bundle_v1(
    bundle: &ActivationBundleV1,
    input: SeriesActivationBundleInputV1<'_>,
) -> SeriesActivationResultV1<()> {
    let (inherited, constant_tail) = template_input(input)?;
    validate_activation_bundle_v1(bundle, inherited.as_input(&constant_tail))
        .map_err(SeriesActivationBundleErrorV1::Bundle)?;
    let descriptor = CapabilityProgramV1::decode(&bundle.descriptor)
        .map_err(|_| SeriesActivationBundleErrorV1::ActionDescriptor)?;
    if descriptor.kind().to_bytes() != digest(SERIES_SUCCESSOR_KIND_PREIMAGE_V3)
        || descriptor.config_schema().to_bytes() != SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3
        || descriptor.root_schema().to_bytes() != digest(SERIES_ROOT_SCHEMA_PREIMAGE_V3)
        || descriptor.derivation_policy().to_bytes() != digest(SERIES_TICKET_DERIVATION_PREIMAGE_V3)
        || descriptor.capacity_profile() != inherited.capacity_profile
        || descriptor.request_schema().to_bytes() != series_activation_request_schema_id_v1()
        || usize::try_from(descriptor.root_state_bytes())
            .map_err(|_| SeriesActivationBundleErrorV1::ForeignRoot)?
            != SERIES_STATE_BYTES_V3
        || descriptor
            .root_account_bytes()
            .map_err(|_| SeriesActivationBundleErrorV1::ForeignRoot)?
            != series_root_account_bytes_v1()
    {
        return Err(SeriesActivationBundleErrorV1::ForeignRoot);
    }
    // Read the OUTPUT, not the instruction list: run the real artifacts and
    // require the family's own decoder to accept what the outer would write.
    let projected = project_series_activation_root_state_v1(bundle, input, 2_672_640)?;
    if projected != SeriesStateV3::new(decode_template(input)?.close_rent()) {
        return Err(SeriesActivationBundleErrorV1::ProjectedState);
    }
    Ok(())
}

/// Run the real activation artifacts and decode the root this bundle creates.
///
/// This is how a Series activation is reviewed. The answer comes back through
/// [`SeriesStateV3::decode`] under the Template's own occurrence count, so a
/// tail of the right width and the wrong content refuses here and never reaches
/// an account. The seam's projected root balance is required to be exactly the
/// parked quote at the same time, which is the statement
/// `outer.rs:1644` will independently make on chain.
pub fn project_series_activation_root_state_v1(
    bundle: &ActivationBundleV1,
    input: SeriesActivationBundleInputV1<'_>,
    rent_quote: u64,
) -> SeriesActivationResultV1<SeriesStateV3> {
    let template = decode_template(input)?;
    let scalars = [0_u64; ACTIVATION_COMMON_SCALARS_V2];
    let identities = [[0_u8; 32]; ACTIVATION_COMMON_IDENTITIES_V2];
    let (tail, lamports) = project_activation_root_tail_v1(
        bundle,
        ActivationSeamImageV1 {
            scalars: &scalars,
            identities: &identities,
            rent_quote,
        },
    )
    .map_err(SeriesActivationBundleErrorV1::Bundle)?;
    if lamports != [rent_quote, 0] {
        return Err(SeriesActivationBundleErrorV1::Bundle(
            ActivationBundleErrorV1::ProjectedRentMismatch,
        ));
    }
    SeriesStateV3::decode(&tail, template.occurrence_count())
        .map_err(|_| SeriesActivationBundleErrorV1::ProjectedState)
}

/// Build the exact six-entry activation-capable current Series release set.
///
/// Five V4 action coordinates in selector order, then the one V1 activation
/// coordinate that makes the release activatable at all. The builder is
/// self-refuting: it re-decodes the bytes it just wrote, requires the canonical
/// activation request to select exactly the descriptor it was handed, and
/// requires every action selector to reach a V4 entry instead.
pub fn build_series_activation_capable_program_set_v1(
    action_descriptor_ids: &[[u8; 32]],
    activation_descriptor_id: [u8; 32],
) -> SeriesActivationResultV1<Vec<u8>> {
    if action_descriptor_ids.len() != SERIES_RELEASE_ACTION_COUNT_V5 {
        return Err(SeriesActivationBundleErrorV1::ProgramSet);
    }
    let mut entries = Vec::with_capacity(SERIES_RELEASE_ENTRY_COUNT_V5);
    for (index, descriptor) in action_descriptor_ids.iter().copied().enumerate() {
        entries.push(CapabilityProgramSetEntryV2::new(
            u32::try_from(index).map_err(|_| SeriesActivationBundleErrorV1::ProgramSet)?,
            CapabilityDescriptorReferenceV2::new(
                content(CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID)?,
                content(descriptor)?,
            ),
        ));
    }
    entries.push(CapabilityProgramSetEntryV2::new(
        SERIES_ACTIVATION_SELECTOR_V5,
        CapabilityDescriptorReferenceV2::new(
            content(series_activation_descriptor_schema_v1())?,
            content(activation_descriptor_id)?,
        ),
    ));
    let mut output = vec![
        0_u8;
        encoded_program_set_bytes_v2(entries.len())
            .map_err(|_| SeriesActivationBundleErrorV1::ProgramSet)?
    ];
    encode_program_set_v2(
        SERIES_ACTION_SELECTOR_OFFSET_V3,
        SelectorWidthV2::U8,
        &entries,
        &mut output,
    )
    .map_err(|_| SeriesActivationBundleErrorV1::ProgramSet)?;

    let set = CapabilityProgramSetV2::decode(&output)
        .map_err(|_| SeriesActivationBundleErrorV1::ProgramSet)?;
    if set.entry_count()
        != u16::try_from(SERIES_RELEASE_ENTRY_COUNT_V5)
            .map_err(|_| SeriesActivationBundleErrorV1::ProgramSet)?
        || set.selector_offset() != SERIES_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U8
    {
        return Err(SeriesActivationBundleErrorV1::ProgramSet);
    }
    // The canonical activation request reaches the V1 coordinate and nothing
    // else, and no action selector can reach it.
    let selected = set
        .select_descriptor(&series_activation_request_v1()?)
        .map_err(|_| SeriesActivationBundleErrorV1::ProgramSet)?;
    if selected.schema().to_bytes() != series_activation_descriptor_schema_v1()
        || selected.program().to_bytes() != activation_descriptor_id
    {
        return Err(SeriesActivationBundleErrorV1::ProgramSet);
    }
    for (index, expected) in action_descriptor_ids.iter().copied().enumerate() {
        let mut request = series_activation_request_v1()?;
        let selector_offset = usize::try_from(SERIES_ACTION_SELECTOR_OFFSET_V3)
            .map_err(|_| SeriesActivationBundleErrorV1::ProgramSet)?;
        put(
            &mut request,
            selector_offset,
            &[u8::try_from(index).map_err(|_| SeriesActivationBundleErrorV1::ProgramSet)?],
        )?;
        let action = set
            .select_descriptor(&request)
            .map_err(|_| SeriesActivationBundleErrorV1::ProgramSet)?;
        if action.schema().to_bytes() != CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID
            || action.program().to_bytes() != expected
        {
            return Err(SeriesActivationBundleErrorV1::ProgramSet);
        }
    }
    Ok(output)
}

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
            seam_fields: &SERIES_ACTIVATION_TAIL_FIELDS_V1,
            funding_ledger_slot_count: self.funding_ledger_slot_count,
        }
    }
}

fn decode_template(
    input: SeriesActivationBundleInputV1<'_>,
) -> SeriesActivationResultV1<TemplateV3> {
    TemplateV3::decode(input.template).map_err(|_| SeriesActivationBundleErrorV1::TemplateRecord)
}

fn template_input(
    input: SeriesActivationBundleInputV1<'_>,
) -> SeriesActivationResultV1<(InheritedV1, [u8; SERIES_STATE_BYTES_V3])> {
    let action = CapabilityProgramV4::decode(input.action_descriptor)
        .map_err(|_| SeriesActivationBundleErrorV1::ActionDescriptor)?;
    if action.kind().to_bytes() != digest(SERIES_SUCCESSOR_KIND_PREIMAGE_V3)
        || action.config_schema().to_bytes() != SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3
        || action.root_schema().to_bytes() != digest(SERIES_ROOT_SCHEMA_PREIMAGE_V3)
        || action.derivation_policy().to_bytes() != digest(SERIES_TICKET_DERIVATION_PREIMAGE_V3)
        || usize::try_from(action.root_state_bytes())
            .map_err(|_| SeriesActivationBundleErrorV1::ForeignRoot)?
            != SERIES_STATE_BYTES_V3
    {
        return Err(SeriesActivationBundleErrorV1::ForeignRoot);
    }
    let template = decode_template(input)?;
    let template_id = template_content_id(input.template)
        .map_err(|_| SeriesActivationBundleErrorV1::TemplateRecord)?;
    // The completeness gate: the release's own descriptors carry the exact
    // Template identity in `capacity_profile`, and the whole tail is derived
    // from that Template. Composing from any other record is a substitution.
    if action.capacity_profile() != template_id {
        return Err(SeriesActivationBundleErrorV1::TemplateSubstitution);
    }
    let constant_tail = series_activation_root_tail_v3(template)
        .map_err(SeriesActivationBundleErrorV1::CreationOracle)?;
    Ok((
        InheritedV1 {
            kind: action.kind(),
            config_schema: action.config_schema(),
            root_schema: action.root_schema(),
            derivation_policy: action.derivation_policy(),
            capacity_profile: action.capacity_profile(),
            root_state_bytes: action.root_state_bytes(),
            funding_ledger_slot_count: input.funding_ledger_slot_count,
            request_schema: content(series_activation_request_schema_id_v1())?,
        },
        constant_tail,
    ))
}

fn content(bytes: [u8; 32]) -> SeriesActivationResultV1<ContentId> {
    ContentId::new(bytes).map_err(|_| SeriesActivationBundleErrorV1::ForeignRoot)
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) -> SeriesActivationResultV1<()> {
    let end = offset
        .checked_add(source.len())
        .ok_or(SeriesActivationBundleErrorV1::Request)?;
    output
        .get_mut(offset..end)
        .ok_or(SeriesActivationBundleErrorV1::Request)?
        .copy_from_slice(source);
    Ok(())
}

#[cfg(test)]
mod tests {
    use dclutch_capability_program_contract::{
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, set_v2::ProgramSetErrorV2,
    };
    use dclutch_series_v3_kernel::{
        AccountKeyV3, generated,
        terminal::{SeriesLifecycleRentSinkV3, SeriesTerminalErrorV3, plan_series_root_closure_v3},
    };

    use super::{
        super::release_v5::{SeriesActionArtifactIdsV5, encode_descriptor},
        *,
    };

    /// The observed Rent-exempt minimum for a 296-byte Series root on a live
    /// bank. Only the exactness of the arithmetic matters here, not the value.
    const ROOT_RENT: u64 = 2_672_640;

    fn wallet() -> AccountKeyV3 {
        AccountKeyV3::new([61; 32]).expect("nonzero")
    }

    fn template_bytes(close_rent: u64) -> [u8; generated::SERIES_TEMPLATE_BYTES_V3] {
        let mut bytes = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        put(
            &mut bytes,
            generated::SERIES_TEMPLATE_CLOSE_RENT_OFFSET_V3,
            &close_rent.to_le_bytes(),
        )
        .expect("close rent");
        put(
            &mut bytes,
            generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3,
            &wallet().to_bytes(),
        )
        .expect("refund owner");
        bytes
    }

    fn ids(seed: u8) -> SeriesActionArtifactIdsV5 {
        SeriesActionArtifactIdsV5 {
            account_profile: [seed; 32],
            request_profile: [seed.wrapping_add(1); 32],
            lifecycle: [seed.wrapping_add(2); 32],
            strategy: [seed.wrapping_add(3); 32],
            transition: [seed.wrapping_add(4); 32],
            effect: [seed.wrapping_add(5); 32],
        }
    }

    /// A real production V4 action descriptor bound to this exact Template.
    fn action_descriptor(template: &[u8], seed: u8) -> Vec<u8> {
        let template_id = template_content_id(template).expect("Template ID");
        encode_descriptor(template_id, ids(seed))
            .expect("action descriptor")
            .to_vec()
    }

    fn input<'a>(
        action_descriptor: &'a [u8],
        template: &'a [u8],
    ) -> SeriesActivationBundleInputV1<'a> {
        SeriesActivationBundleInputV1 {
            action_descriptor,
            template,
            funding_ledger_slot_count: 1,
        }
    }

    fn sink(wallet: AccountKeyV3) -> SeriesLifecycleRentSinkV3 {
        use dclutch_rent_contract::{
            RefundAuthority,
            lifecycle_v2::{LifecycleAccountIdV2, LifecycleRentCreditV2},
        };
        let credit = LifecycleRentCreditV2::new(
            RefundAuthority::new(wallet.to_bytes()).expect("wallet"),
            LifecycleAccountIdV2::new([31; 32]).expect("Market"),
            LifecycleAccountIdV2::new([32; 32]).expect("release"),
            7,
            9,
        )
        .expect("credit");
        SeriesLifecycleRentSinkV3::admit(
            AccountKeyV3::new([30; 32]).expect("credit account"),
            &credit.to_bytes(),
            AccountKeyV3::new([31; 32]).expect("Market"),
            ContentId::new([32; 32]).expect("release set"),
            7,
            wallet,
        )
        .expect("sink")
    }

    fn terminal_state(template: TemplateV3) -> SeriesStateV3 {
        let tail = series_activation_root_tail_v3(template).expect("tail");
        let mut state =
            SeriesStateV3::decode(&tail, template.occurrence_count()).expect("decode tail");
        for _ in 0..template.occurrence_count() {
            state = state
                .prepare_ticket(state.revision())
                .expect("prepare")
                .settle_current(
                    state.revision().wrapping_add(1),
                    template.occurrence_count(),
                )
                .expect("settle");
            state = state.retire_ticket(state.revision()).expect("retire");
        }
        state
    }

    #[test]
    fn the_activation_coordinate_carries_a_schema_no_action_entry_can_carry() {
        // The whole of the wall `authenticate_set_descriptor` enforces.
        assert_ne!(
            CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
            CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(
            series_activation_descriptor_schema_v1(),
            CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1
        );
    }

    #[test]
    fn the_projected_root_is_the_templates_own_initial_state() {
        for close_rent in [0_u64, 1, 7, 5_000, 900_000] {
            let template_bytes = template_bytes(close_rent);
            let descriptor = action_descriptor(&template_bytes, 40);
            let input = input(&descriptor, &template_bytes);
            let bundle = build_series_activation_bundle_v1(input).expect("bundle");
            let state =
                project_series_activation_root_state_v1(&bundle, input, ROOT_RENT).expect("state");
            assert_eq!(state, SeriesStateV3::new(close_rent));
            assert_eq!(state.close_rent_remaining(), close_rent);
            // The descriptor is a V1 activation descriptor over the exact width
            // the outer will allocate.
            let program = CapabilityProgramV1::decode(&bundle.descriptor).expect("descriptor");
            assert_eq!(usize::try_from(program.root_state_bytes()), Ok(64_usize));
            assert_eq!(
                program.root_account_bytes(),
                Ok(series_root_account_bytes_v1())
            );
            assert_eq!(
                program.request_schema().to_bytes(),
                series_activation_request_schema_id_v1()
            );
        }
    }

    #[test]
    fn a_substituted_template_refuses_at_the_completeness_gate() {
        let bound = template_bytes(7);
        let other = template_bytes(8);
        let descriptor = action_descriptor(&bound, 40);
        assert_eq!(
            build_series_activation_bundle_v1(input(&descriptor, &other)),
            Err(SeriesActivationBundleErrorV1::TemplateSubstitution)
        );
        // And a bundle honestly built for the bound Template does not validate
        // against the substituted one.
        let bundle = build_series_activation_bundle_v1(input(&descriptor, &bound)).expect("bundle");
        assert_eq!(
            validate_series_activation_bundle_v1(&bundle, input(&descriptor, &other)),
            Err(SeriesActivationBundleErrorV1::TemplateSubstitution)
        );
    }

    #[test]
    fn a_foreign_or_malformed_action_descriptor_refuses_before_any_artifact() {
        let template = template_bytes(7);
        let descriptor = action_descriptor(&template, 40);
        let mut foreign = descriptor.clone();
        // Byte 16 begins the descriptor's `kind` coordinate.
        *foreign.get_mut(16).expect("kind coordinate") ^= 0xff;
        assert_eq!(
            build_series_activation_bundle_v1(input(&foreign, &template)),
            Err(SeriesActivationBundleErrorV1::ForeignRoot)
        );
        assert_eq!(
            build_series_activation_bundle_v1(input(&[0_u8; 8], &template)),
            Err(SeriesActivationBundleErrorV1::ActionDescriptor)
        );
        assert_eq!(
            build_series_activation_bundle_v1(input(&descriptor, &[0_u8; 8])),
            Err(SeriesActivationBundleErrorV1::TemplateRecord)
        );
    }

    #[test]
    fn the_canonical_activation_request_is_the_only_one_admitted() {
        let request = series_activation_request_v1().expect("request");
        assert_eq!(validate_series_activation_request_v1(&request), Ok(()));
        assert_eq!(
            request.get(usize::try_from(SERIES_ACTION_SELECTOR_OFFSET_V3).expect("offset")),
            Some(&255_u8)
        );
        for index in 0..request.len() {
            let mut hostile = request;
            *hostile.get_mut(index).expect("request byte") ^= 0xff;
            assert_eq!(
                validate_series_activation_request_v1(&hostile),
                Err(SeriesActivationBundleErrorV1::Request),
                "byte {index} was not load bearing"
            );
        }
        assert_eq!(
            validate_series_activation_request_v1(request.get(..15).expect("short request")),
            Err(SeriesActivationBundleErrorV1::Request)
        );
    }

    #[test]
    fn only_an_activation_capable_set_can_reach_a_v1_descriptor() {
        let template = template_bytes(0);
        let descriptor = action_descriptor(&template, 40);
        let bundle =
            build_series_activation_bundle_v1(input(&descriptor, &template)).expect("bundle");
        let actions: Vec<[u8; 32]> = (0..SERIES_RELEASE_ACTION_COUNT_V5)
            .map(|index| {
                digest(&action_descriptor(
                    &template,
                    u8::try_from(index).expect("seed").wrapping_add(50),
                ))
            })
            .collect();
        let set_bytes =
            build_series_activation_capable_program_set_v1(&actions, bundle.descriptor_id)
                .expect("set");
        let set = CapabilityProgramSetV2::decode(&set_bytes).expect("decode set");
        assert_eq!(set.entry_count(), 6);

        // The current five-action set the compiler emits today cannot select an
        // activation descriptor at all: this is the exact refusal a Series
        // Market founded from it hits, before any width or effect check.
        let five: Vec<CapabilityProgramSetEntryV2> = actions
            .iter()
            .copied()
            .enumerate()
            .map(|(index, program)| {
                CapabilityProgramSetEntryV2::new(
                    u32::try_from(index).expect("selector"),
                    CapabilityDescriptorReferenceV2::new(
                        ContentId::new(CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID).expect("schema"),
                        ContentId::new(program).expect("program"),
                    ),
                )
            })
            .collect();
        let mut five_bytes =
            vec![0_u8; encoded_program_set_bytes_v2(five.len()).expect("five width")];
        encode_program_set_v2(
            SERIES_ACTION_SELECTOR_OFFSET_V3,
            SelectorWidthV2::U8,
            &five,
            &mut five_bytes,
        )
        .expect("five entries");
        assert_eq!(
            CapabilityProgramSetV2::decode(&five_bytes)
                .expect("decode five")
                .select_descriptor(&series_activation_request_v1().expect("request"))
                .map(|reference| reference.program().to_bytes()),
            Err(ProgramSetErrorV2::MissingSelector)
        );

        // A caller cannot obtain a set whose action count is wrong.
        assert_eq!(
            build_series_activation_capable_program_set_v1(
                actions.get(..4).expect("four actions"),
                bundle.descriptor_id,
            ),
            Err(SeriesActivationBundleErrorV1::ProgramSet)
        );
    }

    #[test]
    fn the_seam_root_quote_is_the_only_balance_activation_may_move() {
        // Zero principal: the seam's exact quote is the whole opening balance,
        // and the terminal contract closes on it.
        let zero = TemplateV3::decode(&template_bytes(0)).expect("Template");
        assert_eq!(
            series_activation_seam_funding_verdict_v1(zero, ROOT_RENT),
            SeriesActivationSeamVerdictV1::Fundable {
                parked_quote: ROOT_RENT
            }
        );
        let state = terminal_state(zero);
        let closure = plan_series_root_closure_v3(
            zero,
            state,
            state.revision(),
            ROOT_RENT,
            ROOT_RENT,
            sink(wallet()),
        )
        .expect("closure");
        assert_eq!(closure.close_rent(), 0);
        assert_eq!(closure.donation(), 0);
        assert_eq!(closure.total_credit(), Ok(ROOT_RENT));

        // Nonzero principal: the seam admits exactly ROOT_RENT into the root
        // (`outer.rs:1644`), and the terminal contract refuses that balance.
        // The two statements cannot both be satisfied by any release.
        let prepaid = TemplateV3::decode(&template_bytes(5_000)).expect("Template");
        assert_eq!(
            series_activation_seam_funding_verdict_v1(prepaid, ROOT_RENT),
            SeriesActivationSeamVerdictV1::PrepaidPrincipalUnfundable {
                prepaid_principal: 5_000,
                seam_root_quote: ROOT_RENT
            }
        );
        let state = terminal_state(prepaid);
        assert_eq!(
            plan_series_root_closure_v3(
                prepaid,
                state,
                state.revision(),
                ROOT_RENT,
                ROOT_RENT,
                sink(wallet()),
            ),
            Err(SeriesTerminalErrorV3::Balance)
        );
        // The activation still composes the honest tail for that Template: no
        // zero-principal rule is imposed anywhere in this module.
        let template = template_bytes(5_000);
        let descriptor = action_descriptor(&template, 40);
        let bundle =
            build_series_activation_bundle_v1(input(&descriptor, &template)).expect("bundle");
        assert_eq!(
            project_series_activation_root_state_v1(&bundle, input(&descriptor, &template), 1)
                .expect("state")
                .close_rent_remaining(),
            5_000
        );
    }
}
