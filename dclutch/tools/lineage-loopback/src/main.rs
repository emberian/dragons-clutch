//! Stage a loopback genesis holding two activation caches and a Registry.
//!
//! The successor-declaration caller reads two activation caches and creates one
//! account. Getting a loopback chain into that state through the ordinary path
//! means two checked releases, two deploys and two five-transaction activation
//! ladders -- whole-ELF hashing is one compute unit per two bytes, so activation
//! is one role per transaction -- which is upwards of an hour and has no runner.
//! None of it would exercise anything the declaration route reads.
//!
//! So the caches are composed offline and injected at genesis through
//! `solana-test-validator --account-dir`, the way
//! `tools/fractional-exterior` stages its own. That is legal rather than a
//! shortcut past a check: `DeclareSuccessor` asks that a cache be
//! Registry-owned, of the one exact width, decodable, and sitting at the
//! address its own release-set id derives. It asks nothing about who wrote it,
//! because provenance is not what the route authenticates.
//!
//! **What this therefore is and is not evidence of.** It exercises the caller's
//! frame, its refusals, its simulation and its read-back against a real
//! validator and a real compiled Registry. It is NOT evidence that a real
//! activation ladder produced these caches, and no cluster has a genesis anyone
//! can write into -- `plan.rs` says so about its own `Genesis` mode. Cut-day
//! evidence still comes from devnet's real caches.
//!
//! Both caches are composed by `dclutch-registry`'s own builders, so a
//! cache staged here is byte-identical to one the Registry would have written
//! for the same release set.

use std::{env, fs, path::PathBuf, process::ExitCode};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_core_contract::ContentId;
use dclutch_registry::activation_auth_v1::activation_cache_address_v1;
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1, put_activation_cache_bump_v1,
};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, EXECUTION_ROLE_ORDER_V1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ProgramIdentityV1,
};
use serde_json::json;
use solana_program::{hash::hash, rent::Rent};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::bpf_loader_upgradeable;

/// Deterministic seeds. A loopback fixture wants to be reproducible, and none
/// of these keys is ever funded or trusted anywhere but a throwaway ledger.
const PAYER_SEED: [u8; 32] = [0x2f; 32];
const AUTHORITY_SEED: [u8; 32] = [0x44; 32];
const REGISTRY_SEED: [u8; 32] = [0x9e; 32];

/// One role's deployment inside a staged release set.
#[derive(Clone, Copy)]
struct RoleSpec {
    program: u8,
    elf: u8,
    slot: u64,
}

/// Predecessor and successor, with every role moved.
///
/// All five moved because that is six of devnet's seven hops, not because the
/// seventh is out of reach. It was: the DEPLOYED Registry refuses any hop with
/// an unmoved role, because conjunct 1 refused an executable consent slot while
/// conjunct 6 required that slot to be the System Program, which is executable.
/// Conjunct 1 now exempts exactly that account, so the four-moved shape frames
/// and lands -- `programs/dclutch-registry-sbf/tests/lineage_program_test.rs`
/// declares it end to end. Reaching it on devnet needs the Registry redeploy;
/// until then this ladder measures the shape the deployed program admits.
const PREDECESSOR: [RoleSpec; 5] = [
    RoleSpec {
        program: 0x11,
        elf: 0x71,
        slot: 490_697_000,
    },
    RoleSpec {
        program: 0x12,
        elf: 0x72,
        slot: 490_697_100,
    },
    RoleSpec {
        program: 0x13,
        elf: 0x73,
        slot: 490_697_200,
    },
    RoleSpec {
        program: 0x14,
        elf: 0x74,
        slot: 490_693_331,
    },
    RoleSpec {
        program: 0x15,
        elf: 0x75,
        slot: 490_697_400,
    },
];
const SUCCESSOR: [RoleSpec; 5] = [
    RoleSpec {
        program: 0x11,
        elf: 0x81,
        slot: 490_849_793,
    },
    RoleSpec {
        program: 0x12,
        elf: 0x82,
        slot: 490_826_560,
    },
    RoleSpec {
        program: 0x13,
        elf: 0x83,
        slot: 490_830_840,
    },
    RoleSpec {
        program: 0x14,
        elf: 0x84,
        slot: 490_845_000,
    },
    RoleSpec {
        program: 0x15,
        elf: 0x85,
        slot: 490_814_947,
    },
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lineage-loopback: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut work: Option<PathBuf> = None;
    let mut registry_elf: Option<PathBuf> = None;
    let mut cursor = env::args().skip(1);
    while let Some(flag) = cursor.next() {
        let mut value = || cursor.next().ok_or(format!("{flag} requires a value"));
        match flag.as_str() {
            "--work" => work = Some(PathBuf::from(value()?)),
            "--registry-elf" => registry_elf = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let work = work.ok_or("--work ABSOLUTE_DIR is required")?;
    let registry_elf = registry_elf.ok_or("--registry-elf ABSOLUTE_SO is required")?;
    if !work.is_absolute() || !registry_elf.is_absolute() {
        return Err("both paths must be absolute".into());
    }

    let elf = fs::read(&registry_elf).map_err(|error| format!("read Registry ELF: {error}"))?;
    if elf.get(..4) != Some(&[0x7f, b'E', b'L', b'F'][..]) {
        return Err(format!("{} is not an ELF", registry_elf.display()));
    }

    let accounts = work.join("accounts");
    let keys = work.join("keys");
    fs::create_dir_all(&accounts).map_err(|error| format!("create accounts dir: {error}"))?;
    fs::create_dir_all(&keys).map_err(|error| format!("create keys dir: {error}"))?;

    let payer = Keypair::new_from_array(PAYER_SEED);
    let authority = Keypair::new_from_array(AUTHORITY_SEED);
    let registry = Keypair::new_from_array(REGISTRY_SEED).pubkey();
    write_keypair(&keys.join("payer.json"), &payer)?;
    write_keypair(&keys.join("authority.json"), &authority)?;

    // The Registry, as a Loader V3 pair. Immutable (authority tag 0), because
    // nothing in this fixture upgrades it and an immutable program is the
    // simpler thing to be true about.
    let programdata =
        Pubkey::find_program_address(&[registry.as_ref()], &bpf_loader_upgradeable::ID).0;
    write_account(
        &accounts,
        registry,
        bpf_loader_upgradeable::ID,
        &program_account_bytes(programdata),
        true,
    )?;
    write_account(
        &accounts,
        programdata,
        bpf_loader_upgradeable::ID,
        &programdata_bytes(&elf),
        false,
    )?;

    let mut ids = Vec::with_capacity(2);
    for specs in [PREDECESSOR, SUCCESSOR] {
        let (id, address, bytes) = build_cache(registry, authority.pubkey(), specs)?;
        write_account(&accounts, address, registry, &bytes, false)?;
        ids.push(id);
    }
    let predecessor = ids.first().ok_or("predecessor")?;
    let successor = ids.get(1).ok_or("successor")?;

    // Read by the runner script. One line per fact, so a shell `read` is enough
    // and nothing has to parse JSON to start a validator.
    let manifest = json!({
        "schema": "dclutch-lineage-loopback-genesis-v1",
        "registry": registry.to_string(),
        "registry_programdata": programdata.to_string(),
        "predecessor": hex(predecessor.as_bytes()),
        "successor": hex(successor.as_bytes()),
        "payer": payer.pubkey().to_string(),
        "payer_keypair": keys.join("payer.json").display().to_string(),
        "authority": authority.pubkey().to_string(),
        "authority_keypair": keys.join("authority.json").display().to_string(),
        "accounts": accounts.display().to_string(),
    });
    let path = work.join("genesis.json");
    fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| format!("write {}: {error}", path.display()))?;
    println!("REGISTRY={registry}");
    println!("PREDECESSOR={}", hex(predecessor.as_bytes()));
    println!("SUCCESSOR={}", hex(successor.as_bytes()));
    println!("PAYER={}", payer.pubkey());
    println!("PAYER_KEYPAIR={}", keys.join("payer.json").display());
    println!("AUTHORITY={}", authority.pubkey());
    println!(
        "AUTHORITY_KEYPAIR={}",
        keys.join("authority.json").display()
    );
    println!("ACCOUNTS={}", accounts.display());
    Ok(())
}

/// Compose one complete activation cache through the contract's own builders.
fn build_cache(
    registry: Pubkey,
    authority: Pubkey,
    specs: [RoleSpec; 5],
) -> Result<(ContentId, Pubkey, Vec<u8>), String> {
    let releases: Vec<ArtifactReleaseV1> = specs
        .iter()
        .map(|spec| release_for(*spec, authority))
        .collect::<Result<_, _>>()?;
    let bindings: Vec<ExecutionRoleBindingV1> = releases
        .iter()
        .map(|release| ExecutionRoleBindingV1::new(release.program(), artifact_id(*release)))
        .collect();
    let [core, claims, trading, resolution, custody]: [ExecutionRoleBindingV1; 5] =
        bindings.try_into().map_err(|_| "five bindings")?;
    let set = ExecutionReleaseSetV1::new(core, claims, trading, resolution, custody)
        .map_err(|error| format!("release set: {error:?}"))?;
    let id = ContentId::new(hash(&set.to_bytes()).to_bytes())
        .map_err(|error| format!("release set id: {error:?}"))?;

    let mut bytes = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, id)
        .map_err(|error| format!("initialize cache: {error:?}"))?;
    for role in EXECUTION_ROLE_ORDER_V1 {
        let release = *releases.get(role.role_index()).ok_or("role release")?;
        activate_execution_role_into_v1(&mut bytes, id, &set, role, &activation_input(release)?)
            .map_err(|error| format!("activate {role:?}: {error:?}"))?;
    }
    let (address, bump) = Pubkey::find_program_address(
        &[
            dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
            id.as_bytes(),
        ],
        &registry,
    );
    // The real Registry records this at activation and every reader reproduces
    // the address from it rather than searching. A cache left at zero is an
    // account no deployment produces.
    put_activation_cache_bump_v1(&mut bytes, bump)
        .map_err(|error| format!("cache bump: {error:?}"))?;
    if activation_cache_address_v1(&registry, id.as_bytes()) != address {
        return Err("staged cache is not at its own derived address".into());
    }
    Ok((id, address, bytes))
}

fn release_for(spec: RoleSpec, authority: Pubkey) -> Result<ArtifactReleaseV1, String> {
    let program = Pubkey::new_from_array([spec.program; 32]);
    let programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    ArtifactReleaseV1::new(
        ProgramIdentityV1::new(program.to_bytes()).map_err(|error| format!("{error:?}"))?,
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes())
            .map_err(|error| format!("{error:?}"))?,
        programdata.to_bytes(),
        ContentId::new([spec.program | 0x01; 32]).map_err(|error| format!("{error:?}"))?,
        [spec.elf; 32],
        spec.slot,
        // Every moved role must bind an authority to ask consent of, so every
        // role here is upgradeable and every one binds the same key.
        ArtifactUpgradePolicyV1::ExactAuthority,
        Some(authority.to_bytes()),
    )
    .map_err(|error| format!("artifact release: {error:?}"))
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact release id")
}

fn activation_input(release: ArtifactReleaseV1) -> Result<ArtifactActivationInputV1, String> {
    Ok(ArtifactActivationInputV1::new(
        artifact_id(release),
        release,
        DeploymentObservationV1::new(
            release.program().to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            release.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            release.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            release.deployment_slot(),
            release.elf_digest(),
            release.upgrade_authority(),
        )
        .map_err(|error| format!("deployment observation: {error:?}"))?,
    ))
}

/// Loader V3 `Program` account: variant 2 then the ProgramData address.
fn program_account_bytes(programdata: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0_u8; 36];
    bytes[0..4].copy_from_slice(&2_u32.to_le_bytes());
    bytes[4..36].copy_from_slice(&programdata.to_bytes());
    bytes
}

/// Loader V3 `ProgramData`: variant 3, slot 0, authority `None`, then the ELF.
///
/// Written directly rather than through `--upgradeable-program ... none`,
/// because Solana 4.0.2 encodes that spelling as option tag 1 plus the zero
/// Pubkey rather than immutable option tag 0.
fn programdata_bytes(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0_u8; 45 + elf.len()];
    bytes[0..4].copy_from_slice(&3_u32.to_le_bytes());
    bytes[45..].copy_from_slice(elf);
    bytes
}

fn write_account(
    directory: &std::path::Path,
    key: Pubkey,
    owner: Pubkey,
    data: &[u8],
    executable: bool,
) -> Result<(), String> {
    let value = json!({
        "pubkey": key.to_string(),
        "account": {
            "lamports": Rent::default().minimum_balance(data.len()).max(1),
            "data": [BASE64.encode(data), "base64"],
            "owner": owner.to_string(),
            "executable": executable,
            "rentEpoch": 0,
            "space": data.len(),
        }
    });
    let path = directory.join(format!("{key}.json"));
    fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| format!("write {}: {error}", path.display()))
}

/// A Solana CLI keypair file: the 64-byte secret-then-public array.
fn write_keypair(path: &std::path::Path, keypair: &Keypair) -> Result<(), String> {
    let bytes: Vec<u8> = keypair.to_bytes().to_vec();
    fs::write(
        path,
        format!(
            "{}\n",
            serde_json::to_string(&bytes).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| format!("write {}: {error}", path.display()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
