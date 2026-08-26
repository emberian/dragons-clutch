use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
