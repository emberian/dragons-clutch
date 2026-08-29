//! Compile the seven General actions into one publishable, selectable release.
//!
//! # What was missing, and what was not
//!
//! General has had a release VERIFIER for a while:
//! [`authenticate_general_release_v3`] joins all seven action bundles against
//! one `CapabilityProgramSetV2` in a single pass, and
//! `authenticate_general_program_set_v3` validates the set table itself -- a
//! set-level check Series does not even have. What General lacked was the other
//! half: something that PRODUCES a bundle that verifier accepts, and names the
//! result in a form a founded Market can select.
//!
//! The pieces existed but were parked. The seven-action artifact graph was
//! compiled only inside
//! `dclutch-general-accelerator-program-test::joined_artifacts`, a test-harness
//! crate nothing shippable can depend on, and General's only two
//! `encode_program_set_v2` calls in the entire tree were `#[cfg(test)]`.
//!
//! # Deployment facts are NAMED, never defaulted
//!
//! The fixture that compiled this graph supplied its deployment coordinates as
//! literals -- `claim_basis_id: [0x61; 32]`, `generation: 1`, and the
//! certificate's compiler release, toolchain and translation-validation
//! identities as `[0x71; 32]`, `[0x72; 32]`, `[0x73; 32]`. That is correct for a
//! fixture and inadmissible in a release: publishing them would bind a Market to
//! a config nobody chose and a certificate naming a toolchain that never ran.
//!
//! So this module splits its input in two. [`GeneralDeploymentFactsV1`] carries
//! exactly the facts no derivation can know, each a required field with no
//! default -- the Series precedent, where the assembler takes `lifecycle` and
//! `strategy` as typed parameters rather than defaulting them. Everything else
//! is DERIVED: the descriptors are digests of the bundles, the ProgramSet
//! entries are the canonical action order, the config's `program_set_id` is the
//! digest of the set that was just encoded, and every publication identity is
//! read back off those.
//!
//! # The seed order is not an input here, and that is the point
//!
//! Each bundle's lifecycle policy comes from
//! `encode_general_state_lifecycle_v5_atomic`, which reads its seed order from
//! `dclutch_general_adapter_contract::state_seeds_v3`. This module never names a
//! seed, a domain, or a bump ordinal. A release compiler that restated the seed
//! order would be the failure mode the whole exercise exists to prevent: a
//! policy that AUTHENTICATES -- every digest agreeing with itself -- and derives
//! addresses the family does not execute at.

use dclutch_capability_program_contract::{
    set_v2::{
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityDescriptorReferenceV2,
        CapabilityProgramSetEntryV2, CapabilityProgramSetV2, SelectorWidthV2,
        encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::{
        ArtifactReferenceV4, CapabilityArtifactsV4, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
        SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyAdmissionV2,
    ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_general_adapter_contract::{
    account_rules_v3::{
        GeneralExternalAccountWidthsV3, encode_general_account_profile_v3_atomic,
        general_account_profile_bytes_v3,
    },
    artifacts_v3::{
        GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3, GENERAL_CONTROLLER_REQUEST_SCHEMA_ID_V3,
        GeneralArtifactSelectionV3,
    },
    effect_artifacts_v3::{
        GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3, encode_general_effect_program_v4_atomic,
        general_effect_instruction_count_v3, general_effect_program_bytes_v3,
        general_effect_program_bytes_v4, general_effect_template_bytes_v3,
    },
    release_v3::{
        GENERAL_ACTION_PROGRAM_COUNT_V3, GENERAL_ACTIONS_V3, GeneralActionArtifactsV3,
        GeneralArtifactReleaseBytesV3, GeneralReleaseProfileV1, authenticate_general_release_v3,
    },
    specialization::general_request_profile_bytes_v1,
    state_artifacts_v3::{
        GeneralChildRentWidthsV5, encode_general_state_lifecycle_v5_atomic,
        general_state_lifecycle_bytes_v5,
    },
    transition_artifacts_v3::{
        GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3, encode_general_transition_program_v3_atomic,
        general_transition_instruction_count_v3, general_transition_program_bytes_v3,
    },
};
use dclutch_general_codec::{
    Action,
    successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
};
use dclutch_general_config_contract::{
    GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_BYTES_V2, GENERAL_ROOT_SCHEMA_ID_V2,
    v3::{GENERAL_CONFIG_SCHEMA_ID_V3, GeneralConfigV3, GeneralConfigV3Input},
};
use dclutch_release_set_contract::{ArtifactReleaseIdV1, ExecutionRoleV1};
use solana_program::hash::hash;

/// Number of action bundles one selectable General release compiles.
pub const GENERAL_SELECTED_ACTION_COUNT_V1: usize = GENERAL_ACTION_PROGRAM_COUNT_V3;

/// Canonical General publication magic.
pub const GENERAL_SELECTED_PUBLICATION_MAGIC_V1: [u8; 8] = *b"DCGNPB01";

/// Implemented General publication version.
pub const GENERAL_SELECTED_PUBLICATION_VERSION_V1: u16 = 1;

/// Execution role that owns every General commit.
///
/// General emits no dispatch arm of its own: `hot_v3` is family-neutral and
/// already dispatched, so the executor a General release names is Trading.
pub const GENERAL_EXECUTOR_ROLE_V1: ExecutionRoleV1 = ExecutionRoleV1::Trading;

const PUBLICATION_IDENTITY_START_V1: usize = 16;
/// Identities that are not per-action descriptors.
const PUBLICATION_FIXED_IDENTITY_COUNT_V1: usize = 11;
const PUBLICATION_IDENTITY_COUNT_V1: usize =
    PUBLICATION_FIXED_IDENTITY_COUNT_V1 + GENERAL_SELECTED_ACTION_COUNT_V1;
const PUBLICATION_SCALAR_START_V1: usize =
    PUBLICATION_IDENTITY_START_V1 + PUBLICATION_IDENTITY_COUNT_V1 * 32;
const PUBLICATION_SCALAR_BYTES_V1: usize = 8 + 8 + 4 + 4 + 2 + 1 + 1;

/// Exact encoded width of one canonical General publication.
///
/// Derived from the field table rather than written down beside it, so adding a
/// coordinate cannot leave the declared width describing the previous layout.
pub const GENERAL_SELECTED_PUBLICATION_BYTES_V1: usize =
    PUBLICATION_SCALAR_START_V1 + PUBLICATION_SCALAR_BYTES_V1;

/// Deployment facts a General release must NAME.
///
/// None of these is derivable from the artifacts, and none has a default. They
/// describe the accelerator that was actually built and admitted: a release that
/// guessed them would publish a certificate asserting a translation nobody
/// performed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralDeploymentFactsV1 {
    /// Registry-authenticated accelerator ArtifactRelease identity.
    pub accelerator_artifact_release: [u8; 32],
    /// Identity of the compiler release that produced the accelerator.
    pub compiler_release: [u8; 32],
    /// Identity of the toolchain the compiler ran under.
    pub toolchain: [u8; 32],
    /// Identity of the translation-validation evidence for that build.
    pub translation_validation: [u8; 32],
}

/// Immutable policy windows one General config pins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralConfigWindowsV1 {
    /// Positive order-collection window in slots.
    pub collection_slots: u64,
    /// Positive candidate-selection window in slots.
    pub selection_slots: u64,
    /// Positive settlement window in slots.
    pub settlement_slots: u64,
    /// Positive candidate-wide order ceiling.
    pub max_orders_per_candidate: u32,
    /// Positive candidate-wide authenticated page ceiling.
    pub max_pages_per_candidate: u32,
    /// Exact prepaid continuation reward, never collateral or future fees.
    pub continuation_reward_lamports: u64,
}

/// Complete input for one selectable General release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralSelectedReleaseInputV1 {
    /// Selected liftable capacity-profile identity.
    pub capacity_profile: [u8; 32],
    /// Exact ClaimBasis identity the Market Product selected.
    pub claim_basis: [u8; 32],
    /// Immutable interpreted selection-policy identity.
    pub selection_policy: [u8; 32],
    /// Immutable authority owning the replaceable quote-surplus token account.
    pub quote_surplus_beneficiary: [u8; 32],
    /// Immutable Market occurrence generation.
    pub generation: u64,
    /// Positive exact simplex denominator.
    pub price_scale: u64,
    /// Immutable policy windows.
    pub windows: GeneralConfigWindowsV1,
    /// Product-authenticated runtime outcome width.
    pub outcome_count: u32,
    /// Release-selected external account widths.
    pub external_widths: GeneralExternalAccountWidthsV3,
    /// Exact selected collateral token-account byte width.
    pub token_account_bytes: u32,
    /// Facts no derivation can know.
    pub deployment: GeneralDeploymentFactsV1,
}

/// One compiled action bundle, every artifact owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralSelectedBundleV1 {
    /// Action this bundle implements.
    pub action: Action,
    /// CapabilityProgramV4 descriptor bytes.
    pub descriptor: Vec<u8>,
    /// Runtime-width AccountProfile bytes.
    pub account_profile: Vec<u8>,
    /// State lifecycle and rent policy bytes.
    pub lifecycle_policy: Vec<u8>,
    /// Action-specific RequestProfile bytes.
    pub request_profile: Vec<u8>,
    /// Admitted-AOT ExecutionStrategy bytes.
    pub strategy: Vec<u8>,
    /// Semantic-equivalence certificate bytes.
    pub certificate: Vec<u8>,
    /// Registry admission bytes.
    pub admission: Vec<u8>,
    /// TransitionVM program bytes.
    pub transition: Vec<u8>,
    /// V4-envelope EffectProgram bytes.
    pub effect: Vec<u8>,
}

/// Canonical Market-bindable summary of one General release.
///
/// Every field is derived from the compiled release or copied from a named
/// deployment fact. There is no free parameter here: a publication that could be
/// written independently of the bundles it describes would be a second author
/// for the release identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralSelectedPublicationV1 {
    /// General capability kind, the Market manifest entry's `kind_id`.
    pub kind_id: [u8; 32],
    /// ProgramSet identity: the manifest entry's `release_id`.
    pub program_set_id: [u8; 32],
    /// Config identity: the manifest entry's `config_id`.
    pub config_id: [u8; 32],
    /// Selected capacity profile.
    pub capacity_profile: [u8; 32],
    /// Selected ClaimBasis.
    pub claim_basis: [u8; 32],
    /// Immutable selection policy.
    pub selection_policy: [u8; 32],
    /// Immutable quote-surplus beneficiary.
    pub quote_surplus_beneficiary: [u8; 32],
    /// Registry-authenticated accelerator ArtifactRelease.
    pub accelerator_artifact_release: [u8; 32],
    /// Compiler release that produced the accelerator.
    pub compiler_release: [u8; 32],
    /// Toolchain the compiler ran under.
    pub toolchain: [u8; 32],
    /// Translation-validation evidence identity.
    pub translation_validation: [u8; 32],
    /// Descriptor identities in canonical action order.
    pub descriptors: [[u8; 32]; GENERAL_SELECTED_ACTION_COUNT_V1],
    /// Immutable Market occurrence generation.
    pub generation: u64,
    /// Exact simplex denominator.
    pub price_scale: u64,
    /// Product-authenticated runtime outcome width.
    pub outcome_count: u32,
    /// Byte offset of the action selector inside a controller request.
    pub selector_offset: u32,
    /// Number of action-selected coordinates the set declares.
    pub action_count: u16,
    /// Execution role that owns every commit.
    pub executor_role: u8,
    /// Encoded selector width.
    pub selector_width: u8,
}

// The publication layout is closed at compile time: the identity block ends
// exactly where the scalar block begins, and the scalar block ends exactly at
// the declared width. Every offset in `to_bytes` is one of these constants, so
// the indexing there is proven in bounds here rather than checked at runtime.
const _: () = assert!(
    PUBLICATION_IDENTITY_START_V1 + PUBLICATION_IDENTITY_COUNT_V1 * 32
        == PUBLICATION_SCALAR_START_V1,
    "the publication identity block must end where the scalar block begins"
);
const _: () = assert!(
    PUBLICATION_SCALAR_START_V1 + PUBLICATION_SCALAR_BYTES_V1
        == GENERAL_SELECTED_PUBLICATION_BYTES_V1,
    "the publication scalar block must end at the declared width"
);
const _: () = assert!(
    PUBLICATION_IDENTITY_START_V1 >= 10,
    "the publication identity block must clear the magic and version header"
);

impl GeneralSelectedPublicationV1 {
    /// Encode the exact canonical publication bytes.
    ///
    /// Every offset below is a compile-time constant whose bound is established
    /// by the three assertions above, which is why the direct indexing here
    /// cannot panic for any value of `self`.
    #[must_use]
    #[allow(clippy::indexing_slicing)]
    pub fn to_bytes(&self) -> [u8; GENERAL_SELECTED_PUBLICATION_BYTES_V1] {
        let mut bytes = [0_u8; GENERAL_SELECTED_PUBLICATION_BYTES_V1];
        bytes[..8].copy_from_slice(&GENERAL_SELECTED_PUBLICATION_MAGIC_V1);
        bytes[8..10].copy_from_slice(&GENERAL_SELECTED_PUBLICATION_VERSION_V1.to_le_bytes());
        let mut offset = PUBLICATION_IDENTITY_START_V1;
        for identity in self.identities() {
            bytes[offset..offset + 32].copy_from_slice(&identity);
            offset += 32;
        }
        let mut scalar = PUBLICATION_SCALAR_START_V1;
        bytes[scalar..scalar + 8].copy_from_slice(&self.generation.to_le_bytes());
        scalar += 8;
        bytes[scalar..scalar + 8].copy_from_slice(&self.price_scale.to_le_bytes());
        scalar += 8;
        bytes[scalar..scalar + 4].copy_from_slice(&self.outcome_count.to_le_bytes());
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
        digest(&self.to_bytes())
    }

    /// The fixed identity order the encoding commits to.
    fn identities(&self) -> Vec<[u8; 32]> {
        let mut identities = Vec::with_capacity(PUBLICATION_IDENTITY_COUNT_V1);
        identities.push(self.kind_id);
        identities.push(self.program_set_id);
        identities.push(self.config_id);
        identities.push(self.capacity_profile);
        identities.push(self.claim_basis);
        identities.push(self.selection_policy);
        identities.push(self.quote_surplus_beneficiary);
        identities.push(self.accelerator_artifact_release);
        identities.push(self.compiler_release);
        identities.push(self.toolchain);
        identities.push(self.translation_validation);
        identities.extend_from_slice(&self.descriptors);
        identities
    }
}

/// One compiled, self-verified, publishable General release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralSelectedReleaseV1 {
    /// Action bundles in canonical action order.
    pub bundles: Vec<GeneralSelectedBundleV1>,
    /// Exact seven-entry CapabilityProgramSetV2 bytes.
    pub program_set: Vec<u8>,
    /// Exact immutable GeneralConfigV3 bytes.
    pub config: Vec<u8>,
    /// Canonical Market-bindable publication.
    pub publication: GeneralSelectedPublicationV1,
}

/// One record a publication chain must finalize in the Registry.
///
/// The `label` is for operators reading a plan; the `schema` and `body` are the
/// two facts a raw-record PDA is keyed by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralPublicationRecordV1<'a> {
    /// Human-readable name of this record's role in the release.
    pub label: &'static str,
    /// Schema/release identity this record is finalized under.
    pub schema: [u8; 32],
    /// Exact semantic bytes.
    pub body: &'a [u8],
}

impl GeneralPublicationRecordV1<'_> {
    /// Content identity of the exact bytes.
    #[must_use]
    pub fn content_id(&self) -> [u8; 32] {
        digest(self.body)
    }
}

impl GeneralSelectedReleaseV1 {
    /// Enumerate every record the Registry must hold for this release.
    ///
    /// Each record's schema is READ OFF the artifact that names it -- the
    /// descriptor's own `ArtifactReferenceV4` schemas, the strategy's own
    /// certificate and admission schemas, the descriptor's own `config_schema`,
    /// and the set entry's own descriptor schema. Nothing here restates a schema
    /// constant, so a publication plan cannot finalize a record under a schema
    /// the release does not actually select. That is the same single-author rule
    /// the seed contract enforces, applied to the publication chain.
    pub fn publication_records(&self) -> Result<Vec<GeneralPublicationRecordV1<'_>>> {
        let set = CapabilityProgramSetV2::decode(&self.program_set)
            .map_err(|_| GeneralSelectedReleaseErrorV1::ProgramSet)?;
        let first = CapabilityProgramV4::decode(
            &self
                .bundles
                .first()
                .ok_or(GeneralSelectedReleaseErrorV1::Release)?
                .descriptor,
        )
        .map_err(|_| GeneralSelectedReleaseErrorV1::Release)?;

        let mut records = Vec::new();
        records.push(GeneralPublicationRecordV1 {
            label: "program-set",
            schema: CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            body: &self.program_set,
        });
        records.push(GeneralPublicationRecordV1 {
            label: "config",
            schema: first.config_schema().to_bytes(),
            body: &self.config,
        });

        for (index, bundle) in self.bundles.iter().enumerate() {
            let entry = set
                .entry(u16::try_from(index).map_err(|_| GeneralSelectedReleaseErrorV1::ProgramSet)?)
                .map_err(|_| GeneralSelectedReleaseErrorV1::ProgramSet)?;
            let descriptor = CapabilityProgramV4::decode(&bundle.descriptor)
                .map_err(|_| GeneralSelectedReleaseErrorV1::Release)?;
            let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy)
                .map_err(|_| GeneralSelectedReleaseErrorV1::Release)?;
            let artifacts = descriptor.artifacts();
            for (label, schema, body) in [
                (
                    "descriptor",
                    entry.descriptor().schema().to_bytes(),
                    &bundle.descriptor,
                ),
                (
                    "account-profile",
                    artifacts.account_profile.schema().to_bytes(),
                    &bundle.account_profile,
                ),
                (
                    "lifecycle-policy",
                    artifacts.lifecycle.schema().to_bytes(),
                    &bundle.lifecycle_policy,
                ),
                (
                    "request-profile",
                    artifacts.request_profile.schema().to_bytes(),
                    &bundle.request_profile,
                ),
                (
                    "strategy",
                    artifacts.strategy.schema().to_bytes(),
                    &bundle.strategy,
                ),
                (
                    "certificate",
                    strategy.certificate_schema().to_bytes(),
                    &bundle.certificate,
                ),
                (
                    "admission",
                    strategy.admission_schema().to_bytes(),
                    &bundle.admission,
                ),
                (
                    "transition",
                    artifacts.transition.schema().to_bytes(),
                    &bundle.transition,
                ),
                (
                    "effect",
                    artifacts.effect.schema().to_bytes(),
                    &bundle.effect,
                ),
            ] {
                records.push(GeneralPublicationRecordV1 {
                    label,
                    schema,
                    body,
                });
            }
        }
        Ok(records)
    }

    /// Borrow the artifact selection an on-chain admission authenticates against.
    #[must_use]
    pub fn selection(&self) -> GeneralArtifactSelectionV3 {
        GeneralArtifactSelectionV3 {
            program_set: digest(&self.program_set),
            config: digest(&self.config),
            artifact_release: self.publication.accelerator_artifact_release,
        }
    }
}

/// Stable refusal from General release compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralSelectedReleaseErrorV1 {
    /// A named identity was zero, or a scalar the config requires was not positive.
    Input,
    /// A semantic-owner encoder refused an artifact.
    Encoding,
    /// ProgramSet encoding, decoding, or selection refused.
    ProgramSet,
    /// The complete seven-action release join refused.
    Release,
    /// Publication identities or scalars were not exact.
    Publication,
}

/// Result alias for General release compilation.
pub type Result<T> = core::result::Result<T, GeneralSelectedReleaseErrorV1>;

/// Compile the seven General actions into one publishable release.
///
/// Cheap refusals precede compilation: a zero identity or a nonpositive window
/// is rejected before any artifact is encoded. The compiled release is then
/// handed to [`authenticate_general_release_v3`] -- the same verifier an
/// on-chain admission runs -- so a release this function returns is one the
/// family's own admission accepts, not one that merely encoded without error.
pub fn general_selected_release_v1(
    input: GeneralSelectedReleaseInputV1,
) -> Result<GeneralSelectedReleaseV1> {
    validate_input(input)?;

    let mut bundles = Vec::with_capacity(GENERAL_SELECTED_ACTION_COUNT_V1);
    let mut descriptors = [[0_u8; 32]; GENERAL_SELECTED_ACTION_COUNT_V1];
    for (index, action) in GENERAL_ACTIONS_V3.into_iter().enumerate() {
        let bundle = compile_bundle(input, action)?;
        *descriptors
            .get_mut(index)
            .ok_or(GeneralSelectedReleaseErrorV1::Input)? = digest(&bundle.descriptor);
        bundles.push(bundle);
    }

    let program_set = encode_action_program_set(&descriptors)?;
    let config = encode_config(input, digest(&program_set))?;

    let publication = GeneralSelectedPublicationV1 {
        kind_id: GENERAL_CAPABILITY_KIND_ID_V1,
        program_set_id: digest(&program_set),
        config_id: digest(&config),
        capacity_profile: input.capacity_profile,
        claim_basis: input.claim_basis,
        selection_policy: input.selection_policy,
        quote_surplus_beneficiary: input.quote_surplus_beneficiary,
        accelerator_artifact_release: input.deployment.accelerator_artifact_release,
        compiler_release: input.deployment.compiler_release,
        toolchain: input.deployment.toolchain,
        translation_validation: input.deployment.translation_validation,
        descriptors,
        generation: input.generation,
        price_scale: input.price_scale,
        outcome_count: input.outcome_count,
        selector_offset: GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3,
        action_count: action_count_u16()?,
        executor_role: GENERAL_EXECUTOR_ROLE_V1 as u8,
        selector_width: selector_width_byte(),
    };

    let release = GeneralSelectedReleaseV1 {
        bundles,
        program_set,
        config,
        publication,
    };
    validate_general_selected_release_v1(&release, input)?;
    Ok(release)
}

/// Recompile the release from the same input and require exact agreement.
///
/// This is the hostile rejoin: it does not inspect the release for
/// plausibility, it rebuilds every byte and compares. A substituted bundle,
/// ProgramSet, config or publication identity therefore refuses, because the
/// only release that survives is the one this input compiles to.
pub fn validate_general_selected_release_v1(
    release: &GeneralSelectedReleaseV1,
    input: GeneralSelectedReleaseInputV1,
) -> Result<()> {
    validate_input(input)?;
    if release.bundles.len() != GENERAL_SELECTED_ACTION_COUNT_V1 {
        return Err(GeneralSelectedReleaseErrorV1::Release);
    }

    let mut descriptors = [[0_u8; 32]; GENERAL_SELECTED_ACTION_COUNT_V1];
    for (index, action) in GENERAL_ACTIONS_V3.into_iter().enumerate() {
        let bundle = release
            .bundles
            .get(index)
            .ok_or(GeneralSelectedReleaseErrorV1::Release)?;
        if bundle.action != action {
            return Err(GeneralSelectedReleaseErrorV1::Release);
        }
        let expected = compile_bundle(input, action)?;
        if *bundle != expected {
            return Err(GeneralSelectedReleaseErrorV1::Release);
        }
        *descriptors
            .get_mut(index)
            .ok_or(GeneralSelectedReleaseErrorV1::Release)? = digest(&bundle.descriptor);
    }

    if release.program_set != encode_action_program_set(&descriptors)?
        || release.config != encode_config(input, digest(&release.program_set))?
    {
        return Err(GeneralSelectedReleaseErrorV1::ProgramSet);
    }

    // The set is re-decoded and every entry re-selected by a probe request, so
    // the published table is one a live controller request actually routes
    // through rather than one that merely encoded.
    let set = CapabilityProgramSetV2::decode(&release.program_set)
        .map_err(|_| GeneralSelectedReleaseErrorV1::ProgramSet)?;
    if set.selector_offset() != GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U8
        || usize::from(set.entry_count()) != GENERAL_SELECTED_ACTION_COUNT_V1
    {
        return Err(GeneralSelectedReleaseErrorV1::ProgramSet);
    }
    for (index, action) in GENERAL_ACTIONS_V3.into_iter().enumerate() {
        let probe = action_selector_probe(action)?;
        let selected = set
            .select_descriptor(&probe)
            .map_err(|_| GeneralSelectedReleaseErrorV1::ProgramSet)?;
        let expected = *descriptors
            .get(index)
            .ok_or(GeneralSelectedReleaseErrorV1::ProgramSet)?;
        if selected.program().to_bytes() != expected
            || selected.schema().to_bytes() != CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID
        {
            return Err(GeneralSelectedReleaseErrorV1::ProgramSet);
        }
    }

    authenticate_release(release, input)?;

    let expected_publication = GeneralSelectedPublicationV1 {
        kind_id: GENERAL_CAPABILITY_KIND_ID_V1,
        program_set_id: digest(&release.program_set),
        config_id: digest(&release.config),
        capacity_profile: input.capacity_profile,
        claim_basis: input.claim_basis,
        selection_policy: input.selection_policy,
        quote_surplus_beneficiary: input.quote_surplus_beneficiary,
        accelerator_artifact_release: input.deployment.accelerator_artifact_release,
        compiler_release: input.deployment.compiler_release,
        toolchain: input.deployment.toolchain,
        translation_validation: input.deployment.translation_validation,
        descriptors,
        generation: input.generation,
        price_scale: input.price_scale,
        outcome_count: input.outcome_count,
        selector_offset: GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3,
        action_count: action_count_u16()?,
        executor_role: GENERAL_EXECUTOR_ROLE_V1 as u8,
        selector_width: selector_width_byte(),
    };
    if release.publication != expected_publication {
        return Err(GeneralSelectedReleaseErrorV1::Publication);
    }
    Ok(())
}

/// Run the family's own seven-action admission over the compiled release.
///
/// This is the gate that makes the release more than well-formed bytes:
/// `authenticate_general_release_v3` re-derives the ProgramSet identity, pins
/// the selector geometry, refuses duplicate descriptors, and joins every action
/// bundle to its descriptor -- the same pass an on-chain admission performs.
fn authenticate_release(
    release: &GeneralSelectedReleaseV1,
    input: GeneralSelectedReleaseInputV1,
) -> Result<()> {
    let requests: Vec<[u8; CONTROLLER_REQUEST_BYTES_V2]> = GENERAL_ACTIONS_V3
        .into_iter()
        .map(canonical_request)
        .collect::<Result<Vec<_>>>()?;
    // The array needs one seeded element before every coordinate is written;
    // seeding it from the first action keeps the seed a real bundle rather than
    // a placeholder that a missed write could leave behind.
    let mut actions = [GeneralActionArtifactsV3 {
        action: *GENERAL_ACTIONS_V3
            .first()
            .ok_or(GeneralSelectedReleaseErrorV1::Release)?,
        admission_request: requests
            .first()
            .ok_or(GeneralSelectedReleaseErrorV1::Release)?,
        artifacts: bundle_bytes(release, 0)?,
    }; GENERAL_SELECTED_ACTION_COUNT_V1];
    for (index, action) in GENERAL_ACTIONS_V3.into_iter().enumerate() {
        *actions
            .get_mut(index)
            .ok_or(GeneralSelectedReleaseErrorV1::Release)? = GeneralActionArtifactsV3 {
            action,
            admission_request: requests
                .get(index)
                .ok_or(GeneralSelectedReleaseErrorV1::Release)?,
            artifacts: bundle_bytes(release, index)?,
        };
    }
    let joined = authenticate_general_release_v3(
        release.selection(),
        GeneralArtifactReleaseBytesV3 {
            program_set: &release.program_set,
            config: &release.config,
            actions,
        },
        input.outcome_count,
    )
    .map_err(|_| GeneralSelectedReleaseErrorV1::Release)?;
    // The verifier reports the descriptor identities it joined; they must be the
    // ones the publication names, or the publication describes another release.
    for (index, descriptor) in joined.descriptors.into_iter().enumerate() {
        let published = *release
            .publication
            .descriptors
            .get(index)
            .ok_or(GeneralSelectedReleaseErrorV1::Publication)?;
        if descriptor.to_bytes() != published {
            return Err(GeneralSelectedReleaseErrorV1::Publication);
        }
    }
    Ok(())
}

fn bundle_bytes(
    release: &GeneralSelectedReleaseV1,
    index: usize,
) -> Result<dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBytesV3<'_>> {
    let bundle = release
        .bundles
        .get(index)
        .ok_or(GeneralSelectedReleaseErrorV1::Release)?;
    Ok(
        dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBytesV3 {
            program_set: &release.program_set,
            descriptor: &bundle.descriptor,
            config: &release.config,
            account_profile: &bundle.account_profile,
            lifecycle_policy: &bundle.lifecycle_policy,
            request_profile: &bundle.request_profile,
            strategy: &bundle.strategy,
            certificate: &bundle.certificate,
            admission: &bundle.admission,
            transition: &bundle.transition,
            effect: &bundle.effect,
        },
    )
}

fn validate_input(input: GeneralSelectedReleaseInputV1) -> Result<()> {
    for identity in [
        input.capacity_profile,
        input.claim_basis,
        input.selection_policy,
        input.quote_surplus_beneficiary,
        input.deployment.accelerator_artifact_release,
        input.deployment.compiler_release,
        input.deployment.toolchain,
        input.deployment.translation_validation,
    ] {
        if identity == [0; 32] {
            return Err(GeneralSelectedReleaseErrorV1::Input);
        }
    }
    if input.outcome_count == 0
        || input.token_account_bytes == 0
        || input.price_scale == 0
        || input.generation == 0
        || input.windows.collection_slots == 0
        || input.windows.selection_slots == 0
        || input.windows.settlement_slots == 0
        || input.windows.max_orders_per_candidate == 0
        || input.windows.max_pages_per_candidate == 0
    {
        return Err(GeneralSelectedReleaseErrorV1::Input);
    }
    Ok(())
}

fn encode_action_program_set(
    descriptors: &[[u8; 32]; GENERAL_SELECTED_ACTION_COUNT_V1],
) -> Result<Vec<u8>> {
    let mut entries = Vec::with_capacity(GENERAL_SELECTED_ACTION_COUNT_V1);
    let mut previous: Option<u32> = None;
    for (index, action) in GENERAL_ACTIONS_V3.into_iter().enumerate() {
        let selector = u32::from(action as u8);
        // The canonical order is strictly ascending. Checked here rather than
        // trusted from the codec, because two coordinates behind one request
        // byte is the same class of defect as a wrong seed.
        if previous.is_some_and(|prior| prior >= selector) {
            return Err(GeneralSelectedReleaseErrorV1::ProgramSet);
        }
        previous = Some(selector);
        entries.push(CapabilityProgramSetEntryV2::new(
            selector,
            CapabilityDescriptorReferenceV2::new(
                content(CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID)?,
                content(
                    *descriptors
                        .get(index)
                        .ok_or(GeneralSelectedReleaseErrorV1::ProgramSet)?,
                )?,
            ),
        ));
    }
    let width = encoded_program_set_bytes_v2(entries.len())
        .map_err(|_| GeneralSelectedReleaseErrorV1::ProgramSet)?;
    let mut bytes = vec![0_u8; width];
    encode_program_set_v2(
        GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3,
        SelectorWidthV2::U8,
        &entries,
        &mut bytes,
    )
    .map_err(|_| GeneralSelectedReleaseErrorV1::ProgramSet)?;
    Ok(bytes)
}

fn encode_config(input: GeneralSelectedReleaseInputV1, program_set_id: [u8; 32]) -> Result<Vec<u8>> {
    Ok(GeneralConfigV3::new(GeneralConfigV3Input {
        capacity_profile_id: input.capacity_profile,
        claim_basis_id: input.claim_basis,
        program_set_id,
        generation: input.generation,
        price_scale: input.price_scale,
        collection_slots: input.windows.collection_slots,
        selection_slots: input.windows.selection_slots,
        settlement_slots: input.windows.settlement_slots,
        max_orders_per_candidate: input.windows.max_orders_per_candidate,
        max_pages_per_candidate: input.windows.max_pages_per_candidate,
        continuation_reward_lamports: input.windows.continuation_reward_lamports,
        selection_policy_id: input.selection_policy,
        quote_surplus_beneficiary: input.quote_surplus_beneficiary,
    })
    .map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?
    .to_bytes()
    .to_vec())
}

fn compile_bundle(
    input: GeneralSelectedReleaseInputV1,
    action: Action,
) -> Result<GeneralSelectedBundleV1> {
    let account_profile = encode_account_profile(input.external_widths, action)?;
    let lifecycle_policy = encode_lifecycle(input, action)?;
    let request_profile = general_request_profile_bytes_v1(action).to_vec();
    let transition = encode_transition(action)?;
    let effect = encode_effect(action)?;

    let certificate = ExecutionStrategyCertificateV2::new(
        content(digest(&account_profile))?,
        content(dclutch_request_profile_contract::SCHEMA_RELEASE_ID)?,
        content(digest(&request_profile))?,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        content(digest(&transition))?,
        content(digest(&effect))?,
        ArtifactReleaseIdV1::new(input.deployment.accelerator_artifact_release)
            .map_err(|_| GeneralSelectedReleaseErrorV1::Input)?,
        content(input.deployment.compiler_release)?,
        content(input.deployment.toolchain)?,
        content(input.deployment.translation_validation)?,
    )
    .to_bytes()
    .to_vec();
    let admission = ExecutionStrategyAdmissionV2::new(content(digest(&certificate))?)
        .to_bytes()
        .to_vec();
    // AdmittedAot, not Interpreted: General's sole dynamic span is
    // AccountProfile-owned and no RequestProfile writes its selector, so the
    // real accelerator ELF must be deployed and admitted for this to execute.
    let strategy = ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::AdmittedAot,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        content(digest(&transition))?,
        content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        Some(content(digest(&certificate))?),
        content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        Some(content(digest(&admission))?),
        content(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?,
        content(ACCELERATOR_ACK_SCHEMA_ID_V2)?,
    )
    .map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?
    .to_bytes()
    .to_vec();

    let descriptor = CapabilityProgramV4::new(
        content(GENERAL_CAPABILITY_KIND_ID_V1)?,
        content(GENERAL_CONFIG_SCHEMA_ID_V3)?,
        content(GENERAL_CONTROLLER_REQUEST_SCHEMA_ID_V3)?,
        content(GENERAL_ROOT_SCHEMA_ID_V2)?,
        content(digest(&lifecycle_policy))?,
        content(input.capacity_profile)?,
        CapabilityArtifactsV4 {
            account_profile: ArtifactReferenceV4::new(
                content(dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID)?,
                content(digest(&account_profile))?,
            ),
            request_profile: ArtifactReferenceV4::new(
                content(dclutch_request_profile_contract::SCHEMA_RELEASE_ID)?,
                content(digest(&request_profile))?,
            ),
            lifecycle: ArtifactReferenceV4::new(
                content(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5)?,
                content(digest(&lifecycle_policy))?,
            ),
            strategy: ArtifactReferenceV4::new(
                content(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)?,
                content(digest(&strategy))?,
            ),
            transition: ArtifactReferenceV4::new(
                content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
                content(digest(&transition))?,
            ),
            effect: ArtifactReferenceV4::new(
                content(dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4)?,
                content(digest(&effect))?,
            ),
        },
        u32::try_from(GENERAL_ROOT_BYTES_V2).map_err(|_| GeneralSelectedReleaseErrorV1::Input)?,
    )
    .map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?
    .encode()
    .to_vec();

    Ok(GeneralSelectedBundleV1 {
        action,
        descriptor,
        account_profile,
        lifecycle_policy,
        request_profile,
        strategy,
        certificate,
        admission,
        transition,
        effect,
    })
}

fn encode_account_profile(
    widths: GeneralExternalAccountWidthsV3,
    action: Action,
) -> Result<Vec<u8>> {
    let bytes =
        general_account_profile_bytes_v3(action).map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_account_profile_v3_atomic(action, widths, &mut scratch, &mut output)
        .map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?;
    Ok(output)
}

/// Encode one action's lifecycle policy.
///
/// The seed order is NOT named here. It comes from `state_seeds_v3` through the
/// encoder, which is the single-author property this whole release depends on.
fn encode_lifecycle(input: GeneralSelectedReleaseInputV1, action: Action) -> Result<Vec<u8>> {
    let bytes =
        general_state_lifecycle_bytes_v5(action).map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    let child_widths = if action == Action::InitializeSettlement {
        Some(
            GeneralChildRentWidthsV5::new(input.outcome_count, input.token_account_bytes)
                .map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?,
        )
    } else {
        None
    };
    encode_general_state_lifecycle_v5_atomic(action, child_widths, &mut scratch, &mut output)
        .map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?;
    Ok(output)
}

fn encode_transition(action: Action) -> Result<Vec<u8>> {
    let (prelude, item, epilogue) = general_transition_instruction_count_v3(action);
    let count = prelude
        .checked_add(item)
        .and_then(|value| value.checked_add(epilogue))
        .ok_or(GeneralSelectedReleaseErrorV1::Encoding)?;
    let mut instructions = vec![GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3; count];
    let bytes = general_transition_program_bytes_v3(action)
        .map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_transition_program_v3_atomic(action, &mut instructions, &mut scratch, &mut output)
        .map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?;
    Ok(output)
}

/// Encode one action's EffectProgram as a V4 envelope.
///
/// V4 is not a preference: `process_hot_execution_v3` decodes exactly one effect
/// schema, so a release publishing the bare V3 record is refused by the Hot path
/// before any caller matters.
fn encode_effect(action: Action) -> Result<Vec<u8>> {
    let (fixed, item) = general_effect_instruction_count_v3(action);
    let count = fixed
        .checked_add(item)
        .ok_or(GeneralSelectedReleaseErrorV1::Encoding)?;
    let mut instructions = vec![GENERAL_EFFECT_INSTRUCTION_PLACEHOLDER_V3; count];
    let mut templates = vec![0_u8; general_effect_template_bytes_v3(action)];
    let base =
        general_effect_program_bytes_v3(action).map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?;
    let mut base_scratch = vec![0_u8; base];
    let mut base_output = vec![0_u8; base];
    let bytes =
        general_effect_program_bytes_v4(action).map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_effect_program_v4_atomic(
        action,
        &mut instructions,
        &mut templates,
        &mut base_scratch,
        &mut base_output,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)?;
    Ok(output)
}

/// The canonical admission witness for one action.
///
/// These are admission witnesses only: live execution re-runs the selected
/// RequestProfile against the actual family request.
fn canonical_request(action: Action) -> Result<[u8; CONTROLLER_REQUEST_BYTES_V2]> {
    ControllerRequestV2 {
        action,
        expected_revision: 0,
        candidate_id: (!matches!(action, Action::Freeze)).then_some([0x81; 32]),
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        state_bump: 0,
        terminal_record_bump: 0,
    }
    .to_bytes()
    .map_err(|_| GeneralSelectedReleaseErrorV1::Encoding)
}

/// A minimal request carrying only the action byte, for set-selection rejoin.
///
/// The selector offset comes from the contract rather than a literal, so a probe
/// can never test a byte the published set does not select on.
fn action_selector_probe(action: Action) -> Result<[u8; CONTROLLER_REQUEST_BYTES_V2]> {
    let mut probe = [0_u8; CONTROLLER_REQUEST_BYTES_V2];
    let offset = usize::try_from(GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3)
        .map_err(|_| GeneralSelectedReleaseErrorV1::ProgramSet)?;
    *probe
        .get_mut(offset)
        .ok_or(GeneralSelectedReleaseErrorV1::ProgramSet)? = action as u8;
    Ok(probe)
}

fn action_count_u16() -> Result<u16> {
    u16::try_from(GENERAL_SELECTED_ACTION_COUNT_V1)
        .map_err(|_| GeneralSelectedReleaseErrorV1::ProgramSet)
}

fn selector_width_byte() -> u8 {
    // One byte, because the General action tag IS the family discriminant.
    match SelectorWidthV2::U8 {
        SelectorWidthV2::U8 => 1,
        SelectorWidthV2::U16 => 2,
        SelectorWidthV2::U32 => 4,
    }
}

fn content(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| GeneralSelectedReleaseErrorV1::Input)
}

/// SHA-256 content digest, the identity every artifact edge is keyed by.
fn digest(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}

/// The profile a General release publishes, named rather than implied.
///
/// Seven entries is [`GeneralReleaseProfileV1::SettlementOnly`]. The wider
/// profiles are legal SETS and not yet admissible RELEASES: the collection and
/// candidate actions have no authored artifact triple, so there is nothing to
/// join at those coordinates.
#[must_use]
pub const fn general_selected_release_profile_v1() -> GeneralReleaseProfileV1 {
    GeneralReleaseProfileV1::SettlementOnly
}

#[cfg(test)]
mod tests;
