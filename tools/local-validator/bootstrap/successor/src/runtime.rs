use std::{
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
};

use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1, ProtocolInfrastructureProfileV1,
};
use reqwest::Url;
use sha2::Digest as _;
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{
    Error, Result,
    model::{ProgramPin, ProviderEvidenceInput, SuccessorPlan},
    plan::{hex, hex32, loader_programdata_bytes, pubkey, validate_program_ids},
};

#[derive(Debug)]
pub(crate) struct RunArgs {
    pub(crate) rpc_url: String,
    pub(crate) plan_path: PathBuf,
    pub(crate) provider_evidence_path: PathBuf,
    pub(crate) output: PathBuf,
}

/// Validate the immutable substrate, then fail closed before any RPC call or
/// signature while the canonical Core infrastructure/Found instructions are
/// absent. This function intentionally cannot produce lifecycle evidence.
pub(crate) fn execute(args: &RunArgs) -> Result<()> {
    validate_existing_absolute(&args.plan_path, "--plan")?;
    validate_existing_absolute(&args.provider_evidence_path, "--provider-evidence")?;
    if !args.output.is_absolute() || args.output.exists() {
        return Err(Error::new("--output must be absolute and nonexistent"));
    }
    let rpc_url = validate_loopback_url(&args.rpc_url)?;
    let plan: SuccessorPlan = serde_json::from_slice(&fs::read(&args.plan_path)?)?;
    let provider: ProviderEvidenceInput =
        serde_json::from_slice(&fs::read(&args.provider_evidence_path)?)?;
    validate_plan(&plan)?;
    validate_provider(&provider, rpc_url.as_str())?;
    Err(Error::new(format!(
        "successor execution unavailable without submitting a transaction: {}",
        plan.execution_blocker
    )))
}

fn validate_plan(plan: &SuccessorPlan) -> Result<()> {
    if plan.schema != "dclutch-local-successor-infrastructure-plan-v2"
        || plan.genesis_boundary.len() != 2
        || plan.bootstrap_order.len() != 5
        || plan.execution_blocker.is_empty()
    {
        return Err(Error::new("invalid successor infrastructure plan header"));
    }
    let programs = [
        pubkey(&plan.registry.program_id)?,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.claims.program_id)?,
        pubkey(&plan.trading.program_id)?,
        pubkey(&plan.resolution.program_id)?,
        pubkey(&plan.custody.program_id)?,
        pubkey(&plan.rent_credit.program_id)?,
    ];
    validate_program_ids(&programs)?;
    let core_authority = pubkey(&plan.core_bootstrap.upgrade_authority)?;
    if core_authority == Pubkey::default()
        || programs.contains(&core_authority)
        || !plan.core_bootstrap.release_recognition_requires_revoke
        || plan.core_bootstrap.genesis_programdata_sha256
            == plan.core_bootstrap.post_revoke_programdata_sha256
    {
        return Err(Error::new(
            "Core bootstrap authority/revocation boundary is not canonical",
        ));
    }
    for (label, pin, authority) in [
        ("registry", &plan.registry, None),
        ("core", &plan.core, Some(core_authority)),
        ("claims", &plan.claims, None),
        ("trading", &plan.trading, None),
        ("resolution", &plan.resolution, None),
        ("custody", &plan.custody, None),
        ("rent-credit", &plan.rent_credit, None),
    ] {
        validate_program_pin(plan, label, pin, authority)?;
    }
    let core_programdata = plan
        .genesis_accounts
        .get("loader.core.programdata")
        .ok_or_else(|| Error::new("missing Core ProgramData genesis pin"))?;
    if core_programdata.data_sha256 != plan.core_bootstrap.genesis_programdata_sha256 {
        return Err(Error::new(
            "Core genesis ProgramData is not the authority-bearing pre-init observation",
        ));
    }

    let profile_address = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &programs[1],
    )
    .0;
    if plan.infrastructure_profile.address != profile_address.to_string()
        || hex32(&plan.infrastructure_profile.schema_id)?
            != PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_ID_V1
    {
        return Err(Error::new(
            "infrastructure profile address or schema is not canonical",
        ));
    }
    let body = decode_hex(&plan.infrastructure_profile.body_hex)?;
    let profile = ProtocolInfrastructureProfileV1::decode(&body)
        .map_err(|error| Error::new(format!("infrastructure profile: {error:?}")))?;
    let registry_artifact = artifact_id(&plan.registry.artifact_release_id)?;
    let rent_artifact = artifact_id(&plan.rent_credit.artifact_release_id)?;
    if profile.registry().program().to_bytes() != programs[0].to_bytes()
        || profile.registry().artifact_release() != registry_artifact
        || profile.rent().program().to_bytes() != programs[6].to_bytes()
        || profile.rent().artifact_release() != rent_artifact
        || plan.infrastructure_profile.registry_artifact_release_id
            != plan.registry.artifact_release_id
        || plan.infrastructure_profile.rent_artifact_release_id
            != plan.rent_credit.artifact_release_id
    {
        return Err(Error::new(
            "infrastructure profile substituted a Registry or Rent binding",
        ));
    }
    let body_hash = sha2::Sha256::digest(&body);
    if plan.infrastructure_profile.body_sha256 != hex(&body_hash) {
        return Err(Error::new("infrastructure profile body hash mismatch"));
    }
    for label in [
        "execution_release_set",
        "registry_artifact_release",
        "core_artifact_release",
        "claims_artifact_release",
        "trading_artifact_release",
        "resolution_artifact_release",
        "custody_artifact_release",
        "rent_artifact_release",
        "pyth_release",
    ] {
        if !plan.records.contains_key(label) {
            return Err(Error::new(format!("missing finalized record {label}")));
        }
    }
    if plan.genesis_accounts.len() != 23 {
        return Err(Error::new(
            "infrastructure plan must contain fourteen Loader and nine finalized record accounts",
        ));
    }
    Ok(())
}

fn validate_program_pin(
    plan: &SuccessorPlan,
    label: &str,
    pin: &ProgramPin,
    bootstrap_authority: Option<Pubkey>,
) -> Result<()> {
    let program = pubkey(&pin.program_id)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    if pubkey(&pin.programdata_id)? != expected_programdata
        || pin.upgrade_authority.is_some()
        || !PathBuf::from(&pin.elf_path).is_absolute()
    {
        return Err(Error::new(
            "program pin is not an immutable canonical Loader-v3 binding",
        ));
    }
    let expected_elf = hex32(&pin.elf_sha256)?;
    let _ = hex32(&pin.semantic_release_id)?;
    let _ = artifact_id(&pin.artifact_release_id)?;
    let elf = fs::read(&pin.elf_path)?;
    if sha2::Sha256::digest(&elf).as_slice() != expected_elf {
        return Err(Error::new(format!("{label} ELF digest mismatch")));
    }
    let expected_programdata = loader_programdata_bytes(&elf, bootstrap_authority);
    let genesis = plan
        .genesis_accounts
        .get(&format!("loader.{label}.programdata"))
        .ok_or_else(|| Error::new(format!("missing {label} ProgramData genesis pin")))?;
    if genesis.data_sha256 != hex(&sha2::Sha256::digest(&expected_programdata)) {
        return Err(Error::new(format!(
            "{label} ProgramData header/ELF genesis hash mismatch"
        )));
    }
    if label == "core" {
        let post_revoke = loader_programdata_bytes(&elf, None);
        if plan.core_bootstrap.post_revoke_programdata_sha256
            != hex(&sha2::Sha256::digest(&post_revoke))
        {
            return Err(Error::new(
                "Core post-revoke immutable ProgramData hash mismatch",
            ));
        }
    }
    Ok(())
}

fn validate_provider(provider: &ProviderEvidenceInput, expected_rpc: &str) -> Result<()> {
    let observed = validate_loopback_url(&provider.rpc_url)?;
    if observed.as_str() != expected_rpc
        || !provider.provider_state_initialized
        || provider.captured_release_identity_claimed
        || provider.price_update_reclaimed
        || pubkey(&provider.price_update).is_err()
    {
        return Err(Error::new(
            "provider evidence is not the initialized, unreclaimed, non-production localhost fixture",
        ));
    }
    for name in ["pyth-receiver", "pyth-router"] {
        let program = provider
            .programs
            .iter()
            .find(|program| program.name == name)
            .ok_or_else(|| Error::new(format!("provider evidence omits {name}")))?;
        if pubkey(&program.program_id).is_err()
            || pubkey(&program.programdata_id).is_err()
            || program.observed_deployment_slot != 0
            || program.observed_upgrade_authority_effectively_disabled
            || hex32(&program.elf_tail_sha256).is_err()
        {
            return Err(Error::new(format!(
                "provider evidence has invalid local projection for {name}"
            )));
        }
    }
    Ok(())
}

fn artifact_id(value: &str) -> Result<ArtifactReleaseIdV1> {
    ArtifactReleaseIdV1::new(hex32(value)?)
        .map_err(|error| Error::new(format!("artifact release ID: {error:?}")))
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    if value.len() & 1 == 1
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new("invalid lowercase hexadecimal bytes"));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = core::str::from_utf8(pair).map_err(|_| Error::new("non-UTF8 hex"))?;
            u8::from_str_radix(pair, 16).map_err(|_| Error::new("invalid hexadecimal byte"))
        })
        .collect()
}

fn validate_existing_absolute(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute() || !path.is_file() {
        return Err(Error::new(format!(
            "{label} must be an existing absolute regular file"
        )));
    }
    Ok(())
}

fn validate_loopback_url(value: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(|error| Error::new(format!("invalid RPC URL: {error}")))?;
    if url.scheme() != "http"
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(Error::new("RPC URL must be an exact loopback HTTP origin"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| Error::new("RPC URL has no host"))?;
    let address: IpAddr = host
        .parse()
        .map_err(|_| Error::new("RPC host must be a numeric loopback address"))?;
    if !address.is_loopback() || url.port().is_none() {
        return Err(Error::new(
            "RPC URL must use a numeric loopback host and explicit port",
        ));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_origin_refuses_public_redirect_and_path_surfaces() {
        assert!(validate_loopback_url("http://127.0.0.1:20890/").is_ok());
        assert!(validate_loopback_url("https://127.0.0.1:20890/").is_err());
        assert!(validate_loopback_url("http://8.8.8.8:20890/").is_err());
        assert!(validate_loopback_url("http://localhost:20890/").is_err());
        assert!(validate_loopback_url("http://127.0.0.1:20890/rpc").is_err());
    }

    #[test]
    fn profile_body_hex_decoder_refuses_odd_uppercase_and_non_hex() {
        assert_eq!(decode_hex("00ff").expect("hex"), [0, 255]);
        assert!(decode_hex("0").is_err());
        assert!(decode_hex("AA").is_err());
        assert!(decode_hex("gg").is_err());
    }

    #[test]
    fn zero_artifact_identity_refuses() {
        assert!(artifact_id(&"00".repeat(32)).is_err());
    }

    #[test]
    fn program_identity_type_matches_profile_wire() {
        let program = Pubkey::new_unique();
        assert!(dclutch_release_set_contract::ProgramIdentityV1::new(program.to_bytes()).is_ok());
    }
}
