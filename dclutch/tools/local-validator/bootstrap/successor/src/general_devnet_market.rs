//! The devnet General market compiler: the facts a General release must NAME,
//! read off a real deployment instead of projected off the plan.
//!
//! # What this replaces
//!
//! `general_market::demo_general_market_input` compiles a General-selected
//! market whose four deployment identities are a domain-separated projection
//! of the plan's own release-set id, labelled LAB FACTS in its own docstring
//! "because no local accelerator deployment exists to observe". That is the
//! honest shape for a lab: nothing on a local validator can author them.
//!
//! A devnet accelerator deployment can author exactly one of the four, and it
//! is the load-bearing one. `accelerator_artifact_release` is the identity of
//! the `ArtifactReleaseV1` record the Registry finalizes over a deployment,
//! and it is what every action's `ExecutionStrategyCertificateV2` pins — so a
//! General market compiled with a projected value names an accelerator that
//! does not exist and cannot execute one batch. Here it is OBSERVED: the
//! deployment slot and the upgrade authority are hostile-decoded out of the
//! finalized ProgramData image by `ProgramDataV3View`, the ELF digest is the
//! SHA-256 of the observed tail, and the record is minted by `plan::release_facts` —
//! the same author that mints the seven cohort roles' releases, never a second
//! one.
//!
//! # The other three, and why they are files rather than flags
//!
//! `compiler_release`, `toolchain` and `translation_validation` describe how
//! the accelerator's ELF was PRODUCED. No chain read can author them, and the
//! Direct devnet path does not author them either — Direct's strategies are
//! `Interpreted`, so Direct compiles no `ExecutionStrategyCertificateV2` at
//! all and there is no Direct authority to borrow. They are therefore
//! authenticated inputs, and each one is the digest of REAL EVIDENCE BYTES on
//! disk rather than a hex an operator typed:
//!
//! * `translation_validation` is decoded as a canonical
//!   `CheckedTranslationValidationV1` and its identity comes from that type's
//!   own `translation_validation_id()` — the tree's single author for it. A
//!   file that is not one refuses; there is no hex path.
//! * `compiler_release`, `toolchain` and the selection policy are the SHA-256
//!   of the exact named file, which must begin with that fact's stated header
//!   line. The header is what makes a swapped file refuse by name instead of
//!   hashing to a plausible number, and hashing the exact bytes is what lets a
//!   reader re-derive the identity with `shasum -a 256` and no tooling.
//!
//! This is a real narrowing and not a solved problem: the identities now
//! commit to evidence a reader can inspect, but nothing yet PROVES that the
//! evidence describes this ELF. `docs/evidence/GENERAL_ACCELERATOR_DEVNET_2026_09_02.md`
//! names that as owed.
//!
//! # The policy file
//!
//! The windows and the eleven external account widths are an authenticated
//! input, read as exact canonical JSON by the same reader Direct's devnet plan
//! goes through: an unknown, defaulted or noncanonical field refuses. They are
//! a file rather than seventeen flags because seventeen positional numbers is
//! the shape that goes wrong silently, and because the file is the artifact a
//! cohort runbook carries forward unchanged.

use std::path::{Path, PathBuf};

use dclutch_operator::general_selected_release_v1::{
    GeneralConfigWindowsV1, GeneralDeploymentFactsV1, GeneralSelectedReleaseInputV1,
    general_external_account_widths_v3,
};
use dclutch_registry_contract::ArtifactReleaseV1;
use dclutch_registry_svm::{LOADER_V3_PROGRAM_BYTES, ProgramDataV3View, ProgramV3View};
use dclutch_release_tool::{
    CHECKED_TRANSLATION_VALIDATION_BYTES_V1, CheckedTranslationValidationV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::general_market::{GeneralMarketDerivationV1, general_market_derivation_v1};
use crate::rpc::Rpc;
use crate::{Error, Result};

/// Largest evidence file this compiler will read.
///
/// A toolchain or compiler-release manifest is prose plus digests; a megabyte
/// is three orders of magnitude of headroom and still refuses a file handed in
/// by accident.
const MAX_EVIDENCE_BYTES_V1: usize = 1 << 20;

/// Header line one `compiler_release` manifest must begin with.
const COMPILER_RELEASE_HEADER_V1: &str = "dclutch-general-compiler-release-v1";
/// Header line one `toolchain` manifest must begin with.
const TOOLCHAIN_HEADER_V1: &str = "dclutch-general-toolchain-v1";
/// Header line one interpreted selection policy must begin with.
const SELECTION_POLICY_HEADER_V1: &str = "dclutch-general-selection-policy-v1";
/// Schema every General devnet policy file must state.
///
/// v2 is v1 without the `external_widths` block. The bump is deliberate and the
/// reader refuses v1 rather than ignoring the field: a v1 file states eleven
/// widths, and a compiler that silently dropped them would leave the operator
/// believing the file it wrote still decided something.
const POLICY_SCHEMA_V1: &str = "dclutch-general-devnet-policy-v2";

fn refusal(code: &str, reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: [{code}] {}", reason.as_ref()))
}

/// The deployed accelerator this market's certificates will pin.
pub(crate) struct GeneralDevnetAcceleratorArgumentsV1 {
    /// The deployed accelerator's program address.
    pub(crate) program: Pubkey,
    /// The exact ELF the deployment must be carrying.
    pub(crate) built_elf: PathBuf,
    /// Semantic release identity for the record. Not derivable from bytes:
    /// it says which release this artifact IS, and only its author knows.
    pub(crate) semantic_release_id: [u8; 32],
    /// The authority the observation is CHECKED against. `None` asserts the
    /// deployment is immutable, and an observed authority then refuses.
    pub(crate) expected_upgrade_authority: Option<Pubkey>,
}

/// The three identities no chain can author, plus the selection policy.
pub(crate) struct GeneralDevnetEvidenceArgumentsV1 {
    pub(crate) compiler_release: PathBuf,
    pub(crate) toolchain: PathBuf,
    pub(crate) translation_validation: PathBuf,
    pub(crate) selection_policy: PathBuf,
}

/// Everything the devnet General compiler takes that Direct's does not.
pub(crate) struct GeneralDevnetCompilerArgumentsV1 {
    pub(crate) accelerator: GeneralDevnetAcceleratorArgumentsV1,
    pub(crate) evidence: GeneralDevnetEvidenceArgumentsV1,
    pub(crate) policy: PathBuf,
    /// Immutable authority owning the replaceable quote-surplus token account.
    pub(crate) quote_surplus_beneficiary: Pubkey,
}

/// Exact policy windows a devnet General release pins.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GeneralDevnetWindowsFileV1 {
    pub(crate) collection_slots: u64,
    pub(crate) selection_slots: u64,
    pub(crate) settlement_slots: u64,
    pub(crate) max_orders_per_candidate: u32,
    pub(crate) max_pages_per_candidate: u32,
    pub(crate) continuation_reward_lamports: u64,
}

// THE POLICY FILE NO LONGER STATES EXTERNAL WIDTHS, and the deleted block is
// why the schema moved to v2.
//
// It carried eleven of them, transcribed from `account_rules_v3.rs`'s unit-test
// fixture, and three were wrong against everything the protocol produces: the
// RentCredit at 48 where `LIFECYCLE_RENT_CREDIT_BYTES_V2` is 128, the
// activation cache at 160 where the Registry's is 1,288, and the Core Market at
// 320 where the codec's is 368. Cohort-14 founded
// `8ExdC1RwbyuJweEqT1F6Gk9rgN87uuVaLwtaY2wmr5x` with them, so its `OpenBatch`
// AccountProfile names an `Exact(48)` RentCredit coordinate that no producible
// account fits -- and nothing on the commit path reads it, so the only symptom
// is that the action cannot be delivered.
//
// Nine of the eleven are protocol constants and two are functions of the run
// spec's own Product graph, so none of them was ever a policy choice. They come
// from `general_external_account_widths_v3` now, which is the operator's single
// author for them and the same one the General-hot program-test reads.

/// One authenticated General devnet policy document.
///
/// The `schema` line the file states is checked by the reader and then
/// dropped: carrying it forward would be a second copy of a fact whose only
/// job was to gate the read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GeneralDevnetPolicyFileV1 {
    pub(crate) windows: GeneralDevnetWindowsFileV1,
    pub(crate) token_account_bytes: u32,
}

/// The same document with an owned schema string, which is what serde needs to
/// deserialize it. `GeneralDevnetPolicyFileV1` exists only so the schema can be
/// a `&'static str` in the checked projection.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GeneralDevnetPolicyDocumentV1 {
    schema: String,
    windows: GeneralDevnetWindowsFileV1,
    token_account_bytes: u32,
}

/// One complete accelerator deployment as the chain currently holds it.
pub(crate) struct ObservedAcceleratorDeploymentV1 {
    pub(crate) program: Pubkey,
    pub(crate) programdata: Pubkey,
    pub(crate) deployment_slot: u64,
    pub(crate) elf_digest: [u8; 32],
    pub(crate) upgrade_authority: Option<Pubkey>,
    pub(crate) programdata_bytes: usize,
    pub(crate) live_elf_padding_bytes: usize,
    pub(crate) release: ArtifactReleaseV1,
    pub(crate) artifact_release_id: [u8; 32],
}

/// Observe one deployed accelerator and mint the ArtifactRelease over it.
///
/// The three deployment-derived facts are never supplied. `deployment_slot`
/// and `upgrade_authority` are hostile-decoded out of the finalized ProgramData
/// image by exactly the parse the on-chain authenticator runs, and `elf_digest`
/// is the SHA-256 of the observed tail — the same digest
/// `dclutch_shadow_accelerator_auth_v4::deployment` computes and the same one
/// the Registry compares at finalization. What IS supplied is which program,
/// which built ELF the deployment must be carrying, which semantic release it
/// is, and which authority the observation is checked against — so an
/// accelerator that quietly became mutable under a stranger's key refuses here
/// instead of minting a release that hands them the hot path.
///
/// The minted record is then re-authenticated against its own observation by
/// `ArtifactReleaseV1::authenticate_deployment`, which is the SAME eight
/// conjuncts the Registry runs at `Finalize`. Two authorities over one
/// observation: if this compiler and the chain can disagree, they disagree
/// here, offline, and not after a cohort has founded a market on it.
pub(crate) fn observe_accelerator_deployment_v1(
    rpc: &mut Rpc,
    arguments: &GeneralDevnetAcceleratorArgumentsV1,
    floor_slot: u64,
) -> Result<ObservedAcceleratorDeploymentV1> {
    let program = arguments.program;
    let programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    let (finalized_slot, accounts) = rpc.finalized_accounts(&[program, programdata], floor_slot)?;
    if finalized_slot < floor_slot || accounts.len() != 2 {
        return Err(refusal(
            "general-devnet/accelerator-snapshot",
            format!(
                "finalized accelerator snapshot was below its floor {floor_slot} or changed its exact two-account width"
            ),
        ));
    }
    let mut accounts = accounts.into_iter();
    let program_account = accounts.next().flatten().ok_or_else(|| {
        refusal(
            "general-devnet/accelerator-missing",
            format!("no Program account at {program}"),
        )
    })?;
    let programdata_account = accounts.next().flatten().ok_or_else(|| {
        refusal(
            "general-devnet/accelerator-missing",
            format!("no ProgramData account at {programdata} for {program}"),
        )
    })?;

    let program_view = ProgramV3View::parse(&program_account.data).map_err(|error| {
        refusal(
            "general-devnet/accelerator-loader",
            format!("accelerator Program account: {error:?}"),
        )
    })?;
    let programdata_view =
        ProgramDataV3View::parse(&programdata_account.data).map_err(|error| {
            refusal(
                "general-devnet/accelerator-loader",
                format!("accelerator ProgramData account: {error:?}"),
            )
        })?;
    if program_account.owner != bpf_loader_upgradeable::ID
        || !program_account.executable
        || program_account.data.len() != LOADER_V3_PROGRAM_BYTES
        || programdata_account.owner != bpf_loader_upgradeable::ID
        || programdata_account.executable
        || program_view.programdata() != programdata.to_bytes()
    {
        return Err(refusal(
            "general-devnet/accelerator-loader",
            format!(
                "{program} is not a Loader V3 deployment linked to {programdata}: owner {}, executable {}, program bytes {}",
                program_account.owner,
                program_account.executable,
                program_account.data.len()
            ),
        ));
    }

    let built = std::fs::read(&arguments.built_elf).map_err(|error| {
        refusal(
            "general-devnet/accelerator-elf",
            format!("read {}: {error}", arguments.built_elf.display()),
        )
    })?;
    let live = programdata_view.elf();
    let padding = live.len().checked_sub(built.len()).ok_or_else(|| {
        refusal(
            "general-devnet/accelerator-elf",
            format!(
                "the deployment carries {} ELF bytes, fewer than the {} the named build has",
                live.len(),
                built.len()
            ),
        )
    })?;
    if live.get(..built.len()) != Some(built.as_slice())
        || live[built.len()..].iter().any(|byte| *byte != 0)
    {
        return Err(refusal(
            "general-devnet/accelerator-elf",
            format!(
                "the ELF deployed at {program} is not {} followed by {padding} zero bytes",
                arguments.built_elf.display()
            ),
        ));
    }

    let observed_authority = programdata_view.upgrade_authority();
    let expected_authority = arguments
        .expected_upgrade_authority
        .map(|key| key.to_bytes());
    if observed_authority != expected_authority {
        return Err(refusal(
            "general-devnet/accelerator-authority",
            match (observed_authority, expected_authority) {
                (Some(observed), Some(_)) => format!(
                    "{program} is upgradeable under {}, not the authority named",
                    Pubkey::new_from_array(observed)
                ),
                (Some(observed), None) => format!(
                    "{program} was named as immutable and is upgradeable under {}",
                    Pubkey::new_from_array(observed)
                ),
                (None, Some(expected)) => format!(
                    "{program} carries no upgrade authority; {} was named",
                    Pubkey::new_from_array(expected)
                ),
                (None, None) => unreachable!("equal authorities cannot reach this arm"),
            },
        ));
    }

    let elf_digest: [u8; 32] = Sha256::digest(live).into();
    // The seven cohort roles' releases and this one are minted by ONE author.
    let facts = crate::plan::release_facts(
        program,
        arguments.semantic_release_id,
        elf_digest,
        programdata_view.deployment_slot(),
        observed_authority,
    )?;
    let observation = crate::plan::deployment_observation_v1(
        program,
        program_account.owner,
        program_account.executable,
        programdata,
        programdata_account.owner,
        programdata_account.executable,
        program_view.programdata(),
        programdata_view.deployment_slot(),
        elf_digest,
        observed_authority,
    )?;
    facts
        .release
        .authenticate_deployment(observation)
        .map_err(|error| {
            refusal(
                "general-devnet/accelerator-observation",
                format!(
                    "the minted ArtifactRelease does not authenticate its own observation of {program}: {error:?}"
                ),
            )
        })?;

    Ok(ObservedAcceleratorDeploymentV1 {
        program,
        programdata,
        deployment_slot: programdata_view.deployment_slot(),
        elf_digest,
        upgrade_authority: observed_authority.map(Pubkey::new_from_array),
        programdata_bytes: programdata_account.data.len(),
        live_elf_padding_bytes: padding,
        release: facts.release,
        artifact_release_id: facts.id.to_bytes(),
    })
}

impl ObservedAcceleratorDeploymentV1 {
    /// The deployment, as a block a runbook can carry and a reader can check.
    ///
    /// It goes to STDERR, never stdout: stdout is the market document a
    /// campaign consumes and a second object on it would corrupt the only
    /// output this command has. The `artifact_release_body` is the exact
    /// 216-byte record a cohort must publish into its Registry under
    /// `ARTIFACT_RELEASE_SCHEMA_ID_V1` — finalizing it is what makes the
    /// certificates this market compiles executable, because the Registry
    /// observes the deployment at that moment and refuses a record that
    /// describes a different one.
    pub(crate) fn render_provenance_v1(&self) -> String {
        let hex =
            |bytes: &[u8]| -> String { bytes.iter().map(|byte| format!("{byte:02x}")).collect() };
        let mut output = String::new();
        output.push_str("dclutch-general-devnet-accelerator-observation-v1\n");
        output.push_str(&format!("program                {}\n", self.program));
        output.push_str(&format!("programdata            {}\n", self.programdata));
        output.push_str(&format!(
            "deployment_slot        {}\n",
            self.deployment_slot
        ));
        output.push_str(&format!(
            "elf_digest             {}\n",
            hex(&self.elf_digest)
        ));
        output.push_str(&format!(
            "programdata_bytes      {}\n",
            self.programdata_bytes
        ));
        output.push_str(&format!(
            "live_elf_padding_bytes {}\n",
            self.live_elf_padding_bytes
        ));
        output.push_str(&format!(
            "upgrade_policy         {}\n",
            match self.upgrade_authority {
                Some(_) => "exact-authority",
                None => "immutable",
            }
        ));
        output.push_str(&format!(
            "upgrade_authority      {}\n",
            match self.upgrade_authority {
                Some(key) => key.to_string(),
                None => "(none)".to_owned(),
            }
        ));
        output.push_str(&format!(
            "artifact_release_id    {}\n",
            hex(&self.artifact_release_id)
        ));
        output.push_str(&format!(
            "artifact_release_body  {}\n",
            hex(&self.release.to_bytes())
        ));
        output
    }
}

/// One evidence file's identity: the SHA-256 of its exact bytes, after its
/// stated header line is checked.
fn named_evidence_identity_v1(path: &Path, header: &str, label: &str) -> Result<[u8; 32]> {
    let bytes = std::fs::read(path).map_err(|error| {
        refusal(
            "general-devnet/evidence-unreadable",
            format!("{label}: read {}: {error}", path.display()),
        )
    })?;
    if bytes.len() > MAX_EVIDENCE_BYTES_V1 {
        return Err(refusal(
            "general-devnet/evidence-unreadable",
            format!(
                "{label}: {} is {} bytes, past the {MAX_EVIDENCE_BYTES_V1}-byte ceiling",
                path.display(),
                bytes.len()
            ),
        ));
    }
    let first = bytes
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    if first != header.as_bytes() {
        return Err(refusal(
            "general-devnet/evidence-header",
            format!(
                "{label}: {} does not begin with the line {header:?}. The header is what makes a \
                 swapped evidence file refuse by name instead of hashing to a plausible identity",
                path.display()
            ),
        ));
    }
    if bytes.len() <= header.len() {
        return Err(refusal(
            "general-devnet/evidence-header",
            format!(
                "{label}: {} is its header line and nothing else",
                path.display()
            ),
        ));
    }
    Ok(Sha256::digest(&bytes).into())
}

/// The translation-validation identity, from the tree's single author for it.
///
/// There is no hex path. A `CheckedTranslationValidationV1` names 21 exact
/// evidence digests and `translation_validation_id` is a function of all of
/// them, so a file that is not one cannot be substituted for one.
fn translation_validation_identity_v1(path: &Path) -> Result<[u8; 32]> {
    let bytes = std::fs::read(path).map_err(|error| {
        refusal(
            "general-devnet/evidence-unreadable",
            format!("translation validation: read {}: {error}", path.display()),
        )
    })?;
    if bytes.len() != CHECKED_TRANSLATION_VALIDATION_BYTES_V1 {
        return Err(refusal(
            "general-devnet/translation-validation",
            format!(
                "{} is {} bytes; one canonical checked translation-validation manifest is {CHECKED_TRANSLATION_VALIDATION_BYTES_V1}",
                path.display(),
                bytes.len()
            ),
        ));
    }
    let manifest = CheckedTranslationValidationV1::decode(&bytes).map_err(|error| {
        refusal(
            "general-devnet/translation-validation",
            format!("{}: {error:?}", path.display()),
        )
    })?;
    let identity = manifest.translation_validation_id().map_err(|error| {
        refusal(
            "general-devnet/translation-validation",
            format!("{}: {error:?}", path.display()),
        )
    })?;
    Ok(identity.to_bytes())
}

/// Read one authenticated General devnet policy document.
pub(crate) fn read_general_devnet_policy_v1(path: &Path) -> Result<GeneralDevnetPolicyFileV1> {
    let document: GeneralDevnetPolicyDocumentV1 =
        crate::direct_market::read_exact_json_v1(path, "General devnet policy")?;
    if document.schema != POLICY_SCHEMA_V1 {
        return Err(refusal(
            "general-devnet/policy-schema",
            format!(
                "{} states schema {:?}; this compiler reads {POLICY_SCHEMA_V1}",
                path.display(),
                document.schema
            ),
        ));
    }
    Ok(GeneralDevnetPolicyFileV1 {
        windows: document.windows,
        token_account_bytes: document.token_account_bytes,
    })
}

/// Compile one devnet General-selected market.
///
/// The market graph is the flagship's own — `devnet_sponsored_market_input_base`,
/// the identical capability-free document `devnet-sponsored-market` compiles —
/// and the only difference is which family is attached to it through the
/// neutral seam. Direct appears nowhere in the result.
pub(crate) fn devnet_general_market_input(
    plan_path: &Path,
    rpc_url: &str,
    devnet_acknowledgment: Option<&str>,
    registry: Pubkey,
    spec: crate::market::DevnetPythMarketSpecV1<'_>,
    arguments: &GeneralDevnetCompilerArgumentsV1,
    sponsored_release: dclutch_pyth_svm::PythSponsoredPushReleaseV1,
) -> Result<crate::model::MarketRunInput> {
    let (plan, mut rpc, observation) = crate::direct_market::observe_devnet_market_policy_v1(
        plan_path,
        rpc_url,
        devnet_acknowledgment,
        registry,
    )?;
    let resolution_release = crate::direct_market::authenticated_resolution_release_v1(&plan)?;
    let mut input = crate::market::devnet_sponsored_market_input_base(
        spec,
        resolution_release,
        sponsored_release,
    )?;

    // The accelerator is observed against the SAME finalized floor the market
    // policy snapshot was taken at, so the deployment this market's
    // certificates pin cannot be older than the substrate it is founded on.
    let deployment = observe_accelerator_deployment_v1(
        &mut rpc,
        &arguments.accelerator,
        observation.finalized_slot,
    )?;
    eprint!("{}", deployment.render_provenance_v1());

    attach_devnet_general_capability_v1(
        &mut input,
        &observation,
        deployment.artifact_release_id,
        arguments,
    )?;
    crate::market::validate_market_input(&input)?;
    Ok(input)
}

/// The PURE half of the devnet General compile: everything after the two
/// observations.
///
/// It is separated for one reason — this is the part a test can pin. The
/// observations need a cluster and the accelerator's artifact release id is
/// their only output that reaches here, so a test that supplies the REAL id
/// this lane read off devnet exercises every remaining derivation: the four
/// evidence readers, the policy document, the five facts read off the market
/// body, the release compiler, the root width, the Rent quote at that width,
/// and the neutral seam's merge.
pub(crate) fn attach_devnet_general_capability_v1(
    input: &mut crate::model::MarketRunInput,
    observation: &crate::direct_market::DirectDevnetPolicyObservationV1,
    accelerator_artifact_release: [u8; 32],
    arguments: &GeneralDevnetCompilerArgumentsV1,
) -> Result<()> {
    let policy = read_general_devnet_policy_v1(&arguments.policy)?;
    let selection_policy = named_evidence_identity_v1(
        &arguments.evidence.selection_policy,
        SELECTION_POLICY_HEADER_V1,
        "selection policy",
    )?;
    let compiler_release = named_evidence_identity_v1(
        &arguments.evidence.compiler_release,
        COMPILER_RELEASE_HEADER_V1,
        "compiler release",
    )?;
    let toolchain = named_evidence_identity_v1(
        &arguments.evidence.toolchain,
        TOOLCHAIN_HEADER_V1,
        "toolchain",
    )?;
    let translation_validation =
        translation_validation_identity_v1(&arguments.evidence.translation_validation)?;

    let GeneralMarketDerivationV1 {
        capacity_profile,
        claim_basis,
        outcome_count,
        price_scale,
        generation,
        linked_basis_prefix,
        result_domain,
    } = general_market_derivation_v1(input)?;

    let release_input = GeneralSelectedReleaseInputV1 {
        capacity_profile,
        claim_basis,
        selection_policy,
        quote_surplus_beneficiary: arguments.quote_surplus_beneficiary.to_bytes(),
        generation,
        price_scale,
        windows: GeneralConfigWindowsV1 {
            collection_slots: policy.windows.collection_slots,
            selection_slots: policy.windows.selection_slots,
            settlement_slots: policy.windows.settlement_slots,
            max_orders_per_candidate: policy.windows.max_orders_per_candidate,
            max_pages_per_candidate: policy.windows.max_pages_per_candidate,
            continuation_reward_lamports: policy.windows.continuation_reward_lamports,
        },
        outcome_count,
        external_widths: general_external_account_widths_v3(linked_basis_prefix, result_domain),
        token_account_bytes: policy.token_account_bytes,
        deployment: GeneralDeploymentFactsV1 {
            accelerator_artifact_release,
            compiler_release,
            toolchain,
            translation_validation,
        },
    };

    let closure = crate::general_market::general_selected_closure_v1(release_input)?;
    let root_bytes = crate::general_market::general_root_bytes_v1(&closure)?;
    let payload = crate::general_market::general_selected_payload_v1(
        &closure,
        observation.activation_deadline_slot_v1()?,
        observation.root_rent_minimum_for_width_v1(root_bytes)?,
    );
    crate::selected_capability::attach_selected_capability_v1(input, payload)?;
    if input.direct_capability.is_some() {
        return Err(refusal(
            "general-devnet/direct-attached",
            "a General-selected devnet market carries no Direct closure",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence_file(name: &str, body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "dclutch-general-devnet-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::write(&path, body).expect("write evidence fixture");
        path
    }

    /// The header is load-bearing: a file whose bytes are perfectly good
    /// evidence for something ELSE hashes to a plausible 32 bytes, and the
    /// only thing standing between that and a certificate asserting it is
    /// this line. So the refusal is named and the identity is not produced.
    #[test]
    fn a_swapped_evidence_file_refuses_by_name_instead_of_hashing() {
        let wrong = evidence_file("wrong", "dclutch-general-toolchain-v1\nrustc 1.97.1\n");
        let error =
            named_evidence_identity_v1(&wrong, COMPILER_RELEASE_HEADER_V1, "compiler release")
                .expect_err("a toolchain manifest is not a compiler release");
        assert!(
            error.0.contains("general-devnet/evidence-header"),
            "{}",
            error.0
        );
        let right = named_evidence_identity_v1(&wrong, TOOLCHAIN_HEADER_V1, "toolchain")
            .expect("the same bytes under their own header");
        assert_eq!(
            right,
            Sha256::digest(std::fs::read(&wrong).expect("read")).as_slice(),
            "the identity is the SHA-256 of the exact file, re-derivable with shasum",
        );
        let _ = std::fs::remove_file(&wrong);
    }

    /// A header line and nothing else is not evidence, and would otherwise
    /// give every fact of that class one shared identity.
    #[test]
    fn a_header_with_no_evidence_under_it_refuses() {
        let empty = evidence_file("empty", "dclutch-general-toolchain-v1");
        let error = named_evidence_identity_v1(&empty, TOOLCHAIN_HEADER_V1, "toolchain")
            .expect_err("a bare header is not evidence");
        assert!(
            error.0.contains("general-devnet/evidence-header"),
            "{}",
            error.0
        );
        let _ = std::fs::remove_file(&empty);
    }

    /// There is no hex path to `translation_validation`. A 32-byte file that
    /// an operator might reasonably think IS the identity refuses on width,
    /// because the identity is a function of 21 evidence digests and nothing
    /// shorter can carry them.
    #[test]
    fn the_translation_validation_identity_has_no_hex_path() {
        let hex = evidence_file("hex", &"a".repeat(64));
        let error = translation_validation_identity_v1(&hex)
            .expect_err("a hex string is not a checked translation-validation manifest");
        assert!(
            error.0.contains("general-devnet/translation-validation"),
            "{}",
            error.0
        );
        let _ = std::fs::remove_file(&hex);
    }

    /// The policy document is read by Direct's exact-JSON reader, so an
    /// unknown field is a refusal rather than a silently ignored belief.
    #[test]
    fn the_policy_document_refuses_an_unknown_field() {
        let path = evidence_file(
            "policy.json",
            r#"{"schema":"dclutch-general-devnet-policy-v2","windows":{"collection_slots":16,"selection_slots":16,"settlement_slots":64,"max_orders_per_candidate":32,"max_pages_per_candidate":32,"continuation_reward_lamports":1},"token_account_bytes":165,"settlement_slots":64}"#,
        );
        let error = read_general_devnet_policy_v1(&path).expect_err("an unknown field refuses");
        assert!(
            error.0.contains("unknown field `settlement_slots`"),
            "{}",
            error.0
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A stale policy document refuses, and the two arms say different things.
    ///
    /// THE FIRST FIXTURE IS THE REAL v1 DOCUMENT cohort-14 compiled with,
    /// widths and all -- the file an operator still has on disk. It refuses on
    /// the FIELD, not on the schema, because `read_exact_json_v1` deserializes
    /// before the schema string is read; that ordering is worth keeping rather
    /// than working around, because "unknown field `external_widths`" names the
    /// thing that stopped being a policy question and "wrong schema" does not.
    ///
    /// The second arm is the schema check itself, which still has to exist: a
    /// document that is well-formed for this reader and meant for another
    /// compiler must not be read.
    #[test]
    fn the_policy_document_refuses_a_foreign_schema() {
        let stale = evidence_file(
            "policy-foreign.json",
            r#"{"schema":"dclutch-general-devnet-policy-v1","windows":{"collection_slots":16,"selection_slots":16,"settlement_slots":64,"max_orders_per_candidate":32,"max_pages_per_candidate":32,"continuation_reward_lamports":1},"external_widths":{"linked_basis_prefix":256,"result_domain":192,"rent_sysvar":17,"core_market":320,"activation_cache":160,"upgradeable_program":36,"trading_programdata_prefix":45,"claims_programdata_prefix":45,"core_programdata_prefix":45,"realm_record":112,"rent_credit":48},"token_account_bytes":165}"#,
        );
        let error = read_general_devnet_policy_v1(&stale).expect_err("the v1 document refuses");
        assert!(
            error.0.contains("unknown field `external_widths`"),
            "{}",
            error.0
        );
        let _ = std::fs::remove_file(&stale);

        let foreign = evidence_file(
            "policy-foreign-schema.json",
            r#"{"schema":"dclutch-general-devnet-policy-v1","windows":{"collection_slots":16,"selection_slots":16,"settlement_slots":64,"max_orders_per_candidate":32,"max_pages_per_candidate":32,"continuation_reward_lamports":1},"token_account_bytes":165}"#,
        );
        let error = read_general_devnet_policy_v1(&foreign).expect_err("a foreign schema refuses");
        assert!(
            error.0.contains("general-devnet/policy-schema"),
            "{}",
            error.0
        );
        let _ = std::fs::remove_file(&foreign);
    }

    /// And the canonical document reads back exactly what it states -- which
    /// no longer includes a single account width.
    #[test]
    fn the_policy_document_states_windows_and_no_external_width() {
        let path = evidence_file(
            "policy-ok.json",
            r#"{"schema":"dclutch-general-devnet-policy-v2","windows":{"collection_slots":16,"selection_slots":16,"settlement_slots":64,"max_orders_per_candidate":32,"max_pages_per_candidate":32,"continuation_reward_lamports":1},"token_account_bytes":165}"#,
        );
        let policy = read_general_devnet_policy_v1(&path).expect("canonical policy");
        assert_eq!(policy.windows.settlement_slots, 64);
        assert_eq!(policy.token_account_bytes, 165);
        let _ = std::fs::remove_file(&path);
        // A v2 document that still carries the block refuses: the widths did
        // not become optional, they stopped being a policy question.
        let stale = evidence_file(
            "policy-stale-widths.json",
            r#"{"schema":"dclutch-general-devnet-policy-v2","windows":{"collection_slots":16,"selection_slots":16,"settlement_slots":64,"max_orders_per_candidate":32,"max_pages_per_candidate":32,"continuation_reward_lamports":1},"external_widths":{"rent_credit":48},"token_account_bytes":165}"#,
        );
        let error = read_general_devnet_policy_v1(&stale).expect_err("a stated width refuses");
        assert!(
            error.0.contains("unknown field `external_widths`"),
            "{}",
            error.0
        );
        let _ = std::fs::remove_file(&stale);
    }

    fn write_evidence_fixtures() -> GeneralDevnetCompilerArgumentsV1 {
        use dclutch_release_tool::build_checked_translation_validation;

        // A real canonical manifest, built by the tree's own author over
        // twenty-one evidence blobs, so the identity below is the one
        // `translation_validation_id` computes and not a fixture constant.
        let lean = [b"lean-0".as_slice(); 8];
        let rust = [b"rust-0".as_slice(); 8];
        let manifest = build_checked_translation_validation(
            dclutch_release_tool::TranslationValidationEvidenceV1 {
                corpus: b"dclutch-direct-translation-corpus-v1\nrow\n",
                lean_sources: lean,
                rust_sources: rust,
                validator_result: b"translation validation passed: 1 case\n",
                rustc_verbose: b"rustc 1.97.1\n",
                lake_version: b"Lake version 5.0.0\n",
                validator_cargo_lock:
                    b"# This file is automatically @generated by Cargo.\n[[package]]\n",
            },
        )
        .expect("checked translation validation");
        let translation = evidence_file("tv.bin", "");
        std::fs::write(&translation, manifest.encode()).expect("write manifest");

        GeneralDevnetCompilerArgumentsV1 {
            accelerator: GeneralDevnetAcceleratorArgumentsV1 {
                program: Pubkey::new_from_array([0x11; 32]),
                built_elf: PathBuf::from("/unused-by-the-pure-half"),
                semantic_release_id: [0x12; 32],
                expected_upgrade_authority: None,
            },
            evidence: GeneralDevnetEvidenceArgumentsV1 {
                compiler_release: evidence_file(
                    "compiler.txt",
                    "dclutch-general-compiler-release-v1\nrevision 324528a4\n",
                ),
                toolchain: evidence_file(
                    "toolchain.txt",
                    "dclutch-general-toolchain-v1\nrustc 1.97.1\nsbf-solana-solana\n",
                ),
                translation_validation: translation,
                selection_policy: evidence_file(
                    "selection.txt",
                    "dclutch-general-selection-policy-v1\nbest-valid-submitted-candidate\n",
                ),
            },
            policy: evidence_file(
                "policy-compile.json",
                r#"{"schema":"dclutch-general-devnet-policy-v2","windows":{"collection_slots":16,"selection_slots":16,"settlement_slots":64,"max_orders_per_candidate":32,"max_pages_per_candidate":32,"continuation_reward_lamports":1},"token_account_bytes":165}"#,
            ),
            quote_surplus_beneficiary: Pubkey::new_from_array([0x44; 32]),
        }
    }

    /// The identity of the accelerator this lane really deployed, derived
    /// through `plan::release_facts` from the facts the chain really carries —
    /// never transcribed, so a change to the record layout moves it here too.
    fn live_devnet_accelerator_release_id() -> [u8; 32] {
        let program = "8pgnyNvgdue7Jc8aw75BGWoghsKGevWJvFom8omUWvQY"
            .parse::<Pubkey>()
            .expect("deployed accelerator");
        let authority = "4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP"
            .parse::<Pubkey>()
            .expect("deployer");
        let elf_digest = {
            let text = "61b2d73d44f2470051b40e39cda1d31a5f67679429eacd5448d5e5ac583b74ae";
            let mut bytes = [0_u8; 32];
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("hex");
            }
            bytes
        };
        crate::plan::release_facts(
            program,
            // The accelerator has no protocol-owned semantic release identity;
            // see the evidence doc. Not one of the record's checked conjuncts.
            elf_digest,
            elf_digest,
            491_959_038,
            Some(authority.to_bytes()),
        )
        .expect("the deployed accelerator's release")
        .id
        .to_bytes()
    }

    fn devnet_spec<'a>(
        registry: Pubkey,
        price: &'a [u8],
        update: &dclutch_pyth_svm::FullPriceUpdateV2,
    ) -> crate::market::DevnetPythMarketSpecV1<'a> {
        crate::market::DevnetPythMarketSpecV1 {
            founding_band: crate::market::LocalMarketShapeV1::default().founding_band,
            registry,
            price_update: price,
            product_name: "product/sol-usd-sponsored-range-protection",
            coordinate_domain_name: "coordinate-domain/usd-cents-per-sol",
            feed_label: b"sol-usd-sponsored",
            window_start: update.publish_time() - 1_800,
            window_width_seconds: 1_800,
            max_age_seconds: 7_200,
            cut_denominator: 100,
            cuts: vec![14_800, 15_200],
            coefficients: vec![1, 0, 1, 0],
            generation: 1,
        }
    }

    /// **The whole compile, minus the two chain reads, over the REAL
    /// accelerator this lane deployed.**
    ///
    /// The devnet flagship graph with General attached instead of Direct: the
    /// founding validator accepts it, Direct appears nowhere in it, and the
    /// manifest carries exactly one General entry whose four coordinates are
    /// the ones the publication authors. That last part is the seam's
    /// capability-neutrality claim checked over a devnet document rather than
    /// a lab one.
    #[test]
    fn the_devnet_compiler_founds_a_general_market_over_the_real_accelerator_release() {
        use dclutch_capability_contract::CapabilityManifestV1;
        use dclutch_general_config_contract::GENERAL_CAPABILITY_KIND_ID_V1;

        let registry = Pubkey::new_from_array([0x41; 32]);
        let release =
            dclutch_pyth_svm::devnet_sponsored_sol_usd_release_v1().expect("sponsored release row");
        let mut price = crate::market::FIXTURE_PRICE_UPDATE.to_vec();
        price[8..40].copy_from_slice(&release.price_account());
        price[41..73].copy_from_slice(&release.feed_id());
        let update = dclutch_pyth_svm::FullPriceUpdateV2::parse(&price).expect("price update");

        let mut input = crate::market::devnet_sponsored_market_input_base(
            devnet_spec(registry, &price, &update),
            dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V7,
            release,
        )
        .expect("the capability-free devnet flagship graph");

        let arguments = write_evidence_fixtures();
        let observation =
            crate::direct_market::DirectDevnetPolicyObservationV1::for_test(491_959_100);
        let accelerator = live_devnet_accelerator_release_id();
        attach_devnet_general_capability_v1(&mut input, &observation, accelerator, &arguments)
            .expect("the devnet General closure attaches through the neutral seam");
        crate::market::validate_market_input(&input)
            .expect("the founding validator accepts the devnet General market input");

        assert!(
            input.direct_capability.is_none(),
            "a General-selected devnet market carries no Direct closure"
        );
        let payload = input
            .selected_capability
            .as_ref()
            .expect("a selected closure");
        assert_eq!(payload.family, "general");

        // Exactly one General entry, and its coordinates are the publication's.
        let manifest_bytes =
            crate::runtime::decode_hex(&input.capability_manifest_hex).expect("manifest hex");
        let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest");
        let general = (0..manifest.entry_count())
            .filter_map(|index| manifest.entry(index).ok())
            .filter(|entry| entry.kind_id().to_bytes() == GENERAL_CAPABILITY_KIND_ID_V1)
            .collect::<Vec<_>>();
        assert_eq!(general.len(), 1, "exactly one General entry");
        assert!(
            !payload.publication_hex.is_empty(),
            "the entry travels with the publication that authors it"
        );

        // **The accelerator is INSIDE the manifest entry, not beside it.**
        // Recompiling the same market against a different accelerator must
        // move the entry's release id, because the artifact release is bound
        // in every action's certificate, which is hashed into the strategy,
        // which is named by the descriptor, which is in the program set. If it
        // did not move, a market founded here could execute against an
        // accelerator nobody admitted. Checked this way rather than by reading
        // the publication, which has no decoder to borrow and whose byte
        // layout a test must not become a second author for.
        let mut other = accelerator;
        other[0] ^= 0x01;
        let mut rival = crate::market::devnet_sponsored_market_input_base(
            devnet_spec(registry, &price, &update),
            dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V7,
            release,
        )
        .expect("a second capability-free graph");
        attach_devnet_general_capability_v1(&mut rival, &observation, other, &arguments)
            .expect("the same market against a different accelerator");
        let rival_bytes =
            crate::runtime::decode_hex(&rival.capability_manifest_hex).expect("rival hex");
        let rival_manifest = CapabilityManifestV1::decode(&rival_bytes).expect("rival manifest");
        let rival_entry = (0..rival_manifest.entry_count())
            .filter_map(|index| rival_manifest.entry(index).ok())
            .find(|entry| entry.kind_id().to_bytes() == GENERAL_CAPABILITY_KIND_ID_V1)
            .expect("the rival General entry");
        assert_ne!(
            general[0].release_id().to_bytes(),
            rival_entry.release_id().to_bytes(),
            "one flipped bit of the accelerator identity must move the release id"
        );
        // The CONFIG moves too, and finding that out is worth the test.
        // `GeneralConfigV3` carries no deployment FIELD -- capacity, claim
        // basis, program-set identity, generation, windows -- but it binds
        // `program_set_id`, and the program set is downstream of the
        // certificate that names the accelerator. So one flipped bit of the
        // artifact release moves the whole entry, not merely its release id.
        // This does not disturb why General is foundable: the dependency runs
        // accelerator -> certificate -> strategy -> descriptor -> program set
        // -> config -> manifest -> Market, strictly one way, and no step reads
        // the Market. Acyclic, exactly as the selection seam claims.
        assert_ne!(
            general[0].config_id().to_bytes(),
            rival_entry.config_id().to_bytes(),
            "the config binds the program set, so the accelerator reaches it too"
        );
        assert_eq!(
            general[0].capacity_profile_id().to_bytes(),
            rival_entry.capacity_profile_id().to_bytes(),
            "and the capacity profile is the market's, untouched by either"
        );

        for path in [
            &arguments.evidence.compiler_release,
            &arguments.evidence.toolchain,
            &arguments.evidence.translation_validation,
            &arguments.evidence.selection_policy,
            &arguments.policy,
        ] {
            let _ = std::fs::remove_file(path);
        }
    }

    /// The devnet compiler and the lab compiler mint the SAME closure from the
    /// same release input, so "devnet" is a difference in where the facts came
    /// from and never a difference in what is compiled from them.
    #[test]
    fn the_devnet_compiler_shares_the_lab_compilers_closure_author() {
        let input = crate::general_market::test_release_input_v1();
        let lab = crate::general_market::general_selected_closure_v1(input).expect("lab closure");
        let devnet = crate::general_market::general_selected_closure_v1(input).expect("devnet");
        assert_eq!(lab.publication, devnet.publication);
        assert_eq!(lab.publication_id, devnet.publication_id);
    }
}
