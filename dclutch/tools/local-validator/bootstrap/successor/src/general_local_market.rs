//! Read-only owned-loopback General market compiler.
//!
//! The older `local-private-validator-market-v1` General branch is a lab
//! fixture: it derives four deployment identities from the local plan.  This
//! command instead observes the accelerator Program and ProgramData pair that
//! `local-mutable-prepare-v1` installed from the eighth checked gate link, and
//! derives the ArtifactRelease from that observed deployment.  The remaining
//! provenance facts are exact inspectable files; none is accepted as a hex
//! command-line value.

use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::PathBuf,
};

use solana_sdk::pubkey::Pubkey;

use crate::{
    Error, Result,
    general_devnet_market::{
        GeneralDevnetAcceleratorArgumentsV1, GeneralDevnetCompilerArgumentsV1,
        GeneralDevnetEvidenceArgumentsV1, observe_accelerator_deployment_v1,
        read_general_devnet_policy_v1,
    },
};

/// Named compiler for a General selected market on one owned local validator.
pub(crate) const COMMAND_V1: &str = "local-private-validator-general-market-v1";

#[derive(Debug, Default)]
struct ArgumentsV1 {
    plan: Option<String>,
    rpc_url: Option<String>,
    policy: Option<String>,
    compiler_release: Option<String>,
    toolchain: Option<String>,
    translation_validation: Option<String>,
    selection_policy: Option<String>,
    quote_surplus_beneficiary: Option<String>,
    output: Option<String>,
}

impl ArgumentsV1 {
    fn parse(arguments: Vec<String>) -> Result<Self> {
        let mut parsed = Self::default();
        let mut iterator = arguments.into_iter();
        while let Some(argument) = iterator.next() {
            let value = iterator
                .next()
                .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
            let slot = match argument.as_str() {
                "--plan" => &mut parsed.plan,
                "--rpc-url" => &mut parsed.rpc_url,
                "--general-policy" => &mut parsed.policy,
                "--general-compiler-release" => &mut parsed.compiler_release,
                "--general-toolchain" => &mut parsed.toolchain,
                "--general-translation-validation" => &mut parsed.translation_validation,
                "--general-selection-policy" => &mut parsed.selection_policy,
                "--general-quote-surplus-beneficiary" => &mut parsed.quote_surplus_beneficiary,
                "--output" => &mut parsed.output,
                _ => {
                    return Err(Error::new(format!(
                        "unknown {COMMAND_V1} argument: {argument}"
                    )));
                }
            };
            if slot.replace(value).is_some() {
                return Err(Error::new(format!("{argument} may be supplied only once")));
            }
        }
        Ok(parsed)
    }
}

fn required(value: Option<String>, flag: &str) -> Result<String> {
    value.ok_or_else(|| Error::new(format!("{flag} is required")))
}

fn canonical_regular(value: String, flag: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::new(format!("{flag} must be an absolute path")));
    }
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(Error::new(format!(
            "{flag} must name one regular non-symlink file"
        )));
    }
    let canonical = fs::canonicalize(&path)?;
    if canonical != path {
        return Err(Error::new(format!("{flag} path is not canonical")));
    }
    Ok(path)
}

fn absolute_new(value: String, flag: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() || path.exists() || fs::symlink_metadata(&path).is_ok() {
        return Err(Error::new(format!(
            "{flag} must be an absolute path that does not exist"
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
    let canonical_parent = fs::canonicalize(parent)?;
    Ok(canonical_parent.join(
        path.file_name()
            .ok_or_else(|| Error::new(format!("{flag} omitted its filename")))?,
    ))
}

/// Observe the checked local accelerator and write one canonical General
/// `MarketRunInput`.  This is read-only: the following campaign and General
/// activation commands remain the only writers.
pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = ArgumentsV1::parse(arguments)?;
    let plan_path = canonical_regular(required(arguments.plan, "--plan")?, "--plan")?;
    let rpc_url = required(arguments.rpc_url, "--rpc-url")?;
    let policy = canonical_regular(
        required(arguments.policy, "--general-policy")?,
        "--general-policy",
    )?;
    let compiler_release = canonical_regular(
        required(arguments.compiler_release, "--general-compiler-release")?,
        "--general-compiler-release",
    )?;
    let toolchain = canonical_regular(
        required(arguments.toolchain, "--general-toolchain")?,
        "--general-toolchain",
    )?;
    let translation_validation = canonical_regular(
        required(
            arguments.translation_validation,
            "--general-translation-validation",
        )?,
        "--general-translation-validation",
    )?;
    let selection_policy = canonical_regular(
        required(arguments.selection_policy, "--general-selection-policy")?,
        "--general-selection-policy",
    )?;
    let quote_surplus_beneficiary = required(
        arguments.quote_surplus_beneficiary,
        "--general-quote-surplus-beneficiary",
    )?
    .parse::<Pubkey>()
    .map_err(|_| Error::new("--general-quote-surplus-beneficiary must be a base58 Pubkey"))?;
    let output = absolute_new(required(arguments.output, "--output")?, "--output")?;

    let plan: crate::model::SuccessorPlan = serde_json::from_slice(&fs::read(&plan_path)?)?;
    crate::local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)?;
    let registry = crate::plan::pubkey(&plan.registry.program_id)?;
    let accelerator = plan.general_accelerator.as_ref().ok_or_else(|| {
        Error::new(
            "checked local plan omitted the accelerator Loader pair; regenerate it with local-mutable-prepare-v1 from an eight-link checked gate",
        )
    })?;
    let expected_upgrade_authority = accelerator
        .upgrade_authority
        .as_deref()
        .map(crate::plan::pubkey)
        .transpose()?;
    let compiler = GeneralDevnetCompilerArgumentsV1 {
        accelerator: GeneralDevnetAcceleratorArgumentsV1 {
            program: crate::plan::pubkey(&accelerator.program_id)?,
            built_elf: PathBuf::from(&accelerator.checked_candidate_elf_path),
            semantic_release_id: crate::plan::hex32(&accelerator.semantic_release_id)?,
            expected_upgrade_authority,
        },
        evidence: GeneralDevnetEvidenceArgumentsV1 {
            compiler_release,
            toolchain,
            translation_validation,
            selection_policy,
        },
        policy: policy.clone(),
        quote_surplus_beneficiary,
    };

    // Read the policy once before a network request so a caller gets the exact
    // account-profile choice it must make: ordinary Token accounts are 165
    // bytes and the selected ImmutableOwner profile is 170 bytes.
    let token_account_bytes = read_general_devnet_policy_v1(&policy)?.token_account_bytes;
    if !matches!(token_account_bytes, 165 | 170) {
        return Err(Error::new(format!(
            "General token_account_bytes is {token_account_bytes}; the selected local General release admits exactly 165-byte Token or 170-byte ImmutableOwner Token-2022 accounts"
        )));
    }

    let (observed_plan, mut rpc, observation) =
        crate::direct_market::observe_local_market_policy_with_rpc_v1(
            &plan_path, &rpc_url, registry,
        )?;
    let resolution_release =
        crate::direct_market::authenticated_resolution_release_v1(&observed_plan)?;
    let mut input = crate::market::demo_market_input_base(registry, resolution_release)?;
    let deployment = observe_accelerator_deployment_v1(
        &mut rpc,
        &compiler.accelerator,
        observation.finalized_slot,
    )?;
    eprint!("{}", deployment.render_provenance_v1());
    crate::general_devnet_market::attach_devnet_general_capability_v1(
        &mut input,
        &observation,
        deployment.artifact_release_id,
        &compiler,
    )?;
    crate::market::validate_market_input(&input)?;

    let mut bytes = serde_json::to_vec_pretty(&input)?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    println!(
        "{{\"schema\":\"dclutch-local-general-market-observation-v1\",\"output\":{},\"token_account_bytes\":{token_account_bytes},\"accelerator_program\":{}}}",
        serde_json::to_string(&output.display().to_string())?,
        serde_json::to_string(&accelerator.program_id)?,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_refuses_accelerator_identity_flags() {
        let error = ArgumentsV1::parse(vec![
            "--general-accelerator-program-id".into(),
            "11111111111111111111111111111111".into(),
        ])
        .expect_err("loopback compiler derives accelerator identity from its plan");
        assert!(error.to_string().contains("unknown"), "{error}");
    }
}
