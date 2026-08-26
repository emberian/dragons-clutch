use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ProgramPin {
    pub(crate) program_id: String,
    pub(crate) programdata_id: String,
    pub(crate) elf_path: String,
    pub(crate) elf_sha256: String,
    pub(crate) semantic_release_id: String,
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
pub(crate) struct SourceCase {
    pub(crate) market: String,
    pub(crate) state: String,
    pub(crate) certificates: BTreeMap<String, String>,
    pub(crate) funding: BTreeMap<String, String>,
    pub(crate) hostile_certificate_preoccupied: bool,
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
    pub(crate) account_dir: String,
    pub(crate) registry: ProgramPin,
    pub(crate) core: ProgramPin,
    pub(crate) claims: ProgramPin,
    pub(crate) trading: ProgramPin,
    pub(crate) resolution: ProgramPin,
    pub(crate) custody: ProgramPin,
    pub(crate) activation: String,
    pub(crate) release_set_id: String,
    pub(crate) records: BTreeMap<String, RecordPair>,
    pub(crate) result_domain_id: String,
    pub(crate) source_material_id: String,
    pub(crate) funded_source_material_id: String,
    pub(crate) capability_manifest_id: String,
    pub(crate) provider_release_id: String,
    pub(crate) recovery_allocation_id: String,
    pub(crate) exhaustion_allocation_id: String,
    pub(crate) fixture_publish_time: i64,
    pub(crate) configured_max_age_seconds: u32,
    pub(crate) funded_max_age_seconds: u32,
    pub(crate) generation: u64,
    pub(crate) primary: SourceCase,
    pub(crate) lifecycle: SourceCase,
    pub(crate) rollback: SourceCase,
    pub(crate) genesis_accounts: BTreeMap<String, GenesisAccountPin>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ProviderEvidenceInput {
    pub(crate) rpc_url: String,
    pub(crate) provider_state_initialized: bool,
    pub(crate) captured_release_identity_claimed: bool,
    pub(crate) price_update: String,
    pub(crate) price_update_reclaimed: bool,
    pub(crate) programs: Vec<ProviderProgramInput>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ProviderProgramInput {
    pub(crate) name: String,
    pub(crate) program_id: String,
    pub(crate) programdata_id: String,
    pub(crate) observed_deployment_slot: u64,
    pub(crate) observed_upgrade_authority_effectively_disabled: bool,
    pub(crate) elf_tail_sha256: String,
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
pub(crate) struct LoaderProgramEvidence {
    pub(crate) program_id: String,
    pub(crate) programdata_id: String,
    pub(crate) deployment_slot: u64,
    pub(crate) upgrade_authority: Option<String>,
    pub(crate) upgrade_authority_effectively_disabled: bool,
    pub(crate) elf_sha256: String,
    pub(crate) loader_header_sha256: String,
    pub(crate) program: AccountEvidence,
    pub(crate) programdata: AccountEvidence,
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

#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct RollbackEvidence {
    pub(crate) transaction: TransactionEvidence,
    pub(crate) state_unchanged: bool,
    pub(crate) certificate_unchanged: bool,
    pub(crate) funding_unchanged: bool,
    pub(crate) worker_unchanged: bool,
    pub(crate) before: BTreeMap<String, AccountEvidence>,
    pub(crate) after: BTreeMap<String, AccountEvidence>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReplayEvidence {
    pub(crate) transaction: TransactionEvidence,
    pub(crate) state_unchanged: bool,
    pub(crate) certificate_unchanged: bool,
    pub(crate) before: BTreeMap<String, AccountEvidence>,
    pub(crate) after: BTreeMap<String, AccountEvidence>,
}

#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct ExecutionEvidence {
    pub(crate) schema: &'static str,
    pub(crate) evidence_class: &'static str,
    pub(crate) rpc_url: String,
    pub(crate) provider_evidence_path: String,
    pub(crate) plan_path: String,
    pub(crate) genesis_fixture_boundary: Vec<String>,
    pub(crate) semantic_records_created_onchain: bool,
    pub(crate) markets_created_onchain: bool,
    pub(crate) source_states_created_onchain: bool,
    pub(crate) funding_created_onchain: bool,
    pub(crate) certificates_created_by_resolution: bool,
    pub(crate) captured_release_identity_claimed: bool,
    pub(crate) checked_production_release_claimed: bool,
    pub(crate) registry_activated: bool,
    pub(crate) core_reauthenticated: bool,
    pub(crate) claims_reauthenticated: bool,
    pub(crate) trading_reauthenticated: bool,
    pub(crate) custody_reauthenticated: bool,
    pub(crate) registry_reauthenticated: bool,
    pub(crate) real_pyth_price_update_consumed: bool,
    pub(crate) primary_resolution_executed: bool,
    pub(crate) primary_replay_refused: bool,
    pub(crate) sequential_recovery_exhaustion_failure_executed: bool,
    pub(crate) rollback_proved: bool,
    pub(crate) programs: BTreeMap<String, LoaderProgramEvidence>,
    pub(crate) accounts: BTreeMap<String, AccountEvidence>,
    pub(crate) transactions: Vec<TransactionEvidence>,
    pub(crate) primary_replay: ReplayEvidence,
    pub(crate) rollback: RollbackEvidence,
}
