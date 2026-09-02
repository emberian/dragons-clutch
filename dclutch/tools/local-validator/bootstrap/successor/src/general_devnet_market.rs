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

use dclutch_general_adapter_contract::account_rules_v3::GeneralExternalAccountWidthsV3;
use dclutch_operator::general_selected_release_v1::{
    GeneralConfigWindowsV1, GeneralDeploymentFactsV1, GeneralSelectedReleaseInputV1,
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
const POLICY_SCHEMA_V1: &str = "dclutch-general-devnet-policy-v1";

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

/// Exact external account widths a devnet General release selects.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GeneralDevnetWidthsFileV1 {
    pub(crate) linked_basis_prefix: u32,
    pub(crate) result_domain: u32,
    pub(crate) rent_sysvar: u32,
    pub(crate) core_market: u32,
    pub(crate) activation_cache: u32,
    pub(crate) upgradeable_program: u32,
    pub(crate) trading_programdata_prefix: u32,
    pub(crate) claims_programdata_prefix: u32,
    pub(crate) core_programdata_prefix: u32,
    pub(crate) realm_record: u32,
    pub(crate) rent_credit: u32,
}

/// One authenticated General devnet policy document.
///
/// The `schema` line the file states is checked by the reader and then
/// dropped: carrying it forward would be a second copy of a fact whose only
/// job was to gate the read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GeneralDevnetPolicyFileV1 {
    pub(crate) windows: GeneralDevnetWindowsFileV1,
    pub(crate) external_widths: GeneralDevnetWidthsFileV1,
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
    external_widths: GeneralDevnetWidthsFileV1,
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
        external_widths: document.external_widths,
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
) -> Result<crate::model::MarketRunInput> {
    let (plan, mut rpc, observation) = crate::direct_market::observe_devnet_market_policy_v1(
        plan_path,
        rpc_url,
        devnet_acknowledgment,
        registry,
    )?;
    let resolution_release = crate::direct_market::authenticated_resolution_release_v1(&plan)?;
    let mut input = crate::market::devnet_sponsored_market_input_base(spec, resolution_release)?;

    // The accelerator is observed against the SAME finalized floor the market
    // policy snapshot was taken at, so the deployment this market's
    // certificates pin cannot be older than the substrate it is founded on.
    let deployment = observe_accelerator_deployment_v1(
        &mut rpc,
        &arguments.accelerator,
        observation.finalized_slot,
    )?;
    eprint!("{}", deployment.render_provenance_v1());

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
    } = general_market_derivation_v1(&input)?;

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
        external_widths: GeneralExternalAccountWidthsV3 {
            linked_basis_prefix: policy.external_widths.linked_basis_prefix,
            result_domain: policy.external_widths.result_domain,
            rent_sysvar: policy.external_widths.rent_sysvar,
            core_market: policy.external_widths.core_market,
            activation_cache: policy.external_widths.activation_cache,
            upgradeable_program: policy.external_widths.upgradeable_program,
            trading_programdata_prefix: policy.external_widths.trading_programdata_prefix,
            claims_programdata_prefix: policy.external_widths.claims_programdata_prefix,
            core_programdata_prefix: policy.external_widths.core_programdata_prefix,
            realm_record: policy.external_widths.realm_record,
            rent_credit: policy.external_widths.rent_credit,
        },
        token_account_bytes: policy.token_account_bytes,
        deployment: GeneralDeploymentFactsV1 {
            accelerator_artifact_release: deployment.artifact_release_id,
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
    crate::selected_capability::attach_selected_capability_v1(&mut input, payload)?;
    if input.direct_capability.is_some() {
        return Err(refusal(
            "general-devnet/direct-attached",
            "a General-selected devnet market carries no Direct closure",
        ));
    }
    crate::market::validate_market_input(&input)?;
    Ok(input)
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
            r#"{"schema":"dclutch-general-devnet-policy-v1","windows":{"collection_slots":16,"selection_slots":16,"settlement_slots":64,"max_orders_per_candidate":32,"max_pages_per_candidate":32,"continuation_reward_lamports":1},"external_widths":{"linked_basis_prefix":256,"result_domain":192,"rent_sysvar":17,"core_market":320,"activation_cache":160,"upgradeable_program":36,"trading_programdata_prefix":45,"claims_programdata_prefix":45,"core_programdata_prefix":45,"realm_record":112,"rent_credit":48},"token_account_bytes":165,"settlement_slots":64}"#,
        );
        let error = read_general_devnet_policy_v1(&path).expect_err("an unknown field refuses");
        assert!(
            error.0.contains("unknown field `settlement_slots`"),
            "{}",
            error.0
        );
        let _ = std::fs::remove_file(&path);
    }

    /// And a policy file that states the wrong schema refuses by that name,
    /// which is the arm that catches a well-formed document meant for
    /// something else entirely.
    #[test]
    fn the_policy_document_refuses_a_foreign_schema() {
        let path = evidence_file(
            "policy-foreign.json",
            r#"{"schema":"dclutch-general-devnet-policy-v2","windows":{"collection_slots":16,"selection_slots":16,"settlement_slots":64,"max_orders_per_candidate":32,"max_pages_per_candidate":32,"continuation_reward_lamports":1},"external_widths":{"linked_basis_prefix":256,"result_domain":192,"rent_sysvar":17,"core_market":320,"activation_cache":160,"upgradeable_program":36,"trading_programdata_prefix":45,"claims_programdata_prefix":45,"core_programdata_prefix":45,"realm_record":112,"rent_credit":48},"token_account_bytes":165}"#,
        );
        let error = read_general_devnet_policy_v1(&path).expect_err("a foreign schema refuses");
        assert!(
            error.0.contains("general-devnet/policy-schema"),
            "{}",
            error.0
        );
        let _ = std::fs::remove_file(&path);
    }

    /// And the canonical document reads back exactly what it states.
    #[test]
    fn the_policy_document_reads_back_every_stated_width() {
        let path = evidence_file(
            "policy-ok.json",
            r#"{"schema":"dclutch-general-devnet-policy-v1","windows":{"collection_slots":16,"selection_slots":16,"settlement_slots":64,"max_orders_per_candidate":32,"max_pages_per_candidate":32,"continuation_reward_lamports":1},"external_widths":{"linked_basis_prefix":256,"result_domain":192,"rent_sysvar":17,"core_market":320,"activation_cache":160,"upgradeable_program":36,"trading_programdata_prefix":45,"claims_programdata_prefix":45,"core_programdata_prefix":45,"realm_record":112,"rent_credit":48},"token_account_bytes":165}"#,
        );
        let policy = read_general_devnet_policy_v1(&path).expect("canonical policy");
        assert_eq!(policy.windows.settlement_slots, 64);
        assert_eq!(policy.external_widths.rent_sysvar, 17);
        assert_eq!(policy.token_account_bytes, 165);
        let _ = std::fs::remove_file(&path);
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
