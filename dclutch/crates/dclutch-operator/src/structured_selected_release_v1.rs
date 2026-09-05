//! Compile the five Structured open-capability actions into one publishable,
//! selectable release.
//!
//! # The house shape, and the two places Structured differs
//!
//! This follows the shape Fractional established and General, Series and
//! Rational repeated: bundles per action, one `CapabilityProgramSetV2`, a
//! canonical publication, and every fact DERIVED rather than supplied. The
//! compiled release is handed to the layer's own admission before it is
//! returned, so a release this function yields is one
//! [`authenticate_open_capability_release_v1`] accepts -- not one that merely
//! encoded without error.
//!
//! FIRST DIFFERENCE: Structured authors no artifacts of its own. Decision 0011
//! §3c ruled that, and it reads like poverty until you ask the fixed-point
//! question -- at which point it is the protection. The five bundles come from
//! the shared open capability layer, which consumes the Structured descriptor
//! for SHAPE and for RUNTIME REFUSALS and never bakes it, and a check that
//! refuses does not move a byte. So Structured arrived at the selection membrane
//! already market-free, the first family that did.
//!
//! SECOND DIFFERENCE, and it removes a hazard rather than adding one: Structured
//! names FEWER free facts than Rational does. Its capability kind and its
//! capacity profile are not caller choices at all -- they are
//! `STRUCTURED_CAPABILITY_KIND_ID_V2` and `STRUCTURED_CAPACITY_PROFILE_ID_V2`,
//! generated from Lean with their domain preimages beside them. A release
//! compiler that took them as parameters would be offering a caller the chance
//! to publish a Structured capability under someone else's kind, which is
//! exactly the hole that let a placeholder `identity(0x10)` pass validation for
//! months.
//!
//! # What a caller must still name
//!
//! Only facts that are genuinely choices: which Realm and release set the
//! capability belongs to, which mutable root schema and width the manifest entry
//! will carry, the representation width `K`, the Product basis its payoff is
//! read against, and the exact per-coordinate state width. Everything else is
//! derived: kind and capacity from the family constants, the lifecycle policy
//! from its own parameterless encoder, each action's account observations from
//! the basis width and the two geometries, and the publication read back off the
//! compiled artifacts.
//!
//! # Nothing here can observe a Market
//!
//! That is the property the whole exercise exists for, and it is structural:
//! [`StructuredSelectedReleaseInputV1`] has no Market field and no type that
//! carries one, so the emitted ProgramSet identity -- which a founded Market's
//! capability manifest names as `release_id`, and whose manifest digest is a
//! SEED of the Market PDA -- is fixed before the Market address exists.

use crate::bearer::RationalOpenCapabilityProgramSetInputV6;
use crate::bearer::{
    OPEN_CAPABILITY_SELECTED_ACTION_COUNT_V1, OpenCapabilityActionArtifactBytesV1,
    OpenCapabilityArtifactReleaseBytesV1, OpenCapabilityArtifactSelectionV1,
    RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3, RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3,
    RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3, RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3,
    RationalOpenSelectedBundleInputV6, RationalOpenSelectedHotBundleV3,
    RationalOpenStructuredHotBundleV3, RationalOpenStructuredSelectedBundleInputV6,
    RationalTerminalAccountProfileInputV3, RationalTerminalHotBundleV3,
    RationalTerminalSelectedBundleInputV6, RepresentationActionV2,
    authenticate_open_capability_release_v1, build_rational_open_capability_program_set_v6,
    build_rational_open_selected_bundle_v6, build_rational_open_structured_selected_bundle_v6,
    build_rational_terminal_selected_bundle_v6, encode_open_capability_lifecycle_policy_v5,
};
use dclutch_claims::structured_kernel::{
    STRUCTURED_CAPABILITY_KIND_ID_V2, STRUCTURED_CAPACITY_PROFILE_ID_V2,
};
use dclutch_custody::token_svm::{TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TokenBehaviorSelectionV2};
use dclutch_registry::release_set::ExecutionRoleV1;
use solana_program::hash::hash;

/// Number of action bundles one selectable Structured release compiles.
pub const STRUCTURED_SELECTED_ACTION_COUNT_V1: usize = OPEN_CAPABILITY_SELECTED_ACTION_COUNT_V1;

/// Canonical Structured publication magic.
pub const STRUCTURED_SELECTED_PUBLICATION_MAGIC_V1: [u8; 8] = *b"DCSTPB01";

/// Implemented Structured publication version.
pub const STRUCTURED_SELECTED_PUBLICATION_VERSION_V1: u16 = 1;

/// Largest representation width one Structured release can dispatch.
///
/// The bound belongs to the open RequestProfile V1 artifact -- its 1,312-byte
/// ceiling over a 29-operation prefix plus eight operations per row -- and is
/// restated here as this compiler's own published limit because
/// [`structured_selected_release_v1`] is what refuses a wider one. A caller
/// should not need a second crate edge to learn the bound this function
/// enforces.
pub const STRUCTURED_MAXIMUM_REPRESENTATION_WIDTH_V1: u32 =
    RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3;

/// Execution role that owns every Structured commit.
///
/// Structured's open actions are dispatched by Trading like every other selected
/// capability; Claims is the callee its effects route to, not the executor the
/// manifest names.
pub const STRUCTURED_EXECUTOR_ROLE_V1: ExecutionRoleV1 = ExecutionRoleV1::Trading;

const PUBLICATION_IDENTITY_START_V1: usize = 16;
/// Identities that are not per-action descriptors.
const PUBLICATION_FIXED_IDENTITY_COUNT_V1: usize = 8;
const PUBLICATION_IDENTITY_COUNT_V1: usize =
    PUBLICATION_FIXED_IDENTITY_COUNT_V1 + STRUCTURED_SELECTED_ACTION_COUNT_V1;
const PUBLICATION_SCALAR_START_V1: usize =
    PUBLICATION_IDENTITY_START_V1 + PUBLICATION_IDENTITY_COUNT_V1 * 32;
const PUBLICATION_SCALAR_BYTES_V1: usize = 4 + 4 + 4 + 4 + 2 + 1 + 1;

/// Exact encoded width of one canonical Structured publication.
///
/// Derived from the field table rather than written beside it, so adding a
/// coordinate cannot leave the declared width describing the previous layout.
pub const STRUCTURED_SELECTED_PUBLICATION_BYTES_V1: usize =
    PUBLICATION_SCALAR_START_V1 + PUBLICATION_SCALAR_BYTES_V1;

const _: () = assert!(
    PUBLICATION_IDENTITY_START_V1 + PUBLICATION_IDENTITY_COUNT_V1 * 32
        == PUBLICATION_SCALAR_START_V1,
    "the publication identity block must end where the scalar block begins"
);
const _: () = assert!(
    PUBLICATION_SCALAR_START_V1 + PUBLICATION_SCALAR_BYTES_V1
        == STRUCTURED_SELECTED_PUBLICATION_BYTES_V1,
    "the publication scalar block must end at the declared width"
);
const _: () = assert!(
    PUBLICATION_IDENTITY_START_V1 >= 10,
    "the publication identity block must clear the magic and version header"
);

/// Complete input for one selectable Structured release.
///
/// No field here is, or contains, a Market. The capability kind and capacity
/// profile are deliberately absent: they are the family's own generated
/// constants, and a parameter for either would let a caller publish a Structured
/// release under another family's identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredSelectedReleaseInputV1<'a> {
    /// Immutable Realm the capability belongs to.
    ///
    /// Also a SEED of the Market PDA this release will be selected by, which is
    /// why naming it here is acyclic rather than circular.
    pub realm: [u8; 32],
    /// Immutable execution release set of the deployed adapter.
    pub release_set: [u8; 32],
    /// Manifest-selected mutable root schema.
    pub root_schema: [u8; 32],
    /// Exact mutable root-tail byte width.
    pub root_state_bytes: u32,
    /// Representation coordinate width `K`.
    pub representation_outcome_count: u32,
    /// Exact per-coordinate Claims state width the item rows observe.
    pub item_state_bytes: u32,
    /// Exact finalized ProductBasisV3 bytes the payoff is read against.
    pub product_basis: &'a [u8],
}

/// Canonical Market-bindable summary of one Structured release.
///
/// Every field is derived from the compiled release. There is no free parameter
/// here: a publication that could be written independently of the bundles it
/// describes would be a second author for the release identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredSelectedPublicationV1 {
    /// Structured capability kind, the manifest entry's `kind_id`.
    pub kind_id: [u8; 32],
    /// ProgramSet identity: the manifest entry's `release_id`.
    pub program_set_id: [u8; 32],
    /// Config identity: the manifest entry's `config_id`.
    pub config_id: [u8; 32],
    /// Selected capacity profile.
    pub capacity_profile: [u8; 32],
    /// Selected mutable root schema.
    pub root_schema: [u8; 32],
    /// Derivation policy every descriptor names.
    pub derivation_policy: [u8; 32],
    /// Immutable Realm the config record binds.
    pub realm: [u8; 32],
    /// Immutable release set the config record binds.
    pub release_set: [u8; 32],
    /// Descriptor identities in canonical action order.
    pub descriptors: [[u8; 32]; STRUCTURED_SELECTED_ACTION_COUNT_V1],
    /// Exact mutable root width.
    pub root_state_bytes: u32,
    /// Representation width `K`.
    pub outcome_count: u32,
    /// Exact Product result width `N`, read off the basis the release names.
    pub product_width: u32,
    /// Byte offset of the action selector inside an open request.
    pub selector_offset: u32,
    /// Number of action-selected coordinates the set declares.
    pub action_count: u16,
    /// Execution role that owns every commit.
    pub executor_role: u8,
    /// Encoded selector width.
    pub selector_width: u8,
}

impl StructuredSelectedPublicationV1 {
    /// Encode the exact canonical publication bytes.
    #[must_use]
    #[allow(clippy::indexing_slicing)]
    pub fn to_bytes(&self) -> [u8; STRUCTURED_SELECTED_PUBLICATION_BYTES_V1] {
        let mut bytes = [0_u8; STRUCTURED_SELECTED_PUBLICATION_BYTES_V1];
        bytes[..8].copy_from_slice(&STRUCTURED_SELECTED_PUBLICATION_MAGIC_V1);
        bytes[8..10].copy_from_slice(&STRUCTURED_SELECTED_PUBLICATION_VERSION_V1.to_le_bytes());
        let mut offset = PUBLICATION_IDENTITY_START_V1;
        for identity in self.identities() {
            bytes[offset..offset + 32].copy_from_slice(&identity);
            offset += 32;
        }
        let mut scalar = PUBLICATION_SCALAR_START_V1;
        bytes[scalar..scalar + 4].copy_from_slice(&self.root_state_bytes.to_le_bytes());
        scalar += 4;
        bytes[scalar..scalar + 4].copy_from_slice(&self.outcome_count.to_le_bytes());
        scalar += 4;
        bytes[scalar..scalar + 4].copy_from_slice(&self.product_width.to_le_bytes());
        scalar += 4;
        bytes[scalar..scalar + 4].copy_from_slice(&self.selector_offset.to_le_bytes());
        scalar += 4;
        bytes[scalar..scalar + 2].copy_from_slice(&self.action_count.to_le_bytes());
        scalar += 2;
        bytes[scalar] = self.executor_role;
        bytes[scalar + 1] = self.selector_width;
        bytes
    }

    /// SHA-256 identity of the canonical publication bytes.
    #[must_use]
    pub fn publication_id(&self) -> [u8; 32] {
        hash(&self.to_bytes()).to_bytes()
    }

    fn identities(&self) -> Vec<[u8; 32]> {
        let mut identities = Vec::with_capacity(PUBLICATION_IDENTITY_COUNT_V1);
        identities.push(self.kind_id);
        identities.push(self.program_set_id);
        identities.push(self.config_id);
        identities.push(self.capacity_profile);
        identities.push(self.root_schema);
        identities.push(self.derivation_policy);
        identities.push(self.realm);
        identities.push(self.release_set);
        identities.extend_from_slice(&self.descriptors);
        identities
    }
}

/// One record a publication chain must finalize in the Registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredPublicationRecordV1<'a> {
    /// Human-readable name of this record's role in the release.
    pub label: &'static str,
    /// Schema/release identity this record is finalized under.
    pub schema: [u8; 32],
    /// Exact semantic bytes.
    pub body: &'a [u8],
}

impl StructuredPublicationRecordV1<'_> {
    /// Content identity of the exact bytes.
    #[must_use]
    pub fn content_id(&self) -> [u8; 32] {
        hash(self.body).to_bytes()
    }
}

/// One compiled, self-authenticated, publishable Structured release.
#[derive(Clone, Debug)]
pub struct StructuredSelectedReleaseV1 {
    /// Denominate and Reconstitute, in canonical action order.
    pub selected: Vec<RationalOpenSelectedHotBundleV3>,
    /// IssueStructured and UnwrapStructured, in canonical action order.
    pub structured: Vec<RationalOpenStructuredHotBundleV3>,
    /// RedeemTerminal.
    pub terminal: RationalTerminalHotBundleV3,
    /// Exact five-entry CapabilityProgramSetV2 bytes.
    pub program_set: Vec<u8>,
    /// Exact immutable config-record bytes.
    pub config: Vec<u8>,
    /// Canonical Market-bindable publication.
    pub publication: StructuredSelectedPublicationV1,
}

/// Stable refusal from Structured release compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredSelectedReleaseErrorV1 {
    /// A named identity was zero, or a required scalar was out of range.
    Input,
    /// A semantic-owner encoder refused an artifact.
    Encoding,
    /// ProgramSet encoding, decoding, or selection refused.
    ProgramSet,
    /// The five-action release admission refused.
    Release,
    /// Publication identities or scalars were not exact.
    Publication,
    /// `dclutch_custody::token_svm` refused; the cause is its own.
    Token(dclutch_custody::token_svm::Error),
    /// `crate::bearer` refused; the cause is its own.
    Bearer(crate::bearer::Error),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    ProgramSetContract(dclutch_market::capability_program::set_v2::ProgramSetErrorV2),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    CapabilityProgram(dclutch_market::capability_program::Error),
    /// `dclutch_product::payoff` refused; the cause is its own.
    ProductBasis(dclutch_product::payoff::runtime_v3::Error),
}

/// Result alias for Structured release compilation.
pub type Result<T> = core::result::Result<T, StructuredSelectedReleaseErrorV1>;

/// Compile the five Structured actions into one publishable release.
pub fn structured_selected_release_v1(
    input: StructuredSelectedReleaseInputV1<'_>,
) -> Result<StructuredSelectedReleaseV1> {
    validate_input(input)?;
    let selection = TokenBehaviorSelectionV2::new(input.realm, input.release_set)
        .map_err(StructuredSelectedReleaseErrorV1::Token)?;
    let lifecycle = encode_open_capability_lifecycle_policy_v5()
        .map_err(StructuredSelectedReleaseErrorV1::Bearer)?;

    let mut selected = Vec::with_capacity(2);
    for action in [
        RepresentationActionV2::Denominate,
        RepresentationActionV2::Reconstitute,
    ] {
        selected.push(compile_selected(input, selection, &lifecycle, action)?);
    }
    let mut structured = Vec::with_capacity(2);
    for action in [
        RepresentationActionV2::IssueStructured,
        RepresentationActionV2::UnwrapStructured,
    ] {
        structured.push(compile_structured(input, selection, &lifecycle, action)?);
    }
    let terminal = compile_terminal(input, selection, &lifecycle)?;

    let set =
        build_rational_open_capability_program_set_v6(RationalOpenCapabilityProgramSetInputV6 {
            token_behavior_selection: selection,
            denominate: selected
                .first()
                .ok_or(StructuredSelectedReleaseErrorV1::Release)?,
            reconstitute: selected
                .get(1)
                .ok_or(StructuredSelectedReleaseErrorV1::Release)?,
            issue_structured: structured
                .first()
                .ok_or(StructuredSelectedReleaseErrorV1::Release)?,
            unwrap_structured: structured
                .get(1)
                .ok_or(StructuredSelectedReleaseErrorV1::Release)?,
            redeem_terminal: &terminal,
        })
        .map_err(StructuredSelectedReleaseErrorV1::Bearer)?;

    let release = StructuredSelectedReleaseV1 {
        selected,
        structured,
        terminal,
        program_set: set.program_set,
        config: set.token_behavior_selection.to_vec(),
        // Placeholder replaced below from the admission's own report, so no
        // publication field is ever written from anything but a joined fact.
        publication: StructuredSelectedPublicationV1 {
            kind_id: [0; 32],
            program_set_id: [0; 32],
            config_id: [0; 32],
            capacity_profile: [0; 32],
            root_schema: [0; 32],
            derivation_policy: [0; 32],
            realm: [0; 32],
            release_set: [0; 32],
            descriptors: [[0; 32]; STRUCTURED_SELECTED_ACTION_COUNT_V1],
            root_state_bytes: 0,
            outcome_count: 0,
            product_width: 0,
            selector_offset: 0,
            action_count: 0,
            executor_role: 0,
            selector_width: 0,
        },
    };
    let publication = publish(&release, input)?;
    Ok(StructuredSelectedReleaseV1 {
        publication,
        ..release
    })
}

impl StructuredSelectedReleaseV1 {
    /// The artifact selection an on-chain admission authenticates against.
    ///
    /// All three identities a Market's manifest entry names, so the admission
    /// this feeds is checking the release against exactly what was bound.
    #[must_use]
    pub fn selection(&self) -> OpenCapabilityArtifactSelectionV1 {
        OpenCapabilityArtifactSelectionV1 {
            kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
            program_set: hash(&self.program_set).to_bytes(),
            config: hash(&self.config).to_bytes(),
        }
    }

    /// The untrusted-bytes view the layer's admission consumes.
    #[must_use]
    pub fn artifact_bytes(&self) -> Option<OpenCapabilityArtifactReleaseBytesV1<'_>> {
        let selected = |index: usize, action| {
            self.selected
                .get(index)
                .map(|bundle| OpenCapabilityActionArtifactBytesV1 {
                    action,
                    descriptor: &bundle.descriptor,
                    account_profile: &bundle.account_profile,
                    request_profile: &bundle.request_profile,
                    lifecycle_policy: &bundle.lifecycle_policy,
                    strategy: &bundle.strategy,
                    transition: &bundle.transition,
                    effect: &bundle.effect,
                })
        };
        let structured = |index: usize, action| {
            self.structured
                .get(index)
                .map(|bundle| OpenCapabilityActionArtifactBytesV1 {
                    action,
                    descriptor: &bundle.descriptor,
                    account_profile: &bundle.account_profile,
                    request_profile: &bundle.request_profile,
                    lifecycle_policy: &bundle.lifecycle_policy,
                    strategy: &bundle.strategy,
                    transition: &bundle.transition,
                    effect: &bundle.effect,
                })
        };
        Some(OpenCapabilityArtifactReleaseBytesV1 {
            program_set: &self.program_set,
            config: &self.config,
            actions: [
                selected(0, RepresentationActionV2::Denominate)?,
                selected(1, RepresentationActionV2::Reconstitute)?,
                structured(0, RepresentationActionV2::IssueStructured)?,
                structured(1, RepresentationActionV2::UnwrapStructured)?,
                OpenCapabilityActionArtifactBytesV1 {
                    action: RepresentationActionV2::RedeemTerminal,
                    descriptor: &self.terminal.descriptor,
                    account_profile: &self.terminal.account_profile,
                    request_profile: &self.terminal.request_profile,
                    lifecycle_policy: &self.terminal.lifecycle_policy,
                    strategy: &self.terminal.strategy,
                    transition: &self.terminal.transition,
                    effect: &self.terminal.effect,
                },
            ],
        })
    }

    /// Enumerate every record the Registry must hold for this release.
    ///
    /// Each schema is READ OFF the artifact that names it, never restated, so a
    /// publication plan cannot finalize a record under a schema the release does
    /// not actually select.
    pub fn publication_records(&self) -> Result<Vec<StructuredPublicationRecordV1<'_>>> {
        use dclutch_market::capability_program::{
            set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
            v4::CapabilityProgramV4,
        };
        let set = CapabilityProgramSetV2::decode(&self.program_set)
            .map_err(StructuredSelectedReleaseErrorV1::ProgramSetContract)?;
        let bytes = self
            .artifact_bytes()
            .ok_or(StructuredSelectedReleaseErrorV1::Release)?;
        let first = CapabilityProgramV4::decode(
            bytes
                .actions
                .first()
                .ok_or(StructuredSelectedReleaseErrorV1::Release)?
                .descriptor,
        )
        .map_err(StructuredSelectedReleaseErrorV1::CapabilityProgram)?;

        let mut records = Vec::new();
        records.push(StructuredPublicationRecordV1 {
            label: "program-set",
            schema: CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            body: &self.program_set,
        });
        records.push(StructuredPublicationRecordV1 {
            label: "config",
            schema: first.config_schema().to_bytes(),
            body: &self.config,
        });
        for (ordinal, supplied) in bytes.actions.into_iter().enumerate() {
            let entry = set
                .entry(
                    u16::try_from(ordinal)
                        .map_err(|_| StructuredSelectedReleaseErrorV1::ProgramSet)?,
                )
                .map_err(StructuredSelectedReleaseErrorV1::ProgramSetContract)?;
            let descriptor = CapabilityProgramV4::decode(supplied.descriptor)
                .map_err(StructuredSelectedReleaseErrorV1::CapabilityProgram)?;
            let artifacts = descriptor.artifacts();
            for (label, schema, body) in [
                (
                    "descriptor",
                    entry.descriptor().schema().to_bytes(),
                    supplied.descriptor,
                ),
                (
                    "account-profile",
                    artifacts.account_profile.schema().to_bytes(),
                    supplied.account_profile,
                ),
                (
                    "request-profile",
                    artifacts.request_profile.schema().to_bytes(),
                    supplied.request_profile,
                ),
                (
                    "lifecycle-policy",
                    artifacts.lifecycle.schema().to_bytes(),
                    supplied.lifecycle_policy,
                ),
                (
                    "strategy",
                    artifacts.strategy.schema().to_bytes(),
                    supplied.strategy,
                ),
                (
                    "transition",
                    artifacts.transition.schema().to_bytes(),
                    supplied.transition,
                ),
                (
                    "effect",
                    artifacts.effect.schema().to_bytes(),
                    supplied.effect,
                ),
            ] {
                records.push(StructuredPublicationRecordV1 {
                    label,
                    schema,
                    body,
                });
            }
        }
        Ok(records)
    }
}

/// Run the layer's admission and read the publication off what it joined.
///
/// The publication is built from the ADMISSION'S report rather than from the
/// compiler's own variables, so a fact the admission did not establish cannot
/// reach the manifest.
fn publish(
    release: &StructuredSelectedReleaseV1,
    input: StructuredSelectedReleaseInputV1<'_>,
) -> Result<StructuredSelectedPublicationV1> {
    use dclutch_market::capability_program::set_v2::{CapabilityProgramSetV2, SelectorWidthV2};
    use dclutch_product::payoff::runtime_v3::ProductBasisV3;

    let bytes = release
        .artifact_bytes()
        .ok_or(StructuredSelectedReleaseErrorV1::Release)?;
    let joined = authenticate_open_capability_release_v1(release.selection(), bytes)
        .map_err(StructuredSelectedReleaseErrorV1::Bearer)?;
    let set = CapabilityProgramSetV2::decode(&release.program_set)
        .map_err(StructuredSelectedReleaseErrorV1::ProgramSetContract)?;
    // The Product width is read off the basis the release actually names, not
    // taken as a scalar beside it: a publication stating a width the basis
    // contradicts would be a second author for the payoff geometry.
    let basis = ProductBasisV3::decode(input.product_basis)
        .map_err(StructuredSelectedReleaseErrorV1::ProductBasis)?;
    let publication = StructuredSelectedPublicationV1 {
        kind_id: joined.kind,
        program_set_id: hash(&release.program_set).to_bytes(),
        config_id: hash(&release.config).to_bytes(),
        capacity_profile: joined.capacity_profile,
        root_schema: joined.root_schema,
        derivation_policy: joined.derivation_policy,
        realm: joined.realm,
        release_set: joined.release_set,
        descriptors: joined.descriptors,
        root_state_bytes: joined.root_state_bytes,
        outcome_count: input.representation_outcome_count,
        product_width: basis.basis_width(),
        selector_offset: set.selector_offset(),
        action_count: set.entry_count(),
        executor_role: STRUCTURED_EXECUTOR_ROLE_V1 as u8,
        selector_width: match set.selector_width() {
            SelectorWidthV2::U8 => 1,
            SelectorWidthV2::U16 => 2,
            SelectorWidthV2::U32 => 4,
        },
    };
    if publication.kind_id != STRUCTURED_CAPABILITY_KIND_ID_V2
        || publication.capacity_profile != STRUCTURED_CAPACITY_PROFILE_ID_V2
        || publication.realm != input.realm
        || publication.release_set != input.release_set
        || publication.root_schema != input.root_schema
        || publication.root_state_bytes != input.root_state_bytes
        || usize::from(publication.action_count) != STRUCTURED_SELECTED_ACTION_COUNT_V1
    {
        return Err(StructuredSelectedReleaseErrorV1::Publication);
    }
    Ok(publication)
}

fn validate_input(input: StructuredSelectedReleaseInputV1<'_>) -> Result<()> {
    if [input.realm, input.release_set, input.root_schema]
        .into_iter()
        .any(|identity| identity == [0; 32])
        || input.realm == input.release_set
        || input.root_state_bytes == 0
        || input.representation_outcome_count == 0
        // The open RequestProfile V1 artifact's 1,312-byte bound makes this the
        // largest executable geometry; a wider release would encode and then
        // refuse at its first dispatch.
        || input.representation_outcome_count > RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3
        || input.item_state_bytes == 0
        || input.product_basis.is_empty()
    {
        return Err(StructuredSelectedReleaseErrorV1::Input);
    }
    Ok(())
}

/// The four per-coordinate item widths, of which the builder reads exactly one.
///
/// `open_structured_v3.rs:610` marks item rows 1 through 3 opaque and forces
/// their observed width to zero, so only row 0 reaches a byte. Passing zeros for
/// the other three states that fact structurally instead of carrying three
/// literals that look like measurements. The accompanying test pins the
/// deadness, so if those rows ever become live this compiler goes red rather
/// than silently publishing zeros.
fn item_lengths(input: StructuredSelectedReleaseInputV1<'_>) -> [u32; 4] {
    [input.item_state_bytes, 0, 0, 0]
}

/// Account observations for the two selected-coordinate actions, derived not
/// supplied: the basis body is observed at its own coordinate and at its alias.
fn selected_lengths(input: StructuredSelectedReleaseInputV1<'_>) -> Result<Vec<u32>> {
    let mut lengths = vec![0_u32; usize::from(RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3)];
    let width = basis_width(input)?;
    *lengths
        .get_mut(4)
        .ok_or(StructuredSelectedReleaseErrorV1::Encoding)? = width;
    *lengths
        .get_mut(29)
        .ok_or(StructuredSelectedReleaseErrorV1::Encoding)? = width;
    Ok(lengths)
}

/// Fixed account observations for the two structured actions.
fn structured_lengths(input: StructuredSelectedReleaseInputV1<'_>) -> Result<Vec<u32>> {
    let mut lengths = vec![0_u32; usize::from(RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3)];
    let width = basis_width(input)?;
    *lengths
        .get_mut(4)
        .ok_or(StructuredSelectedReleaseErrorV1::Encoding)? = width;
    *lengths
        .get_mut(29)
        .ok_or(StructuredSelectedReleaseErrorV1::Encoding)? = width;
    Ok(lengths)
}

/// Account observations for terminal redemption, which also observes the config.
fn terminal_lengths(input: StructuredSelectedReleaseInputV1<'_>) -> Result<Vec<u32>> {
    let mut lengths = vec![0_u32; usize::from(RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3)];
    let width = basis_width(input)?;
    *lengths
        .get_mut(1)
        .ok_or(StructuredSelectedReleaseErrorV1::Encoding)? =
        u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2)
            .map_err(|_| StructuredSelectedReleaseErrorV1::Encoding)?;
    *lengths
        .get_mut(4)
        .ok_or(StructuredSelectedReleaseErrorV1::Encoding)? = width;
    *lengths
        .get_mut(29)
        .ok_or(StructuredSelectedReleaseErrorV1::Encoding)? = width;
    Ok(lengths)
}

fn basis_width(input: StructuredSelectedReleaseInputV1<'_>) -> Result<u32> {
    u32::try_from(input.product_basis.len()).map_err(|_| StructuredSelectedReleaseErrorV1::Input)
}

fn compile_selected(
    input: StructuredSelectedReleaseInputV1<'_>,
    selection: TokenBehaviorSelectionV2,
    lifecycle: &[u8],
    action: RepresentationActionV2,
) -> Result<RationalOpenSelectedHotBundleV3> {
    let lengths = selected_lengths(input)?;
    build_rational_open_selected_bundle_v6(RationalOpenSelectedBundleInputV6 {
        action,
        logical_data_lengths: &lengths,
        product_basis: input.product_basis,
        kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
        token_behavior_selection: selection,
        root_schema: input.root_schema,
        lifecycle_policy: lifecycle,
        capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
        root_state_bytes: input.root_state_bytes,
    })
    .map_err(StructuredSelectedReleaseErrorV1::Bearer)
}

fn compile_structured(
    input: StructuredSelectedReleaseInputV1<'_>,
    selection: TokenBehaviorSelectionV2,
    lifecycle: &[u8],
    action: RepresentationActionV2,
) -> Result<RationalOpenStructuredHotBundleV3> {
    let lengths = structured_lengths(input)?;
    build_rational_open_structured_selected_bundle_v6(RationalOpenStructuredSelectedBundleInputV6 {
        action,
        fixed_data_lengths: &lengths,
        item_data_lengths: item_lengths(input),
        product_basis: input.product_basis,
        representation_outcome_count: input.representation_outcome_count,
        token_behavior_selection: selection,
        kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
        root_schema: input.root_schema,
        lifecycle_policy: lifecycle,
        capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
        root_state_bytes: input.root_state_bytes,
    })
    .map_err(StructuredSelectedReleaseErrorV1::Bearer)
}

fn compile_terminal(
    input: StructuredSelectedReleaseInputV1<'_>,
    selection: TokenBehaviorSelectionV2,
    lifecycle: &[u8],
) -> Result<RationalTerminalHotBundleV3> {
    let lengths = terminal_lengths(input)?;
    build_rational_terminal_selected_bundle_v6(RationalTerminalSelectedBundleInputV6 {
        account_profile: RationalTerminalAccountProfileInputV3 {
            logical_data_lengths: &lengths,
            product_basis: input.product_basis,
        },
        kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
        token_behavior_selection: selection,
        root_schema: input.root_schema,
        lifecycle_policy: lifecycle,
        capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
        root_state_bytes: input.root_state_bytes,
    })
    .map_err(StructuredSelectedReleaseErrorV1::Bearer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_product::payoff::runtime_v3::{
        BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
    };

    const PRODUCT_N: u32 = 258;
    const K: u32 = 3;
    const ITEM_STATE_BYTES: u32 = 64;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn basis() -> [u8; BASIS_HEADER_BYTES_V3] {
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: id(1),
                result_domain_id: id(2),
                coordinate_domain_id: id(3),
                result_unit_id: id(4),
                evaluator_release_id: id(5),
                basis_width: PRODUCT_N,
                payout_scale: 1,
                knot_denominator: 1,
                knots: &[],
                terms: &[],
                failure_payouts: &[],
                // Exempt by proof: degree 0 and 1 need no price gate,
                // and a digest offered alongside one is refused.
                price_gate_certificate_digest: [0_u8; 32],
            },
            &mut output,
        )
        .expect("ProductBasisV3");
        output
    }

    fn input(basis: &[u8]) -> StructuredSelectedReleaseInputV1<'_> {
        StructuredSelectedReleaseInputV1 {
            realm: id(18),
            release_set: id(15),
            root_schema: id(42),
            root_state_bytes: 8,
            representation_outcome_count: K,
            item_state_bytes: ITEM_STATE_BYTES,
            product_basis: basis,
        }
    }

    /// The compiled release is one the layer's own admission accepts.
    ///
    /// The compiler runs `authenticate_open_capability_release_v1` before
    /// returning, so this is not merely "it encoded" -- it is "the admission
    /// joined it", over untrusted bytes, with every artifact decoded under its
    /// own type.
    #[test]
    fn the_release_compiles_and_its_own_admission_accepts_it() {
        let basis = basis();
        let release = structured_selected_release_v1(input(&basis)).expect("release");
        let joined = authenticate_open_capability_release_v1(
            release.selection(),
            release.artifact_bytes().expect("artifact bytes"),
        )
        .expect("admission");
        assert_eq!(joined.kind, STRUCTURED_CAPABILITY_KIND_ID_V2);
        assert_eq!(joined.capacity_profile, STRUCTURED_CAPACITY_PROFILE_ID_V2);
        assert_eq!(joined.realm, id(18));
        assert_eq!(joined.release_set, id(15));
        assert_eq!(joined.root_schema, id(42));
        assert_eq!(joined.root_state_bytes, 8);
        assert_eq!(joined.descriptors, release.publication.descriptors);
    }

    /// *** THE SEAM'S INVARIANT, AT COMPILER LEVEL. ***
    ///
    /// Two independent compilations of the same input yield byte-identical
    /// ProgramSet, config and publication. Nothing in the input is or contains a
    /// Market, so the `release_id` a manifest entry names -- and therefore the
    /// manifest digest that is a SEED of the Market PDA -- is fully determined
    /// before any Market exists.
    #[test]
    fn the_release_precedes_the_market_and_is_byte_stable() {
        let basis = basis();
        let first = structured_selected_release_v1(input(&basis)).expect("first");
        let second = structured_selected_release_v1(input(&basis)).expect("second");
        assert_eq!(first.program_set, second.program_set);
        assert_eq!(first.config, second.config);
        assert_eq!(first.publication, second.publication);
        assert_eq!(first.publication.to_bytes(), second.publication.to_bytes());
        assert_eq!(
            first.publication.publication_id(),
            second.publication.publication_id()
        );
    }

    /// The publication encodes at exactly its declared width, and the width is
    /// the one the field table computes.
    #[test]
    fn the_publication_encodes_at_its_declared_width() {
        let basis = basis();
        let release = structured_selected_release_v1(input(&basis)).expect("release");
        let bytes = release.publication.to_bytes();
        assert_eq!(bytes.len(), STRUCTURED_SELECTED_PUBLICATION_BYTES_V1);
        // 8 fixed identities + 5 descriptors, after a 16-byte header, then the
        // scalar block. Stated independently of the constant arithmetic so a
        // change to either has to agree with the other.
        assert_eq!(STRUCTURED_SELECTED_PUBLICATION_BYTES_V1, 16 + 13 * 32 + 20);
        assert_eq!(&bytes[..8], &STRUCTURED_SELECTED_PUBLICATION_MAGIC_V1);
        assert_eq!(
            release.publication.publication_id(),
            hash(&bytes).to_bytes()
        );
    }

    /// Every published record is one the release itself names.
    ///
    /// Two plus seven per action: the ProgramSet, the config, and each action's
    /// descriptor and six artifacts. Every schema is read off the artifact that
    /// names it, so this also pins that no record is finalized under a schema
    /// the release does not select.
    #[test]
    fn every_published_record_is_named_by_the_release() {
        let basis = basis();
        let release = structured_selected_release_v1(input(&basis)).expect("release");
        let records = release.publication_records().expect("records");
        assert_eq!(records.len(), 2 + 7 * STRUCTURED_SELECTED_ACTION_COUNT_V1);

        let program_set = records.first().expect("program-set record");
        assert_eq!(program_set.label, "program-set");
        assert_eq!(program_set.content_id(), release.publication.program_set_id);
        let config = records.get(1).expect("config record");
        assert_eq!(config.label, "config");
        assert_eq!(config.content_id(), release.publication.config_id);

        // The five descriptor records ARE the publication's five descriptors,
        // in canonical action order.
        let descriptors: Vec<[u8; 32]> = records
            .iter()
            .filter(|record| record.label == "descriptor")
            .map(StructuredPublicationRecordV1::content_id)
            .collect();
        assert_eq!(descriptors.as_slice(), &release.publication.descriptors);
    }

    /// *** THE DEADNESS PIN. ***
    ///
    /// `open_structured_v3.rs:610` marks item rows 1 through 3 opaque and
    /// forces their observed width to zero, so of the four per-coordinate item
    /// widths only row 0 reaches a byte. Nine call sites in the tree pass the
    /// literal `[64, 82, 165, 165]`, of which three quarters is decoration.
    ///
    /// This compiler passes `[item_state_bytes, 0, 0, 0]` on that basis, which
    /// is a load-bearing assumption rather than a style choice: if those rows
    /// ever become live, this compiler would silently publish zeros where a real
    /// width belongs, and five descriptor identities plus the ProgramSet
    /// identity above them would change meaning without changing shape.
    ///
    /// So the assumption is measured instead of trusted. Compiling with the
    /// fixture's decorated literal and with zeros must yield byte-identical
    /// bundles. The day that stops being true, this test names it.
    #[test]
    fn the_three_dead_item_rows_really_are_dead() {
        use crate::bearer::build_rational_open_structured_selected_bundle_v6;

        let basis = basis();
        let policy = encode_open_capability_lifecycle_policy_v5().expect("policy");
        let selection = TokenBehaviorSelectionV2::new(id(18), id(15)).expect("selection");
        let fixed = structured_lengths(input(&basis)).expect("fixed lengths");

        let compile = |items: [u32; 4]| {
            build_rational_open_structured_selected_bundle_v6(
                RationalOpenStructuredSelectedBundleInputV6 {
                    action: RepresentationActionV2::IssueStructured,
                    fixed_data_lengths: &fixed,
                    item_data_lengths: items,
                    product_basis: &basis,
                    representation_outcome_count: K,
                    token_behavior_selection: selection,
                    kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
                    root_schema: id(42),
                    lifecycle_policy: &policy,
                    capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
                    root_state_bytes: 8,
                },
            )
            .expect("structured bundle")
        };

        // The fixture's literal, and this compiler's honest form.
        let decorated = compile([ITEM_STATE_BYTES, 82, 165, 165]);
        let zeroed = compile([ITEM_STATE_BYTES, 0, 0, 0]);
        assert_eq!(
            decorated, zeroed,
            "item rows 1..=3 are opaque; if this fails they became live and this \
             compiler must start deriving them"
        );

        // The positive control: row 0 is NOT dead, so the comparison above is a
        // statement about rows 1..=3 rather than about a builder that ignores
        // every item width.
        let moved = compile([ITEM_STATE_BYTES + 1, 0, 0, 0]);
        assert_ne!(
            moved, zeroed,
            "item row 0 must reach the artifacts, or nothing was measured"
        );
    }

    /// Nonsense inputs refuse before anything is encoded.
    #[test]
    fn nonsense_inputs_refuse_before_anything_is_encoded() {
        let basis = basis();
        let cases = [
            StructuredSelectedReleaseInputV1 {
                realm: [0; 32],
                ..input(&basis)
            },
            StructuredSelectedReleaseInputV1 {
                release_set: id(18),
                ..input(&basis)
            },
            StructuredSelectedReleaseInputV1 {
                root_schema: [0; 32],
                ..input(&basis)
            },
            StructuredSelectedReleaseInputV1 {
                root_state_bytes: 0,
                ..input(&basis)
            },
            StructuredSelectedReleaseInputV1 {
                representation_outcome_count: 0,
                ..input(&basis)
            },
            // Wider than the open RequestProfile V1 artifact can dispatch.
            StructuredSelectedReleaseInputV1 {
                representation_outcome_count: RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3 + 1,
                ..input(&basis)
            },
            StructuredSelectedReleaseInputV1 {
                item_state_bytes: 0,
                ..input(&basis)
            },
            StructuredSelectedReleaseInputV1 {
                product_basis: &[],
                ..input(&basis)
            },
        ];
        for case in cases {
            assert_eq!(
                structured_selected_release_v1(case).err(),
                Some(StructuredSelectedReleaseErrorV1::Input)
            );
        }
    }

    /// A release presented under another family's kind refuses.
    ///
    /// This is the hole `validate_rational_open_capability_program_set_v3` left
    /// open -- it reads no kind at all -- made into a refusal. The bytes are a
    /// perfectly good Structured release; only the kind the manifest claims is
    /// wrong, and that is enough.
    #[test]
    fn a_release_selected_under_the_wrong_kind_refuses() {
        let basis = basis();
        let release = structured_selected_release_v1(input(&basis)).expect("release");
        let mut selection = release.selection();
        selection.kind = id(0x10);
        assert!(
            authenticate_open_capability_release_v1(
                selection,
                release.artifact_bytes().expect("artifact bytes"),
            )
            .is_err(),
            "a placeholder kind must not authenticate a Structured release"
        );
        // And the all-zero kind a never-set constant would carry.
        let mut zeroed = release.selection();
        zeroed.kind = [0; 32];
        assert!(
            authenticate_open_capability_release_v1(
                zeroed,
                release.artifact_bytes().expect("artifact bytes"),
            )
            .is_err()
        );
    }

    /// A different representation width is a different release.
    ///
    /// The positive control for every stability assertion above: the compiler is
    /// not simply emitting a constant.
    #[test]
    fn another_representation_is_another_release() {
        let basis = basis();
        let first = structured_selected_release_v1(input(&basis)).expect("first");
        let second = structured_selected_release_v1(StructuredSelectedReleaseInputV1 {
            representation_outcome_count: K - 1,
            ..input(&basis)
        })
        .expect("second");
        assert_ne!(first.program_set, second.program_set);
        assert_ne!(
            first.publication.program_set_id,
            second.publication.program_set_id
        );
        // The config is Realm and release set only, so it does NOT move with the
        // representation -- which is exactly why it is the manifest's config_id.
        assert_eq!(first.config, second.config);
        assert_eq!(first.publication.config_id, second.publication.config_id);
    }
}
