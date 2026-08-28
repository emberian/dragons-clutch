//! Read-only production of one Rust-authored wallet terminal-payout manifest.
//!
//! The JSON emitted here is intentionally smaller than the semantic input. The
//! input only selects immutable content and wallet coordinates; one finalized
//! observation reauthenticates every selected record, current deployment,
//! mutable prestate, and the sole canonical lookup table. `dclutch-operator`
//! remains the owner of request, SignedDelta, payout, account-frame, and v0
//! packet construction.

use std::{collections::BTreeMap, io::Write, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_claims_svm::{
    liability_basis_state_v2::{LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2},
    product_basis_terminal_v3::{
        ProductClaimsTerminalAdmissionV3, TERMINAL_COORDINATE_BYTES_V2,
        TERMINAL_COORDINATE_MAGIC_V2, TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CallerRoleV1 as CustodyCallerRoleV1, CompartmentV1,
    CustodyReplaySeedsV1, CustodyVaultSeedsV1,
};
use dclutch_market_core_codec::{
    CoreState, MarketCoreStateSeedsV2, Phase as CorePhase, STATE_BYTES,
};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    wallet_terminal_payout_v3::{
        WalletTerminalPayoutInputV3, WalletTerminalPayoutReportV3, WalletTerminalPayoutRouteV3,
        build_wallet_terminal_payout_v3, canonical_wallet_terminal_payout_lookup_addresses_v3,
        compile_wallet_terminal_payout_v0,
    },
};
use dclutch_product_payoff_v2_codec::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3, runtime_v3::BasisKindV3,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_rational_representation_v2_kernel::{
    REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3, product_v3::TerminalScenarioV3,
};
use dclutch_realm_contract::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_activation_auth_v1::{
    activation_cache_address_v1, authenticate_activated_role_v1,
};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_representation_composition_v3_kernel::{
    COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3, COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
    COMPOSITION_GRAPH_SCHEMA_ID_V3, COMPOSITION_TRANSLATION_SCHEMA_ID_V3, RecordAdmissionV3,
};
use dclutch_representation_composition_v3_operator::{
    CompositionChainObservationV3, FinalizedRecordObservationV3, ProductCompositionObservationV3,
    RepresentationCompositionObservationV3, authenticate_composition_v3,
};
use dclutch_token_svm::{Mint, TokenProgram};
use serde::{Deserialize, Serialize};
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_program::{
    account_info::AccountInfo, hash::hash, instruction::Instruction, pubkey::Pubkey, rent::Rent,
};
use solana_sdk::hash::Hash;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG},
    plan::{hex, hex32, pubkey},
    rpc::{Rpc, RpcAccount, WritePolicyV1},
};

use dclutch_versioned_message_operator::EXTEND_ADDRESSES_PER_TRANSACTION_V1;

pub(crate) const INPUT_FORMAT: &str = "dclutch-wallet-terminal-payout-plan-input-v1";
const OUTPUT_FORMAT: &str = "dclutch-wallet-terminal-payout-v3";
const ALT_OUTPUT_FORMAT: &str = "dclutch-wallet-terminal-payout-alt-plan-v1";
// Compilation is a geometry/ALT admission check only. The browser obtains a
// fresh blockhash immediately before wallet signing; this hash is never emitted.
const GEOMETRY_BLOCKHASH: [u8; 32] = [0xa5; 32];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProgramSelectorsV1 {
    pub(crate) registry: String,
    pub(crate) core: String,
    pub(crate) claims: String,
    pub(crate) custody: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RecordSelectorsV1 {
    pub(crate) realm: String,
    pub(crate) product: String,
    pub(crate) result_domain: String,
    pub(crate) portfolio: String,
    pub(crate) product_basis: String,
    pub(crate) execution_descriptor: String,
    pub(crate) composition_descriptor: String,
    pub(crate) composition_graph: String,
    pub(crate) composition_translation: String,
    pub(crate) composition_exposure: String,
    pub(crate) terminal_record: String,
}

/// Immutable selectors and wallet coordinates for one checked payout.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PlanInputV1 {
    pub(crate) format: String,
    pub(crate) market: String,
    pub(crate) owner: String,
    pub(crate) recipient_owner: String,
    pub(crate) recipient: String,
    pub(crate) collateral_mint: String,
    pub(crate) token_program: String,
    pub(crate) quantity: String,
    pub(crate) claim_index: u32,
    pub(crate) transfer_index: u16,
    pub(crate) parent_context: String,
    pub(crate) custody_context: String,
    pub(crate) release_set: String,
    #[serde(
        default,
        deserialize_with = "optional_lookup_table",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) lookup_table: Option<String>,
    pub(crate) programs: ProgramSelectorsV1,
    pub(crate) records: RecordSelectorsV1,
}

#[derive(Clone, Copy)]
pub(crate) struct RecordPairV1 {
    pub(crate) schema: [u8; 32],
    pub(crate) digest: [u8; 32],
    pub(crate) raw: Pubkey,
    pub(crate) staging: Pubkey,
}

#[derive(Clone)]
pub(crate) struct SelectedInputV1 {
    pub(crate) market: Pubkey,
    pub(crate) owner: Pubkey,
    recipient_owner: Pubkey,
    pub(crate) recipient: Pubkey,
    collateral_mint: Pubkey,
    token_program: Pubkey,
    quantity: u64,
    claim_index: u32,
    transfer_index: u16,
    parent_context: [u8; 32],
    custody_context: [u8; 32],
    pub(crate) release_set: [u8; 32],
    lookup_table: Option<Pubkey>,
    registry: Pubkey,
    core: Pubkey,
    claims: Pubkey,
    custody: Pubkey,
    pub(crate) realm: RecordPairV1,
    pub(crate) product: RecordPairV1,
    pub(crate) result_domain: RecordPairV1,
    pub(crate) portfolio: RecordPairV1,
    pub(crate) product_basis: RecordPairV1,
    pub(crate) execution_descriptor: RecordPairV1,
    pub(crate) composition_descriptor: RecordPairV1,
    pub(crate) composition_graph: RecordPairV1,
    pub(crate) composition_translation: RecordPairV1,
    pub(crate) composition_exposure: RecordPairV1,
    pub(crate) terminal_record_digest: [u8; 32],
    pub(crate) terminal_coordinate: RecordPairV1,
    pub(crate) aggregate: Pubkey,
    pub(crate) position: Pubkey,
    activation_cache: Pubkey,
    claims_programdata: Pubkey,
    core_programdata: Pubkey,
    custody_programdata: Pubkey,
    pub(crate) custody_replay: Pubkey,
    custody_authority: Pubkey,
    pub(crate) hoard: Pubkey,
}

#[derive(Clone, Copy)]
pub(crate) enum LookupTableRequirementV1 {
    Present,
    Absent,
}

impl SelectedInputV1 {
    pub(crate) fn parse(
        input: &PlanInputV1,
        requirement: LookupTableRequirementV1,
    ) -> Result<Self> {
        if input.format != INPUT_FORMAT {
            return Err(Error::new(format!(
                "wallet payout input format must be {INPUT_FORMAT}"
            )));
        }
        let market = nonzero_pubkey(&input.market, "market")?;
        let owner = nonzero_pubkey(&input.owner, "owner")?;
        let recipient_owner = nonzero_pubkey(&input.recipient_owner, "recipientOwner")?;
        let recipient = nonzero_pubkey(&input.recipient, "recipient")?;
        let collateral_mint = nonzero_pubkey(&input.collateral_mint, "collateralMint")?;
        let token_program = nonzero_pubkey(&input.token_program, "tokenProgram")?;
        let lookup_table = match (&input.lookup_table, requirement) {
            (Some(value), LookupTableRequirementV1::Present) => {
                Some(nonzero_pubkey(value, "lookupTable")?)
            }
            (None, LookupTableRequirementV1::Absent) => None,
            (None, LookupTableRequirementV1::Present) => {
                return Err(Error::new("lookupTable is required for a payout manifest"));
            }
            (Some(_), LookupTableRequirementV1::Absent) => {
                return Err(Error::new(
                    "lookupTable must be omitted while preparing its canonical ALT plan",
                ));
            }
        };
        let registry = nonzero_pubkey(&input.programs.registry, "programs.registry")?;
        let core = nonzero_pubkey(&input.programs.core, "programs.core")?;
        let claims = nonzero_pubkey(&input.programs.claims, "programs.claims")?;
        let custody = nonzero_pubkey(&input.programs.custody, "programs.custody")?;
        if recipient_owner != owner {
            return Err(Error::new(
                "recipientOwner must equal owner for a wallet terminal payout",
            ));
        }
        if input.transfer_index != 0 {
            return Err(Error::new(
                "transferIndex must be zero for a wallet terminal payout",
            ));
        }
        let quantity = canonical_u64(&input.quantity, "quantity", true)?;
        let parent_context = nonzero_hex(&input.parent_context, "parentContext")?;
        let custody_context = nonzero_hex(&input.custody_context, "custodyContext")?;
        let release_set = nonzero_hex(&input.release_set, "releaseSet")?;
        let record = |schema, value: &str, label: &str| -> Result<RecordPairV1> {
            let digest = nonzero_hex(value, label)?;
            Ok(record_pair(registry, schema, digest))
        };
        let realm = record(REALM_SCHEMA_RELEASE_ID_V1, &input.records.realm, "realm")?;
        let product = record(
            PRODUCT_RECORD_SCHEMA_ID_V2,
            &input.records.product,
            "product",
        )?;
        let result_domain = record(
            RESULT_DOMAIN_SCHEMA_ID_V2,
            &input.records.result_domain,
            "resultDomain",
        )?;
        let portfolio = record(
            PORTFOLIO_SCHEMA_ID_V2,
            &input.records.portfolio,
            "portfolio",
        )?;
        let product_basis = record(
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            &input.records.product_basis,
            "productBasis",
        )?;
        let execution_descriptor = record(
            REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
            &input.records.execution_descriptor,
            "executionDescriptor",
        )?;
        let composition_descriptor = record(
            COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
            &input.records.composition_descriptor,
            "compositionDescriptor",
        )?;
        let composition_graph = record(
            COMPOSITION_GRAPH_SCHEMA_ID_V3,
            &input.records.composition_graph,
            "compositionGraph",
        )?;
        let composition_translation = record(
            COMPOSITION_TRANSLATION_SCHEMA_ID_V3,
            &input.records.composition_translation,
            "compositionTranslation",
        )?;
        let composition_exposure = record(
            COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
            &input.records.composition_exposure,
            "compositionExposure",
        )?;
        let terminal_record_digest = nonzero_hex(&input.records.terminal_record, "terminalRecord")?;
        let terminal_coordinate = record_pair(
            core,
            TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2,
            terminal_record_digest,
        );
        let aggregate = Pubkey::find_program_address(
            &[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()],
            &claims,
        )
        .0;
        let position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), owner.to_bytes())
                .map_err(|error| Error::new(format!("Position seeds: {error:?}")))?
                .as_slices(),
            &claims,
        )
        .0;
        let custody_replay = Pubkey::find_program_address(
            &CustodyReplaySeedsV1::new(
                market.to_bytes(),
                release_set,
                CustodyCallerRoleV1::Claims,
                custody_context,
            )
            .as_slices(),
            &custody,
        )
        .0;
        let custody_authority = Pubkey::find_program_address(
            &[
                CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
                market.as_ref(),
                release_set.as_slice(),
            ],
            &custody,
        )
        .0;
        let hoard = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(
                market.to_bytes(),
                release_set,
                custody_context,
                CompartmentV1::HoardPrincipal,
            )
            .as_slices(),
            &custody,
        )
        .0;
        Ok(Self {
            market,
            owner,
            recipient_owner,
            recipient,
            collateral_mint,
            token_program,
            quantity,
            claim_index: input.claim_index,
            transfer_index: input.transfer_index,
            parent_context,
            custody_context,
            release_set,
            lookup_table,
            registry,
            core,
            claims,
            custody,
            realm,
            product,
            result_domain,
            portfolio,
            product_basis,
            execution_descriptor,
            composition_descriptor,
            composition_graph,
            composition_translation,
            composition_exposure,
            terminal_record_digest,
            terminal_coordinate,
            aggregate,
            position,
            activation_cache: activation_cache_address_v1(&registry, &release_set),
            claims_programdata: programdata_address(claims),
            core_programdata: programdata_address(core),
            custody_programdata: programdata_address(custody),
            custody_replay,
            custody_authority,
            hoard,
        })
    }

    pub(crate) fn addresses(&self) -> Vec<Pubkey> {
        let mut values = vec![
            self.market,
            self.recipient,
            self.collateral_mint,
            self.token_program,
            self.registry,
            self.core,
            self.claims,
            self.custody,
            self.activation_cache,
            self.claims_programdata,
            self.core_programdata,
            self.custody_programdata,
            self.aggregate,
            self.position,
            self.custody_replay,
            self.hoard,
            sysvar::rent::ID,
        ];
        if let Some(lookup_table) = self.lookup_table {
            values.push(lookup_table);
        }
        for pair in [
            self.realm,
            self.product,
            self.result_domain,
            self.portfolio,
            self.product_basis,
            self.execution_descriptor,
            self.composition_descriptor,
            self.composition_graph,
            self.composition_translation,
            self.composition_exposure,
            self.terminal_coordinate,
        ] {
            values.push(pair.raw);
            values.push(pair.staging);
        }
        values.sort_unstable();
        values.dedup();
        values
    }
}

pub(crate) struct FinalizedSnapshotV1 {
    pub(crate) observation: Observation,
    pub(crate) accounts: BTreeMap<Pubkey, ObservedAccount>,
}

impl FinalizedSnapshotV1 {
    pub(crate) fn from_rpc(
        slot: u64,
        unix_timestamp: i64,
        keys: &[Pubkey],
        values: Vec<Option<RpcAccount>>,
    ) -> Result<Self> {
        if slot == 0 || keys.len() != values.len() {
            return Err(Error::new(
                "wallet payout snapshot had a zero slot or changed width",
            ));
        }
        let observation = Observation {
            slot,
            unix_timestamp,
            finality: Finality::Finalized,
        };
        let accounts = keys
            .iter()
            .copied()
            .zip(values)
            .map(|(key, value)| {
                let observed = match value {
                    Some(account) => ObservedAccount {
                        observation,
                        key,
                        owner: account.owner,
                        lamports: account.lamports,
                        executable: account.executable,
                        data: account.data,
                    },
                    None => ObservedAccount {
                        observation,
                        key,
                        owner: system_program::ID,
                        lamports: 0,
                        executable: false,
                        data: Vec::new(),
                    },
                };
                (key, observed)
            })
            .collect();
        Ok(Self {
            observation,
            accounts,
        })
    }

    fn account(&self, key: Pubkey) -> Result<&ObservedAccount> {
        self.accounts
            .get(&key)
            .ok_or_else(|| Error::new(format!("wallet payout snapshot omitted {key}")))
    }

    pub(crate) fn required(&self, key: Pubkey, label: &str) -> Result<&ObservedAccount> {
        let account = self.account(key)?;
        if account.lamports == 0 {
            return Err(Error::new(format!(
                "wallet payout snapshot is missing {label} {key}"
            )));
        }
        Ok(account)
    }

    fn record(&self, pair: RecordPairV1, rent: &Rent) -> Result<FinalizedRecordObservationV3<'_>> {
        let raw = self.required(pair.raw, "finalized record")?;
        let staging = self.account(pair.staging)?;
        Ok(FinalizedRecordObservationV3 {
            schema_id: pair.schema,
            raw,
            staging,
            raw_rent_minimum: rent.minimum_balance(raw.data.len()),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestRouteV3 {
    aggregate: String,
    linked_basis_raw: String,
    linked_basis_staging: String,
    product_raw: String,
    product_staging: String,
    result_domain_raw: String,
    result_domain_staging: String,
    portfolio_raw: String,
    portfolio_staging: String,
    market: String,
    activation_cache: String,
    registry_program: String,
    claims_program: String,
    claims_program_data: String,
    core_program: String,
    core_program_data: String,
    position: String,
    exposure_raw: String,
    exposure_staging: String,
    custody_program: String,
    terminal_coordinate_raw: String,
    terminal_coordinate_staging: String,
    realm_raw: String,
    realm_staging: String,
    custody_replay: String,
    collateral_mint: String,
    hoard: String,
    recipient: String,
    custody_authority: String,
    token_program: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestRequestV3 {
    release_set: String,
    market: String,
    realm: String,
    parent_context: String,
    product_record_digest: String,
    exposure_id: String,
    exposure_digest: String,
    terminal_record_digest: String,
    owner: String,
    position: String,
    recipient_owner: String,
    recipient: String,
    claims_program: String,
    custody_program: String,
    collateral_mint: String,
    token_program: String,
    semantic_basis_id: String,
    linked_basis_record_digest: String,
    generation: String,
    expected_market_revision: String,
    expected_position_revision: String,
    expected_custody_revision: String,
    quantity: String,
    claim_index: u32,
    transfer_index: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WalletTerminalPayoutManifestV3 {
    format: &'static str,
    route: ManifestRouteV3,
    custody_context: String,
    request: ManifestRequestV3,
    signed_packet_base64: String,
    payout: String,
    lookup_table: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstructionAccountV1 {
    address: String,
    signer: bool,
    writable: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstructionManifestV1 {
    program_id: String,
    accounts: Vec<InstructionAccountV1>,
    data_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WalletTerminalPayoutAltPlanV1 {
    format: &'static str,
    source_input_sha256: String,
    observation_slot: String,
    payer: String,
    authority: String,
    lookup_table: String,
    addresses: Vec<String>,
    create: InstructionManifestV1,
    extensions: Vec<InstructionManifestV1>,
    payout_input: PlanInputV1,
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let (origin, _source, decoded) = command_input(arguments, "wallet-terminal-payout-plan")?;
    let selected = SelectedInputV1::parse(&decoded, LookupTableRequirementV1::Present)?;
    let snapshot = finalized_snapshot(&origin, &selected)?;
    stdout_json(&build_manifest(&selected, &snapshot)?)
}

pub(crate) fn run_alt(arguments: Vec<String>) -> Result<()> {
    let (origin, source, decoded) = command_input(arguments, "wallet-terminal-payout-alt-plan")?;
    let selected = SelectedInputV1::parse(&decoded, LookupTableRequirementV1::Absent)?;
    let snapshot = finalized_snapshot(&origin, &selected)?;
    let report = build_report(&selected, &snapshot)?;
    stdout_json(&build_alt_plan(decoded, &source, &report)?)
}

fn command_input(
    arguments: Vec<String>,
    command: &str,
) -> Result<(ClusterOriginV1, Vec<u8>, PlanInputV1)> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut input = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--input" => &mut input,
            _ => {
                return Err(Error::new(format!(
                    "unknown {command} argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let rpc_url = rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?;
    let origin = ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?;
    let input_path = absolute(input, "--input")?;
    let source = std::fs::read(input_path)?;
    let decoded: PlanInputV1 = serde_json::from_slice(&source)?;
    Ok((origin, source, decoded))
}

fn finalized_snapshot(
    origin: &ClusterOriginV1,
    selected: &SelectedInputV1,
) -> Result<FinalizedSnapshotV1> {
    let addresses = selected.addresses();
    let mut rpc = Rpc::connect_cluster(origin, WritePolicyV1::ReadsOnly)?;
    let floor = rpc.finalized_slot()?;
    let (slot, values) = rpc.finalized_accounts(&addresses, floor)?;
    FinalizedSnapshotV1::from_rpc(slot, rpc.block_time(slot)?, &addresses, values)
}

fn stdout_json(value: &impl Serialize) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap wallet-terminal-payout-alt-plan --rpc-url URL \\
     [--i-mean-devnet DEVNET_GENESIS_HASH] --input ABSOLUTE_JSON\n  \
     dclutch-local-successor-bootstrap wallet-terminal-payout-plan --rpc-url URL \\
     [--i-mean-devnet DEVNET_GENESIS_HASH] --input ABSOLUTE_JSON\n\nThese commands are read-only. \
     Each reauthenticates one exact Market, Product/composition graph, current \
     Claims/Core/Custody deployments, wallet Position, Custody and token prestates at one finalized \
     account observation. The first emits the owner-authorized create and ordered extensions for \
     this payout's canonical lookup table. After finalization, the second verifies that table and \
     emits the exact payout manifest the SDK and web app can execute. Mainnet-beta is refused \
     unconditionally."
}

pub(crate) fn build_report(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<WalletTerminalPayoutReportV3> {
    let rent_account = snapshot.required(sysvar::rent::ID, "Rent sysvar")?;
    let rent: Rent = bincode::deserialize(&rent_account.data)
        .map_err(|error| Error::new(format!("Rent sysvar: {error}")))?;
    let registry = snapshot.required(selected.registry, "Registry program")?;
    let claims = snapshot.required(selected.claims, "Claims program")?;
    let core_program = snapshot.required(selected.core, "Core program")?;
    let custody_program = snapshot.required(selected.custody, "Custody program")?;
    let cache = snapshot.required(selected.activation_cache, "activation cache")?;
    authenticate_role(
        registry,
        cache,
        selected.release_set,
        ExecutionRoleV1::Claims,
        claims,
        snapshot.required(selected.claims_programdata, "Claims ProgramData")?,
    )?;
    authenticate_role(
        registry,
        cache,
        selected.release_set,
        ExecutionRoleV1::Core,
        core_program,
        snapshot.required(selected.core_programdata, "Core ProgramData")?,
    )?;
    authenticate_role(
        registry,
        cache,
        selected.release_set,
        ExecutionRoleV1::Custody,
        custody_program,
        snapshot.required(selected.custody_programdata, "Custody ProgramData")?,
    )?;

    let admitted = authenticate_composition_v3(CompositionChainObservationV3 {
        registry_program: registry,
        claims_program: claims,
        product: ProductCompositionObservationV3 {
            product: snapshot.record(selected.product, &rent)?,
            result_domain: snapshot.record(selected.result_domain, &rent)?,
            portfolio: snapshot.record(selected.portfolio, &rent)?,
            product_basis: snapshot.record(selected.product_basis, &rent)?,
        },
        representation: RepresentationCompositionObservationV3 {
            execution_descriptor: snapshot.record(selected.execution_descriptor, &rent)?,
            descriptor: snapshot.record(selected.composition_descriptor, &rent)?,
            graph: snapshot.record(selected.composition_graph, &rent)?,
            translation: snapshot.record(selected.composition_translation, &rent)?,
            exposure: snapshot.record(selected.composition_exposure, &rent)?,
        },
    })
    .map_err(|error| Error::new(format!("Product/composition admission: {error:?}")))?;
    let realm_record = authenticate_record(selected.realm, snapshot, &rent, selected.registry)?;
    let realm = RealmV1::decode(&realm_record.data)
        .map_err(|error| Error::new(format!("Realm record: {error:?}")))?;
    if realm.token_program() != &selected.token_program.to_bytes()
        || realm.collateral_mint() != &selected.collateral_mint.to_bytes()
    {
        return Err(Error::new(
            "Realm selects another token program or collateral Mint",
        ));
    }
    let token_program = snapshot.required(selected.token_program, "Realm token program")?;
    let collateral_mint = snapshot.required(selected.collateral_mint, "Realm collateral Mint")?;
    if !token_program.executable
        || collateral_mint.executable
        || collateral_mint.owner != selected.token_program
    {
        return Err(Error::new(
            "Realm token program or collateral Mint has another account shape",
        ));
    }
    TokenProgram::parse(selected.token_program.to_bytes())
        .map_err(|error| Error::new(format!("Realm token program: {error:?}")))?;
    let mint = Mint::parse(&collateral_mint.data)
        .map_err(|error| Error::new(format!("Realm collateral Mint: {error:?}")))?;
    if !mint.is_initialized {
        return Err(Error::new("Realm collateral Mint is not initialized"));
    }

    let aggregate_account = snapshot.required(selected.aggregate, "Claims aggregate")?;
    let position_account = snapshot.required(selected.position, "Claims Position")?;
    let replay_account = snapshot.required(selected.custody_replay, "Claims Custody replay")?;
    let hoard_account = snapshot.required(selected.hoard, "Hoard token account")?;
    let recipient_account = snapshot.required(selected.recipient, "recipient token account")?;
    if aggregate_account.owner != selected.claims
        || aggregate_account.executable
        || position_account.owner != selected.claims
        || position_account.executable
        || replay_account.owner != selected.custody
        || replay_account.executable
        || hoard_account.owner != selected.token_program
        || hoard_account.executable
        || recipient_account.owner != selected.token_program
        || recipient_account.executable
    {
        return Err(Error::new(
            "Claims, Custody, Hoard, or recipient prestate has another owner or executable bit",
        ));
    }
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    let market_account = snapshot.required(selected.market, "Core Market")?;
    if market_account.owner != selected.core || market_account.data.len() != STATE_BYTES {
        return Err(Error::new("Core Market has another owner or width"));
    }
    let core = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    let expected_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        &selected.core,
    )
    .0;
    let product = admitted.product();
    let basis = admitted.product_basis();
    let exposure = admitted.exposure();
    let descriptor = admitted.composition().descriptor();
    let terminal_digest = core
        .terminal_receipt
        .ok_or_else(|| Error::new("Core Market has no terminal record"))?
        .to_bytes();
    if core.phase != CorePhase::Terminal
        || expected_market != selected.market
        || core.identity.market_id.to_bytes() != selected.market.to_bytes()
        || core.identity.registry_program.to_bytes() != selected.registry.to_bytes()
        || core.identity.selected_release_set.to_bytes() != selected.release_set
        || core.identity.realm_id.to_bytes() != selected.realm.digest
        || core.identity.product_record.to_bytes() != selected.product.digest
        || core.identity.product_id.to_bytes() != product.join.product_id.to_bytes()
        || terminal_digest != selected.terminal_record_digest
        || aggregate.logical_market != selected.market.to_bytes()
        || aggregate.release_set != selected.release_set
        || aggregate.realm_id != selected.realm.digest
        || aggregate.product_instance_id != product.join.product_id.to_bytes()
        || aggregate.basis_id != descriptor.native_basis()
        || aggregate.custody_context != selected.custody_context
        || exposure.market() != selected.market.to_bytes()
        || exposure.release_set() != selected.release_set
    {
        return Err(Error::new(
            "Core, Claims, Realm, Product, exposure, release, or Custody context did not join",
        ));
    }
    let terminal = terminal_scenario(
        selected,
        snapshot,
        &rent,
        core,
        basis.kind(),
        product.join.outcome_count,
    )?;
    let admission = ProductClaimsTerminalAdmissionV3::new(
        exposure.bundle_id(),
        exposure.bundle_digest(),
        basis.product_id(),
        basis.result_domain_id(),
        basis.coordinate_domain_id(),
        basis.result_unit_id(),
        exposure.representation_basis(),
        exposure.product_basis(),
        exposure.market(),
        exposure.release_set(),
        basis.evaluator_release_id(),
        exposure.representation_width(),
        basis.payout_scale(),
    )
    .map_err(|error| Error::new(format!("terminal admission: {error:?}")))?;
    let (terminal_coordinate_raw, terminal_coordinate_staging) = match terminal {
        TerminalScenarioV3::Rational { .. } => (
            selected.terminal_coordinate.raw,
            selected.terminal_coordinate.staging,
        ),
        TerminalScenarioV3::Categorical(_) | TerminalScenarioV3::Failure => {
            (sysvar::rent::ID, sysvar::rent::ID)
        }
    };
    let route = WalletTerminalPayoutRouteV3 {
        aggregate: selected.aggregate,
        linked_basis_raw: selected.product_basis.raw,
        linked_basis_staging: selected.product_basis.staging,
        product_raw: selected.product.raw,
        product_staging: selected.product.staging,
        result_domain_raw: selected.result_domain.raw,
        result_domain_staging: selected.result_domain.staging,
        portfolio_raw: selected.portfolio.raw,
        portfolio_staging: selected.portfolio.staging,
        market: selected.market,
        activation_cache: selected.activation_cache,
        registry_program: selected.registry,
        claims_program: selected.claims,
        claims_programdata: selected.claims_programdata,
        core_program: selected.core,
        core_programdata: selected.core_programdata,
        position: selected.position,
        exposure_raw: selected.composition_exposure.raw,
        exposure_staging: selected.composition_exposure.staging,
        custody_program: selected.custody,
        terminal_coordinate_raw,
        terminal_coordinate_staging,
        realm_raw: selected.realm.raw,
        realm_staging: selected.realm.staging,
        custody_replay: selected.custody_replay,
        collateral_mint: selected.collateral_mint,
        hoard: selected.hoard,
        recipient: selected.recipient,
        custody_authority: selected.custody_authority,
        token_program: selected.token_program,
    };
    let report = build_wallet_terminal_payout_v3(WalletTerminalPayoutInputV3 {
        observation: snapshot.observation,
        route,
        parent_context: selected.parent_context,
        terminal_record_digest: selected.terminal_record_digest,
        recipient_owner: selected.recipient_owner.to_bytes(),
        transfer_index: selected.transfer_index,
        admission,
        product_basis_bytes: &snapshot
            .required(selected.product_basis.raw, "ProductBasis")?
            .data,
        composition_exposure_bytes: &snapshot
            .required(selected.composition_exposure.raw, "CompositionExposure")?
            .data,
        composition_exposure_admission: RecordAdmissionV3 {
            selected_id: exposure.bundle_id(),
            finalized_id: exposure.bundle_id(),
            recomputed_digest: exposure.bundle_digest(),
            finalized_digest: exposure.bundle_digest(),
            record_authenticated: true,
        },
        product_record_digest: selected.product.digest,
        aggregate_bytes: &aggregate_account.data,
        position_bytes: &position_account.data,
        custody_replay_bytes: &replay_account.data,
        hoard_token_bytes: &hoard_account.data,
        recipient_token_bytes: &recipient_account.data,
        terminal,
        owner: selected.owner.to_bytes(),
        claim_index: selected.claim_index,
        quantity: selected.quantity,
        expected_generation: aggregate.generation,
        expected_market_revision: aggregate.revision,
        expected_position_revision: position_revision(&position_account.data)?,
    })
    .map_err(|error| Error::new(format!("wallet terminal payout builder: {error:?}")))?;
    Ok(report)
}

fn build_manifest(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<WalletTerminalPayoutManifestV3> {
    let report = build_report(selected, snapshot)?;
    let lookup_table = selected
        .lookup_table
        .ok_or_else(|| Error::new("lookupTable is required for a payout manifest"))?;
    let table = snapshot.required(lookup_table, "wallet payout lookup table")?;
    let transaction = compile_wallet_terminal_payout_v0(
        report,
        selected.owner,
        Hash::new_from_array(GEOMETRY_BLOCKHASH),
        table,
    )
    .map_err(|error| Error::new(format!("wallet terminal payout v0 compiler: {error:?}")))?;
    if transaction.required_signers.as_slice() != [selected.owner] {
        return Err(Error::new("wallet payout requires another signer"));
    }
    Ok(manifest(&transaction.payout, lookup_table, selected))
}

fn build_alt_plan(
    mut payout_input: PlanInputV1,
    source: &[u8],
    report: &WalletTerminalPayoutReportV3,
) -> Result<WalletTerminalPayoutAltPlanV1> {
    let addresses = canonical_wallet_terminal_payout_lookup_addresses_v3(report, report.owner)
        .map_err(|error| Error::new(format!("wallet payout ALT addresses: {error:?}")))?;
    let (create, lookup_table) =
        create_lookup_table(report.owner, report.owner, report.observation.slot);
    let extensions = addresses
        .chunks(EXTEND_ADDRESSES_PER_TRANSACTION_V1)
        .map(|page| {
            instruction_manifest(extend_lookup_table(
                lookup_table,
                report.owner,
                Some(report.owner),
                page.to_vec(),
            ))
        })
        .collect();
    payout_input.lookup_table = Some(lookup_table.to_string());
    Ok(WalletTerminalPayoutAltPlanV1 {
        format: ALT_OUTPUT_FORMAT,
        source_input_sha256: hex(&hash(source).to_bytes()),
        observation_slot: report.observation.slot.to_string(),
        payer: report.owner.to_string(),
        authority: report.owner.to_string(),
        lookup_table: lookup_table.to_string(),
        addresses: addresses
            .into_iter()
            .map(|address| address.to_string())
            .collect(),
        create: instruction_manifest(create),
        extensions,
        payout_input,
    })
}

fn instruction_manifest(instruction: Instruction) -> InstructionManifestV1 {
    InstructionManifestV1 {
        program_id: instruction.program_id.to_string(),
        accounts: instruction
            .accounts
            .into_iter()
            .map(|account| InstructionAccountV1 {
                address: account.pubkey.to_string(),
                signer: account.is_signer,
                writable: account.is_writable,
            })
            .collect(),
        data_base64: BASE64.encode(instruction.data),
    }
}

fn terminal_scenario(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    rent: &Rent,
    core: CoreState,
    kind: BasisKindV3,
    outcome_count: u32,
) -> Result<TerminalScenarioV3> {
    if core.terminal_winner >= outcome_count {
        return Err(Error::new(
            "Core terminal winner is outside the Product result domain",
        ));
    }
    match kind {
        BasisKindV3::CategoricalQ1 => Ok(TerminalScenarioV3::Categorical(core.terminal_winner)),
        BasisKindV3::GradedExactComplement => {
            if core.terminal_winner == outcome_count.saturating_sub(1) {
                return Ok(TerminalScenarioV3::Failure);
            }
            let record =
                authenticate_record(selected.terminal_coordinate, snapshot, rent, selected.core)?;
            if record.data.len() != TERMINAL_COORDINATE_BYTES_V2
                || record.data.get(..8) != Some(TERMINAL_COORDINATE_MAGIC_V2.as_slice())
                || u16_at(&record.data, 8)? != 2
                || record
                    .data
                    .get(10..16)
                    .is_none_or(|bytes| bytes.iter().any(|byte| *byte != 0))
                || record
                    .data
                    .get(28..32)
                    .is_none_or(|bytes| bytes.iter().any(|byte| *byte != 0))
            {
                return Err(Error::new("terminal coordinate has another canonical wire"));
            }
            let numerator = i64_at(&record.data, 16)?;
            let denominator = u32_at(&record.data, 24)?;
            if denominator == 0 {
                return Err(Error::new("terminal coordinate denominator is zero"));
            }
            Ok(TerminalScenarioV3::Rational {
                numerator: i128::from(numerator),
                denominator: u64::from(denominator),
            })
        }
    }
}

fn authenticate_record<'a>(
    pair: RecordPairV1,
    snapshot: &'a FinalizedSnapshotV1,
    rent: &Rent,
    owner: Pubkey,
) -> Result<&'a ObservedAccount> {
    let raw = snapshot.required(pair.raw, "finalized record")?;
    let staging = snapshot.account(pair.staging)?;
    if raw.owner != owner
        || raw.executable
        || raw.data.is_empty()
        || hash(&raw.data).to_bytes() != pair.digest
        || raw.lamports < rent.minimum_balance(raw.data.len())
        || staging.owner != system_program::ID
        || staging.executable
        || !staging.data.is_empty()
    {
        return Err(Error::new(
            "finalized record owner, PDA, digest, rent, or vacancy refused",
        ));
    }
    Ok(raw)
}

fn authenticate_role(
    registry: &ObservedAccount,
    cache: &ObservedAccount,
    release_set: [u8; 32],
    role: ExecutionRoleV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<()> {
    let mut registry_lamports = registry.lamports;
    let mut registry_data = registry.data.clone();
    let registry_info = AccountInfo::new(
        &registry.key,
        false,
        false,
        &mut registry_lamports,
        &mut registry_data,
        &registry.owner,
        registry.executable,
    );
    let mut cache_lamports = cache.lamports;
    let mut cache_data = cache.data.clone();
    let cache_info = AccountInfo::new(
        &cache.key,
        false,
        false,
        &mut cache_lamports,
        &mut cache_data,
        &cache.owner,
        cache.executable,
    );
    let mut program_lamports = program.lamports;
    let mut program_data = program.data.clone();
    let program_info = AccountInfo::new(
        &program.key,
        false,
        false,
        &mut program_lamports,
        &mut program_data,
        &program.owner,
        program.executable,
    );
    let mut programdata_lamports = programdata.lamports;
    let mut programdata_data = programdata.data.clone();
    let programdata_info = AccountInfo::new(
        &programdata.key,
        false,
        false,
        &mut programdata_lamports,
        &mut programdata_data,
        &programdata.owner,
        programdata.executable,
    );
    authenticate_activated_role_v1(
        &registry_info,
        &cache_info,
        &release_set,
        role,
        &program_info,
        &programdata_info,
    )
    .map_err(|error| Error::new(format!("current {role:?} deployment: {error:?}")))?;
    Ok(())
}

fn manifest(
    report: &WalletTerminalPayoutReportV3,
    lookup_table: Pubkey,
    selected: &SelectedInputV1,
) -> WalletTerminalPayoutManifestV3 {
    let route = report.route;
    let request = report.request.input();
    WalletTerminalPayoutManifestV3 {
        format: OUTPUT_FORMAT,
        route: ManifestRouteV3 {
            aggregate: route.aggregate.to_string(),
            linked_basis_raw: route.linked_basis_raw.to_string(),
            linked_basis_staging: route.linked_basis_staging.to_string(),
            product_raw: route.product_raw.to_string(),
            product_staging: route.product_staging.to_string(),
            result_domain_raw: route.result_domain_raw.to_string(),
            result_domain_staging: route.result_domain_staging.to_string(),
            portfolio_raw: route.portfolio_raw.to_string(),
            portfolio_staging: route.portfolio_staging.to_string(),
            market: route.market.to_string(),
            activation_cache: route.activation_cache.to_string(),
            registry_program: route.registry_program.to_string(),
            claims_program: route.claims_program.to_string(),
            claims_program_data: route.claims_programdata.to_string(),
            core_program: route.core_program.to_string(),
            core_program_data: route.core_programdata.to_string(),
            position: route.position.to_string(),
            exposure_raw: route.exposure_raw.to_string(),
            exposure_staging: route.exposure_staging.to_string(),
            custody_program: route.custody_program.to_string(),
            terminal_coordinate_raw: route.terminal_coordinate_raw.to_string(),
            terminal_coordinate_staging: route.terminal_coordinate_staging.to_string(),
            realm_raw: route.realm_raw.to_string(),
            realm_staging: route.realm_staging.to_string(),
            custody_replay: route.custody_replay.to_string(),
            collateral_mint: route.collateral_mint.to_string(),
            hoard: route.hoard.to_string(),
            recipient: route.recipient.to_string(),
            custody_authority: route.custody_authority.to_string(),
            token_program: route.token_program.to_string(),
        },
        custody_context: hex(&selected.custody_context),
        request: ManifestRequestV3 {
            release_set: hex(&request.release_set),
            market: Pubkey::new_from_array(request.market).to_string(),
            realm: hex(&request.realm),
            parent_context: hex(&request.parent_context),
            product_record_digest: hex(&request.product_record_digest),
            exposure_id: hex(&request.exposure_id),
            exposure_digest: hex(&request.exposure_digest),
            terminal_record_digest: hex(&request.terminal_record_digest),
            owner: Pubkey::new_from_array(request.owner).to_string(),
            position: Pubkey::new_from_array(request.position).to_string(),
            recipient_owner: Pubkey::new_from_array(request.recipient_owner).to_string(),
            recipient: Pubkey::new_from_array(request.recipient_token_account).to_string(),
            claims_program: Pubkey::new_from_array(request.claims_program).to_string(),
            custody_program: Pubkey::new_from_array(request.custody_program).to_string(),
            collateral_mint: Pubkey::new_from_array(request.collateral_mint).to_string(),
            token_program: Pubkey::new_from_array(request.token_program).to_string(),
            semantic_basis_id: hex(&request.semantic_basis_id),
            linked_basis_record_digest: hex(&request.linked_basis_record_digest),
            generation: request.generation.to_string(),
            expected_market_revision: request.expected_market_revision.to_string(),
            expected_position_revision: request.expected_position_revision.to_string(),
            expected_custody_revision: request.expected_custody_revision.to_string(),
            quantity: request.quantity.to_string(),
            claim_index: request.claim_index,
            transfer_index: request.transfer_index,
        },
        signed_packet_base64: BASE64.encode(&report.signed_packet),
        payout: report.payout.to_string(),
        lookup_table: lookup_table.to_string(),
    }
}

fn record_pair(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> RecordPairV1 {
    RecordPairV1 {
        schema,
        digest,
        raw: Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
            &registry,
        )
        .0,
        staging: Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                schema.as_slice(),
                digest.as_slice(),
            ],
            &registry,
        )
        .0,
    }
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn canonical_u64(value: &str, label: &str, nonzero: bool) -> Result<u64> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::new(format!(
            "{label} must be canonical decimal u64 text"
        )));
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|_| Error::new(format!("{label} is outside u64")))?;
    if nonzero && parsed == 0 {
        return Err(Error::new(format!("{label} must be positive")));
    }
    Ok(parsed)
}

fn optional_lookup_table<'de, D>(deserializer: D) -> core::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    String::deserialize(deserializer).map(Some)
}

fn nonzero_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    let parsed = pubkey(value).map_err(|error| Error::new(format!("{label}: {error}")))?;
    if parsed == Pubkey::default() {
        return Err(Error::new(format!("{label} is the zero identity")));
    }
    Ok(parsed)
}

fn nonzero_hex(value: &str, label: &str) -> Result<[u8; 32]> {
    let parsed = hex32(value).map_err(|error| Error::new(format!("{label}: {error}")))?;
    if parsed == [0; 32] {
        return Err(Error::new(format!("{label} is the zero identity")));
    }
    Ok(parsed)
}

fn absolute(value: Option<String>, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value.ok_or_else(|| Error::new(format!("{label} is required")))?);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

fn position_revision(bytes: &[u8]) -> Result<u64> {
    u64_at(
        bytes,
        dclutch_claims_svm::liability_basis_state_v2::LiabilityBasisPositionLayoutV2::REVISION,
    )
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array_at(bytes, offset)?))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array_at(bytes, offset)?))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array_at(bytes, offset)?))
}

fn i64_at(bytes: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(array_at(bytes, offset)?))
}

fn array_at<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(offset..offset.saturating_add(N))
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| Error::new("wallet payout byte coordinate is outside its account"))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::borrow::Cow;

    use dclutch_claims_svm::{
        CallerRole,
        terminal_settlement_v3::{TerminalSettlementRequestInputV3, TerminalSettlementRequestV3},
    };
    use dclutch_operator::wallet_terminal_payout_v3::{
        WalletTerminalPayoutErrorV3, canonical_wallet_terminal_payout_lookup_addresses_v3,
    };
    use sha2::{Digest as _, Sha256};
    use solana_address_lookup_table_interface::{
        program as lookup_table_program,
        state::{AddressLookupTable, LookupTableMeta},
    };
    use solana_program::instruction::{AccountMeta, Instruction};

    use super::*;

    fn key(byte: u8) -> String {
        Pubkey::new_from_array([byte; 32]).to_string()
    }

    fn id(byte: u8) -> String {
        hex(&[byte; 32])
    }

    pub(crate) fn input() -> PlanInputV1 {
        PlanInputV1 {
            format: INPUT_FORMAT.into(),
            market: key(1),
            owner: key(2),
            recipient_owner: key(2),
            recipient: key(3),
            collateral_mint: key(4),
            token_program: key(5),
            quantity: "7".into(),
            claim_index: 1,
            transfer_index: 0,
            parent_context: id(6),
            custody_context: id(7),
            release_set: id(8),
            lookup_table: Some(key(9)),
            programs: ProgramSelectorsV1 {
                registry: key(10),
                core: key(11),
                claims: key(12),
                custody: key(13),
            },
            records: RecordSelectorsV1 {
                realm: id(20),
                product: id(21),
                result_domain: id(22),
                portfolio: id(23),
                product_basis: id(24),
                execution_descriptor: id(25),
                composition_descriptor: id(26),
                composition_graph: id(27),
                composition_translation: id(28),
                composition_exposure: id(29),
                terminal_record: id(30),
            },
        }
    }

    #[test]
    fn input_is_exact_and_derivations_are_stable() {
        let value = input();
        let selected = SelectedInputV1::parse(&value, LookupTableRequirementV1::Present)
            .expect("selected input");
        assert_eq!(selected.quantity, 7);
        assert_eq!(selected.position, programdata_free_position(&selected));
        assert_eq!(
            selected.activation_cache,
            activation_cache_address_v1(&selected.registry, &selected.release_set)
        );
        assert!(
            selected
                .addresses()
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
    }

    fn programdata_free_position(selected: &SelectedInputV1) -> Pubkey {
        Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(selected.aggregate.to_bytes(), selected.owner.to_bytes())
                .expect("position seeds")
                .as_slices(),
            &selected.claims,
        )
        .0
    }

    fn payout_report() -> WalletTerminalPayoutReportV3 {
        let owner = Pubkey::new_from_array([90; 32]);
        let route = WalletTerminalPayoutRouteV3 {
            aggregate: Pubkey::new_from_array([1; 32]),
            linked_basis_raw: Pubkey::new_from_array([2; 32]),
            linked_basis_staging: Pubkey::new_from_array([3; 32]),
            product_raw: Pubkey::new_from_array([4; 32]),
            product_staging: Pubkey::new_from_array([5; 32]),
            result_domain_raw: Pubkey::new_from_array([6; 32]),
            result_domain_staging: Pubkey::new_from_array([7; 32]),
            portfolio_raw: Pubkey::new_from_array([8; 32]),
            portfolio_staging: Pubkey::new_from_array([9; 32]),
            market: Pubkey::new_from_array([10; 32]),
            activation_cache: Pubkey::new_from_array([11; 32]),
            registry_program: Pubkey::new_from_array([12; 32]),
            claims_program: Pubkey::new_from_array([13; 32]),
            claims_programdata: Pubkey::new_from_array([14; 32]),
            core_program: Pubkey::new_from_array([15; 32]),
            core_programdata: Pubkey::new_from_array([16; 32]),
            position: Pubkey::new_from_array([17; 32]),
            exposure_raw: Pubkey::new_from_array([18; 32]),
            exposure_staging: Pubkey::new_from_array([19; 32]),
            custody_program: Pubkey::new_from_array([20; 32]),
            terminal_coordinate_raw: sysvar::rent::ID,
            terminal_coordinate_staging: sysvar::rent::ID,
            realm_raw: Pubkey::new_from_array([21; 32]),
            realm_staging: Pubkey::new_from_array([22; 32]),
            custody_replay: Pubkey::new_from_array([23; 32]),
            collateral_mint: Pubkey::new_from_array([24; 32]),
            hoard: Pubkey::new_from_array([25; 32]),
            recipient: Pubkey::new_from_array([26; 32]),
            custody_authority: Pubkey::new_from_array([27; 32]),
            token_program: Pubkey::new_from_array([28; 32]),
        };
        let request = TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
            caller_role: CallerRole::Claims,
            release_set: [31; 32],
            market: route.market.to_bytes(),
            realm: [32; 32],
            parent_context: [33; 32],
            product_record_digest: [34; 32],
            exposure_id: [35; 32],
            exposure_digest: [36; 32],
            terminal_record_digest: [37; 32],
            owner: owner.to_bytes(),
            position: route.position.to_bytes(),
            recipient_owner: [91; 32],
            recipient_token_account: route.recipient.to_bytes(),
            claims_program: route.claims_program.to_bytes(),
            custody_program: route.custody_program.to_bytes(),
            collateral_mint: route.collateral_mint.to_bytes(),
            token_program: route.token_program.to_bytes(),
            semantic_basis_id: [38; 32],
            linked_basis_record_digest: [39; 32],
            generation: 2,
            expected_market_revision: 3,
            expected_position_revision: 4,
            expected_custody_revision: 5,
            quantity: 7,
            claim_index: 1,
            transfer_index: 0,
        })
        .expect("request");
        let mut accounts = vec![AccountMeta::new_readonly(owner, true)];
        accounts.extend((40_u8..75).map(|byte| {
            let key = Pubkey::new_from_array([byte; 32]);
            if byte % 3 == 0 {
                AccountMeta::new(key, false)
            } else {
                AccountMeta::new_readonly(key, false)
            }
        }));
        WalletTerminalPayoutReportV3 {
            instruction: Instruction {
                program_id: route.claims_program,
                accounts,
                data: request.to_bytes().to_vec(),
            },
            observation: Observation {
                slot: 44,
                unix_timestamp: 1_800_000_000,
                finality: Finality::Finalized,
            },
            request,
            request_digest: [40; 32],
            signed_packet: vec![1, 2, 3, 4, 5],
            signed_packet_digest: [41; 32],
            signed_table_digest: [42; 32],
            payout: 11,
            custody_caller: Pubkey::new_from_array([76; 32]),
            custody_request_digest: [43; 32],
            owner,
            route,
            pre_aggregate_bytes: vec![1],
            pre_position_bytes: vec![2],
            pre_custody_replay_bytes: vec![3],
            pre_hoard_token_bytes: vec![4],
            pre_recipient_token_bytes: vec![5],
        }
    }

    fn lookup(
        report: &WalletTerminalPayoutReportV3,
        substituted: bool,
        slot: u64,
    ) -> ObservedAccount {
        let mut addresses =
            canonical_wallet_terminal_payout_lookup_addresses_v3(report, report.owner)
                .expect("canonical lookup addresses");
        if substituted {
            addresses[0] = Pubkey::new_from_array([201; 32]);
        }
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(Pubkey::new_from_array([202; 32])),
                deactivation_slot: u64::MAX,
                last_extended_slot: slot - 1,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        ObservedAccount {
            observation: Observation {
                slot,
                unix_timestamp: 1_800_000_000,
                finality: Finality::Finalized,
            },
            key: Pubkey::new_from_array([203; 32]),
            owner: lookup_table_program::id(),
            lamports: 1,
            executable: false,
            data: table.serialize_for_tests().expect("lookup table bytes"),
        }
    }

    #[test]
    fn manifest_json_has_one_stable_golden_vector() {
        let report = payout_report();
        let value = input();
        let selected = SelectedInputV1::parse(&value, LookupTableRequirementV1::Present)
            .expect("selected input");
        let lookup_table = selected.lookup_table.expect("lookup table");
        let encoded =
            serde_json::to_vec(&manifest(&report, lookup_table, &selected)).expect("manifest JSON");
        assert_eq!(
            Sha256::digest(&encoded).as_slice(),
            &[
                52, 169, 11, 85, 125, 206, 162, 246, 75, 76, 228, 55, 122, 166, 101, 116, 92, 238,
                121, 134, 109, 248, 211, 143, 111, 129, 80, 254, 88, 13, 28, 250,
            ],
            "update only after inspecting the exact seven-field JSON vector",
        );
        let value: serde_json::Value = serde_json::from_slice(&encoded).expect("manifest value");
        assert_eq!(value.as_object().expect("object").len(), 7);
        assert_eq!(value["format"], OUTPUT_FORMAT);
        assert_eq!(value["signedPacketBase64"], "AQIDBAU=");
    }

    #[test]
    fn alt_plan_preserves_first_use_order_and_patches_only_lookup_table() {
        let mut report = payout_report();
        report.instruction.accounts.swap(1, 2);
        let mut payout_input = input();
        payout_input.lookup_table = None;
        payout_input.owner = report.owner.to_string();
        payout_input.recipient_owner = report.owner.to_string();
        let source = serde_json::to_vec(&payout_input).expect("source input");
        let expected = canonical_wallet_terminal_payout_lookup_addresses_v3(&report, report.owner)
            .expect("canonical addresses");
        let mut sorted = expected.clone();
        sorted.sort_unstable();
        assert_ne!(
            expected, sorted,
            "fixture must detect a sorting substitution"
        );

        let plan = build_alt_plan(payout_input, &source, &report).expect("ALT plan");
        assert_eq!(plan.format, ALT_OUTPUT_FORMAT);
        assert_eq!(plan.source_input_sha256, hex(&hash(&source).to_bytes()));
        assert_eq!(
            plan.addresses,
            expected.iter().map(ToString::to_string).collect::<Vec<_>>()
        );
        assert_eq!(plan.extensions.len(), 2);
        assert_eq!(
            plan.create.program_id,
            lookup_table_program::id().to_string()
        );
        assert_eq!(
            plan.payout_input.lookup_table.as_deref(),
            Some(plan.lookup_table.as_str())
        );
        let encoded = serde_json::to_vec(&plan).expect("ALT plan JSON");
        assert_eq!(
            Sha256::digest(&encoded).as_slice(),
            &[
                163, 28, 105, 100, 50, 126, 24, 26, 198, 83, 240, 15, 112, 202, 43, 188, 166, 245,
                230, 199, 91, 194, 19, 118, 226, 104, 103, 22, 233, 224, 148, 142,
            ],
            "update only after inspecting the exact ordered ALT-plan JSON vector",
        );
    }

    #[test]
    fn hostile_zero_noncanonical_and_snapshot_slot_refuse() {
        let mut value = input();
        value.quantity = "07".into();
        assert!(SelectedInputV1::parse(&value, LookupTableRequirementV1::Present).is_err());
        let mut value = input();
        value.records.product = "00".repeat(32);
        assert!(SelectedInputV1::parse(&value, LookupTableRequirementV1::Present).is_err());
        let mut value = input();
        value.programs.claims = Pubkey::default().to_string();
        assert!(SelectedInputV1::parse(&value, LookupTableRequirementV1::Present).is_err());
        let mut value = input();
        value.recipient_owner = key(99);
        assert!(SelectedInputV1::parse(&value, LookupTableRequirementV1::Present).is_err());
        let mut value = input();
        value.transfer_index = 1;
        assert!(SelectedInputV1::parse(&value, LookupTableRequirementV1::Present).is_err());
        let mut value = input();
        value.lookup_table = None;
        assert!(SelectedInputV1::parse(&value, LookupTableRequirementV1::Present).is_err());
        assert!(SelectedInputV1::parse(&value, LookupTableRequirementV1::Absent).is_ok());
        let mut json = serde_json::to_value(input()).expect("input JSON");
        json["lookupTable"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<PlanInputV1>(json).is_err());
        assert!(FinalizedSnapshotV1::from_rpc(0, 1, &[], Vec::new()).is_err());
    }

    #[test]
    fn substituted_record_slot_and_alt_are_named_refusals() {
        let value = input();
        let selected = SelectedInputV1::parse(&value, LookupTableRequirementV1::Present)
            .expect("selected input");
        let observation = Observation {
            slot: 44,
            unix_timestamp: 1,
            finality: Finality::Finalized,
        };
        let raw = ObservedAccount {
            observation,
            key: selected.realm.raw,
            owner: selected.registry,
            lamports: u64::MAX,
            executable: false,
            data: vec![1; 32],
        };
        let staging = ObservedAccount {
            observation,
            key: selected.realm.staging,
            owner: system_program::ID,
            lamports: 0,
            executable: false,
            data: Vec::new(),
        };
        let snapshot = FinalizedSnapshotV1 {
            observation,
            accounts: BTreeMap::from([(raw.key, raw), (staging.key, staging)]),
        };
        assert!(
            authenticate_record(
                selected.realm,
                &snapshot,
                &Rent::default(),
                selected.registry
            )
            .is_err()
        );

        let report = payout_report();
        let canonical = lookup(&report, false, report.observation.slot);
        assert!(
            compile_wallet_terminal_payout_v0(
                report.clone(),
                report.owner,
                Hash::new_from_array(GEOMETRY_BLOCKHASH),
                &canonical,
            )
            .is_ok()
        );
        let substituted = lookup(&report, true, report.observation.slot);
        assert_eq!(
            compile_wallet_terminal_payout_v0(
                report.clone(),
                report.owner,
                Hash::new_from_array(GEOMETRY_BLOCKHASH),
                &substituted,
            ),
            Err(WalletTerminalPayoutErrorV3::LookupTable),
        );
        let stale = lookup(&report, false, report.observation.slot - 1);
        assert_eq!(
            compile_wallet_terminal_payout_v0(
                report.clone(),
                report.owner,
                Hash::new_from_array(GEOMETRY_BLOCKHASH),
                &stale,
            ),
            Err(WalletTerminalPayoutErrorV3::LookupTable),
        );
    }
}
