//! Parent-market normalization facts for the real Series Found -> Prepare entrance.
//!
//! A Series Template commits *child* Markets.  The Market that selects the
//! Series capability is a separate parent: it receives the compiled Series
//! publication only after the child Template has been admitted.  This module
//! owns the boundary between those two identities.  In particular, it derives
//! the parent root from the parent Market's final selected manifest, rather
//! than accidentally using the child Market or a root assembled with default
//! Registry bumps.

use dclutch_core_contract::ContentId;
use dclutch_market::{
    capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1, v4::CapabilityProgramV4},
};
use dclutch_registry::{
    record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId},
    release_set::{CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1, ExecutionRoleV1},
};
use sha2::{Digest as _, Sha256};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    Error, Result,
    market::{
        FutureMarketImmutablePublicationV1, derive_founding_targets, open_market_generation_v1,
        record_identity,
    },
    model::{MarketRunInput, SelectedCapabilityV1, SuccessorPlan},
    plan::{hex32, pubkey},
    rpc::{Rpc, RpcAccount},
    runtime::decode_hex,
    selected_capability_activation::{
        SelectedActivationRecordPairV1, SelectedCapabilityActivationInputV1,
        build_selected_capability_activation_plan_v1, execute_selected_capability_activation_v1,
    },
    series_geometry::{
        SeriesPrepareRoleLayoutV1, SeriesPrepareRoleSourceV1, observe_series_prepare_geometry_v1,
    },
};

type SeriesPrepareFixedDataLengthsV1 = [u32;
    dclutch_trading_sbf::series::prepare_funding_artifacts_v5::SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5
        as usize];

/// The one composite entrance for a local Series Found -> first Prepare run.
/// Its selected accelerator build is always the checked-candidate script; an
/// arbitrary executable or textual certificate identity is deliberately not
/// an argument of this command.
pub(crate) const SERIES_FOUND_PREPARE_COMMAND_V1: &str =
    "local-private-validator-series-found-prepare-v1";

#[derive(Debug)]
struct SeriesFoundPrepareArgumentsV1 {
    plan: PathBuf,
    rpc_url: String,
    payer_keypair: PathBuf,
    direct_fee_basis_points: u16,
    direct_fee_recipient: Pubkey,
    compiler_manifest: PathBuf,
    toolchain_manifest: PathBuf,
    translation_validation: PathBuf,
    shadow_stage_dir: PathBuf,
    checked_candidate_work: PathBuf,
    node: PathBuf,
    node_archive: PathBuf,
    predecessor_profile: Option<PathBuf>,
    genesis_cohort: bool,
    output: PathBuf,
    execute: bool,
}

fn canonical_regular_v1(path: PathBuf, flag: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{flag} must be an absolute path")));
    }
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(Error::new(format!("{flag} must name a regular file")));
    }
    let canonical = fs::canonicalize(&path)?;
    if canonical != path {
        return Err(Error::new(format!("{flag} must be canonical")));
    }
    Ok(path)
}

fn absolute_new_v1(path: PathBuf, flag: &str) -> Result<PathBuf> {
    if !path.is_absolute() || path.exists() || fs::symlink_metadata(&path).is_ok() {
        return Err(Error::new(format!(
            "{flag} must name an absolute path that does not exist"
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(format!("{flag} omitted its parent")))?;
    let metadata = fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(Error::new(format!(
            "{flag} parent must be a regular directory"
        )));
    }
    Ok(fs::canonicalize(parent)?.join(
        path.file_name()
            .ok_or_else(|| Error::new(format!("{flag} omitted its filename")))?,
    ))
}

fn parse_series_found_prepare_arguments_v1(
    arguments: Vec<String>,
) -> Result<SeriesFoundPrepareArgumentsV1> {
    let mut values = BTreeMap::new();
    let mut predecessor_profile = None;
    let mut genesis_cohort = false;
    let mut execute = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--execute" => {
                if execute {
                    return Err(Error::new("Series Found/Prepare repeats --execute"));
                }
                execute = true;
                index += 1;
            }
            "--genesis-cohort" => {
                if genesis_cohort {
                    return Err(Error::new("Series Found/Prepare repeats --genesis-cohort"));
                }
                genesis_cohort = true;
                index += 1;
            }
            "--predecessor-profile" => {
                let value = arguments.get(index + 1).ok_or_else(|| {
                    Error::new("Series Found/Prepare --predecessor-profile needs a value")
                })?;
                if predecessor_profile.replace(PathBuf::from(value)).is_some() {
                    return Err(Error::new(
                        "Series Found/Prepare repeats --predecessor-profile",
                    ));
                }
                index += 2;
            }
            flag => {
                let value = arguments.get(index + 1).ok_or_else(|| {
                    Error::new(format!("Series Found/Prepare {flag} needs a value"))
                })?;
                if !matches!(
                    flag,
                    "--plan"
                        | "--rpc-url"
                        | "--payer-keypair"
                        | "--direct-fee-basis-points"
                        | "--direct-fee-recipient"
                        | "--series-compiler-manifest"
                        | "--series-toolchain-manifest"
                        | "--series-translation-validation"
                        | "--series-shadow-stage-dir"
                        | "--checked-candidate-work"
                        | "--node"
                        | "--node-archive"
                        | "--output"
                ) || values.insert(flag.to_owned(), value.to_owned()).is_some()
                {
                    return Err(Error::new(format!(
                        "Series Found/Prepare rejects argument {flag}"
                    )));
                }
                index += 2;
            }
        }
    }
    if genesis_cohort == predecessor_profile.is_some() {
        return Err(Error::new(
            "Series Found/Prepare requires exactly one of --genesis-cohort or --predecessor-profile",
        ));
    }
    let required = |flag: &str| {
        values
            .get(flag)
            .cloned()
            .ok_or_else(|| Error::new(format!("Series Found/Prepare requires {flag}")))
    };
    let direct_fee_recipient: Pubkey = required("--direct-fee-recipient")?
        .parse()
        .map_err(|_| Error::new("--direct-fee-recipient must be a base58 Pubkey"))?;
    if direct_fee_recipient == Pubkey::default() {
        return Err(Error::new(
            "--direct-fee-recipient must not be the default Pubkey",
        ));
    }
    Ok(SeriesFoundPrepareArgumentsV1 {
        plan: canonical_regular_v1(PathBuf::from(required("--plan")?), "--plan")?,
        rpc_url: required("--rpc-url")?,
        payer_keypair: canonical_regular_v1(
            PathBuf::from(required("--payer-keypair")?),
            "--payer-keypair",
        )?,
        direct_fee_basis_points: required("--direct-fee-basis-points")?
            .parse()
            .map_err(|_| Error::new("--direct-fee-basis-points must be a decimal u16"))?,
        direct_fee_recipient,
        compiler_manifest: canonical_regular_v1(
            PathBuf::from(required("--series-compiler-manifest")?),
            "--series-compiler-manifest",
        )?,
        toolchain_manifest: canonical_regular_v1(
            PathBuf::from(required("--series-toolchain-manifest")?),
            "--series-toolchain-manifest",
        )?,
        translation_validation: canonical_regular_v1(
            PathBuf::from(required("--series-translation-validation")?),
            "--series-translation-validation",
        )?,
        shadow_stage_dir: absolute_new_v1(
            PathBuf::from(required("--series-shadow-stage-dir")?),
            "--series-shadow-stage-dir",
        )?,
        checked_candidate_work: absolute_new_v1(
            PathBuf::from(required("--checked-candidate-work")?),
            "--checked-candidate-work",
        )?,
        node: canonical_regular_v1(PathBuf::from(required("--node")?), "--node")?,
        node_archive: canonical_regular_v1(
            PathBuf::from(required("--node-archive")?),
            "--node-archive",
        )?,
        predecessor_profile: predecessor_profile
            .map(|path| canonical_regular_v1(path, "--predecessor-profile"))
            .transpose()?,
        genesis_cohort,
        output: absolute_new_v1(PathBuf::from(required("--output")?), "--output")?,
        execute,
    })
}

struct StagedSeriesShadowSourcesV1 {
    directory: PathBuf,
    source_manifest: PathBuf,
    generated_include: PathBuf,
    certificate: PathBuf,
    compiler_manifest: PathBuf,
    toolchain_manifest: PathBuf,
}

/// The normalization pass may replace only the compiler-context root.  The
/// source-selected Certificate and its generated include are immutable
/// publication inputs, so rebuilding with the final activated root must leave
/// all generator bytes and every digest handoff exactly unchanged.
fn require_series_shadow_preselection_invariance_v1(
    provisional: &dclutch_series_shadow_bundle_generator::BuiltSeriesShadowSourceV1,
    finalized: &dclutch_series_shadow_bundle_generator::BuiltSeriesShadowSourceV1,
) -> Result<()> {
    if provisional != finalized {
        return Err(Error::new(
            "Series final parent root changed preselection Certificate, manifest, include, or build inputs",
        ));
    }
    let certificate: [u8; 32] = Sha256::digest(finalized.certificate).into();
    if certificate != finalized.build_inputs.certificate.to_bytes() {
        return Err(Error::new(
            "Series final preselection Certificate bytes differ from its content identity",
        ));
    }
    Ok(())
}

fn write_new_bytes_atomically_v1(path: &Path, bytes: &[u8], label: &str) -> Result<()> {
    if path.exists() {
        return Err(Error::new(format!(
            "Series Found/Prepare refuses to replace existing {label} {}",
            path.display()
        )));
    }
    let temporary = path.with_extension(format!("new-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| Error::new(format!("create staged {label}: {error}")))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| Error::new(format!("write staged {label}: {error}")))?;
    fs::rename(&temporary, path)
        .map_err(|error| Error::new(format!("publish staged {label}: {error}")))
}

/// Persist the exact source witnesses once, after the typed generator and its
/// evidence owner accepted them. The checked candidate archives these files,
/// re-emits the include from the archived source manifest, then uses only the
/// staged include for its accelerator links.
fn stage_series_shadow_sources_v1(
    stage: &Path,
    evidence: &crate::series_checked_evidence::CheckedSeriesShadowEvidenceV1,
    built: &dclutch_series_shadow_bundle_generator::BuiltSeriesShadowSourceV1,
) -> Result<StagedSeriesShadowSourcesV1> {
    if stage.exists() || fs::symlink_metadata(stage).is_ok() {
        return Err(Error::new(format!(
            "Series Shadow stage directory already exists {}",
            stage.display()
        )));
    }
    fs::create_dir(stage)?;
    let directory = fs::canonicalize(stage)?;
    let source_manifest = directory.join("series_shadow_source_manifest.bin");
    let generated_include = directory.join("series_shadow_generated.rs");
    let certificate = directory.join("series_shadow_certificate.bin");
    let compiler_manifest = directory.join("compiler_source_manifest.bin");
    let toolchain_manifest = directory.join("toolchain_manifest.bin");
    write_new_bytes_atomically_v1(&source_manifest, &built.manifest, "Shadow source manifest")?;
    write_new_bytes_atomically_v1(
        &generated_include,
        &built.generated_include,
        "Shadow generated include",
    )?;
    write_new_bytes_atomically_v1(&certificate, &built.certificate, "Shadow Certificate")?;
    write_new_bytes_atomically_v1(
        &compiler_manifest,
        &evidence.compiler_manifest,
        "Shadow compiler manifest",
    )?;
    write_new_bytes_atomically_v1(
        &toolchain_manifest,
        &evidence.toolchain,
        "Shadow toolchain manifest",
    )?;
    crate::series_checked_evidence::authenticate_series_shadow_generated_v1(evidence, built)?;
    Ok(StagedSeriesShadowSourcesV1 {
        directory,
        source_manifest,
        generated_include,
        certificate,
        compiler_manifest,
        toolchain_manifest,
    })
}

fn workspace_root_v1() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .map(Path::to_path_buf)
        .ok_or_else(|| Error::new("Series Found/Prepare could not locate workspace root"))
}

/// The selected Trading ledger carries exactly its manifest-selected row.
/// Derive its width from that one owned mask instead of retaining a fixture
/// count beside the Series activation compiler.
fn selected_series_funding_ledger_slot_count_v1(selected_manifest_entry_index: u16) -> Result<u16> {
    let mask = 1_u16
        .checked_shl(u32::from(selected_manifest_entry_index))
        .ok_or_else(|| Error::new("Series selected manifest entry exceeds the funding mask"))?;
    dclutch_market::capability_manifest::funding_ledger_slot_count_v2(mask)
        .map_err(|error| Error::new(format!("Series selected funding ledger: {error:?}")))
}

/// Derive the Series entry position before the selected descriptor and
/// Certificate exist. The selected-capability owner validates the canonical
/// Resolution base and shares its kind ordering with the later final merge.
fn preselection_series_manifest_entry_index_v1(parent_base_manifest: &[u8]) -> Result<u16> {
    let kind: [u8; 32] =
        Sha256::digest(dclutch_trading::series::SERIES_SUCCESSOR_KIND_PREIMAGE_V3).into();
    crate::selected_capability::preselection_manifest_entry_index_v1(parent_base_manifest, kind)
}

/// Run exactly the checked-candidate producer that authenticates and selects
/// the staged Series include. It is intentionally hbox/swarm-build only; the
/// candidate itself checks `SWARM_BUILD_INNER` before a heavy Linux link.
fn build_selected_series_accelerator_v1(
    arguments: &SeriesFoundPrepareArgumentsV1,
    plan: &SuccessorPlan,
    staged: &StagedSeriesShadowSourcesV1,
) -> Result<(PathBuf, PathBuf)> {
    if !arguments.execute {
        return Err(Error::new(
            "Series Found/Prepare refuses a dry run: selected build and validator poststates are one execution",
        ));
    }
    if std::env::var_os("SWARM_BUILD_INNER").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return Err(Error::new(
            "Series Found/Prepare selected accelerator build must run on hbox through swarm-build",
        ));
    }
    let checked = plan
        .checked_local_mutable_set
        .as_ref()
        .ok_or_else(|| Error::new("Series Found/Prepare requires a checked-local mutable plan"))?;
    let root = workspace_root_v1()?;
    let revision = Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| Error::new(format!("Series Found/Prepare git revision: {error}")))?;
    if !revision.status.success()
        || std::str::from_utf8(&revision.stdout).ok().map(str::trim)
            != Some(checked.source_revision.as_str())
    {
        return Err(Error::new(
            "Series Found/Prepare HEAD differs from the baseline checked source revision",
        ));
    }
    let script = root.join("tools/release/checked-release-candidate.sh");
    let mut command = Command::new(&script);
    command.current_dir(&root).args([
        "--repo",
        root.to_str()
            .ok_or_else(|| Error::new("workspace path was not UTF-8"))?,
        "--commit",
        &checked.source_revision,
        "--work",
        arguments
            .checked_candidate_work
            .to_str()
            .ok_or_else(|| Error::new("checked work path was not UTF-8"))?,
        "--builder",
        "hbox",
        "--node",
        arguments
            .node
            .to_str()
            .ok_or_else(|| Error::new("node path was not UTF-8"))?,
        "--node-archive",
        arguments
            .node_archive
            .to_str()
            .ok_or_else(|| Error::new("node archive path was not UTF-8"))?,
        "--series-shadow-generated-include",
        staged
            .generated_include
            .to_str()
            .ok_or_else(|| Error::new("generated include path was not UTF-8"))?,
        "--series-shadow-source-manifest",
        staged
            .source_manifest
            .to_str()
            .ok_or_else(|| Error::new("source manifest path was not UTF-8"))?,
        "--series-shadow-compiler-source",
        staged
            .compiler_manifest
            .to_str()
            .ok_or_else(|| Error::new("compiler manifest path was not UTF-8"))?,
        "--series-shadow-toolchain-manifest",
        staged
            .toolchain_manifest
            .to_str()
            .ok_or_else(|| Error::new("toolchain manifest path was not UTF-8"))?,
    ]);
    if arguments.genesis_cohort {
        command.arg("--genesis-cohort");
    } else {
        command.args([
            "--predecessor-profile",
            arguments
                .predecessor_profile
                .as_ref()
                .and_then(|path| path.to_str())
                .ok_or_else(|| Error::new("predecessor profile path was not UTF-8"))?,
        ]);
    }
    let status = command
        .status()
        .map_err(|error| Error::new(format!("run selected Series checked candidate: {error}")))?;
    if !status.success() {
        return Err(Error::new(format!(
            "selected Series checked candidate exited {status}"
        )));
    }
    let manifest = arguments
        .checked_candidate_work
        .join("evidence/accelerator/checked.bin");
    let elf = arguments.checked_candidate_work.join("elf/accelerator.so");
    Ok((
        canonical_regular_v1(manifest, "selected accelerator checked manifest")?,
        canonical_regular_v1(elf, "selected accelerator ELF")?,
    ))
}

/// Finalized Registry coordinates for the immutable two-leaf Series founder
/// material.  The Template is also M1's selected config; the occurrence and
/// Ticket records remain M0 child facts and must never be folded into the
/// parent Market manifest.
#[derive(Clone, Debug)]
pub(crate) struct PublishedSeriesFounderRecordsV1 {
    pub(crate) template: crate::runtime::PublishedRecord,
    pub(crate) occurrences: [crate::runtime::PublishedRecord; 2],
    pub(crate) tickets: [crate::runtime::PublishedRecord; 2],
}

/// Publish the immutable child facts before the first Prepare can acquire
/// them.  `publish_record` is idempotent only after re-reading and checking
/// the derived raw record, so accepting an already-published Template still
/// proves that M1's config bytes equal the founder's M0 template bytes.
pub(crate) fn publish_series_founder_records_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    payer: &Keypair,
    founder: &crate::series_founder::PreparedSeriesFounderV1,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<PublishedSeriesFounderRecordsV1> {
    use dclutch_trading::series::{
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    };

    let publish = |schema,
                   bytes: &[u8],
                   label: &str,
                   rpc: &mut Rpc,
                   transactions: &mut Vec<crate::model::TransactionEvidence>| {
        let record = crate::runtime::publish_record(
            rpc,
            registry,
            payer,
            schema,
            bytes,
            None,
            transactions,
        )?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if record.schema != schema || record.digest != digest {
            return Err(Error::new(format!(
                "Series founder {label} publication changed schema or content"
            )));
        }
        Ok(record)
    };
    let template = publish(
        SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        founder.admitted.template(),
        "Template",
        rpc,
        transactions,
    )?;
    let occurrences = [
        publish(
            SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
            &founder.admitted.occurrences()[0],
            "first occurrence",
            rpc,
            transactions,
        )?,
        publish(
            SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
            &founder.admitted.occurrences()[1],
            "second occurrence",
            rpc,
            transactions,
        )?,
    ];
    let tickets = [
        publish(
            SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
            &founder.admitted.tickets()[0],
            "first Ticket",
            rpc,
            transactions,
        )?,
        publish(
            SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
            &founder.admitted.tickets()[1],
            "second Ticket",
            rpc,
            transactions,
        )?,
    ];
    Ok(PublishedSeriesFounderRecordsV1 {
        template,
        occurrences,
        tickets,
    })
}

/// Activate the just-founded Series parent from its durable founding report.
///
/// The command is deliberately separate from the generic founding executable:
/// it accepts only the parent Market document already consumed by Found and
/// the report Found wrote.  It cannot substitute child M0 for parent M1, and
/// it cannot claim activation without the signed transaction evidence it
/// emits to `--output`.
pub(crate) const SERIES_ROOT_ACTIVATE_COMMAND_V1: &str =
    "local-private-validator-series-root-activate-v1";

struct RootActivateArgumentsV1 {
    rpc_url: String,
    plan: PathBuf,
    parent_market: PathBuf,
    founding_report: PathBuf,
    collateral_mint: Pubkey,
    payer_keypair: PathBuf,
    output: PathBuf,
    execute: bool,
}

pub(crate) fn run_root_activate_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_root_activate_arguments_v1(arguments)?;
    if !arguments.execute {
        return Err(Error::new(
            "Series root activation refuses a dry run: this command exists to record an actual finalized activation",
        ));
    }
    if arguments.output.exists() {
        return Err(Error::new(format!(
            "Series root activation refuses to overwrite {}",
            arguments.output.display()
        )));
    }
    let plan: SuccessorPlan = serde_json::from_slice(&fs::read(&arguments.plan)?)?;
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)?;
    let parent_input: MarketRunInput =
        serde_json::from_slice(&fs::read(&arguments.parent_market)?)?;
    crate::market::validate_market_input(&parent_input)?;
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&arguments.founding_report)?)?;
    let evidence = report
        .pointer("/execution/market")
        .cloned()
        .unwrap_or(report);
    let founding: crate::market::MarketExecutionEvidence = serde_json::from_value(evidence)
        .map_err(|error| Error::new(format!("Series founding report: {error}")))?;
    let payer = Keypair::new_from_array(crate::campaign::read_keypair_file(
        &arguments.payer_keypair,
        "Series root activation payer",
    )?);
    let mut rpc = Rpc::connect(&arguments.rpc_url)?;
    let mut transactions = Vec::new();
    let activated = activate_series_parent_root_v1(
        &mut rpc,
        &payer,
        &plan,
        &parent_input,
        &founding,
        arguments.collateral_mint,
        &mut transactions,
    )?;
    if transactions.is_empty() {
        return Err(Error::new(
            "Series root activation submitted no transaction evidence",
        ));
    }
    let output = serde_json::json!({
        "schema": "dclutch-series-parent-root-activation-v1",
        "evidenceLevel": "local-validator",
        "parentMarket": activated.market.to_string(),
        "root": activated.root.to_string(),
        "transactions": transactions,
    });
    fs::write(
        &arguments.output,
        format!("{}\n", serde_json::to_string_pretty(&output)?),
    )?;
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn parse_root_activate_arguments_v1(arguments: Vec<String>) -> Result<RootActivateArgumentsV1> {
    let mut values = std::collections::BTreeMap::new();
    let mut position = 0_usize;
    let mut execute = false;
    while position < arguments.len() {
        let flag = arguments
            .get(position)
            .ok_or_else(|| Error::new("Series root activation arguments ended unexpectedly"))?;
        if flag == "--execute" {
            execute = true;
            position += 1;
            continue;
        }
        let value = arguments.get(position + 1).ok_or_else(|| {
            Error::new(format!("Series root activation omitted value for {flag}"))
        })?;
        if !matches!(
            flag.as_str(),
            "--rpc-url"
                | "--plan"
                | "--parent-market"
                | "--founding-report"
                | "--collateral-mint"
                | "--payer-keypair"
                | "--output"
        ) || values.insert(flag.clone(), value.clone()).is_some()
        {
            return Err(Error::new(format!(
                "Series root activation arguments rejected {flag}"
            )));
        }
        position += 2;
    }
    let required = |flag: &str| {
        values
            .get(flag)
            .cloned()
            .ok_or_else(|| Error::new(format!("Series root activation requires {flag}")))
    };
    let collateral_mint = required("--collateral-mint")?
        .parse()
        .map_err(|error| Error::new(format!("Series root activation collateral mint: {error}")))?;
    if collateral_mint == Pubkey::default() {
        return Err(Error::new(
            "Series root activation collateral mint was default",
        ));
    }
    Ok(RootActivateArgumentsV1 {
        rpc_url: required("--rpc-url")?,
        plan: PathBuf::from(required("--plan")?),
        parent_market: PathBuf::from(required("--parent-market")?),
        founding_report: PathBuf::from(required("--founding-report")?),
        collateral_mint,
        payer_keypair: PathBuf::from(required("--payer-keypair")?),
        output: PathBuf::from(required("--output")?),
        execute,
    })
}

/// Final parent identity used to normalize a Series-selected payload before
/// the parent founding begins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SeriesParentRootV1 {
    /// The parent Market which will be founded through Open.  It is never the
    /// child Market precommitted by the Template.
    pub(crate) market: Pubkey,
    /// The Trading PDA that selector-255 will create for this parent.
    pub(crate) root: Pubkey,
    /// Registry-derived raw/staging bumps carried in the root header.
    pub(crate) record_bumps: SelectedRecordBumpsV1,
}

/// Produce the Series parent M1 from the same checked Resolution release as a
/// normal local Market, then attach the already-normalized Series closure.
///
/// The Template remains the M0 child configuration within `selected`; this
/// creates a separate M1 manifest whose selected entry names that Template.
/// Keeping this at the driver boundary prevents a caller from reusing the
/// Direct child input after the Series payload has been attached.
pub(crate) fn build_series_parent_market_v1(
    plan: &SuccessorPlan,
    registry: Pubkey,
    selected: SelectedCapabilityV1,
) -> Result<MarketRunInput> {
    require_series_parent_payload_v1(&selected)?;
    let resolution_release = crate::direct_market::authenticated_resolution_release_v1(plan)?;
    let mut parent = crate::market::demo_market_input_base(registry, resolution_release)?;
    crate::selected_capability::attach_selected_capability_v1(&mut parent, selected)?;
    crate::market::validate_market_input(&parent)?;
    Ok(parent)
}

/// Derive the parent Series root from the complete parent Market input.
///
/// The payload must already be attached to `parent_input`; deriving this from
/// the earlier capability-free child preview would make a root whose header
/// names a different manifest.  No chain state is asserted here: the caller
/// supplies the future parent collateral Mint and later observes this exact
/// vacant PDA before activation.
pub(crate) fn derive_series_parent_root_v1(
    plan: &SuccessorPlan,
    parent_input: &MarketRunInput,
    collateral_mint: Pubkey,
) -> Result<SeriesParentRootV1> {
    let selected = parent_input
        .selected_capability
        .as_ref()
        .ok_or_else(|| Error::new("Series parent Market omitted selected capability"))?;
    if selected.family != "series" {
        return Err(Error::new(
            "Series parent Market selected a capability from another family",
        ));
    }
    let registry = pubkey(&plan.registry.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let targets = derive_founding_targets(plan, parent_input, collateral_mint)?;
    let manifest = decode_hex(&parent_input.capability_manifest_hex)?;
    let descriptor = decode_hex(&selected.selected_descriptor_hex)?;
    let descriptor = CapabilityProgramV4::decode(&descriptor)
        .map_err(|error| Error::new(format!("Series parent selected descriptor: {error:?}")))?;
    let program_set = decode_hex(&selected.program_set_hex)?;
    let config = decode_hex(&selected.config_hex)?;
    let manifest_id = record_identity(&manifest);
    let program_set_id: [u8; 32] = Sha256::digest(&program_set).into();
    let config_id: [u8; 32] = Sha256::digest(&config).into();
    let selection = CapabilityExecutionSelectionV1::new(
        selected.selected_manifest_entry_index,
        ContentId::new(manifest_id).map_err(|_| Error::new("Series parent manifest identity"))?,
        descriptor.kind(),
        ContentId::new(program_set_id)
            .map_err(|_| Error::new("Series parent program-set identity"))?,
        ContentId::new(config_id).map_err(|_| Error::new("Series parent config identity"))?,
    )
    .map_err(|error| Error::new(format!("Series parent execution selection: {error:?}")))?;
    let manifest_bumps = record_bumps_v1(
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_id,
        "Series parent manifest",
    )?;
    let program_set_bumps = record_bumps_v1(
        registry,
        dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        program_set_id,
        "Series parent program set",
    )?;
    let config_bumps = record_bumps_v1(
        registry,
        dclutch_trading::series::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        config_id,
        "Series parent Template",
    )?;
    let record_bumps = SelectedRecordBumpsV1::new(
        manifest_bumps[0],
        manifest_bumps[1],
        config_bumps[0],
        config_bumps[1],
    );
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(hex32(&plan.release_set_id)?)
            .map_err(|_| Error::new("Series parent release-set identity"))?,
        targets.open_market.to_bytes(),
        open_market_generation_v1(parent_input)?,
        selection.with_capability_release_record_bumps(program_set_bumps[0], program_set_bumps[1]),
        record_bumps,
    )
    .map_err(|error| Error::new(format!("Series parent root header: {error:?}")))?;
    Ok(SeriesParentRootV1 {
        market: targets.open_market,
        root: Pubkey::find_program_address(&header.seeds().as_slices(), &trading).0,
        record_bumps,
    })
}

fn record_bumps_v1(
    registry: Pubkey,
    schema: [u8; 32],
    content: [u8; 32],
    label: &str,
) -> Result<[u8; 2]> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|error| Error::new(format!("{label} schema: {error:?}")))?,
        ContentDigest::new(content)
            .map_err(|error| Error::new(format!("{label} content: {error:?}")))?,
    );
    let derive = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &registry,
        )
    };
    let (_, raw) = derive(key.raw_record_pda_seeds());
    let (_, staging) = derive(key.staging_cursor_pda_seeds());
    Ok([raw, staging])
}

/// Refuse a caller that accidentally normalizes against child bytes.  Keeping
/// the check beside the root derivation makes the distinction executable at
/// the one public driver boundary rather than a comment in a compiler test.
pub(crate) fn require_series_parent_payload_v1(payload: &SelectedCapabilityV1) -> Result<()> {
    if payload.family != "series" || payload.records.len() != 39 {
        return Err(Error::new(
            "Series parent activation requires the complete 39-record Series payload",
        ));
    }
    Ok(())
}

/// Publish selector-255 for an already-founded parent and verify the generic
/// root poststate.  The parent founding has already transaction-published all
/// 39 records; this function re-derives each Registry coordinate from those
/// bytes and refuses an evidence map that tries to substitute one.
pub(crate) fn activate_series_parent_root_v1(
    rpc: &mut Rpc,
    payer: &solana_sdk::signature::Keypair,
    plan: &SuccessorPlan,
    parent_input: &MarketRunInput,
    founding: &crate::market::MarketExecutionEvidence,
    collateral_mint: Pubkey,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<SeriesParentRootV1> {
    let selected = parent_input
        .selected_capability
        .as_ref()
        .ok_or_else(|| Error::new("Series root activation omitted selected payload"))?;
    require_series_parent_payload_v1(selected)?;
    let expected = derive_series_parent_root_v1(plan, parent_input, collateral_mint)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let template = dclutch_trading::series::TemplateV3::decode(&decode_hex(&selected.config_hex)?)
        .map_err(|_| Error::new("Series root activation Template refused decode"))?;
    let manifest_body = decode_hex(&parent_input.capability_manifest_hex)?;
    let program_set_body = decode_hex(&selected.program_set_hex)?;
    let config_body = decode_hex(&selected.config_hex)?;
    let manifest = pair_v1(
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &manifest_body,
    )?;
    let program_set = pair_v1(
        registry,
        dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        &program_set_body,
    )?;
    let config = pair_v1(
        registry,
        dclutch_trading::series::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        &config_body,
    )?;
    let realm = pair_from_id_v1(
        registry,
        dclutch_market::realm::REALM_SCHEMA_RELEASE_ID_V1,
        template.realm().to_bytes(),
    )?;
    let record = |index: usize, label: &str| -> Result<SelectedActivationRecordPairV1> {
        let value = selected
            .records
            .get(index)
            .ok_or_else(|| Error::new(format!("Series root activation omitted {label}")))?;
        pair_v1(
            registry,
            hex32(&value.schema_hex)?,
            &decode_hex(&value.body_hex)?,
        )
    };
    // Five action banks occupy seven records each.  The activation bundle is
    // the final three records before the activation-capable ProgramSet.
    let account_profile = record(35, "activation account profile")?;
    let effect = record(36, "activation effect")?;
    let descriptor = record(37, "activation descriptor")?;
    let market = evidence_key_v1(founding, "founding_market")?;
    if market != expected.market {
        return Err(Error::new(
            "Series root activation founding report names a different parent Market",
        ));
    }
    let selected_funding_ledger = evidence_key_v1(founding, "direct_trading_funding_ledger")?;
    let resolution_funding_ledger = founding
        .accounts
        .iter()
        .filter(|(label, _)| label.starts_with("founding_funding_ledger_v2_"))
        .map(|(_, evidence)| pubkey(&evidence.address))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .find(|key| *key != selected_funding_ledger)
        .ok_or_else(|| Error::new("Series root activation omitted Resolution funding ledger"))?;
    let addresses = [
        market,
        realm.raw,
        realm.staging,
        manifest.raw,
        manifest.staging,
        program_set.raw,
        program_set.staging,
        config.raw,
        config.staging,
        account_profile.raw,
        account_profile.staging,
        effect.raw,
        effect.staging,
        descriptor.raw,
        descriptor.staging,
        selected_funding_ledger,
        resolution_funding_ledger,
    ];
    let (slot, observed) = rpc.finalized_accounts(&addresses, 0)?;
    let account = |index: usize, label: &str| -> Result<RpcAccount> {
        observed.get(index).cloned().flatten().ok_or_else(|| {
            Error::new(format!(
                "Series root activation finalized snapshot omitted {label}"
            ))
        })
    };
    let market_account = account(0, "parent Market")?;
    let realm_account = account(1, "Series realm")?;
    let manifest_account = account(3, "parent manifest")?;
    let program_set_account = account(5, "Series ProgramSet")?;
    let config_account = account(7, "Series Template")?;
    let profile_account = account(9, "Series activation profile")?;
    let effect_account = account(11, "Series activation effect")?;
    let descriptor_account = account(13, "Series activation descriptor")?;
    let ledger_account = account(15, "Trading funding ledger")?;
    let market_state = dclutch_market::CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Series root activation Core Market: {error:?}")))?;
    let manifest_id: [u8; 32] = Sha256::digest(&manifest_body).into();
    let record_matches = |account: &RpcAccount, expected: &[u8]| {
        account.owner == registry && account.data == expected
    };
    if slot == 0
        || market_account.owner != core
        || market_state.identity.market_id.to_bytes() != market.to_bytes()
        || market_state.identity.realm_id.to_bytes() != template.realm().to_bytes()
        || market_state.identity.capability_manifest.to_bytes() != manifest_id
        || realm_account.owner != registry
        || Sha256::digest(&realm_account.data).as_slice() != template.realm().to_bytes()
        || !record_matches(&manifest_account, &manifest_body)
        || !record_matches(&program_set_account, &program_set_body)
        || !record_matches(&config_account, &config_body)
        || !record_matches(
            &profile_account,
            &decode_hex(&selected.records[35].body_hex)?,
        )
        || !record_matches(
            &effect_account,
            &decode_hex(&selected.records[36].body_hex)?,
        )
        || !record_matches(
            &descriptor_account,
            &decode_hex(&selected.records[37].body_hex)?,
        )
    {
        return Err(Error::new(
            "Series root activation finalized record bytes differ from the selected closure",
        ));
    }
    let selected_descriptor =
        CapabilityProgramV4::decode(&decode_hex(&selected.selected_descriptor_hex)?)
            .map_err(|error| Error::new(format!("Series root selected descriptor: {error:?}")))?;
    let activation_request =
        dclutch_trading_sbf::series::activation_bundle_v1::series_activation_request_v1()
            .map_err(|error| Error::new(format!("Series activation request: {error:?}")))?;
    let context: [u8; 32] = Sha256::digest(b"dclutch/series-root-activation/v1").into();
    let activation =
        build_selected_capability_activation_plan_v1(SelectedCapabilityActivationInputV1 {
            market,
            market_account: &market_account,
            current_slot: slot,
            core,
            core_programdata: pubkey(&plan.core.programdata_id)?,
            trading,
            trading_programdata: pubkey(&plan.trading.programdata_id)?,
            resolution,
            resolution_programdata: pubkey(&plan.resolution.programdata_id)?,
            registry,
            activation_cache: pubkey(&plan.activation)?,
            realm,
            manifest,
            manifest_body: &manifest_body,
            entry_index: selected.selected_manifest_entry_index,
            program_set,
            program_set_id: Sha256::digest(&program_set_body).into(),
            config,
            config_id: Sha256::digest(&config_body).into(),
            capability_kind: selected_descriptor.kind().to_bytes(),
            account_profile,
            effect,
            descriptor,
            selected_funding_ledger,
            selected_funding_ledger_account: &ledger_account,
            resolution_funding_ledger,
            family_activation_request: &activation_request,
            context,
        })?;
    if activation.root != expected.root {
        return Err(Error::new(
            "Series root activation plan changed the normalized parent root",
        ));
    }
    let outcome = execute_selected_capability_activation_v1(
        rpc,
        payer,
        &activation,
        "activate Series selector-255 parent root",
    )?;
    transactions.extend(outcome.routing_transactions);
    transactions.push(outcome.activation);
    Ok(expected)
}

fn evidence_key_v1(
    founding: &crate::market::MarketExecutionEvidence,
    label: &str,
) -> Result<Pubkey> {
    founding
        .accounts
        .get(label)
        .ok_or_else(|| {
            Error::new(format!(
                "Series root activation founding report omitted {label}"
            ))
        })
        .and_then(|evidence| pubkey(&evidence.address))
}

fn pair_v1(
    registry: Pubkey,
    schema: [u8; 32],
    body: &[u8],
) -> Result<SelectedActivationRecordPairV1> {
    pair_from_id_v1(registry, schema, Sha256::digest(body).into())
}

fn pair_from_id_v1(
    registry: Pubkey,
    schema: [u8; 32],
    content: [u8; 32],
) -> Result<SelectedActivationRecordPairV1> {
    let bumps = record_bumps_v1(registry, schema, content, "Series activation record")?;
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|error| Error::new(format!("Series activation schema: {error:?}")))?,
        ContentDigest::new(content)
            .map_err(|error| Error::new(format!("Series activation content: {error:?}")))?,
    );
    let derive = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &registry,
        )
        .0
    };
    Ok(SelectedActivationRecordPairV1 {
        raw: derive(key.raw_record_pda_seeds()),
        staging: derive(key.staging_cursor_pda_seeds()),
        schema,
        content,
        bumps,
    })
}

/// Persisted authoring facts for the immutable two-occurrence Series founder.
///
/// The source Market determines the M0 Product, Realm, Source and Direct
/// manifest.  These fields deliberately cover only the facts the Template
/// author owns: generator identities, schedule, and the two separately
/// committed funding partitions.  Keeping the boundary narrow prevents a
/// campaign document from substituting the child Market's canonical bodies.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SeriesFounderAuthoringInputV1 {
    pub(crate) product_generator: String,
    pub(crate) occurrence_generator: String,
    pub(crate) capability_template: String,
    pub(crate) product_derivation: String,
    pub(crate) occurrence_derivation: String,
    pub(crate) capability_derivation: String,
    pub(crate) funding_derivation: String,
    pub(crate) first_slot: u64,
    pub(crate) period_slots: u64,
    pub(crate) retry_window: u64,
    pub(crate) close_rent: u64,
    pub(crate) occurrences: [SeriesOccurrenceAuthoringInputV1; 2],
}

/// One exact M0 occurrence funding partition.  These are principal and
/// pre-paid non-principal amounts, not a caller-selected future account.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SeriesOccurrenceAuthoringInputV1 {
    pub(crate) funding_list: String,
    pub(crate) hoard_principal: u64,
    pub(crate) market_rent: u64,
    pub(crate) capability_native: u64,
    pub(crate) founding_work: u64,
}

/// Decode only canonical content identities and the Series owner's bounded
/// funding constructor.  A live driver calls this before
/// `prepare_series_founder_from_market_v1`; no test fixture identity crosses
/// that boundary.
pub(crate) fn decode_series_founder_authoring_v1(
    input: &SeriesFounderAuthoringInputV1,
) -> Result<(
    crate::series_founder::SeriesTemplatePolicyV1,
    [crate::series_founder::SeriesOccurrenceFundingV1; 2],
)> {
    let id = |value: &str, label: &str| {
        ContentId::new(hex32(value)?).map_err(|_| Error::new(format!("Series {label} identity")))
    };
    let policy = crate::series_founder::SeriesTemplatePolicyV1 {
        product_generator: id(&input.product_generator, "product generator")?,
        occurrence_generator: id(&input.occurrence_generator, "occurrence generator")?,
        capability_template: id(&input.capability_template, "capability template")?,
        product_derivation: id(&input.product_derivation, "product derivation")?,
        occurrence_derivation: id(&input.occurrence_derivation, "occurrence derivation")?,
        capability_derivation: id(&input.capability_derivation, "capability derivation")?,
        funding_derivation: id(&input.funding_derivation, "funding derivation")?,
        first_slot: input.first_slot,
        period_slots: input.period_slots,
        retry_window: input.retry_window,
        close_rent: input.close_rent,
    };
    let decode_occurrence = |occurrence: &SeriesOccurrenceAuthoringInputV1| -> Result<crate::series_founder::SeriesOccurrenceFundingV1> {
        Ok(crate::series_founder::SeriesOccurrenceFundingV1 {
            funding_list: id(&occurrence.funding_list, "funding list")?,
            funds: dclutch_trading::series::FoundingFundsV3::new(
                occurrence.hoard_principal,
                occurrence.market_rent,
                occurrence.capability_native,
                occurrence.founding_work,
            )
            .map_err(|error| Error::new(format!("Series occurrence funding: {error:?}")))?,
        })
    };
    let [first, second] = &input.occurrences;
    let funding = [decode_occurrence(first)?, decode_occurrence(second)?];
    Ok((policy, funding))
}

#[cfg(test)]
mod authoring_tests {
    use super::*;

    fn input() -> SeriesFounderAuthoringInputV1 {
        let identity = |byte| format!("{byte:02x}").repeat(32);
        SeriesFounderAuthoringInputV1 {
            product_generator: identity(1),
            occurrence_generator: identity(2),
            capability_template: identity(3),
            product_derivation: identity(4),
            occurrence_derivation: identity(5),
            capability_derivation: identity(6),
            funding_derivation: identity(7),
            first_slot: 100,
            period_slots: 10,
            retry_window: 2,
            close_rent: 1,
            occurrences: [
                SeriesOccurrenceAuthoringInputV1 {
                    funding_list: identity(8),
                    hoard_principal: 9,
                    market_rent: 2,
                    capability_native: 3,
                    founding_work: 4,
                },
                SeriesOccurrenceAuthoringInputV1 {
                    funding_list: identity(9),
                    hoard_principal: 18,
                    market_rent: 2,
                    capability_native: 3,
                    founding_work: 4,
                },
            ],
        }
    }

    #[test]
    fn authoring_decoder_keeps_two_funding_partitions_distinct() {
        let (_, funding) = decode_series_founder_authoring_v1(&input()).unwrap();
        assert_eq!(funding[0].funds.hoard_principal(), 9);
        assert_eq!(funding[1].funds.hoard_principal(), 18);
    }

    #[test]
    fn authoring_decoder_refuses_noncanonical_identity() {
        let mut hostile = input();
        hostile.funding_derivation = "aa".into();
        assert!(decode_series_founder_authoring_v1(&hostile).is_err());
    }
}

/// One finality-bound source observation used to author the local scenario.
/// The rent schedule is kept as its canonical sysvar bytes until the caller
/// has checked both its owner and its bincode round-trip.
#[derive(Clone, Debug)]
pub(crate) struct SeriesLocalScenarioObservationV1 {
    pub(crate) finalized_slot: u64,
    pub(crate) rent: solana_program::rent::Rent,
}

/// A finalized token source observation that the first SeriesEscrow Lock will
/// consume.  The scenario derives both occurrence principals from `amount`,
/// never from a descriptive Market-input scalar.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeriesFounderSourceObservationV1 {
    pub(crate) source: Pubkey,
    pub(crate) amount: u64,
}

/// Read and authenticate the concrete external Token-2022 source before it
/// becomes the first occurrence's SeriesEscrow Lock source.
pub(crate) fn observe_series_founder_source_v1(
    rpc: &mut Rpc,
    source: Pubkey,
    mint: Pubkey,
    founder: Pubkey,
) -> Result<SeriesFounderSourceObservationV1> {
    if source == Pubkey::default() || mint == Pubkey::default() || founder == Pubkey::default() {
        return Err(Error::new(
            "Series founder source observation named a default key",
        ));
    }
    let account = rpc.required_account(source, "Series founder token source")?;
    let amount = authenticate_series_founder_source_account_v1(&account, mint, founder)?;
    Ok(SeriesFounderSourceObservationV1 { source, amount })
}

fn authenticate_series_founder_source_account_v1(
    account: &RpcAccount,
    mint: Pubkey,
    founder: Pubkey,
) -> Result<u64> {
    use dclutch_custody::token_svm::{AccountState, TOKEN_2022_PROGRAM_ID, TokenAccount};

    let token = TokenAccount::parse(&account.data)
        .map_err(|error| Error::new(format!("Series founder token source: {error:?}")))?;
    if account.owner != Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID)
        || account.executable
        || token.mint != mint.to_bytes()
        || token.owner != founder.to_bytes()
        || token.state != AccountState::Initialized
        || token.amount == 0
    {
        return Err(Error::new(
            "Series founder source did not authenticate as an initialized founder Token-2022 account",
        ));
    }
    Ok(token.amount)
}

#[cfg(test)]
mod founder_source_tests {
    use dclutch_custody::token_svm::{TOKEN_2022_PROGRAM_ID, TokenAccount};

    use super::*;

    fn account(mint: Pubkey, founder: Pubkey) -> RpcAccount {
        let initial = TokenAccount::initialized_base_bytes(mint.to_bytes(), founder.to_bytes())
            .expect("canonical base token account");
        let data = TokenAccount::project_amount_poststate(&initial, 41)
            .expect("canonical token amount poststate");
        RpcAccount {
            lamports: 1,
            owner: Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
            executable: false,
            rent_epoch: 0,
            data: data.to_vec(),
        }
    }

    #[test]
    fn founder_source_requires_the_named_mint_owner_and_nonzero_balance() {
        let mint = Pubkey::new_unique();
        let founder = Pubkey::new_unique();
        assert_eq!(
            authenticate_series_founder_source_account_v1(&account(mint, founder), mint, founder)
                .expect("authenticated founder source"),
            41
        );

        let mut hostile = account(mint, founder);
        hostile.owner = Pubkey::new_unique();
        let error = authenticate_series_founder_source_account_v1(&hostile, mint, founder)
            .expect_err("non-Token-2022 owner must refuse");
        assert_eq!(
            error.to_string(),
            "Series founder source did not authenticate as an initialized founder Token-2022 account"
        );
    }
}

/// Read the exact finality point and Rent schedule that bound the first two
/// occurrence partitions.  This is intentionally separate from the scenario
/// constructor so the later M1 compiler can retain the slot it was based on.
pub(crate) fn observe_local_series_scenario_v1(
    rpc: &mut Rpc,
) -> Result<SeriesLocalScenarioObservationV1> {
    let finalized_slot = rpc.finalized_slot()?;
    if finalized_slot == 0 {
        return Err(Error::new("Series local scenario observed slot zero"));
    }
    let account = rpc.required_account(solana_sdk_ids::sysvar::rent::ID, "Rent sysvar")?;
    if account.owner != solana_sdk_ids::sysvar::ID || account.executable {
        return Err(Error::new(
            "Series local scenario Rent sysvar owner or executable bit changed",
        ));
    }
    let rent: solana_program::rent::Rent = bincode::deserialize(&account.data)
        .map_err(|error| Error::new(format!("Series local scenario Rent sysvar: {error}")))?;
    let encoded = bincode::serialize(&rent)
        .map_err(|error| Error::new(format!("Series local scenario Rent encoding: {error}")))?;
    if encoded != account.data {
        return Err(Error::new(
            "Series local scenario Rent sysvar was not canonical",
        ));
    }
    Ok(SeriesLocalScenarioObservationV1 {
        finalized_slot,
        rent,
    })
}

/// Author the one concrete local-validator Series scenario from the M0 Direct
/// manifest, current Rent quote, and a finalized slot.  Every generator and
/// derivation identity is domain-separated from that exact M0 manifest; no
/// repeated-byte fixture identity is admitted.  Both occurrence partitions
/// reserve current fixed-layout rent before dividing the funded collateral,
/// so their committed principal can never exceed the real source budget.
pub(crate) fn local_series_founder_scenario_v1(
    plan: &SuccessorPlan,
    child_market: &MarketRunInput,
    collateral_mint: Pubkey,
    founder_source_amount: u64,
    rent: &solana_program::rent::Rent,
    finalized_slot: u64,
) -> Result<(
    crate::series_founder::SeriesTemplatePolicyV1,
    [crate::series_founder::SeriesOccurrenceFundingV1; 2],
)> {
    if finalized_slot == 0 {
        return Err(Error::new(
            "Series local scenario requires a finalized slot",
        ));
    }
    let registry = pubkey(&plan.registry.program_id)?;
    let preview = crate::market::compile_market_publication_preview_v1(
        registry,
        child_market,
        collateral_mint,
    )?;
    let manifest = record_identity(&preview.manifest);
    let identity = |label: &[u8]| -> Result<ContentId> {
        let mut digest = Sha256::new();
        digest.update(b"dclutch/local-validator/series-founder-scenario/v1\0");
        digest.update(label);
        digest.update([0]);
        digest.update(manifest);
        ContentId::new(digest.finalize().into())
            .map_err(|_| Error::new("Series local scenario identity was zero"))
    };
    let market_rent = rent.minimum_balance(dclutch_market::STATE_BYTES);
    let capability_native = rent.minimum_balance(
        dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
    );
    let founding_work =
        rent.minimum_balance(dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3);
    let one_reserve = market_rent
        .checked_add(capability_native)
        .and_then(|value| value.checked_add(founding_work))
        .ok_or_else(|| Error::new("Series local scenario rent reserve overflow"))?;
    let total_reserve = one_reserve
        .checked_mul(2)
        .ok_or_else(|| Error::new("Series local scenario two-occurrence reserve overflow"))?;
    let distributable = founder_source_amount
        .checked_sub(total_reserve)
        .ok_or_else(|| {
            Error::new(
                "Series local scenario observed founder source cannot prepay two child rents",
            )
        })?;
    let first_principal = distributable / 2;
    let second_principal = distributable
        .checked_sub(first_principal)
        .ok_or_else(|| Error::new("Series local scenario principal partition overflow"))?;
    if first_principal == 0 || second_principal == 0 {
        return Err(Error::new(
            "Series local scenario leaves a zero child Hoard principal",
        ));
    }
    let funding = |label: &[u8],
                   hoard_principal|
     -> Result<crate::series_founder::SeriesOccurrenceFundingV1> {
        Ok(crate::series_founder::SeriesOccurrenceFundingV1 {
            funding_list: identity(label)?,
            funds: dclutch_trading::series::FoundingFundsV3::new(
                hoard_principal,
                market_rent,
                capability_native,
                founding_work,
            )
            .map_err(|error| Error::new(format!("Series local scenario funding: {error:?}")))?,
        })
    };
    let first_slot = finalized_slot
        .checked_add(8)
        .ok_or_else(|| Error::new("Series local scenario first slot overflow"))?;
    Ok((
        crate::series_founder::SeriesTemplatePolicyV1 {
            product_generator: identity(b"product-generator")?,
            occurrence_generator: identity(b"occurrence-generator")?,
            capability_template: identity(b"capability-template")?,
            product_derivation: identity(b"product-derivation")?,
            occurrence_derivation: identity(b"occurrence-derivation")?,
            capability_derivation: identity(b"capability-derivation")?,
            funding_derivation: identity(b"funding-derivation")?,
            first_slot,
            period_slots: 32,
            retry_window: 16,
            close_rent: founding_work,
        },
        [
            funding(b"occurrence-0", first_principal)?,
            funding(b"occurrence-1", second_principal)?,
        ],
    ))
}

/// Prepare the canonical two-leaf M0 founder from a live validator snapshot.
/// This is the first production source hydrator: it joins the child Direct
/// Market bytes, real collateral Mint, authenticated founder source balance,
/// on-chain Rent, and current slot before any Template or occurrence bytes
/// are generated.
pub(crate) fn prepare_local_series_founder_from_market_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    child_market: &MarketRunInput,
    collateral_mint: Pubkey,
    founder: Pubkey,
    refund_owner: Pubkey,
    founder_source: Pubkey,
) -> Result<(
    crate::series_founder::PreparedSeriesFounderV1,
    SeriesLocalScenarioObservationV1,
)> {
    if collateral_mint == Pubkey::default()
        || founder == Pubkey::default()
        || refund_owner == Pubkey::default()
        || founder_source == Pubkey::default()
    {
        return Err(Error::new(
            "Series local founder named a default Mint or actor",
        ));
    }
    let observation = observe_local_series_scenario_v1(rpc)?;
    let source = observe_series_founder_source_v1(rpc, founder_source, collateral_mint, founder)?;
    let (policy, funding) = local_series_founder_scenario_v1(
        plan,
        child_market,
        collateral_mint,
        source.amount,
        &observation.rent,
        observation.finalized_slot,
    )?;
    let prepared = crate::series_founder::prepare_series_founder_from_market_v1(
        plan,
        child_market,
        collateral_mint,
        founder,
        refund_owner,
        policy,
        funding,
    )?;
    Ok((prepared, observation))
}

/// Build the first Consume Shadow certificate before the selected parent has
/// published its ProgramSet and descriptor records.
///
/// The full observed source operator intentionally needs those finalized M1
/// records and therefore runs after Found as a cross-check.  First selection
/// instead derives the descriptor's semantic coordinates from the same
/// canonical Series owners used by `encode_series_action_descriptor_v5`, then
/// hands the opaque Consume child bank and live Profile13 widths to the
/// preselection producer.  The caller has already authenticated the checked
/// accelerator release and evidence bytes used to form `release_sources`;
/// this boundary accepts no text-form certificate identity.
pub(crate) fn build_series_shadow_preselection_v1(
    template_bytes: &[u8],
    fixed_data_lengths: &[u32;
        dclutch_series_shadow_bundle_generator::SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4],
    children: &dclutch_operator::series_child_bank_v1::SeriesChildBankV1,
    evidence: &crate::series_checked_evidence::CheckedSeriesShadowEvidenceV1,
) -> Result<dclutch_series_shadow_bundle_generator::BuiltSeriesShadowSourceV1> {
    use dclutch_series_shadow_bundle_generator::{
        SeriesShadowBundleSourceV4, SeriesShadowDescriptorSemanticsV4,
        SeriesShadowReleaseSourcesV4, build_series_shadow_preselection_v1 as build,
    };
    use dclutch_trading::series::{
        SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3, SERIES_ROOT_SCHEMA_PREIMAGE_V3,
        SERIES_SUCCESSOR_KIND_PREIMAGE_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        SERIES_TICKET_DERIVATION_PREIMAGE_V3, TemplateV3, replay::SERIES_STATE_BYTES_V3,
    };

    let template = TemplateV3::decode(template_bytes)
        .map_err(|error| Error::new(format!("Series Shadow Template decode: {error:?}")))?;
    let identity = |bytes: [u8; 32], label: &str| {
        ContentId::new(bytes).map_err(|_| Error::new(format!("Series Shadow {label} identity")))
    };
    let descriptor = SeriesShadowDescriptorSemanticsV4 {
        kind: identity(
            Sha256::digest(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).into(),
            "kind",
        )?,
        config_schema: identity(SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3, "Template schema")?,
        request_schema: identity(
            Sha256::digest(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3).into(),
            "request schema",
        )?,
        root_schema: identity(
            Sha256::digest(SERIES_ROOT_SCHEMA_PREIMAGE_V3).into(),
            "root schema",
        )?,
        derivation_policy: identity(
            Sha256::digest(SERIES_TICKET_DERIVATION_PREIMAGE_V3).into(),
            "Ticket derivation",
        )?,
        capacity_profile: dclutch_trading::series::template_content_id(template_bytes)
            .map_err(|_| Error::new("Series Shadow Template content identity"))?,
        root_state_bytes: u32::try_from(SERIES_STATE_BYTES_V3)
            .map_err(|_| Error::new("Series Shadow root state width escaped u32"))?,
    };
    // Consume is the only first-accelerated action. Its release owner emits
    // this empty policy directly; it depends on no certificate or parent-root
    // identity, so it can be derived before first selected publication.
    let lifecycle = dclutch_trading_sbf::series::release_v5::series_consume_lifecycle_v5()
        .map_err(|error| Error::new(format!("Series Shadow Consume lifecycle: {error:?}")))?;
    let built = build(SeriesShadowBundleSourceV4 {
        descriptor,
        release_sources: SeriesShadowReleaseSourcesV4 {
            semantic_source: include_bytes!(
                "../../../../../programs/dclutch-trading-sbf/src/series/consume_artifacts_v4.rs"
            ),
            compiler_source: &evidence.compiler_manifest,
            toolchain_manifest: &evidence.toolchain,
            accelerator_semantic_release: evidence.accelerator_semantic_release,
            translation_validation: evidence.translation_validation,
        },
        lifecycle: &lifecycle,
        fixed_data_lengths,
        child_requests: children.consume_requests(),
        occurrence_count: template.occurrence_count(),
    })
    .map_err(|error| Error::new(format!("Series Shadow preselection refused: {error:?}")))?;
    let certificate: [u8; 32] = Sha256::digest(built.certificate).into();
    if certificate != built.build_inputs.certificate.to_bytes() {
        return Err(Error::new(
            "Series Shadow preselection certificate hash changed before Registry finalization",
        ));
    }
    // The generator owns the certificate wire, while the evidence owner owns
    // every checked source/build join.  Authenticate both before these bytes
    // can be written as an accelerator include or finalized in Registry.
    crate::series_checked_evidence::authenticate_series_shadow_generated_v1(evidence, &built)?;
    Ok(built)
}

/// One Registry raw/staging pair that has already reached finality.  The
/// hydrator retains the canonical body so geometry can prove both the raw
/// address and its width, rather than treating a record as an arbitrary
/// Registry-owned account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SeriesPrepareFinalizedRecordV1<'a> {
    pub(crate) schema: [u8; 32],
    pub(crate) body: &'a [u8],
    pub(crate) raw: Pubkey,
    pub(crate) staging: Pubkey,
}

/// A non-record M0 ProjectFound coordinate read at finality.  M0's raw
/// records are represented by [`SeriesPrepareFinalizedRecordV1`] instead;
/// splitting them prevents a caller from claiming that a byte-identical
/// Registry account is the raw PDA for a different record identity.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeriesPrepareFinalizedAccountV1 {
    pub(crate) address: Pubkey,
    pub(crate) expected_owner: Pubkey,
}

/// The finalized M0 frame forwarded by projected-Custody's Initialize child.
/// It is the ordinary Core `ProjectFound36` order, with the Rent sysvar
/// omitted by Core.  The M0 Market remains a vacant coordinate: Prepare never
/// creates Core state, and Consume's later Found is its only creator.
#[derive(Clone, Copy)]
pub(crate) struct SeriesPrepareM0FrameV1<'a> {
    pub(crate) project_found: [Pubkey; dclutch_market::PROJECT_FOUND_ACCOUNT_COUNT_V2],
    pub(crate) records: &'a [SeriesPrepareFinalizedRecordV1<'a>],
    /// ProjectFound record coordinates that the M0 publisher proved vacant.
    /// They stay zero-width vacancies through Prepare; this is distinct from
    /// M0 Core and every future Custody state, which have their own owners.
    pub(crate) vacancies: &'a [Pubkey],
    pub(crate) finalized_accounts: &'a [SeriesPrepareFinalizedAccountV1],
}

/// Borrow the exact M0 Registry records the canonical publisher finalized.
/// This is the only bridge from Market's typed publication owner into Series
/// Prepare: it copies no body and recomputes no record identity.
pub(crate) fn series_prepare_records_from_m0_publication_v1<'a>(
    publication: &'a FutureMarketImmutablePublicationV1,
) -> Vec<SeriesPrepareFinalizedRecordV1<'a>> {
    publication
        .series_prepare_records
        .iter()
        .map(|record| SeriesPrepareFinalizedRecordV1 {
            schema: record.published.schema,
            body: &record.body,
            raw: record.published.raw,
            staging: record.published.staging,
        })
        .collect()
}

/// Select one M0 record from the publisher's exact finalized closure.
///
/// The source compiler already owns the expected schema and body.  This
/// lookup refuses an absent, duplicated, or byte-substituted record instead
/// of deriving a fresh pair from a parallel Market preview.
pub(crate) fn series_prepare_record_from_m0_publication_v1<'a>(
    records: &'a [SeriesPrepareFinalizedRecordV1<'a>],
    schema: [u8; 32],
    body: &[u8],
    label: &str,
) -> Result<SeriesPrepareFinalizedRecordV1<'a>> {
    let matches = records
        .iter()
        .copied()
        .filter(|record| record.schema == schema && record.body == body)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [record] => Ok(*record),
        [] => Err(Error::new(format!(
            "Series Prepare M0 publisher omitted canonical {label} record"
        ))),
        _ => Err(Error::new(format!(
            "Series Prepare M0 publisher duplicated canonical {label} record"
        ))),
    }
}

/// Convert a finalized Series founder publication to the hydratable pair,
/// retaining the exact source-owner body for the later Registry readback.
pub(crate) fn series_prepare_founder_record_v1<'a>(
    published: crate::runtime::PublishedRecord,
    schema: [u8; 32],
    body: &'a [u8],
    label: &str,
) -> Result<SeriesPrepareFinalizedRecordV1<'a>> {
    let digest: [u8; 32] = Sha256::digest(body).into();
    if published.schema != schema || published.digest != digest {
        return Err(Error::new(format!(
            "Series Prepare founder publication changed canonical {label} schema or body"
        )));
    }
    Ok(SeriesPrepareFinalizedRecordV1 {
        schema,
        body,
        raw: published.raw,
        staging: published.staging,
    })
}

/// Assemble the M0 half of the hydrator input directly from the one canonical
/// publisher result.  The caller supplies only the non-record accounts it
/// observed at the same finality floor; no future Core or Custody account can
/// enter this frame through that list.
pub(crate) fn series_prepare_m0_frame_from_publication_v1<'a>(
    publication: &'a FutureMarketImmutablePublicationV1,
    records: &'a [SeriesPrepareFinalizedRecordV1<'a>],
    finalized_accounts: &'a [SeriesPrepareFinalizedAccountV1],
) -> SeriesPrepareM0FrameV1<'a> {
    SeriesPrepareM0FrameV1 {
        project_found: publication.project_found,
        records,
        vacancies: &publication.series_prepare_vacancies,
        finalized_accounts,
    }
}

/// Inputs that turn the semantic-owner child bank into the complete physical
/// first-Prepare layout.  M1 is the active Series parent (`parent_root`),
/// while the ProjectFound frame and Template leaves are M0 child facts.
/// Whether M1's Series root is the pre-activation fixed-width projection or
/// the finalized selector-255 account.  The compiler needs the first form to
/// break the descriptor/root cycle; Prepare itself always uses `Finalized`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SeriesPrepareParentRootStateV1 {
    PreActivationPredicted,
    Finalized,
}

pub(crate) struct SeriesPrepareHydratorInputV1<'a> {
    pub(crate) registry: Pubkey,
    pub(crate) core: Pubkey,
    pub(crate) trading: Pubkey,
    pub(crate) custody: Pubkey,
    pub(crate) rent_program: Pubkey,
    pub(crate) parent_root: Pubkey,
    pub(crate) parent_root_state: SeriesPrepareParentRootStateV1,
    pub(crate) m0: SeriesPrepareM0FrameV1<'a>,
    pub(crate) template: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) occurrence: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) ticket: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) portfolio: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) children: &'a dclutch_operator::series_child_bank_v1::SeriesChildBankV1,
}

/// The four immutable records whose roles are outside M0's ordinary
/// `ProjectFound` subframe.  They are still carried as canonical Registry
/// pairs: Template, occurrence and Ticket are founder facts, while Portfolio
/// is the Product graph record which the M0 publisher finalized.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeriesPrepareHydrationRecordsV1<'a> {
    pub(crate) template: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) occurrence: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) ticket: SeriesPrepareFinalizedRecordV1<'a>,
    pub(crate) portfolio: SeriesPrepareFinalizedRecordV1<'a>,
}

/// Materialize the current source compiler's opaque Prepare child bank into
/// the full 116-role geometry at one finalized slot.
///
/// This is the production connection between the semantic owners and the
/// hydrator.  The caller first invokes it with the release-owned predicted
/// M1 root, then activates that same parent and invokes it again with
/// [`SeriesPrepareParentRootStateV1::Finalized`].  The caller owns the
/// invariance comparison because it is the only layer that sees both
/// observations; this helper owns neither an alternate request bank nor a
/// second geometry representation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn observe_series_prepare_preprofile_geometry_v1(
    rpc: &mut Rpc,
    selection: &mut crate::series_found_prepare_campaign::SeriesFoundPrepareSelectionInputV1<'_>,
    m0: SeriesPrepareM0FrameV1<'_>,
    records: SeriesPrepareHydrationRecordsV1<'_>,
    parent_root_state: SeriesPrepareParentRootStateV1,
    minimum_slot: u64,
) -> Result<SeriesPrepareFixedDataLengthsV1> {
    // The preprofile is the only producer of Prepare children.  Do not admit
    // a caller-authored child byte bank merely to obtain geometry.
    selection.geometry = None;
    let preprofile =
        crate::series_found_prepare_campaign::derive_series_found_prepare_preprofile_v1(selection)?;
    let hydrator = SeriesPrepareHydratorInputV1 {
        registry: Pubkey::new_from_array(selection.registry_program.to_bytes()),
        core: selection.material.core,
        trading: selection.material.trading,
        custody: selection.material.custody,
        rent_program: selection.material.rent_program,
        parent_root: selection.material.parent_root,
        parent_root_state,
        m0,
        template: records.template,
        occurrence: records.occurrence,
        ticket: records.ticket,
        portfolio: records.portfolio,
        children: &preprofile.prepare_children,
    };
    let layout = hydrate_series_prepare_role_layout_v1(&hydrator)?;
    observe_series_prepare_geometry_v1(rpc, &layout, minimum_slot)
}

/// Compile the selected Series closure only after the Prepare profile has been
/// replaced by a full finalized M0/M1 observation.
///
/// Expire and Consume are likewise derived from their semantic child bank and
/// the same finalized M0/M1 facts. Consume's phase states whether its
/// Prepare-created Custody accounts are canonical predictions for selection or
/// finalized facts after the actual Prepare transaction.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compile_series_prepare_from_hydrated_geometry_v1(
    rpc: &mut Rpc,
    mut selection: crate::series_found_prepare_campaign::SeriesFoundPrepareSelectionInputV1<'_>,
    consume_shadow_certificate_program: ContentId,
    m0: SeriesPrepareM0FrameV1<'_>,
    records: SeriesPrepareHydrationRecordsV1<'_>,
    parent_root_state: SeriesPrepareParentRootStateV1,
    parent_root_fact: crate::series_found_prepare_input::SeriesParentRootFactV1,
    consume_prestate: crate::series_consume_geometry::SeriesConsumePrestateV1,
    minimum_slot: u64,
) -> Result<crate::series_found_prepare_campaign::CompiledSeriesFoundPrepareSelectionV1> {
    if selection.geometry.is_some() {
        return Err(Error::new(
            "Series selected compilation refuses caller-supplied action geometry",
        ));
    }
    let preprofile =
        crate::series_found_prepare_campaign::derive_series_found_prepare_preprofile_v1(
            &mut selection,
        )?;
    let expected_root = match parent_root_fact {
        crate::series_found_prepare_input::SeriesParentRootFactV1::Predicted(prediction) => (
            prediction.root,
            SeriesPrepareParentRootStateV1::PreActivationPredicted,
        ),
        crate::series_found_prepare_input::SeriesParentRootFactV1::Finalized { root, .. } => {
            (root, SeriesPrepareParentRootStateV1::Finalized)
        }
    };
    if selection.material.parent_root != expected_root.0 || parent_root_state != expected_root.1 {
        return Err(Error::new(
            "Series Prepare root state disagreed with its typed geometry fact",
        ));
    }
    let consume_fixed_data_lengths =
        crate::series_consume_geometry::derive_series_consume_fixed_data_lengths_v1(
            rpc,
            crate::series_consume_geometry::SeriesConsumeGeometryInputV1 {
                preprofile: &preprofile,
                m0,
                records,
                parent_root: parent_root_fact,
                registry: Pubkey::new_from_array(selection.registry_program.to_bytes()),
                core: selection.material.core,
                trading: selection.material.trading,
                custody: selection.material.custody,
                claims: selection.material.claims,
                rent_program: selection.material.rent_program,
                prestate: consume_prestate,
                minimum_slot,
            },
        )?;
    let expire_fixed_data_lengths =
        crate::series_expire_geometry::derive_series_expire_fixed_data_lengths_v1(
            rpc,
            crate::series_expire_geometry::SeriesExpireGeometryInputV1 {
                preprofile: &preprofile,
                m0,
                records,
                parent_root: parent_root_fact,
                registry: Pubkey::new_from_array(selection.registry_program.to_bytes()),
                trading: selection.material.trading,
                custody: selection.material.custody,
                minimum_slot,
            },
        )?;
    let geometry = crate::series_source::SeriesObservedGeometryV1 {
        prepare_fixed_data_lengths: observe_series_prepare_preprofile_geometry_v1(
            rpc,
            &mut selection,
            m0,
            records,
            parent_root_state,
            minimum_slot,
        )?,
        prepare_ticket_rent_lamports: selection
            .material
            .rent
            .minimum_balance(dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3),
        consume_fixed_data_lengths,
        consume_funding_count: u32::from(selection.funding_ledger_slot_count),
        expire_fixed_data_lengths,
    };
    selection.geometry = Some(geometry);
    crate::series_found_prepare_campaign::compile_series_found_prepare_selection_v1(
        selection,
        consume_shadow_certificate_program,
    )
}

/// Hydrate all 116 Series Prepare roles from the decoded semantic-owner child
/// requests.  This is intentionally a layout constructor, not an RPC reader:
/// `observe_series_prepare_geometry_v1` performs the one finalized snapshot
/// after this function has made every address, owner, canonical record body,
/// and permitted vacancy explicit.
pub(crate) fn hydrate_series_prepare_role_layout_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
) -> Result<SeriesPrepareRoleLayoutV1<'a>> {
    use dclutch_custody::{CustodyRequestV1, ProjectedCustodyRequestV1};

    let children = input.children.prepare_requests();
    let projected_initialize = ProjectedCustodyRequestV1::decode(children.projected_initialize)
        .map_err(|error| {
            Error::new(format!(
                "Series Prepare projected Initialize decode: {error:?}"
            ))
        })?;
    let projected_open =
        ProjectedCustodyRequestV1::decode(children.projected_open).map_err(|error| {
            Error::new(format!(
                "Series Prepare projected OpenHoard decode: {error:?}"
            ))
        })?;
    let replay_initialize =
        CustodyRequestV1::decode(children.replay_initialize).map_err(|error| {
            Error::new(format!(
                "Series Prepare replay Initialize decode: {error:?}"
            ))
        })?;
    let escrow_open = CustodyRequestV1::decode(children.escrow_open)
        .map_err(|error| Error::new(format!("Series Prepare escrow Open decode: {error:?}")))?;
    let escrow_lock = CustodyRequestV1::decode(children.escrow_lock)
        .map_err(|error| Error::new(format!("Series Prepare escrow Lock decode: {error:?}")))?;

    require_prepare_projected_pair_v1(
        projected_initialize,
        projected_open,
        input.parent_root,
        input.trading,
        input.core,
        input.rent_program,
    )?;
    for request in [replay_initialize, escrow_open, escrow_lock] {
        require_prepare_custody_request_v1(request, input)?;
    }

    let root = parent_root_source_v1(input.parent_root, input.parent_root_state, input.trading)?;
    let product = m0_record_by_schema_v1(
        input,
        dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
        "Product",
    )?;
    let basis = m0_record_by_schema_v1(
        input,
        dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        "linked Basis",
    )?;
    let outer = [
        root,
        record_raw_v1("Series Template", input.registry, input.template)?,
        record_raw_v1("M0 Product", input.registry, product)?,
        record_raw_v1("M0 Portfolio", input.registry, input.portfolio)?,
        record_raw_v1("M0 linked Basis", input.registry, basis)?,
        SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "Series Ticket replay",
            address: ticket_state_address_v1(input)?,
            fixed_data_len: u32::try_from(
                dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3,
            )
            .map_err(|_| Error::new("Series Ticket replay width escaped u32"))?,
        },
    ];
    let projected_initialize =
        projected_frame_v1::<47>(input, projected_initialize, children.projected_initialize)?;
    let projected_open = projected_frame_v1::<15>(input, projected_open, children.projected_open)?;
    let replay_initialize =
        custody_frame_v1::<13>(input, replay_initialize, children.replay_initialize)?;
    let escrow_open = custody_frame_v1::<16>(input, escrow_open, children.escrow_open)?;
    let escrow_lock = custody_frame_v1::<14>(input, escrow_lock, children.escrow_lock)?;
    Ok(SeriesPrepareRoleLayoutV1 {
        outer,
        projected_initialize,
        projected_open,
        replay_initialize,
        escrow_open,
        escrow_lock,
        occurrence_evidence: [
            record_raw_v1("Series occurrence", input.registry, input.occurrence)?,
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "Series occurrence staging",
                address: input.occurrence.staging,
                fixed_data_len: 0,
            },
            record_raw_v1("Series Ticket", input.registry, input.ticket)?,
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "Series Ticket staging",
                address: input.ticket.staging,
                fixed_data_len: 0,
            },
        ],
        custody_program: finalized_v1(
            "Custody callee",
            input.custody,
            bpf_loader_upgradeable::ID,
            None,
        ),
    })
}

fn parent_root_source_v1<'a>(
    parent_root: Pubkey,
    state: SeriesPrepareParentRootStateV1,
    trading: Pubkey,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    match state {
        SeriesPrepareParentRootStateV1::PreActivationPredicted => Ok(
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "pre-activation M1 Series root",
                address: parent_root,
                fixed_data_len: u32::try_from(
                    dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
                )
                .map_err(|_| Error::new("Series root width escaped u32"))?,
            },
        ),
        SeriesPrepareParentRootStateV1::Finalized => Ok(finalized_v1(
            "M1 Series root",
            parent_root,
            trading,
            None,
        )),
    }
}

fn finalized_v1<'a>(
    role: &'static str,
    address: Pubkey,
    owner: Pubkey,
    body: Option<&'a [u8]>,
) -> SeriesPrepareRoleSourceV1<'a> {
    SeriesPrepareRoleSourceV1::Finalized {
        role,
        address,
        expected_owner: owner,
        canonical_body: body,
    }
}

fn record_raw_v1<'a>(
    role: &'static str,
    registry: Pubkey,
    record: SeriesPrepareFinalizedRecordV1<'a>,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(record.schema)
            .map_err(|_| Error::new("Series Prepare record schema was zero"))?,
        ContentDigest::new(Sha256::digest(record.body).into())
            .map_err(|_| Error::new("Series Prepare record digest was zero"))?,
    );
    let derive = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &registry,
        )
        .0
    };
    if record.raw != derive(key.raw_record_pda_seeds())
        || record.staging != derive(key.staging_cursor_pda_seeds())
    {
        return Err(Error::new(format!(
            "Series Prepare {role} record pair was noncanonical"
        )));
    }
    Ok(finalized_v1(role, record.raw, registry, Some(record.body)))
}

fn ticket_state_address_v1(input: &SeriesPrepareHydratorInputV1<'_>) -> Result<Pubkey> {
    let ticket = dclutch_trading::series::admit_ticket(input.ticket.body)
        .map_err(|_| Error::new("Series Prepare Ticket record refused"))?
        .content_id();
    Ok(Pubkey::find_program_address(
        &[
            dclutch_trading::series::replay::SERIES_TICKET_STATE_PDA_DOMAIN_V3,
            input.parent_root.as_ref(),
            ticket.as_bytes(),
        ],
        &input.trading,
    )
    .0)
}

fn require_prepare_projected_pair_v1(
    initialize: dclutch_custody::ProjectedCustodyRequestV1,
    open: dclutch_custody::ProjectedCustodyRequestV1,
    parent_root: Pubkey,
    trading: Pubkey,
    core: Pubkey,
    rent_program: Pubkey,
) -> Result<()> {
    use dclutch_custody::ProjectedCustodyOperationV1;
    if initialize.operation != ProjectedCustodyOperationV1::Initialize
        || open.operation != ProjectedCustodyOperationV1::OpenHoard
        || initialize.market != open.market
        || initialize.release_set != open.release_set
        || initialize.parent_capability_root != parent_root.to_bytes()
        || open.parent_capability_root != parent_root.to_bytes()
        || initialize.caller_program != trading.to_bytes()
        || open.caller_program != trading.to_bytes()
        || initialize.core_program != core.to_bytes()
        || open.core_program != core.to_bytes()
        || initialize.rent_program != rent_program.to_bytes()
        || open.rent_program != rent_program.to_bytes()
    {
        return Err(Error::new(
            "Series Prepare projected children did not join the active M1 root",
        ));
    }
    Ok(())
}

fn require_prepare_custody_request_v1(
    request: dclutch_custody::CustodyRequestV1,
    input: &SeriesPrepareHydratorInputV1<'_>,
) -> Result<()> {
    if request.caller_role != dclutch_custody::CallerRoleV1::Trading
        || request.caller_program != input.trading.to_bytes()
        || request.market != input.m0.project_found[1].to_bytes()
    {
        return Err(Error::new(
            "Series Prepare Custody child did not join M0 and Trading",
        ));
    }
    Ok(())
}

fn projected_frame_v1<'a, const N: usize>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    request: dclutch_custody::ProjectedCustodyRequestV1,
    bytes: &[u8],
) -> Result<[SeriesPrepareRoleSourceV1<'a>; N]> {
    use dclutch_custody::{
        ProjectedCustodyCallerSeedsV1, ProjectedCustodyOperationV1, ProjectedCustodyStateSeedsV2,
    };
    let digest = solana_program::hash::hash(bytes).to_bytes();
    let caller = Pubkey::find_program_address(
        &ProjectedCustodyCallerSeedsV1::new(request, digest).as_slices(),
        &input.trading,
    )
    .0;
    let state = Pubkey::find_program_address(
        &ProjectedCustodyStateSeedsV2::from_request(request).as_slices(),
        &input.custody,
    )
    .0;
    let cache = Pubkey::find_program_address(
        &[
            dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
            &request.release_set,
        ],
        &input.registry,
    )
    .0;
    let common = [
        SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "projected caller authority",
            address: caller,
            fixed_data_len: 0,
        },
        SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "projected Custody state",
            address: state,
            fixed_data_len: u32::try_from(dclutch_custody::PROJECTED_CUSTODY_STATE_BYTES_V2)
                .map_err(|_| Error::new("projected state width escaped u32"))?,
        },
        finalized_v1("activation cache", cache, input.registry, None),
        finalized_v1(
            "Registry program",
            input.registry,
            bpf_loader_upgradeable::ID,
            None,
        ),
        finalized_v1(
            "Trading program",
            input.trading,
            bpf_loader_upgradeable::ID,
            None,
        ),
        finalized_v1(
            "Trading ProgramData",
            crate::upgrade::target_programdata(input.trading),
            bpf_loader_upgradeable::ID,
            None,
        ),
        finalized_v1(
            "M0 RentCredit",
            Pubkey::new_from_array(request.rent_credit),
            input.rent_program,
            None,
        ),
    ];
    match request.operation {
        ProjectedCustodyOperationV1::Initialize => {
            let mut values = Vec::from(common);
            values.extend([
                finalized_v1("Core program", input.core, bpf_loader_upgradeable::ID, None),
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.payer))?,
                finalized_v1("Rent sysvar", sysvar::rent::ID, sysvar::ID, None),
                finalized_v1(
                    "System program",
                    system_program::ID,
                    native_loader::ID,
                    None,
                ),
            ]);
            for address in input.m0.project_found {
                values.push(m0_source_v1(input, address)?);
            }
            values
                .try_into()
                .map_err(|_| Error::new("Series Prepare projected Initialize frame drifted"))
        }
        ProjectedCustodyOperationV1::OpenHoard => {
            let mut values = Vec::from(common);
            let vault = Pubkey::new_from_array(request.hoard_vault);
            let expected_vault = Pubkey::find_program_address(
                &dclutch_custody::CustodyVaultSeedsV1::new(
                    request.market,
                    request.release_set,
                    request.context_digest,
                    dclutch_custody::CompartmentV1::HoardPrincipal,
                )
                .as_slices(),
                &input.custody,
            )
            .0;
            if vault != expected_vault {
                return Err(Error::new(
                    "Series Prepare Hoard vault PDA differed from Custody seeds",
                ));
            }
            let authority = Pubkey::find_program_address(
                &dclutch_custody::CustodyAuthoritySeedsV1::new(request.market, request.release_set)
                    .as_slices(),
                &input.custody,
            )
            .0;
            values.extend([
                SeriesPrepareRoleSourceV1::PredictedVacancy {
                    role: "projected Hoard vault",
                    address: vault,
                    fixed_data_len: u32::try_from(dclutch_custody::token_svm::ACCOUNT_BYTES)
                        .map_err(|_| Error::new("Hoard vault width escaped u32"))?,
                },
                SeriesPrepareRoleSourceV1::PredictedVacancy {
                    role: "Custody authority",
                    address: authority,
                    fixed_data_len: 0,
                },
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.mint))?,
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.token_program))?,
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.payer))?,
                finalized_v1("Rent sysvar", sysvar::rent::ID, sysvar::ID, None),
                finalized_v1(
                    "System program",
                    system_program::ID,
                    native_loader::ID,
                    None,
                ),
                m0_source_v1(input, Pubkey::new_from_array(request.market))?,
            ]);
            values
                .try_into()
                .map_err(|_| Error::new("Series Prepare projected OpenHoard frame drifted"))
        }
        _ => Err(Error::new(
            "Series Prepare projected child selected an unsupported operation",
        )),
    }
}

fn custody_frame_v1<'a, const N: usize>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    request: dclutch_custody::CustodyRequestV1,
    bytes: &[u8],
) -> Result<[SeriesPrepareRoleSourceV1<'a>; N]> {
    use dclutch_custody::{CustodyFrameRoleV1, CustodyFrameSpecV1};
    let spec = CustodyFrameSpecV1::new(request.operation);
    if usize::from(spec.account_count()) != N {
        return Err(Error::new(
            "Series Prepare Custody frame cardinality drifted",
        ));
    }
    let digest = solana_program::hash::hash(bytes).to_bytes();
    let authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(request.release_set)
                .map_err(|_| Error::new("Series Prepare Custody release was zero"))?,
            request.market,
            ExecutionRoleV1::Trading,
            request.context,
            digest,
        )
        .map_err(|_| Error::new("Series Prepare Custody caller seeds refused"))?
        .as_slices(),
        &input.trading,
    )
    .0;
    let replay = Pubkey::find_program_address(
        &dclutch_custody::CustodyReplaySeedsV1::from_request(request).as_slices(),
        &input.custody,
    )
    .0;
    let cache = Pubkey::find_program_address(
        &[
            dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
            &request.release_set,
        ],
        &input.registry,
    )
    .0;
    let mut values = Vec::with_capacity(N);
    for index in 0..N {
        let role = spec
            .account(u16::try_from(index).map_err(|_| Error::new("Custody index escaped u16"))?)
            .map_err(|_| Error::new("Series Prepare Custody frame coordinate refused"))?
            .role();
        let source = match role {
            CustodyFrameRoleV1::CallerAuthority => SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "Custody caller authority",
                address: authority,
                fixed_data_len: 0,
            },
            CustodyFrameRoleV1::CoreMarket => {
                m0_source_v1(input, Pubkey::new_from_array(request.market))?
            }
            CustodyFrameRoleV1::ActivationCache => {
                finalized_v1("activation cache", cache, input.registry, None)
            }
            CustodyFrameRoleV1::RegistryProgram => finalized_v1(
                "Registry program",
                input.registry,
                bpf_loader_upgradeable::ID,
                None,
            ),
            CustodyFrameRoleV1::CallerProgram => finalized_v1(
                "Trading program",
                input.trading,
                bpf_loader_upgradeable::ID,
                None,
            ),
            CustodyFrameRoleV1::CallerProgramData => finalized_v1(
                "Trading ProgramData",
                crate::upgrade::target_programdata(input.trading),
                bpf_loader_upgradeable::ID,
                None,
            ),
            CustodyFrameRoleV1::RealmRecord => m0_source_v1(input, input.m0.project_found[4])?,
            CustodyFrameRoleV1::RealmStaging => m0_source_v1(input, input.m0.project_found[5])?,
            CustodyFrameRoleV1::Replay => SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "SeriesEscrow replay",
                address: replay,
                fixed_data_len: u32::try_from(dclutch_custody::CUSTODY_REPLAY_BYTES_V1)
                    .map_err(|_| Error::new("Custody replay width escaped u32"))?,
            },
            CustodyFrameRoleV1::Payer => {
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.payer))?
            }
            CustodyFrameRoleV1::SystemProgram => finalized_v1(
                "System program",
                system_program::ID,
                native_loader::ID,
                None,
            ),
            CustodyFrameRoleV1::RentSysvar => {
                finalized_v1("Rent sysvar", sysvar::rent::ID, sysvar::ID, None)
            }
            CustodyFrameRoleV1::Mint => {
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.mint))?
            }
            CustodyFrameRoleV1::Vault => custody_vault_source_v1(
                input,
                request,
                request.operation == dclutch_custody::OperationV1::OpenVault,
            )?,
            CustodyFrameRoleV1::CustodyAuthority => SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "Custody authority",
                address: Pubkey::find_program_address(
                    &dclutch_custody::CustodyAuthoritySeedsV1::new(
                        request.market,
                        request.release_set,
                    )
                    .as_slices(),
                    &input.custody,
                )
                .0,
                fixed_data_len: 0,
            },
            CustodyFrameRoleV1::TokenProgram => {
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.token_program))?
            }
            CustodyFrameRoleV1::TransferSource => {
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.source))?
            }
            CustodyFrameRoleV1::TransferDestination => {
                custody_transfer_destination_v1(input, request)?
            }
            CustodyFrameRoleV1::RentRefund => {
                m0_nonrecord_v1(input, Pubkey::new_from_array(request.rent_refund))?
            }
        };
        values.push(source);
    }
    values
        .try_into()
        .map_err(|_| Error::new("Series Prepare Custody frame cardinality changed"))
}

fn custody_vault_source_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    request: dclutch_custody::CustodyRequestV1,
    creating: bool,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    let context = if creating {
        request.destination_vault_context
    } else {
        request.source_vault_context
    };
    let vault = if creating {
        request.destination
    } else {
        request.source
    };
    let compartment = if creating {
        request.destination_compartment
    } else {
        request.source_compartment
    };
    let address = Pubkey::new_from_array(vault);
    if address
        != Pubkey::find_program_address(
            &dclutch_custody::CustodyVaultSeedsV1::new(
                request.market,
                request.release_set,
                context,
                compartment,
            )
            .as_slices(),
            &input.custody,
        )
        .0
    {
        return Err(Error::new(
            "Series Prepare Custody vault differed from canonical seeds",
        ));
    }
    Ok(SeriesPrepareRoleSourceV1::PredictedVacancy {
        role: "SeriesEscrow vault",
        address,
        fixed_data_len: u32::try_from(dclutch_custody::token_svm::ACCOUNT_BYTES)
            .map_err(|_| Error::new("Custody vault width escaped u32"))?,
    })
}

fn custody_transfer_destination_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    request: dclutch_custody::CustodyRequestV1,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    if request.destination_compartment == dclutch_custody::CompartmentV1::SeriesEscrow {
        custody_vault_source_v1(input, request, true)
    } else {
        m0_nonrecord_v1(input, Pubkey::new_from_array(request.destination))
    }
}

fn m0_source_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    address: Pubkey,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    m0_frame_source_v1(input.registry, &input.m0, address)
}

/// Select a unique source-owned M0 record by the schema the universal Hot
/// prefix names.  The Prefix consumes Product and linked Basis bodies, never
/// occurrence/Ticket records which belong to the Series child proof.
fn m0_record_by_schema_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    schema: [u8; 32],
    label: &str,
) -> Result<SeriesPrepareFinalizedRecordV1<'a>> {
    let matches = input
        .m0
        .records
        .iter()
        .copied()
        .filter(|record| record.schema == schema)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [record] => Ok(*record),
        [] => Err(Error::new(format!(
            "Series Prepare M0 frame omitted {label} record"
        ))),
        _ => Err(Error::new(format!(
            "Series Prepare M0 frame duplicated {label} record"
        ))),
    }
}

fn m0_frame_source_v1<'a>(
    registry: Pubkey,
    m0: &'a SeriesPrepareM0FrameV1<'a>,
    address: Pubkey,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    if address == m0.project_found[1] {
        return Ok(SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "vacant M0 Core Market",
            address,
            fixed_data_len: 0,
        });
    }
    if m0.vacancies.contains(&address) {
        return Ok(SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "M0 vacant ProjectFound record",
            address,
            fixed_data_len: 0,
        });
    }
    for record in m0.records {
        if address == record.raw {
            return record_raw_v1("M0 finalized record", registry, *record);
        }
        if address == record.staging {
            record_raw_v1("M0 finalized record", registry, *record)?;
            return Ok(SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "M0 vacant record staging",
                address,
                fixed_data_len: 0,
            });
        }
    }
    m0_frame_nonrecord_v1(m0, address)
}

fn m0_nonrecord_v1<'a>(
    input: &'a SeriesPrepareHydratorInputV1<'a>,
    address: Pubkey,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    m0_frame_nonrecord_v1(&input.m0, address)
}

fn m0_frame_nonrecord_v1<'a>(
    m0: &'a SeriesPrepareM0FrameV1<'a>,
    address: Pubkey,
) -> Result<SeriesPrepareRoleSourceV1<'a>> {
    let account = m0
        .finalized_accounts
        .iter()
        .find(|account| account.address == address)
        .ok_or_else(|| {
            Error::new(format!(
                "Series Prepare omitted finalized M0 account {address}"
            ))
        })?;
    Ok(finalized_v1(
        "finalized M0 account",
        address,
        account.expected_owner,
        None,
    ))
}

#[cfg(test)]
mod prepare_hydrator_tests {
    use super::*;
    use dclutch_custody::{
        CompartmentV1, ProjectedCallerRoleV1, ProjectedCustodyOperationV1,
        ProjectedCustodyRequestV1,
    };

    #[test]
    fn found_prepare_command_rejects_repeated_execution_before_paths() {
        let error = parse_series_found_prepare_arguments_v1(vec![
            "--execute".to_owned(),
            "--execute".to_owned(),
        ])
        .expect_err("the command must not accept duplicate execution switches");
        assert_eq!(error.to_string(), "Series Found/Prepare repeats --execute");
    }

    #[test]
    fn found_prepare_command_rejects_unknown_flag_before_paths() {
        let error = parse_series_found_prepare_arguments_v1(vec![
            "--genesis-cohort".to_owned(),
            "--certificate-id".to_owned(),
            "00".to_owned(),
        ])
        .expect_err("certificate identity must be generated, never text input");
        assert_eq!(
            error.to_string(),
            "Series Found/Prepare rejects argument --certificate-id"
        );
    }

    #[test]
    fn selected_series_ledger_width_follows_its_manifest_bit() {
        assert_eq!(
            selected_series_funding_ledger_slot_count_v1(0).expect("first selected manifest bit"),
            1
        );
        assert_eq!(
            selected_series_funding_ledger_slot_count_v1(15).expect("last selected manifest bit"),
            1
        );
        assert_eq!(
            selected_series_funding_ledger_slot_count_v1(16)
                .expect_err("sixteenth index cannot fit the u16 funding mask")
                .to_string(),
            "Series selected manifest entry exceeds the funding mask"
        );
    }

    fn projected(root: Pubkey) -> ProjectedCustodyRequestV1 {
        ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::Initialize,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: [1; 32],
            generation: 1,
            realm: [2; 32],
            product_record: [3; 32],
            product: [4; 32],
            source: [5; 32],
            release_set: [6; 32],
            projection_receipt_digest: [7; 32],
            parent_capability_root: root.to_bytes(),
            context_digest: [8; 32],
            caller_program: [9; 32],
            payer: [10; 32],
            core_program: [11; 32],
            rent_program: [12; 32],
            refund_owner: [13; 32],
            rent_credit: [14; 32],
            hoard_vault: [15; 32],
            funding_source_vault: [16; 32],
            funding_source_context: [17; 32],
            funding_source_compartment: CompartmentV1::SeriesEscrow,
            mint: [18; 32],
            token_program: [19; 32],
            collateral_release: [20; 32],
            expiry_slot: 2,
            expected_revision: 0,
            resulting_revision: 1,
            amount: 0,
            state_rent_lamports: 1,
            vault_rent_lamports: 1,
            funding_source_replay_revision: 3,
            funding_source_state_rent_lamports: 1,
            funding_source_vault_rent_lamports: 1,
        }
    }

    #[test]
    fn projected_prepare_children_cannot_substitute_the_m1_root() {
        let root = Pubkey::new_unique();
        let wrong = Pubkey::new_unique();
        let error = require_prepare_projected_pair_v1(
            projected(wrong),
            ProjectedCustodyRequestV1 {
                operation: ProjectedCustodyOperationV1::OpenHoard,
                expected_revision: 1,
                resulting_revision: 2,
                ..projected(wrong)
            },
            root,
            Pubkey::new_from_array([9; 32]),
            Pubkey::new_from_array([11; 32]),
            Pubkey::new_from_array([12; 32]),
        )
        .expect_err("M1 root substitution must refuse");
        assert_eq!(
            error.to_string(),
            "Series Prepare projected children did not join the active M1 root"
        );
    }

    #[test]
    fn preactivation_root_is_only_the_fixed_width_projection() {
        let root = Pubkey::new_unique();
        assert!(matches!(
            parent_root_source_v1(
                root,
                SeriesPrepareParentRootStateV1::PreActivationPredicted,
                Pubkey::new_unique(),
            )
            .expect("projected root"),
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "pre-activation M1 Series root",
                address,
                fixed_data_len,
            } if address == root && fixed_data_len == dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5 as u32
        ));
    }

    #[test]
    fn m0_core_coordinate_remains_a_vacancy_until_consume_found() {
        let market = Pubkey::new_unique();
        let absent_floor = Pubkey::new_unique();
        let mut project_found = [Pubkey::default(); dclutch_market::PROJECT_FOUND_ACCOUNT_COUNT_V2];
        project_found[1] = market;
        let vacancies = [absent_floor];
        let m0 = SeriesPrepareM0FrameV1 {
            project_found,
            records: &[],
            vacancies: &vacancies,
            finalized_accounts: &[],
        };
        assert!(matches!(
            m0_frame_source_v1(Pubkey::new_unique(), &m0, market)
                .expect("M0 Core coordinate"),
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "vacant M0 Core Market",
                address,
                fixed_data_len: 0,
            } if address == market
        ));
        assert!(matches!(
            m0_frame_source_v1(Pubkey::new_unique(), &m0, absent_floor)
                .expect("M0 absent floor coordinate"),
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "M0 vacant ProjectFound record",
                address,
                fixed_data_len: 0,
            } if address == absent_floor
        ));
    }

    #[test]
    fn m0_token_program_keeps_its_authenticated_upgradeable_loader_owner() {
        let token_program = Pubkey::new_unique();
        let frame = SeriesPrepareM0FrameV1 {
            project_found: [Pubkey::default(); dclutch_market::PROJECT_FOUND_ACCOUNT_COUNT_V2],
            records: &[],
            vacancies: &[],
            finalized_accounts: &[SeriesPrepareFinalizedAccountV1 {
                address: token_program,
                expected_owner: bpf_loader_upgradeable::ID,
            }],
        };
        assert!(matches!(
            m0_frame_nonrecord_v1(&frame, token_program).expect("published Token-2022 program"),
            SeriesPrepareRoleSourceV1::Finalized {
                expected_owner,
                ..
            } if expected_owner == bpf_loader_upgradeable::ID
        ));
    }

    #[test]
    fn m0_publisher_handoff_preserves_its_exact_pair_and_body() {
        let published = crate::runtime::PublishedRecord {
            schema: [7; 32],
            digest: Sha256::digest(b"published M0 body").into(),
            raw: Pubkey::new_unique(),
            staging: Pubkey::new_unique(),
        };
        let publication = FutureMarketImmutablePublicationV1 {
            realm: published,
            product: published,
            domain: published,
            portfolio: published,
            manifest: published,
            rent_credit: Pubkey::new_unique(),
            project_found: [Pubkey::default(); dclutch_market::PROJECT_FOUND_ACCOUNT_COUNT_V2],
            principal_cap_sets: 1,
            series_prepare_records: vec![crate::market::FutureMarketFinalizedRecordV1 {
                published,
                body: b"published M0 body".to_vec(),
            }],
            series_prepare_vacancies: vec![Pubkey::new_unique()],
        };
        let records = series_prepare_records_from_m0_publication_v1(&publication);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].schema, published.schema);
        assert_eq!(records[0].body, b"published M0 body");
        assert_eq!(records[0].raw, published.raw);
        assert_eq!(records[0].staging, published.staging);
        assert_eq!(
            series_prepare_record_from_m0_publication_v1(
                &records,
                published.schema,
                b"published M0 body",
                "published M0"
            )
            .expect("publisher body lookup"),
            records[0]
        );
        let error = series_prepare_record_from_m0_publication_v1(
            &records,
            published.schema,
            b"substituted M0 body",
            "published M0",
        )
        .expect_err("substituted publisher body must refuse");
        assert_eq!(
            error.to_string(),
            "Series Prepare M0 publisher omitted canonical published M0 record"
        );
        assert_eq!(
            series_prepare_founder_record_v1(
                published,
                published.schema,
                b"published M0 body",
                "founder"
            )
            .expect("founder pair"),
            records[0]
        );
        let frame = series_prepare_m0_frame_from_publication_v1(&publication, &records, &[]);
        assert_eq!(frame.records[0].raw, published.raw);
        assert_eq!(frame.vacancies, publication.series_prepare_vacancies);
    }

    #[test]
    fn record_pair_requires_canonical_registry_pdas() {
        let error = record_raw_v1(
            "forged record",
            Pubkey::new_unique(),
            SeriesPrepareFinalizedRecordV1 {
                schema: [1; 32],
                body: b"canonical body",
                raw: Pubkey::new_unique(),
                staging: Pubkey::new_unique(),
            },
        )
        .expect_err("arbitrary Registry pair must refuse");
        assert_eq!(
            error.to_string(),
            "Series Prepare forged record record pair was noncanonical"
        );
    }
}
