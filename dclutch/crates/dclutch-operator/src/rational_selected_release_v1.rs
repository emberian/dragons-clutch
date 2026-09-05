//! Compile the four Rational lifecycle actions into one publishable,
//! selectable release.
//!
//! # The house shape, and the one place Rational differs
//!
//! This follows the shape Fractional established and General and Series
//! repeated: bundles per action, one `CapabilityProgramSetV2`, a canonical
//! publication, and every fact DERIVED rather than supplied. The compiled
//! release is handed to the family's own admission before it is returned, so a
//! release this function yields is one
//! [`authenticate_rational_release_v1`] accepts -- not one that merely encoded
//! without error.
//!
//! The difference from General is worth stating because it REMOVES a hazard
//! rather than adding one. General's release must name four deployment facts no
//! derivation can know -- an accelerator ArtifactRelease, a compiler release, a
//! toolchain, and translation-validation evidence -- because its strategy is
//! `AdmittedAot` and its certificate asserts a build that really happened.
//! Rational's strategies are `Interpreted`: they select a TransitionVM program
//! by digest and carry no certificate and no admission body at all. So there is
//! no deployment fact for a Rational release to get wrong, and this input has no
//! field that could bind a Market to a toolchain nobody ran.
//!
//! # What a caller must still name
//!
//! Only facts that are genuinely choices: which Realm and release set the
//! capability belongs to, which capacity profile and root schema the manifest
//! entry will carry, the mutable root width, and the representation itself --
//! its ordered coefficients and the Product basis its payoff is read against.
//! Everything else is derived: the kind is the family constant, the config
//! record is built from the Realm and release set, the lifecycle policy comes
//! from its own parameterless encoder, each action's account observations are
//! computed from the action and the two widths, and the publication is read
//! back off the compiled artifacts.
//!
//! # Nothing here can observe a Market
//!
//! That is the property the whole exercise exists for, and it is structural:
//! [`RationalSelectedReleaseInputV1`] has no Market field and no type that
//! carries one, so the emitted ProgramSet identity -- which a founded Market's
//! capability manifest names as `release_id`, and whose manifest digest is a
//! seed of the Market PDA -- is fixed before the Market address exists.

use dclutch_rational_lifecycle_hot_v3::{
    RATIONAL_LIFECYCLE_SELECTED_ACTIONS_V6, RationalActionArtifactBytesV1,
    RationalArtifactReleaseBytesV1, RationalArtifactSelectionV1,
    RationalLifecycleCompactArtifactInputV6, RationalLifecycleCompactBundleInputV6,
    RationalLifecycleCompactBundleV4, RationalLifecycleProgramSetInputV6,
    RationalLifecycleSelectedAccountProfileInputV5, RationalLifecycleSelectedBundleInputV6,
    RationalLifecycleSelectedBundleV6, authenticate_rational_release_v1,
    build_rational_lifecycle_compact_bundle_v6, build_rational_lifecycle_program_set_v6,
    build_rational_lifecycle_selected_bundle_v6, encode_rational_lifecycle_policy_v5,
    lifecycle_logical_account_count_v3,
};
use dclutch_claims::rational_kernel::DESCRIPTOR_HEADER_BYTES;
use dclutch_claims::rational_lifecycle::{
    LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2, LifecycleActionV2,
    RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1,
};
use dclutch_registry::release_set::ExecutionRoleV1;
use dclutch_custody::token_svm::{TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TokenBehaviorSelectionV2};
use solana_program::hash::hash;

/// Number of action bundles one selectable Rational release compiles.
pub const RATIONAL_SELECTED_ACTION_COUNT_V1: usize = 4;

/// Canonical Rational publication magic.
pub const RATIONAL_SELECTED_PUBLICATION_MAGIC_V1: [u8; 8] = *b"DCRLPB01";

/// Implemented Rational publication version.
pub const RATIONAL_SELECTED_PUBLICATION_VERSION_V1: u16 = 1;

/// Execution role that owns every Rational commit.
///
/// Rational's lifecycle Hot path is dispatched by Trading like every other
/// selected capability; Claims is the callee its effects route to, not the
/// executor the manifest names.
pub const RATIONAL_EXECUTOR_ROLE_V1: ExecutionRoleV1 = ExecutionRoleV1::Trading;

const PUBLICATION_IDENTITY_START_V1: usize = 16;
/// Identities that are not per-action descriptors.
const PUBLICATION_FIXED_IDENTITY_COUNT_V1: usize = 8;
const PUBLICATION_IDENTITY_COUNT_V1: usize =
    PUBLICATION_FIXED_IDENTITY_COUNT_V1 + RATIONAL_SELECTED_ACTION_COUNT_V1;
const PUBLICATION_SCALAR_START_V1: usize =
    PUBLICATION_IDENTITY_START_V1 + PUBLICATION_IDENTITY_COUNT_V1 * 32;
const PUBLICATION_SCALAR_BYTES_V1: usize = 4 + 4 + 4 + 4 + 2 + 1 + 1;

/// Exact encoded width of one canonical Rational publication.
///
/// Derived from the field table rather than written beside it, so adding a
/// coordinate cannot leave the declared width describing the previous layout.
pub const RATIONAL_SELECTED_PUBLICATION_BYTES_V1: usize =
    PUBLICATION_SCALAR_START_V1 + PUBLICATION_SCALAR_BYTES_V1;

const _: () = assert!(
    PUBLICATION_IDENTITY_START_V1 + PUBLICATION_IDENTITY_COUNT_V1 * 32
        == PUBLICATION_SCALAR_START_V1,
    "the publication identity block must end where the scalar block begins"
);
const _: () = assert!(
    PUBLICATION_SCALAR_START_V1 + PUBLICATION_SCALAR_BYTES_V1
        == RATIONAL_SELECTED_PUBLICATION_BYTES_V1,
    "the publication scalar block must end at the declared width"
);
const _: () = assert!(
    PUBLICATION_IDENTITY_START_V1 >= 10,
    "the publication identity block must clear the magic and version header"
);

/// Complete input for one selectable Rational release.
///
/// No field here is, or contains, a Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalSelectedReleaseInputV1<'a> {
    /// Immutable Realm the capability belongs to.
    ///
    /// Also a SEED of the Market PDA this release will be selected by, which is
    /// why naming it here is acyclic rather than circular.
    pub realm: [u8; 32],
    /// Immutable execution release set of the deployed adapter.
    pub release_set: [u8; 32],
    /// Manifest-selected physical capacity profile.
    pub capacity_profile: [u8; 32],
    /// Manifest-selected mutable root schema.
    pub root_schema: [u8; 32],
    /// Exact mutable root-tail byte width.
    pub root_state_bytes: u32,
    /// Ordered representation coefficients; its length is `K`.
    pub coefficients: &'a [u64],
    /// Exact finalized ProductBasisV3 bytes the payoff is read against.
    pub product_basis: &'a [u8],
}

/// Canonical Market-bindable summary of one Rational release.
///
/// Every field is derived from the compiled release. There is no free parameter
/// here: a publication that could be written independently of the bundles it
/// describes would be a second author for the release identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalSelectedPublicationV1 {
    /// Rational capability kind, the manifest entry's `kind_id`.
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
    pub descriptors: [[u8; 32]; RATIONAL_SELECTED_ACTION_COUNT_V1],
    /// Exact mutable root width.
    pub root_state_bytes: u32,
    /// Representation width `K`.
    pub outcome_count: u32,
    /// Ordered nonzero support width.
    pub support_count: u32,
    /// Byte offset of the action selector inside a lifecycle request.
    pub selector_offset: u32,
    /// Number of action-selected coordinates the set declares.
    pub action_count: u16,
    /// Execution role that owns every commit.
    pub executor_role: u8,
    /// Encoded selector width.
    pub selector_width: u8,
}

impl RationalSelectedPublicationV1 {
    /// Encode the exact canonical publication bytes.
    #[must_use]
    #[allow(clippy::indexing_slicing)]
    pub fn to_bytes(&self) -> [u8; RATIONAL_SELECTED_PUBLICATION_BYTES_V1] {
        let mut bytes = [0_u8; RATIONAL_SELECTED_PUBLICATION_BYTES_V1];
        bytes[..8].copy_from_slice(&RATIONAL_SELECTED_PUBLICATION_MAGIC_V1);
        bytes[8..10].copy_from_slice(&RATIONAL_SELECTED_PUBLICATION_VERSION_V1.to_le_bytes());
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
        bytes[scalar..scalar + 4].copy_from_slice(&self.support_count.to_le_bytes());
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
pub struct RationalPublicationRecordV1<'a> {
    /// Human-readable name of this record's role in the release.
    pub label: &'static str,
    /// Schema/release identity this record is finalized under.
    pub schema: [u8; 32],
    /// Exact semantic bytes.
    pub body: &'a [u8],
}

impl RationalPublicationRecordV1<'_> {
    /// Content identity of the exact bytes.
    #[must_use]
    pub fn content_id(&self) -> [u8; 32] {
        hash(self.body).to_bytes()
    }
}

/// One compiled, self-authenticated, publishable Rational release.
#[derive(Clone, Debug)]
pub struct RationalSelectedReleaseV1 {
    /// The three fixed-cardinality bundles, in canonical action order.
    pub fixed: Vec<RationalLifecycleSelectedBundleV6>,
    /// The complete-retirement bundle.
    pub compact: RationalLifecycleCompactBundleV4,
    /// Exact four-entry CapabilityProgramSetV2 bytes.
    pub program_set: Vec<u8>,
    /// Exact immutable config-record bytes.
    pub config: Vec<u8>,
    /// Canonical Market-bindable publication.
    pub publication: RationalSelectedPublicationV1,
}

/// Stable refusal from Rational release compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RationalSelectedReleaseErrorV1 {
    /// A named identity was zero, or a required scalar was not positive.
    Input,
    /// A semantic-owner encoder refused an artifact.
    Encoding,
    /// ProgramSet encoding, decoding, or selection refused.
    ProgramSet,
    /// The four-action release admission refused.
    Release,
    /// Publication identities or scalars were not exact.
    Publication,
}

/// Result alias for Rational release compilation.
pub type Result<T> = core::result::Result<T, RationalSelectedReleaseErrorV1>;

/// Compile the four Rational actions into one publishable release.
pub fn rational_selected_release_v1(
    input: RationalSelectedReleaseInputV1<'_>,
) -> Result<RationalSelectedReleaseV1> {
    validate_input(input)?;
    let selection = TokenBehaviorSelectionV2::new(input.realm, input.release_set)
        .map_err(|_| RationalSelectedReleaseErrorV1::Input)?;
    let lifecycle = encode_rational_lifecycle_policy_v5()
        .map_err(|_| RationalSelectedReleaseErrorV1::Encoding)?;

    let mut fixed = Vec::with_capacity(3);
    for action in [
        LifecycleActionV2::ActivateReceipt,
        LifecycleActionV2::ActivateCoordinate,
        LifecycleActionV2::RetireCoordinate,
    ] {
        fixed.push(compile_fixed(input, selection, &lifecycle, action)?);
    }
    let compact = compile_compact(input, selection, &lifecycle)?;

    let set = build_rational_lifecycle_program_set_v6(RationalLifecycleProgramSetInputV6 {
        token_behavior_selection: selection,
        activate_receipt: fixed
            .first()
            .ok_or(RationalSelectedReleaseErrorV1::Release)?,
        activate_coordinate: fixed
            .get(1)
            .ok_or(RationalSelectedReleaseErrorV1::Release)?,
        retire_coordinate: fixed
            .get(2)
            .ok_or(RationalSelectedReleaseErrorV1::Release)?,
        retire_receipt: &compact,
    })
    .map_err(|_| RationalSelectedReleaseErrorV1::ProgramSet)?;

    let release = RationalSelectedReleaseV1 {
        fixed,
        compact,
        program_set: set.program_set,
        config: set.token_behavior_selection.to_vec(),
        // Placeholder replaced below from the admission's own report, so no
        // publication field is ever written from anything but a joined fact.
        publication: RationalSelectedPublicationV1 {
            kind_id: RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1,
            program_set_id: [0; 32],
            config_id: [0; 32],
            capacity_profile: [0; 32],
            root_schema: [0; 32],
            derivation_policy: [0; 32],
            realm: [0; 32],
            release_set: [0; 32],
            descriptors: [[0; 32]; RATIONAL_SELECTED_ACTION_COUNT_V1],
            root_state_bytes: 0,
            outcome_count: 0,
            support_count: 0,
            selector_offset: 0,
            action_count: 0,
            executor_role: 0,
            selector_width: 0,
        },
    };
    let publication = publish(&release, input)?;
    Ok(RationalSelectedReleaseV1 {
        publication,
        ..release
    })
}

impl RationalSelectedReleaseV1 {
    /// The artifact selection an on-chain admission authenticates against.
    #[must_use]
    pub fn selection(&self) -> RationalArtifactSelectionV1 {
        RationalArtifactSelectionV1 {
            program_set: hash(&self.program_set).to_bytes(),
            config: hash(&self.config).to_bytes(),
        }
    }

    /// The untrusted-bytes view the family's admission consumes.
    #[must_use]
    pub fn artifact_bytes(&self) -> Option<RationalArtifactReleaseBytesV1<'_>> {
        let fixed = |index: usize| {
            self.fixed
                .get(index)
                .map(|bundle| RationalActionArtifactBytesV1 {
                    action: bundle.action,
                    descriptor: &bundle.descriptor,
                    account_profile: &bundle.account_profile,
                    request_profile: &bundle.request_profile,
                    lifecycle_policy: &bundle.lifecycle_policy,
                    strategy: &bundle.strategy,
                    transition: &bundle.transition,
                    effect: &bundle.effect,
                })
        };
        Some(RationalArtifactReleaseBytesV1 {
            program_set: &self.program_set,
            config: &self.config,
            actions: [
                fixed(0)?,
                fixed(1)?,
                fixed(2)?,
                RationalActionArtifactBytesV1 {
                    action: LifecycleActionV2::RetireReceipt,
                    descriptor: &self.compact.descriptor,
                    account_profile: &self.compact.account_profile,
                    request_profile: &self.compact.request_profile,
                    lifecycle_policy: &self.compact.lifecycle_policy,
                    strategy: &self.compact.strategy,
                    transition: &self.compact.transition,
                    effect: &self.compact.effect,
                },
            ],
        })
    }

    /// Enumerate every record the Registry must hold for this release.
    ///
    /// Each schema is READ OFF the artifact that names it, never restated, so a
    /// publication plan cannot finalize a record under a schema the release
    /// does not actually select.
    pub fn publication_records(&self) -> Result<Vec<RationalPublicationRecordV1<'_>>> {
        use dclutch_market::capability_program::{
            set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
            v4::CapabilityProgramV4,
        };
        let set = CapabilityProgramSetV2::decode(&self.program_set)
            .map_err(|_| RationalSelectedReleaseErrorV1::ProgramSet)?;
        let bytes = self
            .artifact_bytes()
            .ok_or(RationalSelectedReleaseErrorV1::Release)?;
        let first = CapabilityProgramV4::decode(
            bytes
                .actions
                .first()
                .ok_or(RationalSelectedReleaseErrorV1::Release)?
                .descriptor,
        )
        .map_err(|_| RationalSelectedReleaseErrorV1::Release)?;

        let mut records = Vec::new();
        records.push(RationalPublicationRecordV1 {
            label: "program-set",
            schema: CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            body: &self.program_set,
        });
        records.push(RationalPublicationRecordV1 {
            label: "config",
            schema: first.config_schema().to_bytes(),
            body: &self.config,
        });
        for (ordinal, supplied) in bytes.actions.into_iter().enumerate() {
            let entry = set
                .entry(
                    u16::try_from(ordinal)
                        .map_err(|_| RationalSelectedReleaseErrorV1::ProgramSet)?,
                )
                .map_err(|_| RationalSelectedReleaseErrorV1::ProgramSet)?;
            let descriptor = CapabilityProgramV4::decode(supplied.descriptor)
                .map_err(|_| RationalSelectedReleaseErrorV1::Release)?;
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
                records.push(RationalPublicationRecordV1 {
                    label,
                    schema,
                    body,
                });
            }
        }
        Ok(records)
    }
}

/// Run the family's admission and read the publication off what it joined.
///
/// The publication is built from the ADMISSION'S report rather than from the
/// compiler's own variables, so a fact the admission did not establish cannot
/// reach the manifest.
fn publish(
    release: &RationalSelectedReleaseV1,
    input: RationalSelectedReleaseInputV1<'_>,
) -> Result<RationalSelectedPublicationV1> {
    use dclutch_market::capability_program::set_v2::{CapabilityProgramSetV2, SelectorWidthV2};

    let bytes = release
        .artifact_bytes()
        .ok_or(RationalSelectedReleaseErrorV1::Release)?;
    let joined = authenticate_rational_release_v1(release.selection(), bytes)
        .map_err(|_| RationalSelectedReleaseErrorV1::Release)?;
    let set = CapabilityProgramSetV2::decode(&release.program_set)
        .map_err(|_| RationalSelectedReleaseErrorV1::ProgramSet)?;
    let support_count = u32::try_from(
        input
            .coefficients
            .iter()
            .filter(|coefficient| **coefficient != 0)
            .count(),
    )
    .map_err(|_| RationalSelectedReleaseErrorV1::Input)?;
    let publication = RationalSelectedPublicationV1 {
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
        outcome_count: u32::try_from(input.coefficients.len())
            .map_err(|_| RationalSelectedReleaseErrorV1::Input)?,
        support_count,
        selector_offset: set.selector_offset(),
        action_count: set.entry_count(),
        executor_role: RATIONAL_EXECUTOR_ROLE_V1 as u8,
        selector_width: match set.selector_width() {
            SelectorWidthV2::U8 => 1,
            SelectorWidthV2::U16 => 2,
            SelectorWidthV2::U32 => 4,
        },
    };
    if publication.realm != input.realm
        || publication.release_set != input.release_set
        || publication.capacity_profile != input.capacity_profile
        || publication.root_schema != input.root_schema
        || publication.root_state_bytes != input.root_state_bytes
        || usize::from(publication.action_count) != RATIONAL_SELECTED_ACTION_COUNT_V1
    {
        return Err(RationalSelectedReleaseErrorV1::Publication);
    }
    Ok(publication)
}

fn validate_input(input: RationalSelectedReleaseInputV1<'_>) -> Result<()> {
    if [
        input.realm,
        input.release_set,
        input.capacity_profile,
        input.root_schema,
    ]
    .into_iter()
    .any(|identity| identity == [0; 32])
        || input.realm == input.release_set
        || input.root_state_bytes == 0
        || input.coefficients.is_empty()
        || input.coefficients.iter().all(|value| *value == 0)
        || input.product_basis.is_empty()
    {
        return Err(RationalSelectedReleaseErrorV1::Input);
    }
    Ok(())
}

/// Account observations for one fixed-cardinality action, derived not supplied.
fn fixed_lengths(
    input: RationalSelectedReleaseInputV1<'_>,
    action: LifecycleActionV2,
) -> Result<Vec<u32>> {
    let coordinate_count = u32::from(action != LifecycleActionV2::ActivateReceipt);
    let count = usize::from(
        lifecycle_logical_account_count_v3(action, coordinate_count)
            .map_err(|_| RationalSelectedReleaseErrorV1::Encoding)?,
    );
    lengths(input, count)
}

/// Account observations for complete retirement: one vacancy group per support row.
fn compact_lengths(input: RationalSelectedReleaseInputV1<'_>) -> Result<Vec<u32>> {
    let support = input
        .coefficients
        .iter()
        .filter(|coefficient| **coefficient != 0)
        .count();
    let start = usize::from(
        dclutch_rational_lifecycle_hot_v3::RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3,
    )
    .checked_add(LIFECYCLE_COMMON_ACCOUNT_COUNT_V2)
    .ok_or(RationalSelectedReleaseErrorV1::Input)?;
    let count = support
        .checked_mul(LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2)
        .and_then(|rows| start.checked_add(rows))
        .ok_or(RationalSelectedReleaseErrorV1::Input)?;
    lengths(input, count)
}

fn lengths(input: RationalSelectedReleaseInputV1<'_>, count: usize) -> Result<Vec<u32>> {
    let descriptor_bytes = input
        .coefficients
        .len()
        .checked_mul(8)
        .and_then(|tail| DESCRIPTOR_HEADER_BYTES.checked_add(tail))
        .ok_or(RationalSelectedReleaseErrorV1::Input)?;
    let mut lengths = vec![0_u32; count];
    *lengths
        .get_mut(1)
        .ok_or(RationalSelectedReleaseErrorV1::Encoding)? =
        u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2)
            .map_err(|_| RationalSelectedReleaseErrorV1::Encoding)?;
    *lengths
        .get_mut(4)
        .ok_or(RationalSelectedReleaseErrorV1::Encoding)? =
        u32::try_from(input.product_basis.len())
            .map_err(|_| RationalSelectedReleaseErrorV1::Encoding)?;
    *lengths
        .get_mut(14)
        .ok_or(RationalSelectedReleaseErrorV1::Encoding)? =
        u32::try_from(descriptor_bytes).map_err(|_| RationalSelectedReleaseErrorV1::Encoding)?;
    Ok(lengths)
}

fn compile_fixed(
    input: RationalSelectedReleaseInputV1<'_>,
    selection: TokenBehaviorSelectionV2,
    lifecycle: &[u8],
    action: LifecycleActionV2,
) -> Result<RationalLifecycleSelectedBundleV6> {
    let lengths = fixed_lengths(input, action)?;
    build_rational_lifecycle_selected_bundle_v6(RationalLifecycleSelectedBundleInputV6 {
        action,
        account_profile: RationalLifecycleSelectedAccountProfileInputV5 {
            logical_data_lengths: &lengths,
            product_basis: input.product_basis,
        },
        token_behavior_selection: selection,
        kind: RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1,
        root_schema: input.root_schema,
        lifecycle_policy: lifecycle,
        capacity_profile: input.capacity_profile,
        root_state_bytes: input.root_state_bytes,
    })
    .map_err(|_| RationalSelectedReleaseErrorV1::Encoding)
}

fn compile_compact(
    input: RationalSelectedReleaseInputV1<'_>,
    selection: TokenBehaviorSelectionV2,
    lifecycle: &[u8],
) -> Result<RationalLifecycleCompactBundleV4> {
    let lengths = compact_lengths(input)?;
    build_rational_lifecycle_compact_bundle_v6(RationalLifecycleCompactBundleInputV6 {
        artifacts: RationalLifecycleCompactArtifactInputV6 {
            logical_data_lengths: &lengths,
            product_basis: input.product_basis,
            coefficients: input.coefficients,
        },
        kind: RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1,
        token_behavior_selection: selection,
        root_schema: input.root_schema,
        lifecycle_policy: lifecycle,
        capacity_profile: input.capacity_profile,
        root_state_bytes: input.root_state_bytes,
    })
    .map_err(|_| RationalSelectedReleaseErrorV1::Encoding)
}

/// The canonical action order this release publishes.
#[must_use]
pub fn rational_selected_actions_v1() -> [LifecycleActionV2; RATIONAL_SELECTED_ACTION_COUNT_V1] {
    RATIONAL_LIFECYCLE_SELECTED_ACTIONS_V6
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_product::payoff::runtime_v3::{
        BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
    };

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
                basis_width: 258,
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

    const COEFFICIENTS: [u64; 3] = [2, 0, 5];

    fn input<'a>(basis: &'a [u8]) -> RationalSelectedReleaseInputV1<'a> {
        RationalSelectedReleaseInputV1 {
            realm: id(18),
            release_set: id(15),
            capacity_profile: id(43),
            root_schema: id(42),
            root_state_bytes: 64,
            coefficients: &COEFFICIENTS,
            product_basis: basis,
        }
    }

    /// The compiled release is one the family's own admission accepts.
    ///
    /// The compiler runs `authenticate_rational_release_v1` before returning,
    /// so this is not merely "it encoded" -- it is "the admission joined it".
    #[test]
    fn the_release_compiles_and_its_own_admission_accepts_it() {
        let basis = basis();
        let release = rational_selected_release_v1(input(&basis)).expect("release");
        let bytes = release.artifact_bytes().expect("bytes");
        let joined = dclutch_rational_lifecycle_hot_v3::authenticate_rational_release_v1(
            release.selection(),
            bytes,
        )
        .expect("independent admission");
        assert_eq!(joined.descriptors, release.publication.descriptors);
        assert_eq!(release.fixed.len(), 3);
        assert_eq!(release.publication.action_count, 4);
        assert_eq!(release.publication.support_count, 2);
        assert_eq!(release.publication.outcome_count, 3);
        assert_eq!(release.publication.selector_width, 1);
        assert_eq!(
            release.publication.executor_role,
            ExecutionRoleV1::Trading as u8
        );
        assert_eq!(
            release.publication.kind_id,
            RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1
        );
    }

    /// No Market anywhere, and the same bytes every run.
    ///
    /// The counterpart at release level of the ProgramSet pin: the identities a
    /// capability manifest entry carries are fixed before the Market exists,
    /// and they do not drift between builds.
    #[test]
    fn the_release_precedes_the_market_and_is_byte_stable() {
        let basis = basis();
        let first = rational_selected_release_v1(input(&basis)).expect("first");
        let second = rational_selected_release_v1(input(&basis)).expect("second");
        assert_eq!(first.program_set, second.program_set);
        assert_eq!(first.config, second.config);
        assert_eq!(first.publication, second.publication);
        assert_eq!(first.publication.to_bytes(), second.publication.to_bytes());
        assert_eq!(
            first.publication.publication_id(),
            second.publication.publication_id()
        );
    }

    /// The publication's width is its own field table, and its layout closes.
    #[test]
    fn the_publication_encodes_at_its_declared_width() {
        let basis = basis();
        let release = rational_selected_release_v1(input(&basis)).expect("release");
        let bytes = release.publication.to_bytes();
        assert_eq!(bytes.len(), RATIONAL_SELECTED_PUBLICATION_BYTES_V1);
        assert_eq!(
            bytes.get(..8),
            Some(RATIONAL_SELECTED_PUBLICATION_MAGIC_V1.as_slice())
        );
        // Every identity the publication claims is really in the encoding, so a
        // reader joining a manifest entry to this record reads the same bytes
        // the compiler wrote.
        for identity in [
            release.publication.kind_id,
            release.publication.program_set_id,
            release.publication.config_id,
            release.publication.realm,
            release.publication.release_set,
        ] {
            assert!(bytes.windows(32).any(|window| window == identity));
        }
        for descriptor in release.publication.descriptors {
            assert!(bytes.windows(32).any(|window| window == descriptor));
        }
    }

    /// Every record is finalized under a schema the release itself selects.
    #[test]
    fn every_published_record_is_named_by_the_release() {
        let basis = basis();
        let release = rational_selected_release_v1(input(&basis)).expect("release");
        let records = release.publication_records().expect("records");
        // program set + config + 7 artifacts x 4 actions.
        assert_eq!(records.len(), 2 + 7 * 4);

        // The four descriptor records' content ids ARE the descriptor
        // identities the publication names, so a manifest entry can be joined
        // to a record without either side restating a digest.
        let descriptors = records
            .iter()
            .filter(|record| record.label == "descriptor")
            .map(RationalPublicationRecordV1::content_id)
            .collect::<Vec<_>>();
        assert_eq!(descriptors.as_slice(), release.publication.descriptors);

        // The config record is finalized under the descriptor's own
        // config_schema, which is the schema an on-chain activation
        // authenticates the config raw record under.
        let config = records
            .iter()
            .find(|record| record.label == "config")
            .expect("config record");
        assert_eq!(
            config.schema,
            dclutch_custody::token_svm::TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2
        );
        assert_eq!(config.content_id(), release.publication.config_id);
    }

    /// Inputs that could bind a Market to nonsense refuse.
    #[test]
    fn nonsense_inputs_refuse_before_anything_is_encoded() {
        let basis = basis();
        let zero = [0_u64; 3];
        for bad in [
            RationalSelectedReleaseInputV1 {
                realm: [0; 32],
                ..input(&basis)
            },
            RationalSelectedReleaseInputV1 {
                release_set: [0; 32],
                ..input(&basis)
            },
            // A Realm equal to its release set collapses two distinct
            // authorities into one identity.
            RationalSelectedReleaseInputV1 {
                release_set: id(18),
                ..input(&basis)
            },
            RationalSelectedReleaseInputV1 {
                capacity_profile: [0; 32],
                ..input(&basis)
            },
            RationalSelectedReleaseInputV1 {
                root_schema: [0; 32],
                ..input(&basis)
            },
            RationalSelectedReleaseInputV1 {
                root_state_bytes: 0,
                ..input(&basis)
            },
            // An all-zero representation has no support, so complete
            // retirement would have nothing to witness.
            RationalSelectedReleaseInputV1 {
                coefficients: &zero,
                ..input(&basis)
            },
            RationalSelectedReleaseInputV1 {
                coefficients: &[],
                ..input(&basis)
            },
            RationalSelectedReleaseInputV1 {
                product_basis: &[],
                ..input(&basis)
            },
        ] {
            assert!(matches!(
                rational_selected_release_v1(bad),
                Err(RationalSelectedReleaseErrorV1::Input)
            ));
        }
    }

    /// A different representation is a different release.
    ///
    /// The release is per-representation by construction: the support width
    /// reaches the compact artifacts. Two representations must therefore not
    /// collide on a manifest identity.
    #[test]
    fn another_representation_is_another_release() {
        let basis = basis();
        let other = [2_u64, 3, 5];
        let first = rational_selected_release_v1(input(&basis)).expect("first");
        let second = rational_selected_release_v1(RationalSelectedReleaseInputV1 {
            coefficients: &other,
            ..input(&basis)
        })
        .expect("second");
        assert_ne!(
            first.publication.program_set_id,
            second.publication.program_set_id
        );
        assert_ne!(
            first.publication.descriptors,
            second.publication.descriptors
        );
        // But the config is shared: it binds Realm and release set, not shape.
        assert_eq!(first.publication.config_id, second.publication.config_id);
    }
}
