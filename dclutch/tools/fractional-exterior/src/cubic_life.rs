//! One resumable command for the propagated cubic Fractional representative life.

use std::{fs, path::Path, process::Command};

use dclutch_fractional_exterior::bridge::{
    COMPLETED_PHASES_V1, CompactionBridgeV1, ElfPinsV1, FractionalCubicLifeLedgerV1,
    LIFE_SCHEMA_V1, PreterminalBridgeV1, canonical_bytes, digest, read_compaction,
    read_life_ledger, read_preterminal, write_atomic,
};

use crate::{Error, Result, claim_check, journal, validator};

const PRETERMINAL_DIR: &str = "01-preterminal";
const PRETERMINAL_BRIDGE: &str = "preterminal-bridge.json";
const COMPACTION_BRIDGE: &str = "02-compaction-bridge.json";
const POSTCOMPACTION_DIR: &str = "03-postcompaction";
const LEDGER: &str = "fractional-cubic-life-v1.json";

struct EnvironmentV1 {
    commit: String,
    tree_sha256: String,
    elves: ElfPinsV1,
}

/// Run or resume the single propagated cubic life.
pub fn run(source: &Path, elf_dir: &Path, out: &Path) -> Result<()> {
    let environment = environment(source, elf_dir)?;
    fs::create_dir_all(out)?;
    let ledger_path = out.join(LEDGER);
    if ledger_path.exists() {
        let digest = verify(source, elf_dir, out)?;
        println!("fractional cubic life already exact: sha256 {digest}");
        return Ok(());
    }

    let pre_dir = out.join(PRETERMINAL_DIR);
    let pre_path = out.join(PRETERMINAL_BRIDGE);
    let preterminal = if pre_path.exists() {
        let value = read_preterminal(&pre_path).map_err(Error::new)?;
        validate_environment(&value, &environment)?;
        let (_, observed) = journal::verify(&pre_dir)?;
        if observed != value.journal_sha256 {
            return Err(Error::new("preterminal bridge/journal substitution").into());
        }
        println!("resume: preterminal holder life is exact");
        value
    } else {
        validator::run(elf_dir, &pre_dir, false)?;
        validator::write_preterminal_bridge(
            elf_dir,
            &pre_dir,
            &pre_path,
            environment.commit.clone(),
            environment.tree_sha256.clone(),
        )?;
        read_preterminal(&pre_path).map_err(Error::new)?
    };

    let compaction_path = out.join(COMPACTION_BRIDGE);
    let compaction = if compaction_path.exists() {
        let value = read_compaction(&compaction_path).map_err(Error::new)?;
        if value.preterminal != preterminal {
            return Err(Error::new("compaction bridge substituted its preterminal input").into());
        }
        println!("resume: real-ELF permissionless compaction is exact");
        value
    } else {
        run_compaction(source, elf_dir, &pre_path, &compaction_path)?;
        let value = read_compaction(&compaction_path).map_err(Error::new)?;
        if value.preterminal != preterminal {
            return Err(Error::new("real-ELF compaction emitted another campaign identity").into());
        }
        value
    };

    let post_dir = out.join(POSTCOMPACTION_DIR);
    if post_dir.join("claim-check-canonical.json").exists() {
        claim_check::verify_propagated(&post_dir, &compaction_path)?;
        println!("resume: post-compaction claims life is exact");
    } else {
        claim_check::run_propagated(elf_dir, &post_dir, false, &compaction_path)?;
    }
    let (_, post_digest) = claim_check::verify_propagated(&post_dir, &compaction_path)?;
    let ledger = ledger(&preterminal, &compaction, &post_digest)?;
    let ledger_digest = write_atomic(&ledger_path, &ledger).map_err(Error::new)?;
    println!("fractional cubic life complete: sha256 {ledger_digest}");
    Ok(())
}

/// Verify every phase and every bridge against current source and ELFs.
pub fn verify(source: &Path, elf_dir: &Path, out: &Path) -> Result<String> {
    let environment = environment(source, elf_dir)?;
    let pre_path = out.join(PRETERMINAL_BRIDGE);
    let preterminal = read_preterminal(&pre_path).map_err(Error::new)?;
    validate_environment(&preterminal, &environment)?;
    let (_, pre_journal) = journal::verify(&out.join(PRETERMINAL_DIR))?;
    if pre_journal != preterminal.journal_sha256 {
        return Err(Error::new("preterminal journal digest substitution").into());
    }
    let compaction_path = out.join(COMPACTION_BRIDGE);
    let compaction = read_compaction(&compaction_path).map_err(Error::new)?;
    if compaction.preterminal != preterminal {
        return Err(Error::new("compaction/preterminal bridge mismatch").into());
    }
    let (_, post_digest) =
        claim_check::verify_propagated(&out.join(POSTCOMPACTION_DIR), &compaction_path)?;
    let expected = ledger(&preterminal, &compaction, &post_digest)?;
    let ledger_path = out.join(LEDGER);
    let bytes = fs::read(&ledger_path)?;
    let observed = read_life_ledger(&ledger_path).map_err(Error::new)?;
    if observed != expected || canonical_bytes(&observed)? != bytes {
        return Err(Error::new("fractional cubic life ledger is not exact").into());
    }
    Ok(digest(&bytes))
}

fn run_compaction(source: &Path, elf_dir: &Path, preterminal: &Path, output: &Path) -> Result<()> {
    let manifest =
        source.join("programs/dclutch-claims-sbf/program-test/fractional-atomic/Cargo.toml");
    let status = Command::new("cargo")
        .current_dir(source)
        .arg("test")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--test")
        .arg("fractional_compaction")
        .arg("a_degree_three_curve_compacts_and_redeems_without_stranding")
        .arg("--")
        .arg("--exact")
        .arg("--nocapture")
        .env("SBF_OUT_DIR", elf_dir)
        .env("DCLUTCH_CUBIC_PRETERMINAL_BRIDGE", preterminal)
        .env("DCLUTCH_CUBIC_COMPACTION_BRIDGE_OUT", output)
        .status()?;
    if !status.success() {
        return Err(Error::new(format!("real-ELF cubic compaction exited with {status}")).into());
    }
    if !output.is_file() {
        return Err(Error::new("real-ELF compaction emitted no bridge").into());
    }
    Ok(())
}

fn ledger(
    pre: &PreterminalBridgeV1,
    compaction: &CompactionBridgeV1,
    post_digest: &str,
) -> Result<FractionalCubicLifeLedgerV1> {
    Ok(FractionalCubicLifeLedgerV1 {
        schema: LIFE_SCHEMA_V1.into(),
        source_commit: pre.source_commit.clone(),
        source_tree_sha256: pre.source_tree_sha256.clone(),
        elves: pre.elves.clone(),
        preterminal_bridge_sha256: digest(&canonical_bytes(pre)?),
        preterminal_journal_sha256: pre.journal_sha256.clone(),
        compaction_bridge_sha256: digest(&canonical_bytes(compaction)?),
        postcompaction_journal_sha256: post_digest.into(),
        release_set: pre.release_set,
        realm: pre.realm,
        market: pre.market,
        aggregate: pre.aggregate,
        product: pre.product,
        product_basis: pre.product_basis,
        terms: pre.terms,
        root: pre.root,
        shard_mint: pre.shard_mint,
        holder: pre.holder,
        holder_shard_token: pre.holder_shard_token,
        outstanding_shards: pre.outstanding_shards,
        reserve_native_claims: pre.reserve_native_claims,
        payout_per_claim: compaction.payout_per_claim,
        escrowed_collateral_atoms: compaction.escrowed_collateral_atoms,
        completed_phases: COMPLETED_PHASES_V1.map(Into::into),
    })
}

fn environment(source: &Path, elf_dir: &Path) -> Result<EnvironmentV1> {
    if !source.is_absolute() || !elf_dir.is_absolute() {
        return Err(Error::new("source root and ELF directory must be absolute").into());
    }
    let commit = git(source, &["rev-parse", "HEAD"])?;
    if commit.len() != 40 {
        return Err(Error::new("source root has no full Git commit identity").into());
    }
    let dirty = git(source, &["status", "--porcelain", "--untracked-files=all"])?;
    if !dirty.is_empty() {
        return Err(Error::new(
            "source root is dirty; cold evidence requires exact committed source",
        )
        .into());
    }
    let tree = git_bytes(source, &["ls-tree", "-r", "--full-tree", "HEAD"])?;
    let read =
        |name: &str| -> Result<String> { Ok(journal::digest(&crate::read_elf(elf_dir, name)?)) };
    Ok(EnvironmentV1 {
        commit,
        tree_sha256: digest(&tree),
        elves: ElfPinsV1 {
            claims: read("dclutch_claims_sbf.so")?,
            registry: read("dclutch_registry_sbf.so")?,
            core: read("dclutch_core_sbf.so")?,
            custody: read("dclutch_custody_sbf.so")?,
            rent: read("dclutch_rent_sbf.so")?,
            trading: read("dclutch_fractional_compaction_test_caller_sbf.so")?,
            token_2022: read("spl_token_2022.so")?,
        },
    })
}

fn validate_environment(pre: &PreterminalBridgeV1, environment: &EnvironmentV1) -> Result<()> {
    if pre.source_commit != environment.commit {
        return Err(Error::new("source commit substitution").into());
    }
    if pre.source_tree_sha256 != environment.tree_sha256 {
        return Err(Error::new("source tree substitution").into());
    }
    if pre.elves != environment.elves {
        return Err(Error::new("ELF digest substitution").into());
    }
    Ok(())
}

fn git(source: &Path, arguments: &[&str]) -> Result<String> {
    let bytes = git_bytes(source, arguments)?;
    String::from_utf8(bytes)
        .map(|value| value.trim().to_owned())
        .map_err(Into::into)
}

fn git_bytes(source: &Path, arguments: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(source)
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(Error::new(format!("git {:?} refused", arguments)).into());
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pins(byte: u8) -> ElfPinsV1 {
        let value = format!("{byte:02x}").repeat(32);
        ElfPinsV1 {
            claims: value.clone(),
            registry: value.clone(),
            core: value.clone(),
            custody: value.clone(),
            rent: value.clone(),
            trading: value.clone(),
            token_2022: value,
        }
    }

    fn pre() -> PreterminalBridgeV1 {
        PreterminalBridgeV1 {
            schema: dclutch_fractional_exterior::bridge::PRETERMINAL_SCHEMA_V1.into(),
            source_commit: "ab".repeat(20),
            source_tree_sha256: "cd".repeat(32),
            elves: pins(1),
            journal_sha256: "ef".repeat(32),
            release_set: [1; 32],
            realm: [2; 32],
            market: [3; 32],
            aggregate: [4; 32],
            product: [5; 32],
            product_basis: [6; 32],
            terms: [7; 32],
            root: [8; 32],
            shard_mint: [9; 32],
            holder: [10; 32],
            holder_shard_token: [11; 32],
            denominator: 10,
            representation_coordinate: 1,
            outstanding_shards: 40,
            reserve_native_claims: 4,
            curve_degree: 3,
            payout_scale: 11,
            rounding_boundary: dclutch_fractional_exterior::bridge::ROUNDING_BOUNDARY_V1.into(),
        }
    }

    #[test]
    fn source_and_elf_substitution_refuse_before_resume() {
        let expected = EnvironmentV1 {
            commit: "ab".repeat(20),
            tree_sha256: "cd".repeat(32),
            elves: pins(1),
        };
        validate_environment(&pre(), &expected).expect("control");
        let mut wrong_source = pre();
        wrong_source.source_commit = "ac".repeat(20);
        assert_eq!(
            validate_environment(&wrong_source, &expected)
                .expect_err("source substitution")
                .to_string(),
            "source commit substitution"
        );
        let mut wrong_elf = pre();
        wrong_elf.elves = pins(2);
        assert_eq!(
            validate_environment(&wrong_elf, &expected)
                .expect_err("ELF substitution")
                .to_string(),
            "ELF digest substitution"
        );
    }
}
