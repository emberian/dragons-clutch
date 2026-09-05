#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Canonical, hostile-decoded evidence bridge for one cubic Fractional life.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Preterminal bridge schema.
pub const PRETERMINAL_SCHEMA_V1: &str = "dclutch/curved-fractional-life/preterminal/v1";
/// Compiler-to-Found bridge schema.
pub const FOUNDING_SCHEMA_V1: &str = "dclutch/curved-fractional-life/founding/v1";
/// Compaction bridge schema.
pub const COMPACTION_SCHEMA_V1: &str = "dclutch/curved-fractional-life/compaction/v1";
/// Complete propagated life ledger schema.
pub const LIFE_SCHEMA_V1: &str = "dclutch/fractional-cubic-life/v1";
/// The single allowed integer rounding boundary.
pub const ROUNDING_BOUNDARY_V1: &str =
    "whole_claims=floor(shard_atoms/denominator); collateral_atoms=whole_claims*payout_per_claim";
/// Exact ordered phase names required by the complete-life ledger.
pub const COMPLETED_PHASES_V1: [&str; 3] = [
    "wrap-transfer-whole-unwrap",
    "terminal-permissionless-compaction",
    "hostile-partial-settling-close",
];

/// Exact successor role ELF digests authenticated by the founding campaign.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FoundingElfPinsV1 {
    /// Registry ELF SHA-256.
    pub registry: String,
    /// Core ELF SHA-256.
    pub core: String,
    /// Claims ELF SHA-256.
    pub claims: String,
    /// Trading ELF SHA-256.
    pub trading: String,
    /// Resolution ELF SHA-256.
    pub resolution: String,
    /// Custody ELF SHA-256.
    pub custody: String,
    /// Rent-credit ELF SHA-256.
    pub rent: String,
}

impl FoundingElfPinsV1 {
    fn validate(&self) -> Result<(), String> {
        for (name, digest) in [
            ("registry", &self.registry),
            ("core", &self.core),
            ("claims", &self.claims),
            ("trading", &self.trading),
            ("resolution", &self.resolution),
            ("custody", &self.custody),
            ("rent", &self.rent),
        ] {
            validate_digest(name, digest)?;
        }
        Ok(())
    }
}

/// One exact poststate row bound by the finalized DCLTGMF3 journal.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FoundingAccountPinV1 {
    /// Account address.
    pub address: [u8; 32],
    /// Owning program.
    pub owner: [u8; 32],
    /// SHA-256 of the exact account data.
    pub data_sha256: String,
    /// SHA-256 of owner, lamports, executable, rent epoch, length, and data.
    pub account_sha256: String,
}

impl FoundingAccountPinV1 {
    fn validate(&self, name: &str) -> Result<(), String> {
        if self.address == [0; 32] || self.owner == [0; 32] {
            return Err(format!("founding {name} account identity is zero"));
        }
        validate_digest(&format!("{name}.data_sha256"), &self.data_sha256)?;
        validate_digest(&format!("{name}.account_sha256"), &self.account_sha256)
    }
}

/// Exact compiler artifacts and finalized Generic Found poststate which begin
/// one physical cubic Fractional life.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FoundingBridgeV1 {
    /// Schema discriminator.
    pub schema: String,
    /// Exact Git commit used by compiler and Found.
    pub source_commit: String,
    /// SHA-256 over the committed recursive `git ls-tree` manifest.
    pub source_tree_sha256: String,
    /// SHA-256 of the exact compiler input JSON.
    pub compiler_input_sha256: String,
    /// SHA-256 of the canonical compiler report JSON.
    pub compiler_report_sha256: String,
    /// SHA-256 of the exact successor plan JSON.
    pub successor_plan_sha256: String,
    /// SHA-256 of the exact successor Market input JSON.
    pub market_input_sha256: String,
    /// SHA-256 of the complete successor campaign report JSON.
    pub successor_report_sha256: String,
    /// SHA-256 of the canonical embedded founding checkpoint.
    pub founding_checkpoint_sha256: String,
    /// SHA-256 of the canonical finalized DCLTGMF3 journal row.
    pub dcltgmf3_journal_sha256: String,
    /// DCLTGMF3 immutable-intent SHA-256.
    pub dcltgmf3_intent_sha256: String,
    /// DCLTGMF3 finalized state SHA-256.
    pub dcltgmf3_state_sha256: String,
    /// DCLTGMF3 signed transaction SHA-256.
    pub dcltgmf3_transaction_sha256: String,
    /// Exact live ELF identities observed by successor Found.
    pub elves: FoundingElfPinsV1,
    /// Activated execution release set.
    pub release_set: [u8; 32],
    /// Canonical Realm content identity.
    pub realm: [u8; 32],
    /// Found Open Market address.
    pub market: [u8; 32],
    /// Claims aggregate address created by Found.
    pub aggregate: [u8; 32],
    /// Founder Position address created by Found.
    pub founder_position: [u8; 32],
    /// Product content identity.
    pub product: [u8; 32],
    /// Result-domain content identity.
    pub result_domain: [u8; 32],
    /// Portfolio content identity.
    pub portfolio: [u8; 32],
    /// ProductBasisV3 content identity.
    pub product_basis: [u8; 32],
    /// DCLTPGT1 price-gate content identity.
    pub price_gate: [u8; 32],
    /// Exact Product record poststate.
    pub product_account: FoundingAccountPinV1,
    /// Exact result-domain record poststate.
    pub result_domain_account: FoundingAccountPinV1,
    /// Exact portfolio record poststate.
    pub portfolio_account: FoundingAccountPinV1,
    /// Exact ProductBasisV3 record poststate.
    pub product_basis_account: FoundingAccountPinV1,
    /// Exact price-gate record poststate.
    pub price_gate_account: FoundingAccountPinV1,
    /// Exact Open Market poststate.
    pub market_account: FoundingAccountPinV1,
    /// Exact Claims aggregate poststate.
    pub aggregate_account: FoundingAccountPinV1,
    /// Exact founder Position poststate.
    pub founder_position_account: FoundingAccountPinV1,
    /// Product outcome and native-Claims width.
    pub product_width: u32,
    /// ProductBasis spline degree.
    pub curve_degree: u8,
    /// Exact ProductBasis payout scale.
    pub payout_scale: u64,
    /// Total collateral Mint supply before Found.
    pub initial_collateral_atoms: u64,
    /// Complete-set quantity Found admits at every coordinate.
    pub complete_set_quantity: u64,
    /// Exact collateral principal conserved by Found.
    pub collateral_principal_atoms: u64,
}

impl FoundingBridgeV1 {
    /// Refuse a compiler/Found bridge with any substituted identity, digest,
    /// account coordinate, or second rounding boundary.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FOUNDING_SCHEMA_V1 {
            return Err("founding bridge schema mismatch".into());
        }
        validate_commit(&self.source_commit)?;
        for (name, value) in [
            ("source_tree_sha256", &self.source_tree_sha256),
            ("compiler_input_sha256", &self.compiler_input_sha256),
            ("compiler_report_sha256", &self.compiler_report_sha256),
            ("successor_plan_sha256", &self.successor_plan_sha256),
            ("market_input_sha256", &self.market_input_sha256),
            ("successor_report_sha256", &self.successor_report_sha256),
            (
                "founding_checkpoint_sha256",
                &self.founding_checkpoint_sha256,
            ),
            ("dcltgmf3_journal_sha256", &self.dcltgmf3_journal_sha256),
            ("dcltgmf3_intent_sha256", &self.dcltgmf3_intent_sha256),
            ("dcltgmf3_state_sha256", &self.dcltgmf3_state_sha256),
            (
                "dcltgmf3_transaction_sha256",
                &self.dcltgmf3_transaction_sha256,
            ),
        ] {
            validate_digest(name, value)?;
        }
        self.elves.validate()?;
        for (name, value) in [
            ("release_set", self.release_set),
            ("realm", self.realm),
            ("market", self.market),
            ("aggregate", self.aggregate),
            ("founder_position", self.founder_position),
            ("product", self.product),
            ("result_domain", self.result_domain),
            ("portfolio", self.portfolio),
            ("product_basis", self.product_basis),
            ("price_gate", self.price_gate),
        ] {
            if value == [0; 32] {
                return Err(format!("founding {name} must be nonzero"));
            }
        }
        for (name, pin) in [
            ("product", &self.product_account),
            ("result_domain", &self.result_domain_account),
            ("portfolio", &self.portfolio_account),
            ("product_basis", &self.product_basis_account),
            ("price_gate", &self.price_gate_account),
            ("market", &self.market_account),
            ("aggregate", &self.aggregate_account),
            ("founder_position", &self.founder_position_account),
        ] {
            pin.validate(name)?;
        }
        for (name, pin, address) in [
            ("market", &self.market_account, self.market),
            ("aggregate", &self.aggregate_account, self.aggregate),
            (
                "founder_position",
                &self.founder_position_account,
                self.founder_position,
            ),
        ] {
            if pin.address != address {
                return Err(format!("founding {name} account address mismatch"));
            }
        }
        if self.product_width != 4
            || self.curve_degree != 3
            || self.payout_scale != 11
            || self.initial_collateral_atoms != 198
            || self.complete_set_quantity != 9
            || self.collateral_principal_atoms != 99
            || self.complete_set_quantity.checked_mul(self.payout_scale)
                != Some(self.collateral_principal_atoms)
            || self.initial_collateral_atoms.checked_div(2) != Some(self.collateral_principal_atoms)
        {
            return Err("founding cubic scale or exact-reserve contract mismatch".into());
        }
        Ok(())
    }
}

/// Exact executable artifact digests shared by all phases.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ElfPinsV1 {
    /// Claims ELF SHA-256.
    pub claims: String,
    /// Registry ELF SHA-256.
    pub registry: String,
    /// Core ELF SHA-256.
    pub core: String,
    /// Custody ELF SHA-256.
    pub custody: String,
    /// Rent ELF SHA-256.
    pub rent: String,
    /// Unified Trading test-caller ELF SHA-256.
    pub trading: String,
    /// Token-2022 ELF SHA-256.
    pub token_2022: String,
}

impl ElfPinsV1 {
    fn validate(&self) -> Result<(), String> {
        for (name, digest) in [
            ("claims", &self.claims),
            ("registry", &self.registry),
            ("core", &self.core),
            ("custody", &self.custody),
            ("rent", &self.rent),
            ("trading", &self.trading),
            ("token_2022", &self.token_2022),
        ] {
            validate_digest(name, digest)?;
        }
        Ok(())
    }
}

/// Immutable identities and exact state emitted by the preterminal validator.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreterminalBridgeV1 {
    /// Schema discriminator.
    pub schema: String,
    /// Exact Git commit from which the command ran.
    pub source_commit: String,
    /// SHA-256 over the committed recursive `git ls-tree` manifest.
    pub source_tree_sha256: String,
    /// Executable identities.
    pub elves: ElfPinsV1,
    /// SHA-256 of the accepted preterminal canonical journal.
    pub journal_sha256: String,
    /// Activated execution release set.
    pub release_set: [u8; 32],
    /// Canonical Realm record digest.
    pub realm: [u8; 32],
    /// Core Market address.
    pub market: [u8; 32],
    /// Claims aggregate address.
    pub aggregate: [u8; 32],
    /// Product record digest.
    pub product: [u8; 32],
    /// ProductBasisV3 record digest.
    pub product_basis: [u8; 32],
    /// Fractional terms digest.
    pub terms: [u8; 32],
    /// Unified Trading-owned Fractional root.
    pub root: [u8; 32],
    /// Represented shard Mint.
    pub shard_mint: [u8; 32],
    /// Sleeping holder which did not participate after receiving shards.
    pub holder: [u8; 32],
    /// Sleeping holder's Token-2022 shard account.
    pub holder_shard_token: [u8; 32],
    /// Exact denominator.
    pub denominator: u64,
    /// Represented product coordinate.
    pub representation_coordinate: u32,
    /// Live shard supply and holder balance after the actor unwraps.
    pub outstanding_shards: u64,
    /// Native claims locked in the capability reserve.
    pub reserve_native_claims: u64,
    /// ProductBasisV3 spline degree.
    pub curve_degree: u8,
    /// Exact spline payout scale.
    pub payout_scale: u64,
    /// Named rounding boundary.
    pub rounding_boundary: String,
}

impl PreterminalBridgeV1 {
    /// Refuse malformed, substituted, or non-canonical campaign facts.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PRETERMINAL_SCHEMA_V1 {
            return Err("preterminal bridge schema mismatch".into());
        }
        validate_commit(&self.source_commit)?;
        validate_digest("source_tree_sha256", &self.source_tree_sha256)?;
        validate_digest("journal_sha256", &self.journal_sha256)?;
        self.elves.validate()?;
        for (name, value) in [
            ("release_set", self.release_set),
            ("realm", self.realm),
            ("market", self.market),
            ("aggregate", self.aggregate),
            ("product", self.product),
            ("product_basis", self.product_basis),
            ("terms", self.terms),
            ("root", self.root),
            ("shard_mint", self.shard_mint),
            ("holder", self.holder),
            ("holder_shard_token", self.holder_shard_token),
        ] {
            if value == [0; 32] {
                return Err(format!("{name} must be nonzero"));
            }
        }
        if self.denominator != 10
            || self.representation_coordinate != 1
            || self.outstanding_shards != 40
            || self.reserve_native_claims != 4
            || self.curve_degree != 3
            || self.payout_scale != 11
            || self.rounding_boundary != ROUNDING_BOUNDARY_V1
            || self.reserve_native_claims.checked_mul(self.denominator)
                != Some(self.outstanding_shards)
        {
            return Err("preterminal amount, curve, or rounding contract mismatch".into());
        }
        Ok(())
    }
}

/// One exact account image passed from ProgramTest to a private validator.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountImageV1 {
    /// Account address.
    pub address: [u8; 32],
    /// Owning program.
    pub owner: [u8; 32],
    /// Lamports, including any live rent or donation.
    pub lamports: u64,
    /// Rent epoch.
    pub rent_epoch: u64,
    /// Executable bit.
    pub executable: bool,
    /// Exact account data, standard base64.
    pub data_base64: String,
}

impl AccountImageV1 {
    fn validate(&self, name: &str) -> Result<(), String> {
        if self.address == [0; 32] || self.owner == [0; 32] || self.lamports == 0 {
            return Err(format!("{name} account identity/rent is invalid"));
        }
        if self.executable || self.data_base64.is_empty() {
            return Err(format!("{name} account image is not persisted state"));
        }
        Ok(())
    }
}

/// Compaction result which becomes the post-compaction validator's genesis.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompactionBridgeV1 {
    /// Schema discriminator.
    pub schema: String,
    /// Exact preterminal input, embedded rather than restated.
    pub preterminal: PreterminalBridgeV1,
    /// SHA-256 of the canonical embedded preterminal bytes.
    pub preterminal_bridge_sha256: String,
    /// Exact terminal payout rate per native claim.
    pub payout_per_claim: u64,
    /// Exact collateral moved into the claim-check vault.
    pub escrowed_collateral_atoms: u64,
    /// Holder's collateral Token-2022 account.
    pub holder_collateral_token: [u8; 32],
    /// Permissionless closer's collateral Token-2022 account.
    pub closer_collateral_token: [u8; 32],
    /// Permissionless closer identity.
    pub closer: [u8; 32],
    /// Persisted FractionalClaimCheck record.
    pub record: AccountImageV1,
    /// Persisted ClaimCheck escrow.
    pub escrow: AccountImageV1,
    /// Collateral vault after compaction.
    pub vault: AccountImageV1,
    /// Shard Mint after permissioned-burn authority transfer.
    pub shard_mint: AccountImageV1,
    /// Immutable collateral Mint used by the vault and payout accounts.
    pub collateral_mint: AccountImageV1,
    /// Sleeping holder's shard Token-2022 account.
    pub holder_shards: AccountImageV1,
    /// Holder's initially empty collateral Token-2022 account.
    pub holder_collateral: AccountImageV1,
    /// Closer's initially empty collateral Token-2022 account.
    pub closer_collateral: AccountImageV1,
}

impl CompactionBridgeV1 {
    /// Refuse a compaction result not exactly chained to its preterminal input.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != COMPACTION_SCHEMA_V1 {
            return Err("compaction bridge schema mismatch".into());
        }
        self.preterminal.validate()?;
        validate_digest("preterminal_bridge_sha256", &self.preterminal_bridge_sha256)?;
        let expected = digest(&canonical_bytes(&self.preterminal).map_err(|e| e.to_string())?);
        if self.preterminal_bridge_sha256 != expected {
            return Err("compaction bridge does not commit to its preterminal input".into());
        }
        if self.payout_per_claim != 4
            || self.escrowed_collateral_atoms
                != self
                    .preterminal
                    .reserve_native_claims
                    .checked_mul(self.payout_per_claim)
                    .ok_or_else(|| "collateral multiplication overflow".to_string())?
        {
            return Err("compaction payout conservation mismatch".into());
        }
        for (name, image) in [
            ("record", &self.record),
            ("escrow", &self.escrow),
            ("vault", &self.vault),
            ("shard_mint", &self.shard_mint),
            ("collateral_mint", &self.collateral_mint),
            ("holder_shards", &self.holder_shards),
            ("holder_collateral", &self.holder_collateral),
            ("closer_collateral", &self.closer_collateral),
        ] {
            image.validate(name)?;
        }
        if self.shard_mint.address != self.preterminal.shard_mint
            || self.holder_shards.address != self.preterminal.holder_shard_token
            || self.holder_collateral.address != self.holder_collateral_token
            || self.closer_collateral.address != self.closer_collateral_token
        {
            return Err("compaction account image identity mismatch".into());
        }
        Ok(())
    }
}

/// Machine-readable conservation and lifecycle ledger for the complete life.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FractionalCubicLifeLedgerV1 {
    /// Schema discriminator.
    pub schema: String,
    /// Exact Git commit from which every phase ran.
    pub source_commit: String,
    /// SHA-256 over the committed recursive `git ls-tree` manifest.
    pub source_tree_sha256: String,
    /// Exact executable identities shared by all phases.
    pub elves: ElfPinsV1,
    /// Canonical preterminal bridge SHA-256.
    pub preterminal_bridge_sha256: String,
    /// Canonical preterminal journal SHA-256.
    pub preterminal_journal_sha256: String,
    /// Canonical compaction bridge SHA-256.
    pub compaction_bridge_sha256: String,
    /// Canonical post-compaction journal SHA-256.
    pub postcompaction_journal_sha256: String,
    /// Activated execution release set.
    pub release_set: [u8; 32],
    /// Canonical Realm record digest.
    pub realm: [u8; 32],
    /// Core Market address.
    pub market: [u8; 32],
    /// Claims aggregate address.
    pub aggregate: [u8; 32],
    /// Product record digest.
    pub product: [u8; 32],
    /// ProductBasisV3 record digest.
    pub product_basis: [u8; 32],
    /// Fractional terms digest.
    pub terms: [u8; 32],
    /// Unified Trading-owned Fractional root.
    pub root: [u8; 32],
    /// Represented shard Mint.
    pub shard_mint: [u8; 32],
    /// Independent sleeping holder.
    pub holder: [u8; 32],
    /// Sleeping holder's shard account.
    pub holder_shard_token: [u8; 32],
    /// Outstanding shard atoms entering compaction.
    pub outstanding_shards: u64,
    /// Native claims locked before compaction.
    pub reserve_native_claims: u64,
    /// Collateral atoms paid per whole native claim.
    pub payout_per_claim: u64,
    /// Total collateral atoms conserved through burn and pay.
    pub escrowed_collateral_atoms: u64,
    /// Exact ordered completed lifecycle phases.
    pub completed_phases: [String; 3],
}

impl FractionalCubicLifeLedgerV1 {
    /// Refuse a malformed or incomplete complete-life ledger.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != LIFE_SCHEMA_V1 {
            return Err("fractional cubic life ledger schema mismatch".into());
        }
        validate_commit(&self.source_commit)?;
        self.elves.validate()?;
        for (name, value) in [
            ("source_tree_sha256", &self.source_tree_sha256),
            ("preterminal_bridge_sha256", &self.preterminal_bridge_sha256),
            (
                "preterminal_journal_sha256",
                &self.preterminal_journal_sha256,
            ),
            ("compaction_bridge_sha256", &self.compaction_bridge_sha256),
            (
                "postcompaction_journal_sha256",
                &self.postcompaction_journal_sha256,
            ),
        ] {
            validate_digest(name, value)?;
        }
        for (name, value) in [
            ("release_set", self.release_set),
            ("realm", self.realm),
            ("market", self.market),
            ("aggregate", self.aggregate),
            ("product", self.product),
            ("product_basis", self.product_basis),
            ("terms", self.terms),
            ("root", self.root),
            ("shard_mint", self.shard_mint),
            ("holder", self.holder),
            ("holder_shard_token", self.holder_shard_token),
        ] {
            if value == [0; 32] {
                return Err(format!("ledger {name} must be nonzero"));
            }
        }
        if self.outstanding_shards != 40
            || self.reserve_native_claims != 4
            || self.payout_per_claim != 4
            || self.escrowed_collateral_atoms != 16
            || self.reserve_native_claims.checked_mul(10) != Some(self.outstanding_shards)
            || self
                .reserve_native_claims
                .checked_mul(self.payout_per_claim)
                != Some(self.escrowed_collateral_atoms)
            || self.completed_phases.each_ref().map(String::as_str) != COMPLETED_PHASES_V1
        {
            return Err("fractional cubic life ledger is incomplete or non-conserving".into());
        }
        Ok(())
    }
}

/// Serialize one value as stable pretty JSON with one trailing newline.
pub fn canonical_bytes(value: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// SHA-256 in lowercase hexadecimal.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Atomically write canonical JSON without replacing an accepted file on encode failure.
pub fn write_atomic(path: &Path, value: &impl Serialize) -> Result<String, String> {
    let bytes = canonical_bytes(value).map_err(|error| error.to_string())?;
    let parent = path
        .parent()
        .ok_or_else(|| "bridge path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "bridge filename is not UTF-8".to_string())?;
    let temporary = parent.join(format!(".{name}.tmp.{}", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())?;
    Ok(digest(&bytes))
}

/// Strictly decode and validate one compiler-to-Found bridge.
pub fn read_founding(path: &Path) -> Result<FoundingBridgeV1, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let value: FoundingBridgeV1 =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if canonical_bytes(&value).map_err(|error| error.to_string())? != bytes {
        return Err("founding bridge is not canonical JSON".into());
    }
    value.validate()?;
    Ok(value)
}

/// Strictly decode and validate a preterminal bridge.
pub fn read_preterminal(path: &Path) -> Result<PreterminalBridgeV1, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let value: PreterminalBridgeV1 =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if canonical_bytes(&value).map_err(|error| error.to_string())? != bytes {
        return Err("preterminal bridge is not canonical JSON".into());
    }
    value.validate()?;
    Ok(value)
}

/// Strictly decode and validate a compaction bridge.
pub fn read_compaction(path: &Path) -> Result<CompactionBridgeV1, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let value: CompactionBridgeV1 =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if canonical_bytes(&value).map_err(|error| error.to_string())? != bytes {
        return Err("compaction bridge is not canonical JSON".into());
    }
    value.validate()?;
    Ok(value)
}

/// Strictly decode and validate one complete-life ledger.
pub fn read_life_ledger(path: &Path) -> Result<FractionalCubicLifeLedgerV1, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let value: FractionalCubicLifeLedgerV1 =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if canonical_bytes(&value).map_err(|error| error.to_string())? != bytes {
        return Err("fractional cubic life ledger is not canonical JSON".into());
    }
    value.validate()?;
    Ok(value)
}

fn validate_digest(name: &str, value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{name} is not lowercase SHA-256"));
    }
    Ok(())
}

fn validate_commit(value: &str) -> Result<(), String> {
    if value.len() != 40
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("source_commit is not a full lowercase Git identity".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn founding_pins() -> FoundingElfPinsV1 {
        FoundingElfPinsV1 {
            registry: "31".repeat(32),
            core: "32".repeat(32),
            claims: "33".repeat(32),
            trading: "34".repeat(32),
            resolution: "35".repeat(32),
            custody: "36".repeat(32),
            rent: "37".repeat(32),
        }
    }

    fn founding_account(address: u8, owner: u8) -> FoundingAccountPinV1 {
        FoundingAccountPinV1 {
            address: [address; 32],
            owner: [owner; 32],
            data_sha256: format!("{address:02x}").repeat(32),
            account_sha256: format!("{owner:02x}").repeat(32),
        }
    }

    fn founding() -> FoundingBridgeV1 {
        FoundingBridgeV1 {
            schema: FOUNDING_SCHEMA_V1.into(),
            source_commit: "ab".repeat(20),
            source_tree_sha256: "10".repeat(32),
            compiler_input_sha256: "11".repeat(32),
            compiler_report_sha256: "12".repeat(32),
            successor_plan_sha256: "13".repeat(32),
            market_input_sha256: "14".repeat(32),
            successor_report_sha256: "15".repeat(32),
            founding_checkpoint_sha256: "16".repeat(32),
            dcltgmf3_journal_sha256: "17".repeat(32),
            dcltgmf3_intent_sha256: "18".repeat(32),
            dcltgmf3_state_sha256: "19".repeat(32),
            dcltgmf3_transaction_sha256: "1a".repeat(32),
            elves: founding_pins(),
            release_set: [1; 32],
            realm: [2; 32],
            market: [3; 32],
            aggregate: [4; 32],
            founder_position: [5; 32],
            product: [6; 32],
            result_domain: [7; 32],
            portfolio: [8; 32],
            product_basis: [9; 32],
            price_gate: [10; 32],
            product_account: founding_account(20, 0xa2),
            result_domain_account: founding_account(21, 0xa2),
            portfolio_account: founding_account(22, 0xa2),
            product_basis_account: founding_account(23, 0xa2),
            price_gate_account: founding_account(24, 0xa2),
            market_account: founding_account(3, 0xa3),
            aggregate_account: founding_account(4, 0xa1),
            founder_position_account: founding_account(5, 0xa1),
            product_width: 4,
            curve_degree: 3,
            payout_scale: 11,
            initial_collateral_atoms: 198,
            complete_set_quantity: 9,
            collateral_principal_atoms: 99,
        }
    }

    fn pins() -> ElfPinsV1 {
        ElfPinsV1 {
            claims: "01".repeat(32),
            registry: "02".repeat(32),
            core: "03".repeat(32),
            custody: "04".repeat(32),
            rent: "05".repeat(32),
            trading: "06".repeat(32),
            token_2022: "07".repeat(32),
        }
    }

    fn preterminal() -> PreterminalBridgeV1 {
        PreterminalBridgeV1 {
            schema: PRETERMINAL_SCHEMA_V1.into(),
            source_commit: "ab".repeat(20),
            source_tree_sha256: "10".repeat(32),
            elves: pins(),
            journal_sha256: "11".repeat(32),
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
            rounding_boundary: ROUNDING_BOUNDARY_V1.into(),
        }
    }

    fn image(address: [u8; 32], owner: u8) -> AccountImageV1 {
        AccountImageV1 {
            address,
            owner: [owner; 32],
            lamports: 1,
            rent_epoch: 0,
            executable: false,
            data_base64: "AA==".into(),
        }
    }

    fn compaction() -> CompactionBridgeV1 {
        let preterminal = preterminal();
        let holder_collateral_token = [13; 32];
        let closer_collateral_token = [14; 32];
        CompactionBridgeV1 {
            schema: COMPACTION_SCHEMA_V1.into(),
            preterminal_bridge_sha256: digest(
                &canonical_bytes(&preterminal).expect("canonical preterminal"),
            ),
            preterminal,
            payout_per_claim: 4,
            escrowed_collateral_atoms: 16,
            holder_collateral_token,
            closer_collateral_token,
            closer: [12; 32],
            record: image([15; 32], 0xa1),
            escrow: image([16; 32], 0xa1),
            vault: image([17; 32], 0x22),
            shard_mint: image([9; 32], 0x22),
            collateral_mint: image([18; 32], 0x22),
            holder_shards: image([11; 32], 0x22),
            holder_collateral: image(holder_collateral_token, 0x22),
            closer_collateral: image(closer_collateral_token, 0x22),
        }
    }

    #[test]
    fn founding_bridge_refuses_digest_account_and_rounding_substitution() {
        let value = founding();
        value.validate().expect("control");

        let mut report = value.clone();
        report.successor_report_sha256 = "not-a-digest".into();
        assert_eq!(
            report.validate(),
            Err("successor_report_sha256 is not lowercase SHA-256".into())
        );

        let mut account = value.clone();
        account.aggregate_account.address = [90; 32];
        assert_eq!(
            account.validate(),
            Err("founding aggregate account address mismatch".into())
        );

        let mut remainder = value;
        remainder.initial_collateral_atoms = 200;
        assert_eq!(
            remainder.validate(),
            Err("founding cubic scale or exact-reserve contract mismatch".into())
        );
    }

    fn ledger() -> FractionalCubicLifeLedgerV1 {
        let compaction = compaction();
        let preterminal = &compaction.preterminal;
        FractionalCubicLifeLedgerV1 {
            schema: LIFE_SCHEMA_V1.into(),
            source_commit: preterminal.source_commit.clone(),
            source_tree_sha256: preterminal.source_tree_sha256.clone(),
            elves: preterminal.elves.clone(),
            preterminal_bridge_sha256: compaction.preterminal_bridge_sha256.clone(),
            preterminal_journal_sha256: preterminal.journal_sha256.clone(),
            compaction_bridge_sha256: "20".repeat(32),
            postcompaction_journal_sha256: "21".repeat(32),
            release_set: preterminal.release_set,
            realm: preterminal.realm,
            market: preterminal.market,
            aggregate: preterminal.aggregate,
            product: preterminal.product,
            product_basis: preterminal.product_basis,
            terms: preterminal.terms,
            root: preterminal.root,
            shard_mint: preterminal.shard_mint,
            holder: preterminal.holder,
            holder_shard_token: preterminal.holder_shard_token,
            outstanding_shards: preterminal.outstanding_shards,
            reserve_native_claims: preterminal.reserve_native_claims,
            payout_per_claim: compaction.payout_per_claim,
            escrowed_collateral_atoms: compaction.escrowed_collateral_atoms,
            completed_phases: COMPLETED_PHASES_V1.map(Into::into),
        }
    }

    #[test]
    fn unknown_fields_and_amount_substitution_refuse() {
        let value = preterminal();
        value.validate().expect("control");
        let mut json: serde_json::Value =
            serde_json::from_slice(&canonical_bytes(&value).expect("json")).expect("value");
        json.as_object_mut()
            .expect("object")
            .insert("invented".into(), serde_json::json!(true));
        assert!(serde_json::from_value::<PreterminalBridgeV1>(json).is_err());

        let mut substituted = value;
        substituted.outstanding_shards = 41;
        assert_eq!(
            substituted.validate(),
            Err("preterminal amount, curve, or rounding contract mismatch".into())
        );
    }

    #[test]
    fn compaction_refuses_bridge_digest_payout_and_image_substitution() {
        let value = compaction();
        value.validate().expect("control");

        let mut digest_substitution = value.clone();
        digest_substitution.preterminal_bridge_sha256 = "99".repeat(32);
        assert_eq!(
            digest_substitution.validate(),
            Err("compaction bridge does not commit to its preterminal input".into())
        );

        let mut payout_substitution = value.clone();
        payout_substitution.escrowed_collateral_atoms = 15;
        assert_eq!(
            payout_substitution.validate(),
            Err("compaction payout conservation mismatch".into())
        );

        let mut image_substitution = value;
        image_substitution.holder_shards.address = [19; 32];
        assert_eq!(
            image_substitution.validate(),
            Err("compaction account image identity mismatch".into())
        );
    }

    #[test]
    fn complete_life_ledger_refuses_missing_phase_and_nonconservation() {
        let value = ledger();
        value.validate().expect("control");

        let mut missing_phase = value.clone();
        missing_phase.completed_phases[2] = "almost-closed".into();
        assert_eq!(
            missing_phase.validate(),
            Err("fractional cubic life ledger is incomplete or non-conserving".into())
        );

        let mut stranded_atom = value;
        stranded_atom.escrowed_collateral_atoms = 15;
        assert_eq!(
            stranded_atom.validate(),
            Err("fractional cubic life ledger is incomplete or non-conserving".into())
        );
    }
}
