//! Compile every current General action into one publishable, selectable release.
//!
//! # What was missing, and what was not
//!
//! General has had a release VERIFIER for a while:
//! [`authenticate_general_release_v3`] joins all fifteen action bundles against
//! one `CapabilityProgramSetV2` in a single pass, and
//! `authenticate_general_program_set_v3` validates the set table itself -- a
//! set-level check Series does not even have. What General lacked was the other
//! half: something that PRODUCES a bundle that verifier accepts, and names the
//! result in a form a founded Market can select.
//!
//! The pieces existed but were parked. The action artifact graph was
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
//! `encode_general_family_state_lifecycle_v5_atomic`, which reads its seed order from
//! `dclutch_trading::general::state_seeds_v3`. This module never names a
//! seed, a domain, or a bump ordinal. A release compiler that restated the seed
//! order would be the failure mode the whole exercise exists to prevent: a
//! policy that AUTHENTICATES -- every digest agreeing with itself -- and derives
//! addresses the family does not execute at.
//!
//! # The activation entry, and why a release without it publishes a dead Market
//!
//! A General release is not fifteen action bundles. It is fifteen action bundles
//! plus the ONE coordinate that creates the root all fifteen execute against.
//! `programs/dclutch-trading-sbf/src/outer.rs::authenticate_set_descriptor`
//! admits only a descriptor stamped `CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1`,
//! and every action descriptor is stamped `v4::SCHEMA_RELEASE_ID` -- so a Market
//! founded on an action-only set can never create its capability root, and every
//! action it publishes is unreachable forever. That is why this compiler emits
//! `GeneralReleaseProfileV1::CompleteV2WithActivation` and not the narrower
//! profile: an activation-incapable General release is not a smaller release,
//! it is an unfoundable one.
//!
//! The activation triple is not authored here. It comes from
//! `dclutch_trading::general::activation_bundle_v1`, which composes it on
//! the family-neutral `dclutch-market::capability_activation` template and refuses
//! -- rather than returning a bundle -- if the real effect kernel does not
//! project exactly `general_root_creation_tail_v2`. This module publishes what
//! that constructor returns; it neither restates a tail byte nor names a
//! register.

use dclutch_core_contract::ContentId;
use dclutch_market::STATE_BYTES as CORE_MARKET_STATE_BYTES;
use dclutch_market::capability_activation::{
    ActivationBundleV1, activation_account_profile_schema_v1, activation_effect_schema_v1,
};
use dclutch_market::capability_program::{
    set_v2::{
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2, SelectorWidthV2,
    },
    v4::{
        ArtifactReferenceV4, CapabilityArtifactsV4, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
        SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    },
};
use dclutch_market::execution_strategy::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyAdmissionV2,
    ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_market::realm::REALM_BYTES;
use dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
use dclutch_registry::release_set::{ArtifactReleaseIdV1, ExecutionRoleV1};
use dclutch_registry::svm::{LOADER_V3_PROGRAM_BYTES, LOADER_V3_PROGRAMDATA_METADATA_BYTES};
use dclutch_trading::general::{
    account_rules_v3::{
        GeneralExternalAccountWidthsV3, encode_general_account_profile_v3_atomic,
        general_account_profile_bytes_v3,
    },
    activation_bundle_v1::{
        GeneralActivationBundleInputV1, build_general_activation_bundle_v1,
        build_general_activation_capable_program_set_v2, general_activation_descriptor_schema_v1,
        general_activation_request_v1, validate_general_activation_bundle_v1,
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
        GENERAL_ACTION_PROGRAM_COUNT_V5, GENERAL_ACTIONS_V5, GeneralActionArtifactsV3,
        GeneralArtifactReleaseBytesV3, GeneralReleaseErrorV3, GeneralReleaseProfileV1,
        authenticate_general_release_v3,
    },
    specialization::general_request_profile_bytes_v1,
    state_artifacts_v3::{
        GeneralChildRentWidthsV5, encode_general_family_state_lifecycle_v5_atomic,
        general_family_state_lifecycle_bytes_v5,
    },
    transition_artifacts_v3::{
        GENERAL_TRANSITION_INSTRUCTION_PLACEHOLDER_V3, encode_general_transition_program_v3_atomic,
        general_transition_instruction_count_v3, general_transition_program_bytes_v3,
    },
};
use dclutch_trading::general_codec::{
    Action,
    successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
    successor_request_v3::{ControllerActionV3, ControllerRequestV3},
};
use dclutch_trading::general_config::{
    GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_BYTES_V2, GENERAL_ROOT_SCHEMA_ID_V2,
    v3::{GENERAL_CONFIG_SCHEMA_ID_V3, GeneralConfigV3, GeneralConfigV3Input},
};
use solana_program::hash::hash;

/// Number of action bundles one selectable General release compiles.
pub const GENERAL_SELECTED_ACTION_COUNT_V1: usize = GENERAL_ACTION_PROGRAM_COUNT_V5;

/// Coordinates one selectable General release publishes: the actions plus one.
///
/// Derived from the profile the release names, so the two cannot disagree.
pub const GENERAL_SELECTED_ENTRY_COUNT_V1: usize =
    general_selected_release_profile_v1().entry_count();

/// Canonical General publication magic.
pub const GENERAL_SELECTED_PUBLICATION_MAGIC_V1: [u8; 8] = *b"DCGNPB01";

/// Implemented General publication version.
pub const GENERAL_SELECTED_PUBLICATION_VERSION_V1: u16 = 1;

/// Execution role that owns every General commit.
///
/// General emits no dispatch arm of its own: `hot_v3` is family-neutral and
/// already dispatched, so the executor a General release names is Trading.
pub const GENERAL_EXECUTOR_ROLE_V1: ExecutionRoleV1 = ExecutionRoleV1::Trading;

/// Compartment rows a General founding provisions in the selected ledger.
///
/// One, and it is derived rather than chosen: the capability-neutral selection
/// seam authors its manifest entry with an all-zero dependency list, so the
/// entry's dependency closure is the entry itself and the Trading
/// `FundingLedgerV2` it owns carries exactly one row. The activation
/// AccountProfile projects the parked Rent quote at the offset that row count
/// determines, so a release that guessed a different count would publish a
/// profile reading the wrong bytes of a real ledger.
pub const GENERAL_SELECTED_FUNDING_LEDGER_SLOTS_V1: u16 = 1;

const PUBLICATION_IDENTITY_START_V1: usize = 16;
/// Identities that are not per-action descriptors.
///
/// Twelve: the eleven release-wide coordinates plus the activation descriptor,
/// which is a published fact of the release exactly as the fifteen action
/// descriptors are.
const PUBLICATION_FIXED_IDENTITY_COUNT_V1: usize = 12;
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

/// Exact serialized width of the runtime Rent sysvar.
///
/// Eight bytes of `lamports_per_byte_year`, eight of `exemption_threshold` and
/// one of `burn_percent`. The runtime owns this account and publishes no
/// first-party constant for its width, so it is named ONCE here rather than
/// spelled at each of the sites that needs it.
pub const RENT_SYSVAR_ACCOUNT_BYTES_V1: u32 = 17;

/// The eleven external account widths one General AccountProfile publishes.
///
/// # Why this exists, and what it cost not to have it
///
/// Nine of the eleven are protocol constants and two are Product-derived. Until
/// 2026-09-03 the tree had FOUR authors for them: the unit-test fixture in
/// `account_rules_v3.rs`, the General-hot program-test (which read the
/// contracts and was right), `general_market.rs` and the devnet policy file --
/// and the last two were the unit fixture transcribed. Cohort-14's founded
/// General market `8ExdC1Rwby...` therefore published `Exact(48)` for a
/// RentCredit the protocol only ever produces at
/// [`LIFECYCLE_RENT_CREDIT_BYTES_V2`] = 128, so no producible account fit its
/// own `OpenBatch` frame. Two more were wrong the same way and had not yet been
/// reached: the activation cache at 160 against 1,288, and the Core Market at
/// 320 against 368.
///
/// A width published in an `Exact` prestate is a REFUSAL if it disagrees with
/// the account the chain holds, and nothing on the commit path reads most of
/// these -- so a wrong one has no symptom except that the action cannot be
/// delivered. That is precisely the shape a transcribed literal produces and a
/// derivation cannot.
///
/// The two arguments are the ones no constant can answer: they are functions of
/// the Product graph this market is founded on.
#[must_use]
pub fn general_external_account_widths_v3(
    linked_basis_prefix: u32,
    result_domain: u32,
) -> GeneralExternalAccountWidthsV3 {
    GeneralExternalAccountWidthsV3 {
        linked_basis_prefix,
        result_domain,
        rent_sysvar: RENT_SYSVAR_ACCOUNT_BYTES_V1,
        core_market: CORE_MARKET_STATE_BYTES as u32,
        activation_cache: ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1 as u32,
        upgradeable_program: LOADER_V3_PROGRAM_BYTES as u32,
        trading_programdata_prefix: LOADER_V3_PROGRAMDATA_METADATA_BYTES as u32,
        claims_programdata_prefix: LOADER_V3_PROGRAMDATA_METADATA_BYTES as u32,
        core_programdata_prefix: LOADER_V3_PROGRAMDATA_METADATA_BYTES as u32,
        realm_record: REALM_BYTES as u32,
        rent_credit: LIFECYCLE_RENT_CREDIT_BYTES_V2 as u32,
    }
}

// The nine protocol widths fit `u32` by construction, checked here so the casts
// above need no runtime arm on a value that cannot move at run time.
const _: () = assert!(CORE_MARKET_STATE_BYTES <= u32::MAX as usize);
const _: () = assert!(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1 <= u32::MAX as usize);
const _: () = assert!(LOADER_V3_PROGRAM_BYTES <= u32::MAX as usize);
const _: () = assert!(LOADER_V3_PROGRAMDATA_METADATA_BYTES <= u32::MAX as usize);
const _: () = assert!(REALM_BYTES <= u32::MAX as usize);
const _: () = assert!(LIFECYCLE_RENT_CREDIT_BYTES_V2 <= u32::MAX as usize);

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
    /// The activation descriptor the sixteenth set entry names.
    ///
    /// Without it the six coordinates above describe a release no Market can
    /// activate, so it belongs in the summary a Market binds.
    pub activation_descriptor: [u8; 32],
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
        identities.push(self.activation_descriptor);
        identities.extend_from_slice(&self.descriptors);
        identities
    }
}

/// One compiled, self-verified, publishable General release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralSelectedReleaseV1 {
    /// Action bundles in canonical action order.
    pub bundles: Vec<GeneralSelectedBundleV1>,
    /// The activation triple the sixteenth set entry names.
    ///
    /// Obtained from `build_general_activation_bundle_v1`, whose constructor
    /// runs the real effect kernel over the effect it just built and refuses to
    /// return a bundle whose projection is not the family's own creation tail.
    /// A release therefore cannot hold an activation that would brick a root.
    pub activation: ActivationBundleV1,
    /// Exact sixteen-entry CapabilityProgramSetV2 bytes.
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
    ///
    /// The three activation records close the list, and they are the three the
    /// Trading activation frame authenticates: the `AccountProfileV1` at
    /// `PROFILE_RAW`, the `EffectProgramV2` at `EFFECT_RAW`, and the
    /// `CapabilityProgramV1` at `SET_DESCRIPTOR_RAW`. There is no fourth: the
    /// activation transition is EMBEDDED in that descriptor
    /// (`CapabilityProgramV1::decode` reads it off the bytes after the header),
    /// so publishing it separately would finalize a record the seam never reads
    /// and give the transition two authors. This is the same three-record shape
    /// Direct publishes (`direct_activation_{account_profile,effect,descriptor}
    /// _record`).
    pub fn publication_records(&self) -> Result<Vec<GeneralPublicationRecordV1<'_>>> {
        let set = CapabilityProgramSetV2::decode(&self.program_set)
            .map_err(GeneralSelectedReleaseErrorV1::ProgramSetContract)?;
        let first = CapabilityProgramV4::decode(
            &self
                .bundles
                .first()
                .ok_or(GeneralSelectedReleaseErrorV1::Release)?
                .descriptor,
        )
        .map_err(GeneralSelectedReleaseErrorV1::CapabilityProgram)?;

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
                .map_err(GeneralSelectedReleaseErrorV1::ProgramSetContract)?;
            let descriptor = CapabilityProgramV4::decode(&bundle.descriptor)
                .map_err(GeneralSelectedReleaseErrorV1::CapabilityProgram)?;
            let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy)
                .map_err(GeneralSelectedReleaseErrorV1::ExecutionStrategy)?;
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

        // The activation descriptor's schema is read off the set entry that
        // names it, exactly as each action descriptor's is; the profile and
        // effect schemas come from the activation codec, which is their single
        // author -- a `CapabilityProgramV1` carries a content identity for its
        // account profile and effect but no schema for either, and the seam
        // authenticates both under constants of its own.
        let activation_entry = set
            .entry(activation_entry_index()?)
            .map_err(GeneralSelectedReleaseErrorV1::ProgramSetContract)?;
        for (label, schema, body) in [
            (
                "activation-account-profile",
                activation_account_profile_schema_v1(),
                &self.activation.account_profile,
            ),
            (
                "activation-effect",
                activation_effect_schema_v1(),
                &self.activation.effect,
            ),
            (
                "activation-descriptor",
                activation_entry.descriptor().schema().to_bytes(),
                &self.activation.descriptor,
            ),
        ] {
            records.push(GeneralPublicationRecordV1 {
                label,
                schema,
                body,
            });
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
    /// The compiled release did not rebuild byte for byte from its input.
    Release,
    /// The family's own complete-catalogue admission refused, and with what.
    ///
    /// The cause is CARRIED rather than discarded. This was
    /// `map_err(|_| Release)` over the one call in this module that already
    /// knows which of six things went wrong, and the cost of that shape is
    /// exactly what `AGENTS.md` describes: a hostile that reaches the verifier
    /// has no word for what it found, and every refusal from a fifteen-action
    /// join reads as the same code as a one-byte rebuild mismatch.
    ReleaseAdmission(GeneralReleaseErrorV3),
    /// The activation triple refused to build, or the release carried another.
    ///
    /// Building it runs the real effect kernel over the effect it composes, so
    /// this is also how a bundle that would brick a root is refused: the
    /// constructor returns an error instead of an artifact.
    Activation,
    /// Publication identities or scalars were not exact.
    Publication,
    /// `dclutch_market::capability_program` refused; the cause is its own.
    ProgramSetContract(dclutch_market::capability_program::set_v2::ProgramSetErrorV2),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    CapabilityProgram(dclutch_market::capability_program::Error),
    /// `dclutch_market::execution_strategy` refused; the cause is its own.
    ExecutionStrategy(dclutch_market::execution_strategy::v2::Error),
    /// `dclutch_trading::general` refused; the cause is its own.
    GeneralActivationBundle(
        dclutch_trading::general::activation_bundle_v1::GeneralActivationBundleErrorV1,
    ),
    /// `dclutch_trading::general_config` refused; the cause is its own.
    GeneralConfig(dclutch_trading::general_config::v3::GeneralConfigErrorV3),
    /// `dclutch_registry::release_set` refused; the cause is its own.
    ReleaseSet(dclutch_registry::release_set::Error),
    /// `dclutch_trading::general` refused; the cause is its own.
    GeneralAccountRule(dclutch_trading::general::account_rules_v3::GeneralAccountRuleErrorV3),
    /// `dclutch_trading::general` refused; the cause is its own.
    GeneralStateArtifact(dclutch_trading::general::state_artifacts_v3::GeneralStateArtifactErrorV3),
    /// `dclutch_trading::general` refused; the cause is its own.
    GeneralTransitionArtifact(
        dclutch_trading::general::transition_artifacts_v3::GeneralTransitionArtifactErrorV3,
    ),
    /// `dclutch_trading::general` refused; the cause is its own.
    GeneralEffectArtifact(
        dclutch_trading::general::effect_artifacts_v3::GeneralEffectArtifactErrorV3,
    ),
    /// `dclutch_trading::general_codec` refused; the cause is its own.
    General(dclutch_trading::general_codec::Error),
}

/// Result alias for General release compilation.
pub type Result<T> = core::result::Result<T, GeneralSelectedReleaseErrorV1>;

/// Compile all fifteen General actions into one publishable release.
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

    // ONE POLICY, COMPILED ONCE, for all fifteen bundles. Compiling it inside
    // the loop would produce fifteen equal artifacts and re-derive one digest
    // fifteen times; it would also leave the family property looking incidental
    // rather than structural.
    let lifecycle_policy = encode_lifecycle(input)?;
    let mut bundles = Vec::with_capacity(GENERAL_SELECTED_ACTION_COUNT_V1);
    let mut descriptors = [[0_u8; 32]; GENERAL_SELECTED_ACTION_COUNT_V1];
    for (index, action) in GENERAL_ACTIONS_V5.into_iter().enumerate() {
        let bundle = compile_bundle(input, action, &lifecycle_policy)?;
        *descriptors
            .get_mut(index)
            .ok_or(GeneralSelectedReleaseErrorV1::Input)? = digest(&bundle.descriptor);
        bundles.push(bundle);
    }

    let activation = compile_activation(&bundles)?;
    let program_set = encode_program_set(&descriptors, activation.descriptor_id)?;
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
        activation_descriptor: activation.descriptor_id,
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
        activation,
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

    let lifecycle_policy = encode_lifecycle(input)?;
    let mut descriptors = [[0_u8; 32]; GENERAL_SELECTED_ACTION_COUNT_V1];
    for (index, action) in GENERAL_ACTIONS_V5.into_iter().enumerate() {
        let bundle = release
            .bundles
            .get(index)
            .ok_or(GeneralSelectedReleaseErrorV1::Release)?;
        if bundle.action != action {
            return Err(GeneralSelectedReleaseErrorV1::Release);
        }
        let expected = compile_bundle(input, action, &lifecycle_policy)?;
        if *bundle != expected {
            return Err(GeneralSelectedReleaseErrorV1::Release);
        }
        *descriptors
            .get_mut(index)
            .ok_or(GeneralSelectedReleaseErrorV1::Release)? = digest(&bundle.descriptor);
    }

    // The activation triple is rebuilt, not inspected: its constructor is the
    // brick gate, so a substituted profile, effect or descriptor fails here
    // because the only bundle that survives is the one this release compiles to.
    if release.activation != compile_activation(&release.bundles)? {
        return Err(GeneralSelectedReleaseErrorV1::Activation);
    }
    validate_general_activation_bundle_v1(
        &release.activation,
        GeneralActivationBundleInputV1 {
            action_descriptor: &first_action_descriptor(&release.bundles)?,
            funding_ledger_slot_count: GENERAL_SELECTED_FUNDING_LEDGER_SLOTS_V1,
        },
    )
    .map_err(GeneralSelectedReleaseErrorV1::GeneralActivationBundle)?;

    if release.program_set != encode_program_set(&descriptors, release.activation.descriptor_id)?
        || release.config != encode_config(input, digest(&release.program_set))?
    {
        return Err(GeneralSelectedReleaseErrorV1::ProgramSet);
    }

    // The set is re-decoded and every entry re-selected by a probe request, so
    // the published table is one a live controller request actually routes
    // through rather than one that merely encoded.
    let set = CapabilityProgramSetV2::decode(&release.program_set)
        .map_err(GeneralSelectedReleaseErrorV1::ProgramSetContract)?;
    if set.selector_offset() != GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV2::U8
        || usize::from(set.entry_count()) != GENERAL_SELECTED_ENTRY_COUNT_V1
    {
        return Err(GeneralSelectedReleaseErrorV1::ProgramSet);
    }
    // The activation request is the one request no action can produce, and it
    // must reach the activation descriptor under the one schema the seam
    // accepts. Checked here as well as in the set builder, because this
    // function is what a caller runs over a release it did not build.
    let activation_selected = set
        .select_descriptor(
            &general_activation_request_v1()
                .map_err(GeneralSelectedReleaseErrorV1::GeneralActivationBundle)?,
        )
        .map_err(GeneralSelectedReleaseErrorV1::ProgramSetContract)?;
    if activation_selected.program().to_bytes() != release.activation.descriptor_id
        || activation_selected.schema().to_bytes() != general_activation_descriptor_schema_v1()
    {
        return Err(GeneralSelectedReleaseErrorV1::ProgramSet);
    }
    for (index, action) in GENERAL_ACTIONS_V5.into_iter().enumerate() {
        let probe = action_selector_probe(action)?;
        let selected = set
            .select_descriptor(&probe)
            .map_err(GeneralSelectedReleaseErrorV1::ProgramSetContract)?;
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
        activation_descriptor: release.activation.descriptor_id,
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

/// Run the family's own complete-catalogue admission over the compiled release.
///
/// This is the gate that makes the release more than well-formed bytes:
/// `authenticate_general_release_v3` re-derives the ProgramSet identity, pins
/// the selector geometry, refuses duplicate descriptors, and joins every action
/// bundle to its descriptor -- the same pass an on-chain admission performs.
fn authenticate_release(
    release: &GeneralSelectedReleaseV1,
    input: GeneralSelectedReleaseInputV1,
) -> Result<()> {
    let requests: Vec<[u8; CONTROLLER_REQUEST_BYTES_V2]> = GENERAL_ACTIONS_V5
        .into_iter()
        .map(canonical_request)
        .collect::<Result<Vec<_>>>()?;
    // The array needs one seeded element before every coordinate is written;
    // seeding it from the first action keeps the seed a real bundle rather than
    // a placeholder that a missed write could leave behind.
    let mut actions = [GeneralActionArtifactsV3 {
        action: *GENERAL_ACTIONS_V5
            .first()
            .ok_or(GeneralSelectedReleaseErrorV1::Release)?,
        admission_request: requests
            .first()
            .ok_or(GeneralSelectedReleaseErrorV1::Release)?,
        artifacts: bundle_bytes(release, 0)?,
    }; GENERAL_SELECTED_ACTION_COUNT_V1];
    for (index, action) in GENERAL_ACTIONS_V5.into_iter().enumerate() {
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
    .map_err(GeneralSelectedReleaseErrorV1::ReleaseAdmission)?;
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
) -> Result<dclutch_trading::general::artifacts_v3::GeneralArtifactBytesV3<'_>> {
    let bundle = release
        .bundles
        .get(index)
        .ok_or(GeneralSelectedReleaseErrorV1::Release)?;
    Ok(
        dclutch_trading::general::artifacts_v3::GeneralArtifactBytesV3 {
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

/// Encode the sixteen-entry activation-capable set.
///
/// The table is not written down here. `build_general_activation_capable_program
/// _set_v2` owns the selector order, the two schemas and the entry count, and
/// re-authenticates the bytes it just wrote before returning them -- so a caller
/// cannot obtain a set that does not activate. The one check this function keeps
/// is the strictly ascending action order, because two coordinates behind one
/// request byte is the same class of defect as a wrong seed, and it is cheap to
/// refuse a caller-supplied descriptor list that is out of order before the
/// builder ever sees it.
fn encode_program_set(
    descriptors: &[[u8; 32]; GENERAL_SELECTED_ACTION_COUNT_V1],
    activation_descriptor_id: [u8; 32],
) -> Result<Vec<u8>> {
    let mut previous: Option<u32> = None;
    for action in GENERAL_ACTIONS_V5 {
        let selector = u32::from(action as u8);
        if previous.is_some_and(|prior| prior >= selector) {
            return Err(GeneralSelectedReleaseErrorV1::ProgramSet);
        }
        previous = Some(selector);
    }
    build_general_activation_capable_program_set_v2(descriptors, activation_descriptor_id)
        .map_err(GeneralSelectedReleaseErrorV1::GeneralActivationBundle)
}

/// The ONE descriptor every entry-authored coordinate may be read from.
///
/// Both consumers go through here: the activation triple, which inherits five
/// coordinates from an action descriptor, and the founding seam
/// (`tools/local-validator/bootstrap/successor/src/selected_capability.rs`),
/// which authors the capability manifest entry from four of them. Two callers
/// reaching into `bundles.first()` separately is two authors for one choice, and
/// it is how the harness came to compile its entry from OpenBatch while the
/// founding compiled it from Consider.
///
/// The choice is now CHECKED, not asserted. This function used to carry a
/// comment saying `authenticate_general_release_v3` had already required all
/// fifteen descriptors to agree on every entry-authored coordinate; that was
/// false, and its falseness is the whole reason cohort-15's General market
/// activated under one action and could execute no other. The verifier enforces
/// it now (`GeneralReleaseErrorV3::EntryCoordinateMismatch`), and this re-states
/// it over the compiled bundles so that a caller holding a release built by some
/// other path -- a harness, a fixture, a future compiler -- cannot silently pick
/// an action instead of a family.
pub fn general_selected_entry_descriptor_v1(release: &GeneralSelectedReleaseV1) -> Result<Vec<u8>> {
    let first = release
        .bundles
        .first()
        .ok_or(GeneralSelectedReleaseErrorV1::Release)?;
    let decoded = CapabilityProgramV4::decode(&first.descriptor)
        .map_err(GeneralSelectedReleaseErrorV1::CapabilityProgram)?;
    for bundle in &release.bundles {
        let other = CapabilityProgramV4::decode(&bundle.descriptor)
            .map_err(GeneralSelectedReleaseErrorV1::CapabilityProgram)?;
        if other.kind() != decoded.kind()
            || other.capacity_profile() != decoded.capacity_profile()
            || other.root_schema() != decoded.root_schema()
            || other.derivation_policy() != decoded.derivation_policy()
            || other.root_state_bytes() != decoded.root_state_bytes()
        {
            return Err(GeneralSelectedReleaseErrorV1::ReleaseAdmission(
                GeneralReleaseErrorV3::EntryCoordinateMismatch,
            ));
        }
    }
    Ok(first.descriptor.clone())
}

/// The same descriptor, during compilation, when only the bundles exist yet.
fn first_action_descriptor(bundles: &[GeneralSelectedBundleV1]) -> Result<Vec<u8>> {
    Ok(bundles
        .first()
        .ok_or(GeneralSelectedReleaseErrorV1::Release)?
        .descriptor
        .clone())
}

/// Build the activation triple this release publishes.
///
/// Nothing about the General root tail is named here or anywhere in this
/// module: the constructor derives the constant tail from
/// `general_root_creation_tail_v2`, composes it with the three seam-supplied
/// fields, and then runs the REAL effect kernel over the effect it has just
/// built, returning an error rather than a bundle if the projection is not that
/// tail byte for byte. A release therefore cannot carry an activation that would
/// write a root no General action can decode.
fn compile_activation(bundles: &[GeneralSelectedBundleV1]) -> Result<ActivationBundleV1> {
    build_general_activation_bundle_v1(GeneralActivationBundleInputV1 {
        action_descriptor: &first_action_descriptor(bundles)?,
        funding_ledger_slot_count: GENERAL_SELECTED_FUNDING_LEDGER_SLOTS_V1,
    })
    .map_err(GeneralSelectedReleaseErrorV1::GeneralActivationBundle)
}

/// The set index of the activation coordinate: after every action entry.
fn activation_entry_index() -> Result<u16> {
    u16::try_from(GENERAL_SELECTED_ACTION_COUNT_V1)
        .map_err(|_| GeneralSelectedReleaseErrorV1::ProgramSet)
}

fn encode_config(
    input: GeneralSelectedReleaseInputV1,
    program_set_id: [u8; 32],
) -> Result<Vec<u8>> {
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
    .map_err(GeneralSelectedReleaseErrorV1::GeneralConfig)?
    .to_bytes()
    .to_vec())
}

fn compile_bundle(
    input: GeneralSelectedReleaseInputV1,
    action: Action,
    lifecycle_policy: &[u8],
) -> Result<GeneralSelectedBundleV1> {
    let account_profile = encode_account_profile(input.external_widths, action)?;
    let lifecycle_policy = lifecycle_policy.to_vec();
    let request_profile = general_request_profile_bytes_v1(action).to_vec();
    let transition = encode_transition(action)?;
    let effect = encode_effect(action)?;

    let certificate = ExecutionStrategyCertificateV2::new(
        content(digest(&account_profile))?,
        content(dclutch_vm::request_profile::SCHEMA_RELEASE_ID)?,
        content(digest(&request_profile))?,
        content(dclutch_vm::v3::SCHEMA_RELEASE_ID)?,
        content(digest(&transition))?,
        content(digest(&effect))?,
        ArtifactReleaseIdV1::new(input.deployment.accelerator_artifact_release)
            .map_err(GeneralSelectedReleaseErrorV1::ReleaseSet)?,
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
        content(dclutch_vm::v3::SCHEMA_RELEASE_ID)?,
        content(digest(&transition))?,
        content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        Some(content(digest(&certificate))?),
        content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        Some(content(digest(&admission))?),
        content(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?,
        content(ACCELERATOR_ACK_SCHEMA_ID_V2)?,
    )
    .map_err(GeneralSelectedReleaseErrorV1::ExecutionStrategy)?
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
                content(dclutch_vm::account_profile::v2::SCHEMA_RELEASE_ID)?,
                content(digest(&account_profile))?,
            ),
            request_profile: ArtifactReferenceV4::new(
                content(dclutch_vm::request_profile::SCHEMA_RELEASE_ID)?,
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
                content(dclutch_vm::v3::SCHEMA_RELEASE_ID)?,
                content(digest(&transition))?,
            ),
            effect: ArtifactReferenceV4::new(
                content(dclutch_vm::effect::v4::SCHEMA_RELEASE_ID_V4)?,
                content(digest(&effect))?,
            ),
        },
        u32::try_from(GENERAL_ROOT_BYTES_V2).map_err(|_| GeneralSelectedReleaseErrorV1::Input)?,
    )
    .map_err(GeneralSelectedReleaseErrorV1::CapabilityProgram)?
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
    let bytes = general_account_profile_bytes_v3(action)
        .map_err(GeneralSelectedReleaseErrorV1::GeneralAccountRule)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_account_profile_v3_atomic(action, widths, &mut scratch, &mut output)
        .map_err(GeneralSelectedReleaseErrorV1::GeneralAccountRule)?;
    Ok(output)
}

/// Encode THE family lifecycle policy -- one artifact, all fifteen actions.
///
/// The seed order is NOT named here. It comes from `state_seeds_v3` through the
/// encoder, which is the single-author property this whole release depends on.
///
/// THE ACTION IS NOT A PARAMETER, and that is the repair. This function used to
/// take one and produce fifteen distinct artifacts with fifteen distinct
/// digests; `compile_bundle` then set each descriptor's `derivation_policy` to
/// its own digest, and a founded Market -- whose capability manifest holds ONE
/// entry, with ONE `child_derivation_id` -- could bind exactly one of them. That
/// is the wall cohort-15's General market died on: it activated under the first
/// bundle's policy and its OpenBatch refused `0x4015 DescriptorManifestEntry`.
/// One policy for the family makes the fifteen descriptors agree by
/// construction, and leaves `artifacts_v3.rs:522` -- `derivation_policy ==
/// lifecycle().program()` -- true for every action rather than repaired for one.
fn encode_lifecycle(input: GeneralSelectedReleaseInputV1) -> Result<Vec<u8>> {
    let bytes = general_family_state_lifecycle_bytes_v5();
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    let child_widths =
        GeneralChildRentWidthsV5::new(input.outcome_count, input.token_account_bytes)
            .map_err(GeneralSelectedReleaseErrorV1::GeneralStateArtifact)?;
    encode_general_family_state_lifecycle_v5_atomic(child_widths, &mut scratch, &mut output)
        .map_err(GeneralSelectedReleaseErrorV1::GeneralStateArtifact)?;
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
        .map_err(GeneralSelectedReleaseErrorV1::GeneralTransitionArtifact)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_general_transition_program_v3_atomic(
        action,
        &mut instructions,
        &mut scratch,
        &mut output,
    )
    .map_err(GeneralSelectedReleaseErrorV1::GeneralTransitionArtifact)?;
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
    let base = general_effect_program_bytes_v3(action)
        .map_err(GeneralSelectedReleaseErrorV1::GeneralEffectArtifact)?;
    let mut base_scratch = vec![0_u8; base];
    let mut base_output = vec![0_u8; base];
    let bytes = general_effect_program_bytes_v4(action)
        .map_err(GeneralSelectedReleaseErrorV1::GeneralEffectArtifact)?;
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
    .map_err(GeneralSelectedReleaseErrorV1::GeneralEffectArtifact)?;
    Ok(output)
}

/// The canonical admission witness for one action.
///
/// These are admission witnesses only: live execution re-runs the selected
/// RequestProfile against the actual family request.
fn canonical_request(action: Action) -> Result<[u8; CONTROLLER_REQUEST_BYTES_V2]> {
    if action as u8 <= Action::Close as u8 {
        return ControllerRequestV2 {
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
        .map_err(GeneralSelectedReleaseErrorV1::General);
    }

    ControllerRequestV3 {
        action: ControllerActionV3::from(action),
        expected_revision: 0,
        subject_id: Some([0x81; 32]),
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        primary_state_bump: 0,
        secondary_state_bump: 0,
        result_state_bump: 0,
    }
    .to_bytes()
    .map_err(GeneralSelectedReleaseErrorV1::General)
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
/// Sixteen entries: all fifteen current actions plus the activation coordinate.
/// Historical narrower profiles remain decodable for content compatibility,
/// but this compiler emits the sole complete current catalogue and joins every
/// selected descriptor before returning it.
#[must_use]
pub const fn general_selected_release_profile_v1() -> GeneralReleaseProfileV1 {
    GeneralReleaseProfileV1::CompleteV2WithActivation
}

#[cfg(test)]
mod tests;
