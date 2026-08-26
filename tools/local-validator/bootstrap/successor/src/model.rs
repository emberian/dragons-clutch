use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunProgramInput {
    pub(crate) program_id: String,
    pub(crate) elf_path: String,
    pub(crate) elf_sha256: String,
    pub(crate) semantic_release_id: String,
    pub(crate) attestation: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SuccessorRunSpec {
    pub(crate) schema: String,
    pub(crate) rpc_url: String,
    pub(crate) launcher: String,
    pub(crate) ledger: String,
    pub(crate) account_dir: String,
    pub(crate) plan: String,
    pub(crate) output: String,
    pub(crate) registry: RunProgramInput,
    pub(crate) core: RunProgramInput,
    pub(crate) claims: RunProgramInput,
    pub(crate) trading: RunProgramInput,
    pub(crate) resolution: RunProgramInput,
    pub(crate) custody: RunProgramInput,
    pub(crate) rent_credit: RunProgramInput,
    pub(crate) market: MarketRunInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MarketRunInput {
    pub(crate) generation: u64,
    pub(crate) collateral_display_decimals: u8,
    pub(crate) initial_collateral_atoms: u64,
    pub(crate) product_id: String,
    pub(crate) coordinate_domain_id: String,
    pub(crate) result_unit_id: String,
    pub(crate) claim_basis_id: String,
    pub(crate) liability_basis_id: String,
    pub(crate) representation_release_id: String,
    pub(crate) mapping_release_id: String,
    pub(crate) cut_denominator: u64,
    pub(crate) cuts: Vec<String>,
    pub(crate) portfolio_denominator: u64,
    pub(crate) coefficients: Vec<u64>,
    pub(crate) primary_source_spec_id: String,
    pub(crate) window_spec_id: String,
    pub(crate) statistic_spec_id: String,
    pub(crate) failure_policy_release_id: String,
    pub(crate) recovery_policy_hex: String,
    pub(crate) capability_manifest_hex: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ProgramPin {
    pub(crate) program_id: String,
    pub(crate) programdata_id: String,
    pub(crate) elf_path: String,
    pub(crate) elf_sha256: String,
    pub(crate) semantic_release_id: String,
    pub(crate) artifact_release_id: String,
    pub(crate) upgrade_authority: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RecordPair {
    pub(crate) raw: String,
    pub(crate) staging: String,
    pub(crate) schema_id: String,
    pub(crate) content_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct InfrastructureProfilePin {
    pub(crate) address: String,
    pub(crate) schema_id: String,
    pub(crate) body_sha256: String,
    pub(crate) body_hex: String,
    pub(crate) registry_artifact_release_id: String,
    pub(crate) rent_artifact_release_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct CoreBootstrapPin {
    pub(crate) upgrade_authority: String,
    pub(crate) genesis_programdata_sha256: String,
    pub(crate) post_revoke_programdata_sha256: String,
    pub(crate) release_recognition_requires_revoke: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct GenesisAccountPin {
    pub(crate) address: String,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) data_len: usize,
    pub(crate) data_sha256: String,
    pub(crate) account_sha256: String,
    pub(crate) json_file_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SuccessorPlan {
    pub(crate) schema: String,
    pub(crate) genesis_boundary: Vec<String>,
    pub(crate) bootstrap_order: Vec<String>,
    pub(crate) execution_blocker: String,
    pub(crate) account_dir: String,
    pub(crate) registry: ProgramPin,
    pub(crate) core: ProgramPin,
    pub(crate) claims: ProgramPin,
    pub(crate) trading: ProgramPin,
    pub(crate) resolution: ProgramPin,
    pub(crate) custody: ProgramPin,
    pub(crate) rent_credit: ProgramPin,
    pub(crate) activation: String,
    pub(crate) release_set_id: String,
    pub(crate) core_bootstrap: CoreBootstrapPin,
    pub(crate) infrastructure_profile: InfrastructureProfilePin,
    pub(crate) records: BTreeMap<String, RecordPair>,
    pub(crate) provider_release_id: String,
    pub(crate) fixture_publish_time: i64,
    pub(crate) genesis_accounts: BTreeMap<String, GenesisAccountPin>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TransactionEvidence {
    pub(crate) label: String,
    pub(crate) signature: String,
    pub(crate) slot: u64,
    pub(crate) transaction_metadata_available: bool,
    pub(crate) fee_lamports: Option<u64>,
    pub(crate) compute_units_consumed: Option<u64>,
    pub(crate) error: Option<serde_json::Value>,
    pub(crate) logs: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AccountEvidence {
    pub(crate) address: String,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data_len: usize,
    pub(crate) data_sha256: String,
    pub(crate) account_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SuccessorRunEvidence {
    pub(crate) schema: String,
    pub(crate) rpc_url: String,
    pub(crate) ledger: String,
    pub(crate) validator_log: String,
    pub(crate) plan_sha256: String,
    pub(crate) core_upgrade_authority_pubkey: String,
    pub(crate) private_key_persisted: bool,
    pub(crate) completed: Vec<String>,
    pub(crate) transactions: Vec<TransactionEvidence>,
    pub(crate) accounts: BTreeMap<String, AccountEvidence>,
    pub(crate) remaining_execution_seam: String,
}
