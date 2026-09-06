//! Ordered, crash-safe routing for the exterior terminal sequence.
//!
//! This module deliberately does not own any protocol encoding.  Stage
//! producers pass it observations that their existing semantic owners have
//! already authenticated.  It admits exactly one next mutation and refuses
//! every skipped, substituted, or partially committed lifecycle shape.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    str::FromStr as _,
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_claims::liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2;
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyVaultSeedsV1,
};
use dclutch_market::capability_manifest::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, funded_rent_minimum_v2,
};
use dclutch_market::capability_program::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
};
use dclutch_market::realm::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_market::rent::lifecycle_v2::{
    LifecycleAccountIdV2, LifecycleRentCoreCloseAuthoritySeedsV2, LifecycleRentCreditV2,
};
use dclutch_market::{
    Action, Admission, Binding, CoreState, Identity, Phase, Readiness, ReleaseReceipt, ReleaseSet,
    Request, Role, begin_retiring,
};
use dclutch_market_retirement_v1_operator::{
    MarketRetirementSnapshotV1, build_market_retirement_v1,
    terminal_stage_order_v1::{
        TerminalStageOrderErrorV1, TerminalStageV1, authenticate_terminal_stage_prefix_v1,
    },
};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    direct_begin_retiring_v1::{
        DirectBeginRetiringCoordinateInputV1, DirectBeginRetiringMetaClassV1,
        DirectBeginRetiringPlanV1, DirectBeginRetiringSnapshotV1,
        derive_direct_begin_retiring_meta_closure_v1, plan_direct_begin_retiring_v1,
    },
    resolution_core_v3::{ResolutionCloseFundSnapshotV3, build_resolution_direct_close_fund_v1},
    terminal_retirement_v1::{
        DIRECT_NATIVE_CLOSE_FUNDING_LEDGERS_V1, DirectNativeCloseCoordinateInputV1,
        DirectNativeCloseSnapshotV1, RetirementReplayHandoffCoordinateInputV1,
        RetirementReplayHandoffSnapshotV1, TerminalDeploymentCoordinatesV1, TerminalMetaClassV1,
        TerminalRecordCoordinatesV1, build_direct_native_close_v1,
        build_retirement_replay_handoff_v1, preflight_direct_native_close_caller_v1,
        preflight_retirement_replay_handoff_caller_v1,
        project_direct_native_close_coordinate_closure_v1,
        project_retirement_replay_handoff_coordinate_closure_v1,
    },
};
use dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_registry::svm::continuation_v1::{
    RegistryContinuationAdmissionSeedsV1, RegistryContinuationRequestV1,
};
use dclutch_resolution_core_v3_operator::funded_rent_recovery_v1::{
    FundedRentReadingV2, recover_funded_rent_rate_v2,
};
use dclutch_source::relay::SOLANA_DEVNET_GENESIS_HASH_V1;
use dclutch_source::resolution::{
    SOURCE_CLOSURE_RECEIPT_BYTES_V3, SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3, SourceClosureReceiptV3,
};
use dclutch_source::{
    RECOVERY_POLICY_SCHEMA_ID_V2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceResolutionPhaseV1, SourceResolutionStateV2,
};
use dclutch_trading::{
    native_close_bundle_v1::{
        direct_native_close_account_profile_schema_v1, direct_native_close_effect_schema_v1,
    },
    ordinary_bundle_v4::DirectInlineOrdinaryHotBundleV4,
    retirement_v1::{DirectBeginRetiringRequestV1, direct_begin_retiring_context_v1},
    successor::DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
};
use dclutch_versioned_message_operator::{
    EXTEND_ADDRESSES_PER_TRANSACTION_V1, LookupTableCreationPlanV1, PACKET_DATA_BYTES,
    VersionedMessagePlanV0, build_lookup_table_creation_v1, build_lookup_table_freeze,
    compile_v0_message_with_optional_tables,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, SeqAccess, Visitor},
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_address_lookup_table_interface::{
    program as lookup_table_program,
    state::{AddressLookupTable, LOOKUP_TABLE_META_SIZE, LookupTableMeta},
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk::{
    hash::Hash,
    message::VersionedMessage,
    signature::{Keypair, Signature, Signer},
    transaction::VersionedTransaction,
};

use crate::{
    Error, Result,
    campaign::{
        self, CampaignTerminalEvidenceV1, parse_campaign_terminal_evidence_v1,
        parse_campaign_terminal_evidence_with_expected_cluster_v1,
    },
    closure_receipt_projection::{
        ClosureRentPartitionV1, DeployedClosureRentRuleV1, deployed_closure_rent_rule_v1,
        project_closure_rent_partition_v1,
    },
    cluster::{
        ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH, ExpectedClusterV1,
    },
    model::{MarketRunInput, SuccessorPlan},
    plan::{hex32, pubkey},
    rpc::{Rpc, RpcAccount, WritePolicyV1},
    runtime::decode_hex,
    terminal_lifecycle::{
        authenticate_campaign_market_v1, authenticate_plan_source, authenticate_zero_claims,
        decode_routed_market, finalized_snapshot, require_direct_retirement_evidence,
        required_account, routed_record,
    },
    wallet_terminal::{FinalizedSnapshotV1, authenticate_role},
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_sdk_ids::{compute_budget, system_program, sysvar};

const ALT_ADDRESS_BYTES: usize = 32;
const ALT_GEOMETRY_BLOCKHASH: [u8; 32] = [0x5a; 32];
const TERMINAL_JOURNAL_SCHEMA_V1: &str = "dclutch-devnet-terminal-sequence-journal-v1";
/// A terminal session's schema, bumped to v2 on 2026-09-04 when the session
/// began recording the rent rate it was funded at, and to v3 the same day when
/// it began recording the ComputeBudget limits its driver declares.
///
/// A v1 session carries no `fundedRentRate`, and every guard in this file now
/// prices against that field. Reading a v1 session under v2 code is refused at
/// deserialization rather than defaulted to zero -- a zero rate prices every
/// account at nothing, which is exactly the silent success this tree treats as
/// the worst failure mode. There is no migration and no parallel path: a v1
/// session belongs to cohort-15, whose programs cohort-16 abandons in place.
///
/// A v2 session carries no `declaredComputeUnitLimits`, and the same argument
/// applies one level up: a defaulted-empty table says "every route in this
/// sequence fits the 200,000-CU default meter", which is the claim
/// `ResolutionCloseFund` refuted on chain at 252,368 CU.
const TERMINAL_SESSION_SCHEMA_V1: &str = "dclutch-devnet-terminal-sequence-session-v3";
const OWNED_LOOPBACK_TERMINAL_JOURNAL_SCHEMA_V1: &str =
    "dclutch-owned-loopback-terminal-sequence-journal-v1";
const OWNED_LOOPBACK_TERMINAL_SESSION_SCHEMA_V1: &str =
    "dclutch-owned-loopback-terminal-sequence-session-v3";
const TERMINAL_COMPLETION_SCHEMA_V1: &str = "dclutch-devnet-terminal-sequence-completion-v1";
const OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA_V1: &str =
    "dclutch-owned-loopback-terminal-sequence-completion-v1";
const TERMINAL_FINALITY_WAIT: Duration = Duration::from_secs(300);

/// The widest gap this sequence certifies between the Clock a stage PLANNED
/// against and the Clock its EXECUTION wrote into a receipt.
///
/// Derived, not chosen, and derived from the sequence's own deadline. Two
/// independent facts bound how late a planned packet's execution can be, and
/// this is the looser of them, so it refuses nothing the other admits: the
/// packet's blockhash is fetched at plan time and dies 150 blocks later, about
/// 60 seconds at devnet's cadence; and this driver stops waiting for its own
/// signature after `TERMINAL_FINALITY_WAIT`, so an execution later than that is
/// not one this pass can be certifying. Measured on devnet 2026-09-04: market
/// 1's first `ResolutionCloseFund` planned at 1,788,522,293 and executed at
/// 1,788,522,302 -- nine seconds, 3% of this interval.
const TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS: u64 = TERMINAL_FINALITY_WAIT.as_secs();

const fn terminal_journal_schema_v1(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => TERMINAL_JOURNAL_SCHEMA_V1,
        ExpectedClusterV1::OwnedLoopback => OWNED_LOOPBACK_TERMINAL_JOURNAL_SCHEMA_V1,
    }
}

const fn terminal_session_schema_v1(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => TERMINAL_SESSION_SCHEMA_V1,
        ExpectedClusterV1::OwnedLoopback => OWNED_LOOPBACK_TERMINAL_SESSION_SCHEMA_V1,
    }
}

const fn terminal_completion_schema_v1(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => TERMINAL_COMPLETION_SCHEMA_V1,
        ExpectedClusterV1::OwnedLoopback => OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA_V1,
    }
}

fn terminal_journal_cluster_v1(journal: &DurableTerminalJournalV1) -> Result<ExpectedClusterV1> {
    match (journal.schema.as_str(), journal.cluster.as_str()) {
        (TERMINAL_JOURNAL_SCHEMA_V1, "devnet") => Ok(ExpectedClusterV1::Devnet),
        (OWNED_LOOPBACK_TERMINAL_JOURNAL_SCHEMA_V1, "owned-loopback") => {
            Ok(ExpectedClusterV1::OwnedLoopback)
        }
        _ => Err(refusal(
            "terminal journal schema and cluster provenance were not one exact admitted pair",
        )),
    }
}

fn terminal_session_cluster_v1(session: &TerminalSequenceSessionV1) -> Result<ExpectedClusterV1> {
    match (
        session.schema.as_str(),
        session.devnet_genesis_hash.as_deref(),
        session.owned_loopback_genesis_hash.as_deref(),
    ) {
        (TERMINAL_SESSION_SCHEMA_V1, Some(DEVNET_GENESIS_HASH), None) => {
            Ok(ExpectedClusterV1::Devnet)
        }
        (OWNED_LOOPBACK_TERMINAL_SESSION_SCHEMA_V1, None, Some(genesis))
            if !genesis.is_empty() && genesis != Hash::default().to_string() =>
        {
            Ok(ExpectedClusterV1::OwnedLoopback)
        }
        _ => Err(refusal(
            "terminal session schema and cluster-genesis provenance were not one exact admitted pair",
        )),
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TerminalSequenceSessionV1 {
    schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    devnet_genesis_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owned_loopback_genesis_hash: Option<String>,
    rpc_url: String,
    plan_sha256: String,
    market_input_sha256: String,
    evidence_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refreshed_evidence_sha256: Option<String>,
    market: String,
    payer: String,
    source_receipt: String,
    receipt_initial_lamports: u64,
    receipt_rent_lamports: u64,
    /// The exemption-scaled rent rate this session's cluster charged when the
    /// session was opened -- `lamports_per_byte_year * exemption_threshold`.
    ///
    /// A rent-exempt minimum is a CLUSTER parameter, and devnet moved this one
    /// from 6,333 to 5,080 at the epoch-1141 boundary in the middle of
    /// cohort-15's terminal sequence. Every account this session prepaid or
    /// created was priced at the rate recorded here, and every later check
    /// prices them the same way: `(128 + len) * rate`. Re-deriving from the
    /// sysvar of the moment is what refused a correctly prepaid seat with no
    /// word for why.
    funded_rent_rate: u32,
    /// The ComputeBudget limits this session's driver declares, in stage order.
    ///
    /// It is a RECORD, not a second authority: it is checked equal to
    /// `declared_terminal_compute_budgets_v1()` on every load, so a driver
    /// rebuilt mid-sequence with a re-pinned budget refuses here by name
    /// instead of planning its next stage against a number the stages already
    /// signed never saw.
    declared_compute_unit_limits: Vec<TerminalStageComputeBudgetV1>,
    supplied_lookup_table: bool,
    lookup_table: String,
    lookup_recent_slot: u64,
    lookup_addresses: Vec<String>,
    lookup_addresses_sha256: String,
    session_sha256: String,
}

struct TerminalSequenceArgumentsV1 {
    expected_cluster: ExpectedClusterV1,
    origin: ClusterOriginV1,
    plan: PathBuf,
    market_input: PathBuf,
    evidence: PathBuf,
    /// The optional post-founding evidence refresh
    /// (`docs/design/EVIDENCE_REFRESH_V1.md`). Absent, this command behaves
    /// byte-for-byte as it did before the refresh existed.
    refreshed_evidence: Option<PathBuf>,
    market: Pubkey,
    payer: Pubkey,
    payer_keypair: PathBuf,
    session: PathBuf,
    journal_dir: PathBuf,
    completion: PathBuf,
    supplied_lookup_table: Option<Pubkey>,
    /// Retire one ambiguous submission that can never be included, naming its
    /// exact signature. It is deliberately not part of the completion's
    /// invocation record: the act is durable in the retired journal itself,
    /// and a later pass that did not repeat the flag must still reproduce the
    /// same completion.
    supersede_unlandable: Option<String>,
    reconcile_landed: Option<String>,
    execute: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TerminalCompletionInvocationV1 {
    command: String,
    rpc_url: String,
    plan_path: String,
    market_input_path: String,
    evidence_path: String,
    market: String,
    fee_payer: String,
    fee_payer_keypair_path: String,
    session_path: String,
    journal_directory: String,
    completion_path: String,
    supplied_lookup_table: Option<String>,
    execute: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TerminalCompletionSessionV1 {
    path: String,
    sha256: String,
    schema: String,
    session_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TerminalCompletionJournalV1 {
    path: String,
    sha256: String,
    schema: String,
    mutation: TerminalCompletionMutationV1,
    phase: StageJournalPhaseV1,
    fee_payer: String,
    signature: String,
    finalized_slot: String,
    compute_units_consumed: String,
    transaction_fee_lamports: String,
    protocol_lamport_deltas: Vec<TerminalCompletionLamportDeltaV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case", tag = "kind")]
enum TerminalCompletionMutationV1 {
    LookupCreate,
    LookupExtend {
        #[serde(rename = "prefixLen")]
        prefix_len: String,
    },
    LookupFreeze,
    ResolutionReceiptPrepay,
    CoreBeginRetiring,
    DirectBeginRetiring,
    ResolutionCloseFund,
    DirectCloseCapability,
    RetirementReplayHandoff,
    AggregateRetirement,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TerminalCompletionLamportDeltaV1 {
    account_address: String,
    delta_lamports: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TerminalSequenceCompletionV1 {
    schema: String,
    status: String,
    cluster: String,
    genesis_hash: String,
    invocation: TerminalCompletionInvocationV1,
    session: TerminalCompletionSessionV1,
    journal_directory: String,
    market: String,
    payer: String,
    lookup_table: String,
    journals: Vec<TerminalCompletionJournalV1>,
    finalized_slot: String,
    transaction_fees_lamports: String,
    compute_units_consumed: String,
}

// The six protocol mutations and their sole admissible order are DECLARED in
// `dclutch_market_retirement_v1_operator::terminal_stage_order_v1` -- the
// operator crate this driver and the `dclutch-svm-harness` retirement campaign
// both link -- and imported above. This file used to declare them itself, which
// is how the order came to be wrong here for three cohorts with nothing in the
// tree able to disagree: the only other consumer of a stage order was this
// file's own tests. `ORDERED` now says `DirectCloseCapability` before
// `ResolutionCloseFund`, and the declaration holds that ruling with a
// `const _: () = assert!(..)` naming why.

/// `ResolutionCloseFund`'s declared ComputeBudget limit.
///
/// Solana meters a transaction that declares nothing at 200,000 compute units.
/// Every other message this sequence sends fits inside that, and each has
/// landed on devnet to say so -- ALT create 10,508, ALT freeze 1,517,
/// `CoreBeginRetiring` 23,106, `DirectBeginRetiring` 92,137, the Resolution
/// receipt prepay 150. `ResolutionCloseFund` does not: on 2026-09-04 it
/// consumed 200,000 of 200,000 and failed at `exceeded CUs meter at BPF
/// instruction`, which is how market 1's retirement stopped and why no
/// retirement had completed on any chain.
///
/// The number is derived, not chosen, under `tools/gauntlet/CU_BUDGETS.md`'s
/// rule -- `tolerance = roundup(band, 10_000) + 10_000`, floor 15,000;
/// `budget = measured + tolerance`; `measured` is the highest draw, never a
/// single run. The route was simulated against market 1's exact durable
/// message on devnet under a 1,400,000-CU probe: three draws, 252,518 every
/// time (252,368 in the Resolution program plus the 150 the ComputeBudget
/// instruction itself costs), so the band is 0 and the tolerance bottoms out
/// at its floor. 252,518 + 15,000 = 267,518, which is 19.1% of Solana's
/// 1,400,000 per-transaction ceiling.
///
/// The band is 0 rather than the 1,500-CU search-depth grid the gauntlet's
/// rows ride because this is a devnet route over accounts that already exist:
/// the deployed ELFs are fixed for the life of a cohort, so no PDA in the
/// frame can redraw its bump.
const RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1: u32 = 267_518;

/// `DirectCloseCapability`'s declared ComputeBudget limit.
///
/// The second route in this sequence not to fit the 200,000-CU default meter,
/// and the first market ever to reach it said so on chain: cohort-17's market
/// 2, 2026-09-06, consumed 200,000 of 200,000 and failed at `exceeded CUs meter
/// at BPF instruction` inside Core. It is the heaviest stage of the six by a
/// factor of five -- Core walks the two-ledger funding partition, projects the
/// close in place, and CPIs into Trading, which alone draws 211,558.
///
/// The number is derived under `tools/gauntlet/CU_BUDGETS.md`'s rule --
/// `tolerance = roundup(band, 10_000) + 10_000`, floor 15,000; `budget =
/// measured + tolerance`; `measured` is the highest draw, never a single run.
/// Market 2's own unlandable durable message, rebuilt with a 1,400,000-CU probe
/// and simulated on devnet with `sigVerify` off, drew 500,929 three times out
/// of three -- 500,779 inside Core, of which 211,558 is Trading's, plus the 150
/// the ComputeBudget instruction costs itself -- so the band is 0 and the
/// tolerance is its floor. 500,929 + 15,000 = 515,929, which is 36.9% of
/// Solana's 1,400,000 per-transaction ceiling.
///
/// The band is 0 for the same reason `ResolutionCloseFund`'s is: a devnet route
/// over accounts that already exist, against ELFs fixed for the life of a
/// cohort, so no PDA in the frame can redraw its bump.
const DIRECT_CLOSE_CAPABILITY_COMPUTE_UNIT_LIMIT_V1: u32 = 515_929;

/// The ComputeBudget limit a stage's durable message declares, or `None` when
/// the route runs inside the default meter.
///
/// `None` is not "unbudgeted". It is the positive claim that the route fits
/// 200,000 compute units, and every `None` below that has landed on devnet
/// landed inside it -- `CoreBeginRetiring` at 23,106 and `DirectBeginRetiring`
/// at 92,137. `DirectCloseCapability` met the meter on cohort-17's market 2 and
/// now carries its own measured row, which is what that comment said the
/// finding would be. `RetirementReplayHandoff` and `AggregateRetirement` have
/// still never executed on any chain; nothing here can honestly declare a
/// number for them, and if one meets the meter the finding is a third measured
/// row, not a blanket.
const fn terminal_stage_compute_unit_limit_v1(stage: TerminalStageV1) -> Option<u32> {
    match stage {
        TerminalStageV1::ResolutionCloseFund => Some(RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1),
        TerminalStageV1::DirectCloseCapability => {
            Some(DIRECT_CLOSE_CAPABILITY_COMPUTE_UNIT_LIMIT_V1)
        }
        TerminalStageV1::CoreBeginRetiring
        | TerminalStageV1::DirectBeginRetiring
        | TerminalStageV1::RetirementReplayHandoff
        | TerminalStageV1::AggregateRetirement => None,
    }
}

/// Whether a route MUST declare a limit, which is a fact about the route and
/// not about the number.
///
/// The value a session declares may be re-pinned -- a measurement is a
/// measurement -- and a finalized journal must stay verifiable across that.
/// What may never change is that `ResolutionCloseFund` and
/// `DirectCloseCapability` do not fit the default meter, so a durable message
/// for either that declares nothing is refused by name however the table is
/// later edited. Both facts were bought the same way: a packet that consumed
/// 200,000 of 200,000 on devnet and could never land.
const fn terminal_route_requires_declared_budget_v1(mutation: &DurableTerminalMutationV1) -> bool {
    matches!(
        mutation,
        DurableTerminalMutationV1::Protocol {
            stage: TerminalStageV1::ResolutionCloseFund | TerminalStageV1::DirectCloseCapability
        }
    )
}

/// One row of the session's record of what its driver declares.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TerminalStageComputeBudgetV1 {
    stage: TerminalStageV1,
    compute_unit_limit: u32,
}

/// What THIS RUN declares for a stage, read off its own session rather than
/// off the driver's table. The two are checked equal whenever the session is
/// loaded, so this is the same answer with the drift check in front of it.
fn session_stage_compute_unit_limit_v1(
    session: &TerminalSequenceSessionV1,
    stage: TerminalStageV1,
) -> Option<u32> {
    session
        .declared_compute_unit_limits
        .iter()
        .find(|row| row.stage == stage)
        .map(|row| row.compute_unit_limit)
}

/// The declared table, in stage order, as the session records it.
fn declared_terminal_compute_budgets_v1() -> Vec<TerminalStageComputeBudgetV1> {
    TerminalStageV1::ORDERED
        .into_iter()
        .filter_map(|stage| {
            terminal_stage_compute_unit_limit_v1(stage).map(|compute_unit_limit| {
                TerminalStageComputeBudgetV1 {
                    stage,
                    compute_unit_limit,
                }
            })
        })
        .collect()
}

/// Durable phase of one exact stage intent.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) enum StageJournalPhaseV1 {
    /// The complete unsigned message and prestate are durable.
    Planned,
    /// A signed packet and its locally derived signature are durable.
    SignedNotSubmitted,
    /// Submission returned that exact signature; replay is forbidden.
    Submitted,
    /// The exact transaction and poststate were observed at finalized.
    Finalized,
    /// The signed packet can never be included, and the observation that
    /// proves it is durable beside it. Terminal in the other direction: a
    /// superseded journal is never signed, submitted, polled or resumed again.
    Superseded,
}

/// Why an ambiguous submission was retired, and the observation that settled it.
///
/// `Submitted` is deliberately one-way: a driver may not infer permission to
/// re-sign from a missing RPC answer, because "the endpoint did not respond"
/// and "the packet landed" are the same observation. Exactly one thing tells
/// them apart and it is not a timeout -- once the finalized block height passes
/// the packet's `lastValidBlockHeight`, no validator will accept that blockhash
/// from anyone again, so the packet's fate stops being open. A signature absent
/// from transaction history at that point is absent for good.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableSupersededEvidenceV1 {
    reason: String,
    retired_signature: String,
    last_valid_block_height: u64,
    observed_block_height: u64,
    observed_slot: u64,
}

/// Core Market state relevant to terminal routing.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreTerminalStateV1 {
    Terminal,
    Retiring,
    Closed,
}

/// Direct root state relevant to terminal routing.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectTerminalStateV1 {
    Open,
    Retiring,
    Closed,
}

/// Resolution closure state.  A rent prepayment is an operational prerequisite
/// of CloseFund, not a seventh protocol stage.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolutionTerminalStateV1 {
    /// Source and funding ledger are live; the closure receipt coordinate is
    /// vacant and below its exact rent minimum.
    NeedsReceiptPrepayment,
    /// Source and funding ledger are live; the closure receipt coordinate has
    /// exactly the admitted prepaid balance.
    ReadyToClose,
    /// Source and selected ledger are closed and the exact closure receipt is
    /// finalized.
    Closed,
}

/// Custody replay ownership relevant to the handoff and aggregate close.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetirementReplayStateV1 {
    Trading,
    Core,
    Closed,
}

/// Result of routing one fully authenticated finalized graph.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TerminalRouteV1 {
    /// A System transfer must first prepay the exact Resolution receipt rent.
    PrepayResolutionReceipt,
    /// Execute one of the six protocol mutations.
    Execute(TerminalStageV1),
    /// Every terminal resource has closed in the aggregate transaction.
    Complete,
}

/// Next infrastructure mutation while producing the one immutable terminal
/// lookup table.  It is intentionally outside [`TerminalStageV1`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalLookupTableRouteV1 {
    Create(Instruction),
    Extend {
        prefix_len: usize,
        instruction: Instruction,
    },
    Freeze(Instruction),
    Complete,
}

/// Non-finalized account-coordinate closure for one protocol stage. It owns
/// no account bytes, lamports, observation, or claim that the stage can run.
/// A fresh finalized semantic report must later match this frame exactly.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) enum TerminalAddressClassV1 {
    /// Identity invariant across every legal fresh plan and resume. Only this
    /// class is admitted into the dedicated immutable lookup table.
    LookupStable,
    /// Externally signing account that must remain in the static key set.
    InlineSigner,
    /// Executable program identity that must remain in the static key set.
    InlineProgram,
    /// Request-, balance-, fee-, rent-, or predecessor-bound coordinate that
    /// may change across a legal crash/replan and must remain inline.
    InlineRequestBound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalMetaClosureV1 {
    pub(crate) stage: TerminalStageV1,
    pub(crate) program_id: Pubkey,
    pub(crate) program_class: TerminalAddressClassV1,
    pub(crate) accounts: Vec<AccountMeta>,
    pub(crate) classes: Vec<TerminalAddressClassV1>,
}

impl TerminalMetaClosureV1 {
    pub(crate) fn authenticate_instruction(&self, instruction: &Instruction) -> Result<()> {
        if self.program_class != TerminalAddressClassV1::InlineProgram
            || self.classes.len() != self.accounts.len()
            || instruction.program_id != self.program_id
            || instruction.accounts != self.accounts
        {
            return Err(refusal(
                "fresh semantic stage frame differed from its immutable ALT coordinate closure",
            ));
        }
        Ok(())
    }

    pub(crate) fn authenticate_fresh_closure(&self, fresh: &Self) -> Result<()> {
        if self != fresh {
            return Err(refusal(
                "fresh semantic stage keys, metas, or address classes differed from the persisted coordinate closure",
            ));
        }
        Ok(())
    }
}

const fn direct_begin_class(class: DirectBeginRetiringMetaClassV1) -> TerminalAddressClassV1 {
    match class {
        DirectBeginRetiringMetaClassV1::LookupStable => TerminalAddressClassV1::LookupStable,
        DirectBeginRetiringMetaClassV1::InlineSigner => TerminalAddressClassV1::InlineSigner,
        DirectBeginRetiringMetaClassV1::InlineProgram => TerminalAddressClassV1::InlineProgram,
        DirectBeginRetiringMetaClassV1::InlineRequestBound => {
            TerminalAddressClassV1::InlineRequestBound
        }
    }
}

const fn terminal_owner_class(class: TerminalMetaClassV1) -> TerminalAddressClassV1 {
    match class {
        TerminalMetaClassV1::LookupStable => TerminalAddressClassV1::LookupStable,
        TerminalMetaClassV1::InlineSigner => TerminalAddressClassV1::InlineSigner,
        TerminalMetaClassV1::InlineProgram => TerminalAddressClassV1::InlineProgram,
        TerminalMetaClassV1::InlineRequestBound => TerminalAddressClassV1::InlineRequestBound,
    }
}

/// Wrap Direct's semantic coordinate owner; do not reconstruct its 20 metas.
pub(crate) fn direct_begin_retiring_meta_closure_v1(
    input: DirectBeginRetiringCoordinateInputV1,
) -> Result<TerminalMetaClosureV1> {
    let closure = derive_direct_begin_retiring_meta_closure_v1(input)
        .map_err(|error| Error::new(format!("Direct BeginRetiring coordinates: {error:?}")))?;
    Ok(TerminalMetaClosureV1 {
        stage: TerminalStageV1::DirectBeginRetiring,
        program_id: closure.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: closure.accounts.to_vec(),
        classes: closure
            .classes
            .into_iter()
            .map(direct_begin_class)
            .collect(),
    })
}

/// Wrap Core/Trading's production F=2 native-close coordinate owner; do not
/// reconstruct its 38 metas or seven admitted aliases.
pub(crate) fn direct_native_close_meta_closure_v1(
    input: &DirectNativeCloseCoordinateInputV1,
) -> Result<TerminalMetaClosureV1> {
    let closure = project_direct_native_close_coordinate_closure_v1(input)
        .map_err(|error| Error::new(format!("Direct native-close coordinates: {error:?}")))?;
    Ok(TerminalMetaClosureV1 {
        stage: TerminalStageV1::DirectCloseCapability,
        program_id: closure.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: closure.accounts,
        classes: closure
            .classes
            .into_iter()
            .map(terminal_owner_class)
            .collect(),
    })
}

/// Wrap Core/Custody's request-bound replay-handoff coordinate owner; do not
/// reconstruct its 23 metas or infer the payer/caller placement classes.
pub(crate) fn retirement_replay_handoff_meta_closure_v1(
    input: &RetirementReplayHandoffCoordinateInputV1,
) -> Result<TerminalMetaClosureV1> {
    let closure = project_retirement_replay_handoff_coordinate_closure_v1(input)
        .map_err(|error| Error::new(format!("retirement replay coordinates: {error:?}")))?;
    Ok(TerminalMetaClosureV1 {
        stage: TerminalStageV1::RetirementReplayHandoff,
        program_id: closure.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: closure.accounts,
        classes: closure
            .classes
            .into_iter()
            .map(terminal_owner_class)
            .collect(),
    })
}

/// Exact five-role coordinate closure for Core `BeginRetiring`.
pub(crate) fn core_begin_retiring_meta_closure_v1(
    market: Pubkey,
    activation_cache: Pubkey,
    registry_program: Pubkey,
    core_program: Pubkey,
    core_programdata: Pubkey,
) -> TerminalMetaClosureV1 {
    TerminalMetaClosureV1 {
        stage: TerminalStageV1::CoreBeginRetiring,
        program_id: core_program,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: vec![
            AccountMeta::new(market, false),
            AccountMeta::new_readonly(activation_cache, false),
            AccountMeta::new_readonly(registry_program, false),
            AccountMeta::new_readonly(core_program, false),
            AccountMeta::new_readonly(core_programdata, false),
        ],
        classes: vec![
            TerminalAddressClassV1::LookupStable,
            TerminalAddressClassV1::LookupStable,
            TerminalAddressClassV1::InlineProgram,
            TerminalAddressClassV1::InlineProgram,
            TerminalAddressClassV1::LookupStable,
        ],
    }
}

/// Immutable identities for V7's direct permissionless Resolution close.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolutionCloseMetaCoordinatesV2 {
    pub(crate) market: Pubkey,
    pub(crate) activation_cache: Pubkey,
    pub(crate) registry_program: Pubkey,
    pub(crate) core_program: Pubkey,
    pub(crate) core_programdata: Pubkey,
    pub(crate) resolution_program: Pubkey,
    pub(crate) resolution_programdata: Pubkey,
    pub(crate) source_material: Pubkey,
    pub(crate) source_material_staging: Pubkey,
    pub(crate) capability_manifest: Pubkey,
    pub(crate) capability_manifest_staging: Pubkey,
    pub(crate) source_state: Pubkey,
    pub(crate) funding_ledger: Pubkey,
    pub(crate) certificate: Pubkey,
    pub(crate) closure_receipt: Pubkey,
    pub(crate) beneficiary: Pubkey,
    pub(crate) clock_sysvar: Pubkey,
    pub(crate) rent_sysvar: Pubkey,
    pub(crate) system_program: Pubkey,
    pub(crate) recovery_policy: Option<(Pubkey, Pubkey)>,
}

/// Exact V7 direct Resolution CloseFund coordinate closure.
pub(crate) fn resolution_close_meta_closure_v2(
    coordinates: &ResolutionCloseMetaCoordinatesV2,
) -> Result<TerminalMetaClosureV1> {
    let mut accounts = vec![
        AccountMeta::new_readonly(coordinates.market, false),
        AccountMeta::new_readonly(coordinates.activation_cache, false),
        AccountMeta::new_readonly(coordinates.registry_program, false),
        AccountMeta::new_readonly(coordinates.core_program, false),
        AccountMeta::new_readonly(coordinates.core_programdata, false),
        AccountMeta::new_readonly(coordinates.resolution_program, false),
        AccountMeta::new_readonly(coordinates.resolution_programdata, false),
        AccountMeta::new_readonly(coordinates.source_material, false),
        AccountMeta::new_readonly(coordinates.source_material_staging, false),
        AccountMeta::new_readonly(coordinates.capability_manifest, false),
        AccountMeta::new_readonly(coordinates.capability_manifest_staging, false),
        AccountMeta::new(coordinates.source_state, false),
        AccountMeta::new(coordinates.funding_ledger, false),
        AccountMeta::new_readonly(coordinates.certificate, false),
        AccountMeta::new(coordinates.closure_receipt, false),
        AccountMeta::new(coordinates.beneficiary, false),
        AccountMeta::new_readonly(coordinates.clock_sysvar, false),
        AccountMeta::new_readonly(coordinates.rent_sysvar, false),
        AccountMeta::new_readonly(coordinates.system_program, false),
    ];
    if let Some((raw, staging)) = coordinates.recovery_policy {
        accounts.push(AccountMeta::new_readonly(raw, false));
        accounts.push(AccountMeta::new_readonly(staging, false));
    }
    Ok(TerminalMetaClosureV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        program_id: coordinates.resolution_program,
        program_class: TerminalAddressClassV1::InlineProgram,
        classes: resolution_close_meta_classes_v2(coordinates.recovery_policy.is_some()),
        accounts,
    })
}

fn resolution_close_meta_classes_v2(has_recovery_policy: bool) -> Vec<TerminalAddressClassV1> {
    let mut classes = vec![
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::InlineProgram,
        TerminalAddressClassV1::InlineProgram,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::InlineProgram,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::LookupStable,
        TerminalAddressClassV1::InlineProgram,
    ];
    if has_recovery_policy {
        classes.extend([
            TerminalAddressClassV1::LookupStable,
            TerminalAddressClassV1::LookupStable,
        ]);
    }
    classes
}

/// Immutable account identities and typed request commitments for the final
/// Registry-wrapped aggregate-retirement frame. No projected account bytes or
/// balances are accepted here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AggregateRetirementMetaCoordinatesV1 {
    pub(crate) release_set: [u8; 32],
    pub(crate) parent_request_digest: [u8; 32],
    pub(crate) claims_request_body: Vec<u8>,
    pub(crate) custody_context: [u8; 32],
    pub(crate) close_vault_request_body: Vec<u8>,
    pub(crate) close_replay_request_body: Vec<u8>,
    pub(crate) rent_post_resource_digest: [u8; 32],
    pub(crate) continuation: RegistryContinuationRequestV1,
    pub(crate) market: Pubkey,
    pub(crate) rent_credit: Pubkey,
    pub(crate) activation_cache: Pubkey,
    pub(crate) registry_program: Pubkey,
    pub(crate) core_program: Pubkey,
    pub(crate) core_programdata: Pubkey,
    pub(crate) claims_program: Pubkey,
    pub(crate) claims_programdata: Pubkey,
    pub(crate) resolution_program: Pubkey,
    pub(crate) resolution_programdata: Pubkey,
    pub(crate) custody_program: Pubkey,
    pub(crate) custody_programdata: Pubkey,
    pub(crate) rent_program: Pubkey,
    pub(crate) source_receipt: Pubkey,
    pub(crate) claims_aggregate: Pubkey,
    pub(crate) custody_replay: Pubkey,
    pub(crate) hoard_vault: Pubkey,
    pub(crate) custody_authority: Pubkey,
    pub(crate) collateral_mint: Pubkey,
    pub(crate) collateral_token_program: Pubkey,
    pub(crate) realm_raw: Pubkey,
    pub(crate) realm_staging: Pubkey,
    pub(crate) infrastructure_profile: Pubkey,
    pub(crate) registry_artifact_raw: Pubkey,
    pub(crate) registry_artifact_staging: Pubkey,
    pub(crate) registry_programdata: Pubkey,
    pub(crate) rent_artifact_raw: Pubkey,
    pub(crate) rent_artifact_staging: Pubkey,
    pub(crate) rent_programdata: Pubkey,
    pub(crate) rent_sysvar: Pubkey,
    pub(crate) refund_wallet: Pubkey,
}

/// Exact 46-meta aggregate-retirement coordinate closure. The four request-
/// bound Core PDAs and invocation-scoped Registry admission are derived from
/// typed commitments rather than accepted as caller-selected table entries.
pub(crate) fn aggregate_retirement_meta_closure_v1(
    coordinates: &AggregateRetirementMetaCoordinatesV1,
) -> Result<TerminalMetaClosureV1> {
    let claims_authority = aggregate_caller_authority(
        coordinates.release_set,
        coordinates.market,
        coordinates.parent_request_digest,
        &coordinates.claims_request_body,
        coordinates.core_program,
    )?;
    let close_vault_authority = aggregate_caller_authority(
        coordinates.release_set,
        coordinates.market,
        coordinates.custody_context,
        &coordinates.close_vault_request_body,
        coordinates.core_program,
    )?;
    let close_replay_authority = aggregate_caller_authority(
        coordinates.release_set,
        coordinates.market,
        coordinates.custody_context,
        &coordinates.close_replay_request_body,
        coordinates.core_program,
    )?;
    let rent_seeds = LifecycleRentCoreCloseAuthoritySeedsV2::new(
        LifecycleAccountIdV2::new(coordinates.rent_credit.to_bytes())
            .map_err(|error| Error::new(format!("RentCredit identity: {error:?}")))?,
        coordinates.rent_post_resource_digest,
    )
    .map_err(|error| Error::new(format!("rent close authority seeds: {error:?}")))?;
    let credit = rent_seeds.credit().to_bytes();
    let post = rent_seeds.post_resource_digest();
    let rent_close_authority = Pubkey::find_program_address(
        &[rent_seeds.domain(), &credit, &post],
        &coordinates.core_program,
    )
    .0;
    let role_batch = coordinates
        .continuation
        .role_batch_request()
        .map_err(|error| Error::new(format!("retirement role batch: {error:?}")))?;
    let role_batch_digest = ContentId::new(hash(&role_batch.to_bytes()).to_bytes())
        .map_err(|error| Error::new(format!("retirement role-batch digest: {error:?}")))?;
    let admission_seeds = RegistryContinuationAdmissionSeedsV1::new(
        coordinates.continuation,
        coordinates.activation_cache.to_bytes(),
        role_batch_digest,
    )
    .map_err(|error| Error::new(format!("retirement continuation seeds: {error:?}")))?;
    let release = admission_seeds.release_set();
    let cache = admission_seeds.activation_cache();
    let role_batch = admission_seeds.batch_request_digest();
    let role_mask = admission_seeds.role_mask();
    let role = admission_seeds.continuation_role();
    let continuation_digest = admission_seeds.continuation_digest();
    let admission = Pubkey::find_program_address(
        &[
            admission_seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            role_batch.as_slice(),
            role_mask.as_slice(),
            role.as_slice(),
            continuation_digest.as_slice(),
        ],
        &coordinates.registry_program,
    )
    .0;
    let core = vec![
        AccountMeta::new(coordinates.market, false),
        AccountMeta::new(coordinates.rent_credit, false),
        AccountMeta::new_readonly(coordinates.activation_cache, false),
        AccountMeta::new_readonly(coordinates.registry_program, false),
        AccountMeta::new_readonly(coordinates.core_program, false),
        AccountMeta::new_readonly(coordinates.core_programdata, false),
        AccountMeta::new_readonly(coordinates.claims_program, false),
        AccountMeta::new_readonly(coordinates.claims_programdata, false),
        AccountMeta::new_readonly(coordinates.resolution_program, false),
        AccountMeta::new_readonly(coordinates.resolution_programdata, false),
        AccountMeta::new_readonly(coordinates.custody_program, false),
        AccountMeta::new_readonly(coordinates.custody_programdata, false),
        AccountMeta::new_readonly(coordinates.rent_program, false),
        AccountMeta::new_readonly(coordinates.source_receipt, false),
        AccountMeta::new(coordinates.claims_aggregate, false),
        AccountMeta::new(coordinates.custody_replay, false),
        AccountMeta::new(coordinates.hoard_vault, false),
        AccountMeta::new_readonly(coordinates.custody_authority, false),
        AccountMeta::new_readonly(coordinates.collateral_mint, false),
        AccountMeta::new_readonly(coordinates.collateral_token_program, false),
        AccountMeta::new_readonly(coordinates.realm_raw, false),
        AccountMeta::new_readonly(coordinates.realm_staging, false),
        AccountMeta::new_readonly(claims_authority, false),
        AccountMeta::new_readonly(close_vault_authority, false),
        AccountMeta::new_readonly(close_replay_authority, false),
        AccountMeta::new_readonly(coordinates.infrastructure_profile, false),
        AccountMeta::new_readonly(coordinates.registry_artifact_raw, false),
        AccountMeta::new_readonly(coordinates.registry_artifact_staging, false),
        AccountMeta::new_readonly(coordinates.registry_programdata, false),
        AccountMeta::new_readonly(coordinates.rent_artifact_raw, false),
        AccountMeta::new_readonly(coordinates.rent_artifact_staging, false),
        AccountMeta::new_readonly(coordinates.rent_programdata, false),
        AccountMeta::new_readonly(coordinates.rent_sysvar, false),
        AccountMeta::new(coordinates.refund_wallet, false),
        AccountMeta::new_readonly(rent_close_authority, false),
    ];
    if core.iter().any(|meta| meta.pubkey == admission) {
        return Err(refusal(
            "aggregate Registry admission aliased a Core retirement role",
        ));
    }
    let mut accounts = vec![
        AccountMeta::new_readonly(coordinates.activation_cache, false),
        AccountMeta::new_readonly(coordinates.core_program, false),
        AccountMeta::new_readonly(coordinates.core_programdata, false),
        AccountMeta::new_readonly(coordinates.claims_program, false),
        AccountMeta::new_readonly(coordinates.claims_programdata, false),
        AccountMeta::new_readonly(coordinates.resolution_program, false),
        AccountMeta::new_readonly(coordinates.resolution_programdata, false),
        AccountMeta::new_readonly(coordinates.custody_program, false),
        AccountMeta::new_readonly(coordinates.custody_programdata, false),
        AccountMeta::new_readonly(admission, false),
    ];
    accounts.extend(core);
    accounts.push(AccountMeta::new_readonly(admission, false));
    if accounts.len() != 46 {
        return Err(refusal(
            "aggregate retirement coordinate closure has another width than 46",
        ));
    }
    Ok(TerminalMetaClosureV1 {
        stage: TerminalStageV1::AggregateRetirement,
        program_id: coordinates.registry_program,
        program_class: TerminalAddressClassV1::InlineProgram,
        classes: aggregate_retirement_meta_classes_v1(),
        accounts,
    })
}

fn aggregate_retirement_meta_classes_v1() -> Vec<TerminalAddressClassV1> {
    let mut classes = vec![TerminalAddressClassV1::LookupStable; 46];
    for index in [1_usize, 3, 5, 7, 13, 14, 16, 18, 20, 22, 29] {
        classes[index] = TerminalAddressClassV1::InlineProgram;
    }
    for index in [9_usize, 32, 33, 34, 44, 45] {
        classes[index] = TerminalAddressClassV1::InlineRequestBound;
    }
    classes
}

fn aggregate_caller_authority(
    release_set: [u8; 32],
    market: Pubkey,
    context: [u8; 32],
    request_body: &[u8],
    core_program: Pubkey,
) -> Result<Pubkey> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        hash(request_body).to_bytes(),
    )
    .map_err(|error| Error::new(format!("aggregate caller seeds: {error:?}")))?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &core_program).0)
}

/// Exact return data that a finalized transaction must carry.  Absence means
/// the semantic owner does not define transaction return data for that stage;
/// it never weakens the exact account-poststate checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExpectedReturnDataV1 {
    pub(crate) producer: Pubkey,
    pub(crate) body: Vec<u8>,
    /// The one field of `body` the EXECUTION writes and no plan can bind.
    pub(crate) execution_clock: Option<ExecutionClockFieldV1>,
}

/// A poststate field the executing runtime stamps, and the interval it must
/// land in.
///
/// `prestate_is_slot_bound_runtime_account_v1` learned this one level down --
/// the Clock is not a PRESTATE, because its bytes are redefined every 400ms and
/// a guard nothing can satisfy protects nothing. `closed_at` is the same fact
/// one level up: Resolution stamps its closure receipt with the Clock AT
/// EXECUTION while the plan carries the Clock at PLANNING. The clock is not a
/// poststate either.
///
/// Measured on devnet 2026-09-04: market 1's first `ResolutionCloseFund`
/// (`3rDH7V5X...`, slot 493,003,631) wrote a 416-byte receipt whose every byte
/// matched its plan except this field, nine seconds late.
///
/// So the plan states an INTERVAL for this field and every other byte exactly.
/// It is a named set with exactly one member, and the member carries the
/// measurement that says why -- the same discipline the prestate exemption
/// carries. The lower bound is the plan's own Clock: a receipt stamped BEFORE
/// the observation its plan was built on describes an execution that preceded
/// its own inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExecutionClockFieldV1 {
    pub(crate) offset: usize,
    pub(crate) planned_unix_timestamp: u64,
    pub(crate) ceiling_unix_timestamp: u64,
}

/// One account poststate owned by a semantic stage builder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExpectedAccountPoststateV1 {
    pub(crate) key: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data: Vec<u8>,
    pub(crate) data_digest: [u8; 32],
    /// The one field of `data` the EXECUTION writes and no plan can bind.
    pub(crate) execution_clock: Option<ExecutionClockFieldV1>,
}

impl ExpectedAccountPoststateV1 {
    fn exact(key: Pubkey, owner: Pubkey, lamports: u64, executable: bool, data: Vec<u8>) -> Self {
        let data_digest = hash(&data).to_bytes();
        Self {
            key,
            owner,
            lamports,
            executable,
            data,
            data_digest,
            execution_clock: None,
        }
    }

    /// Every byte exact except one interval-bound execution clock.
    fn with_execution_clock(mut self, clock: ExecutionClockFieldV1) -> Self {
        self.execution_clock = Some(clock);
        self
    }
}

/// Caller-facing projection of one existing semantic operator report.
/// Protocol encodings, frames, request PDAs, and expected account bytes stay
/// owned by those reports.  This projection adds only exterior journal facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalSemanticMutationV1 {
    pub(crate) stage: TerminalStageV1,
    pub(crate) observation: Observation,
    pub(crate) instruction: Instruction,
    pub(crate) expected_return_data: Option<ExpectedReturnDataV1>,
    pub(crate) expected_accounts: Vec<ExpectedAccountPoststateV1>,
    /// Exact protocol-only lamport deltas before the transaction fee. The
    /// exterior caller maps these identities onto its payer/refund wallets;
    /// it never assumes those roles are distinct.
    pub(crate) protocol_lamport_deltas: BTreeMap<Pubkey, i128>,
}

/// Durable identity of one mutation. ALT operations are infrastructure
/// preflight and never consume a protocol-stage ordinal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", tag = "kind")]
pub(crate) enum DurableTerminalMutationV1 {
    LookupCreate,
    LookupExtend { prefix_len: usize },
    LookupFreeze,
    ResolutionReceiptPrepay,
    Protocol { stage: TerminalStageV1 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DurableAccountStateV1 {
    address: String,
    owner: String,
    lamports: u64,
    executable: bool,
    data_len: usize,
    data_sha256: String,
    account_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableInstructionAccountV1 {
    address: String,
    signer: bool,
    writable: bool,
    class: TerminalAddressClassV1,
}

/// One instruction meta, as the COMPILED MESSAGE will actually carry it.
///
/// Solana's message compiler unconditionally promotes the fee payer to a
/// writable signer at static index 0, whatever privileges the instruction meta
/// asked for. An intent built from the raw metas therefore describes a packet
/// that cannot exist whenever the frame names the payer — and the ALT routes
/// name it exactly there, as the table's own authority.
///
/// Recording the promotion here keeps the compiled-versus-intent comparison an
/// exact equality rather than teaching that comparison to forgive a difference;
/// a frame that disagrees for any OTHER reason still refuses, and still names
/// which conjunct failed.
///
/// This is a statement about the transaction format, not a relaxation of any
/// frame. §7.13 met the same promotion from the other side, where a program
/// demanded a readonly signer and no bookkeeping could help: there the fix had
/// to be a payer distinct from the account. Here the Address Lookup Table
/// program asks only that its authority sign, so the promotion is harmless and
/// the defect was only that the intent did not admit to it.
fn durable_instruction_account_v1(
    meta: &AccountMeta,
    class: TerminalAddressClassV1,
    payer: Pubkey,
) -> DurableInstructionAccountV1 {
    let is_fee_payer = meta.pubkey == payer;
    DurableInstructionAccountV1 {
        address: meta.pubkey.to_string(),
        signer: meta.is_signer || is_fee_payer,
        writable: meta.is_writable || is_fee_payer,
        class,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableExpectedAccountV1 {
    address: String,
    owner: String,
    lamports_after_protocol: u64,
    lamports_after_fee: u64,
    executable: bool,
    data_base64: String,
    data_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lookup_table: Option<DurableLookupTablePoststateV1>,
    /// Absent from the JSON unless the stage has such a field, so every journal
    /// written before this rule existed still parses and still hashes to the
    /// digest it was written with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    execution_clock: Option<DurableExecutionClockFieldV1>,
}

impl DurableExpectedAccountV1 {
    /// Does this poststate predict exactly these bytes.
    fn body_matches(&self, body: &[u8]) -> bool {
        self.lookup_table.is_none() && self.data_base64 == BASE64.encode(body)
    }
}

/// The persisted twin of `ExecutionClockFieldV1`.
///
/// `ceiling_unix_timestamp` is recorded rather than recomputed on every read,
/// for the reason `bbd01bbeb` records the compute budget it planned with: a
/// later re-pin of the derived ceiling must never retroactively refuse a
/// journal that was already written, signed, and landed under the old one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableExecutionClockFieldV1 {
    offset: usize,
    planned_unix_timestamp: u64,
    ceiling_unix_timestamp: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableLookupTablePoststateV1 {
    authority: Option<String>,
    addresses: Vec<String>,
    deactivation_slot: u64,
    last_extended_slot: DurableLookupLastExtendedSlotV1,
    last_extended_slot_start_index: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    rename_all = "camelCase",
    tag = "kind",
    content = "slot"
)]
enum DurableLookupLastExtendedSlotV1 {
    Exact(u64),
    FinalizedTransaction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableReturnDataV1 {
    producer: String,
    body_base64: String,
    body_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    execution_clock: Option<DurableExecutionClockFieldV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DurableTerminalIntentV1 {
    mutation: DurableTerminalMutationV1,
    observation_slot: u64,
    observation_unix_timestamp: i64,
    payer: String,
    program_id: String,
    program_class: TerminalAddressClassV1,
    accounts: Vec<DurableInstructionAccountV1>,
    instruction_data_base64: String,
    instruction_data_sha256: String,
    /// The ComputeBudget limit this durable message declares, when it declares
    /// one. Absent from the JSON when it does not, so a journal written before
    /// any route declared a budget still hashes to the digest it was written
    /// with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    compute_unit_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lookup_table: Option<String>,
    lookup_table_addresses: Vec<String>,
    lookup_table_addresses_sha256: String,
    loaded_writable: Vec<String>,
    loaded_readonly: Vec<String>,
    resolved_account_keys: Vec<String>,
    pre_balances: Vec<u64>,
    post_balances: Vec<u64>,
    recent_blockhash: String,
    last_valid_block_height: u64,
    transaction_fee_lamports: u64,
    wire_bytes: usize,
    message_base64: String,
    message_sha256: String,
    prestate: BTreeMap<String, DurableAccountStateV1>,
    expected_accounts: BTreeMap<String, DurableExpectedAccountV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_return_data: Option<DurableReturnDataV1>,
    protocol_lamport_deltas: BTreeMap<String, i128>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableFinalizedEvidenceV1 {
    signature: String,
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    packet_sha256: String,
    poststate: BTreeMap<String, DurableAccountStateV1>,
    /// The value each interval-bound execution-clock field actually carried.
    ///
    /// A persisted poststate holds digests, not bytes, so a re-verification of
    /// an archived journal has nothing to substitute into the plan's prediction
    /// unless the certification WROTE DOWN what it admitted. This is that
    /// record: certification checks the interval against the chain, stores the
    /// value it found, and every later re-verification is exact again -- and
    /// checks the interval a second time, against the number in front of it.
    /// Absent from the JSON when no stage in this journal has such a field.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    execution_clocks: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DurableTerminalJournalV1 {
    schema: String,
    cluster: String,
    rpc_url: String,
    authorized_mutation: bool,
    state_sha256: String,
    phase: StageJournalPhaseV1,
    intent_sha256: String,
    intent: DurableTerminalIntentV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    signed_packet_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finalized: Option<DurableFinalizedEvidenceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    superseded: Option<DurableSupersededEvidenceV1>,
    /// Present only on a journal whose PREDICTIONS were re-derived after its
    /// packet landed. Never on one that has not been signed, and never on a
    /// retired one: a packet cannot both have landed and be unlandable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reconciled: Option<DurableReconciledEvidenceV1>,
}

/// Reauthenticated Resolution-fund closure facts handed to the complete-life
/// campaign.
///
/// The four refund classes are copied from the persisted V3 receipt only after
/// their exhaustive sum and the journal's exact finalized packet have both
/// been checked.  They are evidence about this close, not another accounting
/// implementation.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirectResolutionCloseEvidenceV1 {
    pub(crate) journal_sha256: String,
    pub(crate) journal_state_sha256: String,
    pub(crate) market: String,
    pub(crate) source_state: String,
    pub(crate) terminal_certificate: String,
    pub(crate) receipt: String,
    pub(crate) receipt_sha256: String,
    pub(crate) beneficiary: String,
    pub(crate) generation: u64,
    pub(crate) terminal_sequence: u64,
    pub(crate) selector: u32,
    pub(crate) source_refund_lamports: u64,
    pub(crate) ledger_remaining_native_principal: u64,
    pub(crate) ledger_rent_lamports: u64,
    pub(crate) ledger_lamport_surplus: u64,
    pub(crate) refund_lamports: u64,
    pub(crate) permissionless: bool,
    pub(crate) finalized_receipt: Value,
}

/// In-memory proof that the exact Planned intent was reconstructed by the
/// stage's semantic owner from a fresh finalized snapshot. It is deliberately
/// not serializable and cannot be produced by editing or rehashing a journal.
#[derive(Debug)]
pub(crate) struct AuthenticatedPlannedTerminalIntentV1 {
    intent_sha256: String,
}

impl AuthenticatedPlannedTerminalIntentV1 {
    fn authenticate(&self, journal: &DurableTerminalJournalV1) -> Result<()> {
        if journal.phase != StageJournalPhaseV1::Planned
            || self.intent_sha256 != journal.intent_sha256
        {
            return Err(refusal(
                "planned terminal semantic-owner authorization did not bind this exact durable intent",
            ));
        }
        Ok(())
    }
}

/// Construct the exact durable protocol-stage intent. Callers must atomically
/// persist the returned journal before invoking [`resume_terminal_journal_v1`]
/// with execute authorization. This function performs reads only.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_protocol_stage_journal_v1(
    rpc: &mut Rpc,
    origin: &ClusterOriginV1,
    expected_cluster: ExpectedClusterV1,
    payer: Pubkey,
    mutation: &TerminalSemanticMutationV1,
    fresh_closure: &TerminalMetaClosureV1,
    frozen_table: &ObservedAccount,
    frozen_addresses: &[Pubkey],
    funded_rent_rate: u32,
    compute_unit_limit: Option<u32>,
    prestate: &[ObservedAccount],
    authorized_mutation: bool,
) -> Result<DurableTerminalJournalV1> {
    expected_cluster.authenticate(origin)?;
    if mutation.observation.finality != Finality::Finalized {
        return Err(refusal(
            "terminal execution only admits one exact finalized observation on its typed cluster",
        ));
    }
    if checked_terminal_delta_sum_v1(mutation.protocol_lamport_deltas.values().copied())? != 0 {
        return Err(refusal(
            "protocol-stage lamport deltas did not conserve exactly before transaction fee",
        ));
    }
    fresh_closure.authenticate_instruction(&mutation.instruction)?;
    if mutation.stage != fresh_closure.stage
        || frozen_table.observation != mutation.observation
        || prestate
            .iter()
            .any(|account| account.observation != mutation.observation)
    {
        return Err(refusal(
            "terminal intent mixed stages or finalized observations",
        ));
    }
    let mut prestate_by_key = BTreeMap::new();
    for account in prestate {
        if prestate_by_key.insert(account.key, account).is_some() {
            return Err(refusal("terminal prestate contained a duplicate account"));
        }
    }
    for key in std::iter::once(payer)
        .chain(mutation.instruction.accounts.iter().map(|meta| meta.pubkey))
        .chain(std::iter::once(frozen_table.key))
        // The ComputeBudget program becomes a static key of the compiled
        // message, so it is a resolved account with its own pre- and
        // post-balance and its exact finalized prestate is required like any
        // other account in the frame.
        .chain(compute_unit_limit.map(|_| compute_budget::ID))
    {
        if !prestate_by_key.contains_key(&key) {
            return Err(refusal(
                "terminal prestate omitted a payer, instruction, or lookup-table account",
            ));
        }
    }
    authenticate_supplied_terminal_lookup_table_v1(
        frozen_addresses,
        frozen_table,
        funded_rent_rate,
    )
    .map_err(|_| {
        refusal("terminal stage lookup table was not the exact activated frozen stable union")
    })?;
    // THE DECLARATION IS PART OF THE MESSAGE, NOT A FLAG ON THE SEND.
    //
    // A route that does not fit Solana's 200,000-CU default meter has to say so
    // in the bytes that get signed, which is why this is a durability-schema
    // change and not a flag: the limit is compiled in ahead of the one
    // first-party instruction, the exact fee is quoted for the wider message,
    // and `authenticate_terminal_message_decompilation_v1` reads the prefix
    // back by program and by discriminant rather than skipping past it.
    let mut instructions = Vec::with_capacity(2);
    if let Some(limit) = compute_unit_limit {
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(limit));
    }
    instructions.push(mutation.instruction.clone());
    let (recent_blockhash, last_valid_block_height) = terminal_latest_blockhash(rpc)?;
    let compiled = compile_v0_message_with_optional_tables(
        payer,
        &instructions,
        recent_blockhash,
        mutation.observation,
        std::slice::from_ref(frozen_table),
    )
    .map_err(|error| Error::new(format!("terminal v0 message: {error:?}")))?;
    authenticate_terminal_v0_placement_v1(
        payer,
        fresh_closure,
        frozen_table.key,
        frozen_addresses,
        &compiled,
    )?;
    let (loaded_writable, loaded_readonly, resolved_account_keys) =
        resolved_terminal_v0_keys_v1(&compiled, frozen_table.key, frozen_addresses)?;
    let pre_balances = resolved_account_keys
        .iter()
        .map(|key| {
            prestate_by_key
                .get(key)
                .map(|account| account.lamports)
                .ok_or_else(|| refusal("terminal prestate omitted one resolved v0 account"))
        })
        .collect::<Result<Vec<_>>>()?;
    let message_bytes = compiled.message.serialize();
    let message_base64 = BASE64.encode(&message_bytes);
    let transaction_fee_lamports = terminal_fee_for_message(rpc, &message_base64)?;
    let payer_pre = prestate_by_key
        .get(&payer)
        .ok_or_else(|| refusal("terminal prestate omitted fee payer"))?;
    let payer_protocol_delta = mutation
        .protocol_lamport_deltas
        .get(&payer)
        .copied()
        .unwrap_or(0);
    let payer_after_protocol =
        checked_apply_lamport_delta(payer_pre.lamports, payer_protocol_delta)?;
    let payer_after_fee = payer_after_protocol
        .checked_sub(transaction_fee_lamports)
        .ok_or_else(|| refusal("terminal payer cannot cover exact protocol delta plus fee"))?;

    let mut expected_accounts = BTreeMap::new();
    for account in &mutation.expected_accounts {
        if hash(&account.data).to_bytes() != account.data_digest {
            return Err(refusal(
                "semantic expected account digest disagreed with its exact bytes",
            ));
        }
        let pre = prestate_by_key
            .get(&account.key)
            .ok_or_else(|| refusal("semantic expected account omitted from exact prestate"))?;
        let observed_delta = i128::from(account.lamports) - i128::from(pre.lamports);
        if mutation
            .protocol_lamport_deltas
            .get(&account.key)
            .copied()
            .unwrap_or(0)
            != observed_delta
        {
            return Err(refusal(
                "semantic protocol lamport delta disagreed with exact pre/post balance",
            ));
        }
        let lamports_after_fee = if account.key == payer {
            account
                .lamports
                .checked_sub(transaction_fee_lamports)
                .ok_or_else(|| refusal("terminal expected payer fee debit underflowed"))?
        } else {
            account.lamports
        };
        if expected_accounts
            .insert(
                account.key.to_string(),
                DurableExpectedAccountV1 {
                    address: account.key.to_string(),
                    owner: account.owner.to_string(),
                    lamports_after_protocol: account.lamports,
                    lamports_after_fee,
                    executable: account.executable,
                    data_base64: BASE64.encode(&account.data),
                    data_sha256: sha256_hex(&account.data),
                    lookup_table: None,
                    execution_clock: account.execution_clock.map(durable_execution_clock_v1),
                },
            )
            .is_some()
        {
            return Err(refusal("semantic expected accounts contained a duplicate"));
        }
    }
    expected_accounts
        .entry(payer.to_string())
        .or_insert_with(|| DurableExpectedAccountV1 {
            address: payer.to_string(),
            owner: payer_pre.owner.to_string(),
            lamports_after_protocol: payer_after_protocol,
            lamports_after_fee: payer_after_fee,
            executable: payer_pre.executable,
            data_base64: BASE64.encode(&payer_pre.data),
            data_sha256: sha256_hex(&payer_pre.data),
            lookup_table: None,
            execution_clock: None,
        });
    if expected_accounts
        .get(&payer.to_string())
        .is_none_or(|expected| expected.lamports_after_fee != payer_after_fee)
    {
        return Err(refusal(
            "semantic payer poststate disagreed with protocol delta and exact fee",
        ));
    }
    let writable = mutation
        .instruction
        .accounts
        .iter()
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .chain(std::iter::once(payer))
        .collect::<BTreeSet<_>>();
    let missing_poststate = writable
        .iter()
        .filter(|key| !expected_accounts.contains_key(&key.to_string()))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if !missing_poststate.is_empty() {
        return Err(refusal(&format!(
            "terminal semantic report omitted a poststate for writable {missing_poststate:?}"
        )));
    }
    // A DELTA ON A READONLY ACCOUNT IS A CONTRADICTION ONLY WHEN IT IS NONZERO.
    //
    // These two were one refusal, and the pair reads as one accusation only
    // until a plan states that a readonly account's lamports do NOT move. That
    // is not a claim the frame cannot back: it is exactly the claim a readonly
    // account supports, it is checked against the account's own exact
    // pre/post balance above, and refusing it made `ResolutionCloseFund`
    // undrivable -- its plan says the Market's lamports are unchanged and the
    // Market is readonly in its frame, both correctly. A NONZERO delta on an
    // account the instruction cannot write is still a contradiction and is
    // still refused, now under its own name and with the figure.
    let moved_readonly = mutation
        .protocol_lamport_deltas
        .iter()
        .filter(|(key, delta)| **delta != 0 && !writable.contains(key))
        .map(|(key, delta)| format!("{key}:{delta}"))
        .collect::<Vec<_>>();
    if !moved_readonly.is_empty() {
        return Err(refusal(&format!(
            "terminal semantic report moved lamports on readonly accounts {moved_readonly:?}"
        )));
    }
    let post_balances = resolved_account_keys
        .iter()
        .zip(pre_balances.iter().copied())
        .map(|(key, pre)| {
            let protocol = mutation
                .protocol_lamport_deltas
                .get(key)
                .copied()
                .unwrap_or(0);
            let after_protocol = checked_apply_lamport_delta(pre, protocol)?;
            if *key == payer {
                after_protocol
                    .checked_sub(transaction_fee_lamports)
                    .ok_or_else(|| refusal("terminal post-balance fee debit underflowed"))
            } else {
                Ok(after_protocol)
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let intent = DurableTerminalIntentV1 {
        mutation: DurableTerminalMutationV1::Protocol {
            stage: mutation.stage,
        },
        observation_slot: mutation.observation.slot,
        observation_unix_timestamp: mutation.observation.unix_timestamp,
        payer: payer.to_string(),
        program_id: fresh_closure.program_id.to_string(),
        program_class: fresh_closure.program_class,
        accounts: fresh_closure
            .accounts
            .iter()
            .zip(fresh_closure.classes.iter().copied())
            .map(|(meta, class)| durable_instruction_account_v1(meta, class, payer))
            .collect(),
        instruction_data_base64: BASE64.encode(&mutation.instruction.data),
        instruction_data_sha256: sha256_hex(&mutation.instruction.data),
        compute_unit_limit,
        lookup_table: Some(frozen_table.key.to_string()),
        lookup_table_addresses: frozen_addresses.iter().map(ToString::to_string).collect(),
        lookup_table_addresses_sha256: pubkey_vector_sha256(frozen_addresses),
        loaded_writable: loaded_writable.iter().map(ToString::to_string).collect(),
        loaded_readonly: loaded_readonly.iter().map(ToString::to_string).collect(),
        resolved_account_keys: resolved_account_keys
            .iter()
            .map(ToString::to_string)
            .collect(),
        pre_balances,
        post_balances,
        recent_blockhash: recent_blockhash.to_string(),
        last_valid_block_height,
        transaction_fee_lamports,
        wire_bytes: compiled.wire_bytes,
        message_base64,
        message_sha256: sha256_hex(&message_bytes),
        prestate: prestate_by_key
            .into_iter()
            .map(|(key, account)| (key.to_string(), durable_observed_state(account)))
            .collect(),
        expected_accounts,
        expected_return_data: mutation.expected_return_data.as_ref().map(|expected| {
            DurableReturnDataV1 {
                producer: expected.producer.to_string(),
                body_base64: BASE64.encode(&expected.body),
                body_sha256: sha256_hex(&expected.body),
                execution_clock: expected.execution_clock.map(durable_execution_clock_v1),
            }
        }),
        protocol_lamport_deltas: mutation
            .protocol_lamport_deltas
            .iter()
            .map(|(key, delta)| (key.to_string(), *delta))
            .collect(),
    };
    let intent_sha256 = sha256_hex(&serde_json::to_vec(&intent)?);
    let mut journal = DurableTerminalJournalV1 {
        schema: terminal_journal_schema_v1(expected_cluster).into(),
        cluster: expected_cluster.evidence_label().into(),
        rpc_url: origin.redacted_url(),
        authorized_mutation,
        state_sha256: String::new(),
        phase: StageJournalPhaseV1::Planned,
        intent_sha256,
        intent,
        signed_packet_base64: None,
        expected_signature: None,
        finalized: None,
        superseded: None,
        reconciled: None,
    };
    refresh_terminal_journal_digest_v1(&mut journal)?;
    // A BUILDER MAY NOT WRITE A JOURNAL ITS OWN AUTHORITY WOULD REFUSE.
    //
    // The resume path authenticates before it signs, so an undeclared
    // `ResolutionCloseFund` was always going to be refused -- but only after it
    // had been fsynced into the journal directory, where a Planned entry is a
    // durable obligation that has to be preserved and dated out of the way by
    // hand. Refusing here keeps the sequence's one send boundary the only place
    // that has to think about durability.
    authenticate_terminal_journal_v1(&journal)?;
    Ok(journal)
}

/// Build the Resolution receipt-rent top-up through the identical durable
/// message/signature/finalizer machinery. It is explicitly an operational
/// prerequisite, not a seventh protocol stage.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_resolution_receipt_prepay_journal_v1(
    rpc: &mut Rpc,
    origin: &ClusterOriginV1,
    expected_cluster: ExpectedClusterV1,
    payer: &ObservedAccount,
    receipt: &ObservedAccount,
    exact_receipt_rent: u64,
    frozen_table: &ObservedAccount,
    frozen_addresses: &[Pubkey],
    funded_rent_rate: u32,
    prestate: &[ObservedAccount],
    authorized_mutation: bool,
) -> Result<DurableTerminalJournalV1> {
    if payer.observation != receipt.observation
        || payer.observation != frozen_table.observation
        || receipt.owner != solana_sdk_ids::system_program::ID
        || receipt.executable
        || !receipt.data.is_empty()
        || receipt.lamports >= exact_receipt_rent
    {
        return Err(refusal(
            "Resolution receipt prepay requires one finalized vacant below-rent destination",
        ));
    }
    let top_up = exact_receipt_rent
        .checked_sub(receipt.lamports)
        .ok_or_else(|| refusal("Resolution receipt top-up underflowed"))?;
    let payer_after = payer
        .lamports
        .checked_sub(top_up)
        .ok_or_else(|| refusal("Resolution receipt payer cannot cover exact rent top-up"))?;
    let instruction =
        solana_system_interface::instruction::transfer(&payer.key, &receipt.key, top_up);
    let closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        program_id: solana_sdk_ids::system_program::ID,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: instruction.accounts.clone(),
        classes: vec![
            TerminalAddressClassV1::InlineSigner,
            TerminalAddressClassV1::LookupStable,
        ],
    };
    let mutation = TerminalSemanticMutationV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        observation: payer.observation,
        instruction,
        expected_return_data: None,
        expected_accounts: vec![
            ExpectedAccountPoststateV1::exact(
                payer.key,
                payer.owner,
                payer_after,
                payer.executable,
                payer.data.clone(),
            ),
            ExpectedAccountPoststateV1::exact(
                receipt.key,
                receipt.owner,
                exact_receipt_rent,
                receipt.executable,
                receipt.data.clone(),
            ),
        ],
        protocol_lamport_deltas: BTreeMap::from([
            (payer.key, -i128::from(top_up)),
            (receipt.key, i128::from(top_up)),
        ]),
    };
    let mut journal = build_protocol_stage_journal_v1(
        rpc,
        origin,
        expected_cluster,
        payer.key,
        &mutation,
        &closure,
        frozen_table,
        frozen_addresses,
        funded_rent_rate,
        // A System transfer, measured at 150 CU on devnet 2026-09-04. It rides
        // the stage-three slot only because it is the prerequisite of that
        // stage; it is not that route and does not carry its budget.
        None,
        prestate,
        authorized_mutation,
    )?;
    journal.intent.mutation = DurableTerminalMutationV1::ResolutionReceiptPrepay;
    journal.intent_sha256 = sha256_hex(&serde_json::to_vec(&journal.intent)?);
    refresh_terminal_journal_digest_v1(&mut journal)?;
    authenticate_terminal_journal_v1(&journal)?;
    Ok(journal)
}

pub(crate) fn authenticate_resolution_receipt_prepay_planned_journal_v1(
    journal: &DurableTerminalJournalV1,
    payer: &ObservedAccount,
    receipt: &ObservedAccount,
    exact_receipt_rent: u64,
    exact_execution_prestate: &[ObservedAccount],
) -> Result<AuthenticatedPlannedTerminalIntentV1> {
    if payer.observation != receipt.observation
        || receipt.owner != solana_sdk_ids::system_program::ID
        || receipt.executable
        || !receipt.data.is_empty()
        || receipt.lamports >= exact_receipt_rent
    {
        return Err(refusal(
            "Resolution receipt semantic-owner authorization requires one finalized vacant below-rent destination",
        ));
    }
    let top_up = exact_receipt_rent
        .checked_sub(receipt.lamports)
        .ok_or_else(|| refusal("Resolution receipt authorization top-up underflowed"))?;
    let payer_after = payer
        .lamports
        .checked_sub(top_up)
        .ok_or_else(|| refusal("Resolution receipt authorization payer underflowed"))?;
    let instruction =
        solana_system_interface::instruction::transfer(&payer.key, &receipt.key, top_up);
    let closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        program_id: solana_sdk_ids::system_program::ID,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: instruction.accounts.clone(),
        classes: vec![
            TerminalAddressClassV1::InlineSigner,
            TerminalAddressClassV1::LookupStable,
        ],
    };
    let mutation = TerminalSemanticMutationV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        observation: payer.observation,
        instruction,
        expected_return_data: None,
        expected_accounts: vec![
            ExpectedAccountPoststateV1::exact(
                payer.key,
                payer.owner,
                payer_after,
                payer.executable,
                payer.data.clone(),
            ),
            ExpectedAccountPoststateV1::exact(
                receipt.key,
                receipt.owner,
                exact_receipt_rent,
                receipt.executable,
                receipt.data.clone(),
            ),
        ],
        protocol_lamport_deltas: BTreeMap::from([
            (payer.key, -i128::from(top_up)),
            (receipt.key, i128::from(top_up)),
        ]),
    };
    authenticate_planned_protocol_owner_v1(
        journal,
        DurableTerminalMutationV1::ResolutionReceiptPrepay,
        &mutation,
        &closure,
        exact_execution_prestate,
    )
}

fn checked_apply_lamport_delta(pre: u64, delta: i128) -> Result<u64> {
    if delta >= 0 {
        pre.checked_add(
            u64::try_from(delta).map_err(|_| refusal("positive lamport delta exceeded u64"))?,
        )
        .ok_or_else(|| refusal("positive lamport delta overflowed"))
    } else {
        pre.checked_sub(
            u64::try_from(-delta).map_err(|_| refusal("negative lamport delta exceeded u64"))?,
        )
        .ok_or_else(|| refusal("negative lamport delta underflowed"))
    }
}

fn checked_terminal_delta_sum_v1(deltas: impl IntoIterator<Item = i128>) -> Result<i128> {
    deltas.into_iter().try_fold(0_i128, |sum, delta| {
        sum.checked_add(delta)
            .ok_or_else(|| refusal("terminal lamport delta sum overflowed i128"))
    })
}

fn authenticate_planned_protocol_owner_v1(
    journal: &DurableTerminalJournalV1,
    durable_mutation: DurableTerminalMutationV1,
    mutation: &TerminalSemanticMutationV1,
    closure: &TerminalMetaClosureV1,
    exact_prestate: &[ObservedAccount],
) -> Result<AuthenticatedPlannedTerminalIntentV1> {
    authenticate_terminal_journal_v1(journal)?;
    if journal.phase != StageJournalPhaseV1::Planned
        || journal.intent.mutation != durable_mutation
        || mutation.observation.slot < journal.intent.observation_slot
        || mutation.observation.finality != Finality::Finalized
    {
        return Err(refusal(
            "planned protocol journal did not bind the semantic owner's stage and finalized observation",
        ));
    }
    closure.authenticate_instruction(&mutation.instruction)?;
    let journal_payer = pubkey(&journal.intent.payer)?;
    let instruction_accounts = closure
        .accounts
        .iter()
        .zip(closure.classes.iter().copied())
        .map(|(meta, class)| durable_instruction_account_v1(meta, class, journal_payer))
        .collect::<Vec<_>>();
    if journal.intent.program_id != closure.program_id.to_string()
        || journal.intent.program_class != closure.program_class
        || journal.intent.accounts != instruction_accounts
        || journal.intent.instruction_data_base64 != BASE64.encode(&mutation.instruction.data)
        || journal.intent.instruction_data_sha256 != sha256_hex(&mutation.instruction.data)
    {
        return Err(refusal(
            "planned protocol message differed from the freshly rerun semantic owner's exact instruction or closure",
        ));
    }
    let expected_return = mutation
        .expected_return_data
        .as_ref()
        .map(|value| DurableReturnDataV1 {
            producer: value.producer.to_string(),
            body_base64: BASE64.encode(&value.body),
            body_sha256: sha256_hex(&value.body),
            execution_clock: value.execution_clock.map(durable_execution_clock_v1),
        });
    if journal.intent.expected_return_data != expected_return {
        return Err(refusal(
            "planned protocol return-data contract differed from the semantic owner",
        ));
    }
    let mut observed_prestate = BTreeMap::new();
    for account in exact_prestate {
        if account.observation != mutation.observation
            || observed_prestate
                .insert(account.key.to_string(), durable_observed_state(account))
                .is_some()
        {
            return Err(refusal(
                "semantic-owner authorization mixed observations or duplicate prestate accounts",
            ));
        }
    }
    if observed_prestate != journal.intent.prestate {
        return Err(refusal(
            "planned protocol prestate differed from the exact finalized semantic-owner snapshot",
        ));
    }
    let payer = Pubkey::from_str(&journal.intent.payer)
        .map_err(|error| Error::new(format!("terminal semantic payer: {error}")))?;
    let pre_by_key = exact_prestate
        .iter()
        .map(|account| (account.key, account))
        .collect::<BTreeMap<_, _>>();
    let mut expected_accounts = BTreeMap::new();
    for expected in &mutation.expected_accounts {
        if hash(&expected.data).to_bytes() != expected.data_digest {
            return Err(refusal(
                "semantic owner returned expected bytes with another digest",
            ));
        }
        let lamports_after_fee = if expected.key == payer {
            expected
                .lamports
                .checked_sub(journal.intent.transaction_fee_lamports)
                .ok_or_else(|| refusal("semantic owner payer poststate fee underflowed"))?
        } else {
            expected.lamports
        };
        let durable = DurableExpectedAccountV1 {
            address: expected.key.to_string(),
            owner: expected.owner.to_string(),
            lamports_after_protocol: expected.lamports,
            lamports_after_fee,
            executable: expected.executable,
            data_base64: BASE64.encode(&expected.data),
            data_sha256: sha256_hex(&expected.data),
            lookup_table: None,
            execution_clock: expected.execution_clock.map(durable_execution_clock_v1),
        };
        if expected_accounts
            .insert(expected.key.to_string(), durable)
            .is_some()
        {
            return Err(refusal(
                "semantic owner returned duplicate expected poststate accounts",
            ));
        }
    }
    if let std::collections::btree_map::Entry::Vacant(entry) =
        expected_accounts.entry(payer.to_string())
    {
        let payer_pre = pre_by_key
            .get(&payer)
            .ok_or_else(|| refusal("semantic-owner snapshot omitted fee payer"))?;
        let payer_delta = mutation
            .protocol_lamport_deltas
            .get(&payer)
            .copied()
            .unwrap_or(0);
        let after_protocol = checked_apply_lamport_delta(payer_pre.lamports, payer_delta)?;
        let after_fee = after_protocol
            .checked_sub(journal.intent.transaction_fee_lamports)
            .ok_or_else(|| refusal("semantic owner payer could not cover exact fee"))?;
        entry.insert(DurableExpectedAccountV1 {
            address: payer.to_string(),
            owner: payer_pre.owner.to_string(),
            lamports_after_protocol: after_protocol,
            lamports_after_fee: after_fee,
            executable: payer_pre.executable,
            data_base64: BASE64.encode(&payer_pre.data),
            data_sha256: sha256_hex(&payer_pre.data),
            lookup_table: None,
            execution_clock: None,
        });
    }
    let deltas = mutation
        .protocol_lamport_deltas
        .iter()
        .map(|(key, delta)| (key.to_string(), *delta))
        .collect::<BTreeMap<_, _>>();
    if expected_accounts != journal.intent.expected_accounts
        || deltas != journal.intent.protocol_lamport_deltas
    {
        return Err(refusal(
            "planned protocol poststate or lamport arithmetic differed from the freshly rerun semantic owner",
        ));
    }
    Ok(AuthenticatedPlannedTerminalIntentV1 {
        intent_sha256: journal.intent_sha256.clone(),
    })
}

/// Atomically persist a new or updated terminal journal and fsync its parent
/// directory. The caller invokes this before any key file can be opened.
pub(crate) fn write_terminal_journal_v1(
    path: &Path,
    journal: &mut DurableTerminalJournalV1,
    new: bool,
) -> Result<()> {
    if !path.is_absolute() {
        return Err(refusal("terminal journal path must be absolute"));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| refusal("terminal journal needs a UTF-8 file name"))?;
    let lock = path.with_file_name(format!(".{name}.terminal-sequence.lock"));
    let lock_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|error| {
            Error::new(format!(
                "REFUSED: acquire exclusive terminal journal lock {}: {error}; a concurrent or interrupted writer must be reconciled first",
                lock.display()
            ))
        })?;
    lock_file.sync_all()?;
    let persisted = if new {
        if path.exists() {
            let _ = fs::remove_file(&lock);
            return Err(refusal(
                "terminal journal already exists; resume it or choose another absolute path",
            ));
        }
        None
    } else {
        let persisted = read_terminal_journal_v1(path)?;
        authenticate_terminal_journal_v1(&persisted)?;
        if journal.state_sha256 != persisted.state_sha256 {
            let _ = fs::remove_file(&lock);
            return Err(refusal(
                "terminal journal update was based on a stale persisted state",
            ));
        }
        Some(persisted)
    };
    let _ = persisted;
    refresh_terminal_journal_digest_v1(journal)?;
    authenticate_terminal_journal_v1(journal)?;
    let temporary = path.with_file_name(format!(
        ".{name}.terminal-sequence-{}.tmp",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| Error::new(format!("create {}: {error}", temporary.display())))?;
    file.write_all(&serde_json::to_vec_pretty(journal)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    if new {
        fs::hard_link(&temporary, path).map_err(|error| {
            Error::new(format!(
                "atomically publish new terminal journal {} without clobber: {error}",
                path.display()
            ))
        })?;
        fs::remove_file(&temporary)?;
    } else {
        fs::rename(&temporary, path).map_err(|error| {
            Error::new(format!(
                "atomically replace {} from {}: {error}",
                path.display(),
                temporary.display()
            ))
        })?;
    }
    if let Some(parent) = path.parent() {
        OpenOptions::new()
            .read(true)
            .open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| Error::new(format!("fsync terminal journal directory: {error}")))?;
    }
    fs::remove_file(&lock)?;
    Ok(())
}

pub(crate) fn read_terminal_journal_v1(path: &Path) -> Result<DurableTerminalJournalV1> {
    let source = fs::read(path)?;
    let _: UniqueJsonV1 = serde_json::from_slice(&source).map_err(|error| {
        Error::new(format!(
            "terminal journal JSON contains a duplicate key or malformed value: {error}"
        ))
    })?;
    let journal: DurableTerminalJournalV1 = serde_json::from_slice(&source)?;
    // A refusal that does not name the file converts a located defect into a
    // search, and this reader is called over a whole directory of journals.
    authenticate_terminal_journal_v1(&journal)
        .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
    Ok(journal)
}

/// Reauthenticate the exact permissionless DCLRFCQ1 close and its exhaustive
/// Source-closure receipt for a wider Direct lifecycle campaign.
///
/// This deliberately reads the typed journal through its semantic owner and
/// then reopens finalized transaction history.  A file whose phase merely says
/// `finalized` is not evidence.  The receipt body comes from the journal's
/// operator-predicted poststate and must still be byte-identical on chain.
pub(crate) fn authenticate_direct_resolution_close_v1(
    rpc: &mut Rpc,
    origin: &ClusterOriginV1,
    journal_path: &Path,
    expected_market: Pubkey,
    expected_receipt: Pubkey,
) -> Result<DirectResolutionCloseEvidenceV1> {
    ExpectedClusterV1::OwnedLoopback.authenticate(origin)?;
    authenticate_terminal_cluster_v1(rpc, origin, ExpectedClusterV1::OwnedLoopback)?;
    let journal_source = fs::read(journal_path).map_err(|error| {
        Error::new(format!(
            "read direct-life Resolution close journal {}: {error}",
            journal_path.display()
        ))
    })?;
    let journal = read_terminal_journal_v1(journal_path)?;
    if terminal_journal_cluster_v1(&journal)? != ExpectedClusterV1::OwnedLoopback
        || journal.rpc_url != origin.redacted_url()
        || journal.phase != StageJournalPhaseV1::Finalized
        || journal.intent.mutation
            != (DurableTerminalMutationV1::Protocol {
                stage: TerminalStageV1::ResolutionCloseFund,
            })
        || !journal.authorized_mutation
    {
        return Err(refusal(
            "direct-life Resolution close was not the exact finalized owned-loopback DCLRFCQ1 journal",
        ));
    }
    // DCLRFCQ1 has no protocol signer. The transaction fee payer may be any
    // actor and is intentionally absent from the program frame.
    if journal.intent.accounts.iter().any(|account| account.signer) {
        return Err(refusal(
            "direct-life Resolution close carried a protocol signer and was not permissionless",
        ));
    }
    verify_persisted_terminal_finalization_v1(rpc, &journal)?;
    let finalized = journal
        .finalized
        .as_ref()
        .ok_or_else(|| refusal("direct-life Resolution close omitted finalized evidence"))?;
    if finalized.slot == 0 || finalized.compute_units_consumed.is_none() {
        return Err(refusal(
            "direct-life Resolution close omitted its finalized slot or exact compute units",
        ));
    }

    let receipt_key = expected_receipt.to_string();
    let expected = journal
        .intent
        .expected_accounts
        .get(&receipt_key)
        .ok_or_else(|| refusal("DCLRFCQ1 journal omitted its Source closure receipt poststate"))?;
    let persisted = finalized
        .poststate
        .get(&receipt_key)
        .ok_or_else(|| refusal("DCLRFCQ1 finalization omitted its Source closure receipt"))?;
    let receipt_bytes = BASE64
        .decode(&expected.data_base64)
        .map_err(|error| Error::new(format!("Source closure receipt base64: {error}")))?;
    let receipt_sha256 = sha256_hex(&receipt_bytes);
    let resolution_program = Pubkey::from_str(&journal.intent.program_id)
        .map_err(|error| Error::new(format!("DCLRFCQ1 program: {error}")))?;
    if expected.address != receipt_key
        || expected.owner != resolution_program.to_string()
        || expected.executable
        || expected.data_sha256 != receipt_sha256
        || persisted.address != receipt_key
        || persisted.owner != expected.owner
        || persisted.lamports != expected.lamports_after_fee
        || persisted.executable
        || persisted.data_len != receipt_bytes.len()
        || persisted.data_sha256 != receipt_sha256
    {
        return Err(refusal(
            "DCLRFCQ1 predicted and finalized Source closure receipt poststates differ",
        ));
    }
    let standing = rpc
        .account(expected_receipt)?
        .ok_or_else(|| refusal("finalized Source closure receipt disappeared from chain"))?;
    if standing.owner != resolution_program
        || standing.lamports != expected.lamports_after_fee
        || standing.executable
        || standing.data != receipt_bytes
    {
        return Err(refusal(
            "standing Source closure receipt differs from the finalized DCLRFCQ1 poststate",
        ));
    }
    let receipt = SourceClosureReceiptV3::decode(&receipt_bytes)
        .map_err(|error| Error::new(format!("Source closure receipt V3: {error:?}")))?;
    let closure_sequence = receipt
        .terminal_sequence
        .checked_add(1)
        .ok_or_else(|| refusal("Source closure receipt sequence overflowed"))?;
    let source_state = Pubkey::new_from_array(receipt.source_state);
    let canonical_receipt = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &closure_sequence.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;
    let exhaustive_refund = receipt
        .source_refund_lamports
        .checked_add(receipt.ledger_remaining_native_principal)
        .and_then(|value| value.checked_add(receipt.ledger_rent_lamports))
        .and_then(|value| value.checked_add(receipt.ledger_lamport_surplus));
    if receipt.market != expected_market.to_bytes()
        || receipt.receipt_account != expected_receipt.to_bytes()
        || canonical_receipt != expected_receipt
        || receipt.terminal_certificate == [0; 32]
        || receipt.beneficiary == [0; 32]
        || exhaustive_refund != Some(receipt.refund_lamports)
    {
        return Err(refusal(
            "Source closure receipt changed its Market/PDA binding or exhaustive refund classification",
        ));
    }
    Ok(DirectResolutionCloseEvidenceV1 {
        journal_sha256: sha256_hex(&journal_source),
        journal_state_sha256: journal.state_sha256.clone(),
        market: expected_market.to_string(),
        source_state: source_state.to_string(),
        terminal_certificate: Pubkey::new_from_array(receipt.terminal_certificate).to_string(),
        receipt: expected_receipt.to_string(),
        receipt_sha256,
        beneficiary: Pubkey::new_from_array(receipt.beneficiary).to_string(),
        generation: receipt.generation,
        terminal_sequence: receipt.terminal_sequence,
        selector: receipt.selector,
        source_refund_lamports: receipt.source_refund_lamports,
        ledger_remaining_native_principal: receipt.ledger_remaining_native_principal,
        ledger_rent_lamports: receipt.ledger_rent_lamports,
        ledger_lamport_surplus: receipt.ledger_lamport_surplus,
        refund_lamports: receipt.refund_lamports,
        permissionless: true,
        finalized_receipt: serde_json::to_value(finalized)?,
    })
}

struct UniqueJsonV1;

impl<'de> Deserialize<'de> for UniqueJsonV1 {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitorV1)
    }
}

struct UniqueJsonVisitorV1;

impl<'de> Visitor<'de> for UniqueJsonVisitorV1 {
    type Value = UniqueJsonV1;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON with duplicate-free object keys")
    }

    fn visit_map<A>(self, mut map: A) -> core::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate object key {key}"
                )));
            }
            let _: UniqueJsonV1 = map.next_value()?;
        }
        Ok(UniqueJsonV1)
    }

    fn visit_seq<A>(self, mut sequence: A) -> core::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<UniqueJsonV1>()?.is_some() {}
        Ok(UniqueJsonV1)
    }

    fn visit_bool<E>(self, _value: bool) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_i64<E>(self, _value: i64) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_u64<E>(self, _value: u64) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_f64<E>(self, _value: f64) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_str<E>(self, _value: &str) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_string<E>(self, _value: String) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_none<E>(self) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }

    fn visit_unit<E>(self) -> core::result::Result<Self::Value, E> {
        Ok(UniqueJsonV1)
    }
}

/// Resume only the exact durable message/signature. A SignedNotSubmitted or
/// Submitted journal is never re-signed and never auto-resubmitted: after a
/// crash the send boundary is ambiguous, so this reconciles/polls only the
/// locally derived signature and preserves the journal on every refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalResumeRouteV1 {
    VerifyFinalized,
    PollOnly,
    PlannedReadOnly,
    SignAndSubmitOnce,
    RefuseRetired,
}

const fn terminal_resume_route_v1(
    phase: StageJournalPhaseV1,
    execute: bool,
) -> TerminalResumeRouteV1 {
    match (phase, execute) {
        (StageJournalPhaseV1::Finalized, _) => TerminalResumeRouteV1::VerifyFinalized,
        (StageJournalPhaseV1::SignedNotSubmitted | StageJournalPhaseV1::Submitted, _) => {
            TerminalResumeRouteV1::PollOnly
        }
        (StageJournalPhaseV1::Superseded, _) => TerminalResumeRouteV1::RefuseRetired,
        (StageJournalPhaseV1::Planned, false) => TerminalResumeRouteV1::PlannedReadOnly,
        (StageJournalPhaseV1::Planned, true) => TerminalResumeRouteV1::SignAndSubmitOnce,
    }
}

pub(crate) fn resume_terminal_journal_v1(
    rpc: &mut Rpc,
    origin: &ClusterOriginV1,
    expected_cluster: ExpectedClusterV1,
    journal_path: &Path,
    payer_keypair_path: &Path,
    execute: bool,
    journal: &mut DurableTerminalJournalV1,
    planned_authorization: Option<&AuthenticatedPlannedTerminalIntentV1>,
) -> Result<()> {
    authenticate_terminal_journal_v1(journal)?;
    if terminal_journal_cluster_v1(journal)? != expected_cluster {
        return Err(refusal(
            "terminal journal cluster provenance differed from the typed executor policy",
        ));
    }
    if journal.rpc_url != origin.redacted_url() {
        return Err(refusal(
            "terminal journal RPC origin differed from the currently admitted redacted origin",
        ));
    }
    authenticate_terminal_cluster_v1(rpc, origin, expected_cluster)?;
    if execute && !journal.authorized_mutation {
        journal.authorized_mutation = true;
        write_terminal_journal_v1(journal_path, journal, false)?;
    }
    match terminal_resume_route_v1(journal.phase, execute) {
        TerminalResumeRouteV1::VerifyFinalized => {
            verify_persisted_terminal_finalization_v1(rpc, journal)
        }
        TerminalResumeRouteV1::PollOnly => {
            reconcile_terminal_signature_v1(rpc, journal_path, journal)
        }
        // The journal refuses its own resubmission, by signature. Nothing in
        // this file can move a Superseded entry to any other phase; the only
        // way back to a submittable packet is a fresh plan under a fresh
        // blockhash, which is a different signature.
        TerminalResumeRouteV1::RefuseRetired => Err(terminal_retired_refusal_v1(journal)),
        TerminalResumeRouteV1::PlannedReadOnly => Ok(()),
        TerminalResumeRouteV1::SignAndSubmitOnce => {
            planned_authorization
                .ok_or_else(|| {
                    refusal(
                        "planned terminal signing requires fresh stage-specific semantic-owner authorization",
                    )
                })?
                .authenticate(journal)?;
            require_declared_budget_before_signing_v1(&journal.intent)?;
            require_terminal_prestate_unchanged_v1(rpc, journal)?;
            let height = rpc
                .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
                .as_u64()
                .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
            if height > journal.intent.last_valid_block_height {
                return Err(refusal(
                    "terminal journal blockhash expired before key load; preserve it and construct a fresh plan",
                ));
            }
            authenticate_terminal_cluster_v1(rpc, origin, expected_cluster)?;
            let payer = Keypair::new_from_array(campaign::read_keypair_file(
                payer_keypair_path,
                "terminal-fee-payer",
            )?);
            let expected_payer = Pubkey::from_str(&journal.intent.payer)
                .map_err(|error| Error::new(format!("terminal journal payer: {error}")))?;
            if payer.pubkey() != expected_payer {
                return Err(refusal(
                    "terminal fee-payer keypair does not expand to the durable payer",
                ));
            }
            let message_bytes = BASE64
                .decode(&journal.intent.message_base64)
                .map_err(|error| Error::new(format!("terminal message base64: {error}")))?;
            if sha256_hex(&message_bytes) != journal.intent.message_sha256 {
                return Err(refusal("terminal durable message digest changed"));
            }
            let message: VersionedMessage = bincode::deserialize(&message_bytes)
                .map_err(|error| Error::new(format!("terminal versioned message: {error}")))?;
            let transaction = VersionedTransaction::try_new(message, &[&payer])
                .map_err(|error| Error::new(format!("sign terminal transaction: {error}")))?;
            let signature =
                transaction.signatures.first().copied().ok_or_else(|| {
                    refusal("terminal signed transaction omitted payer signature")
                })?;
            let wire = bincode::serialize(&transaction)
                .map_err(|error| Error::new(format!("serialize terminal transaction: {error}")))?;
            if wire.len() != journal.intent.wire_bytes {
                return Err(refusal(
                    "terminal signed packet width differed from durable v0 geometry",
                ));
            }
            journal.signed_packet_base64 = Some(BASE64.encode(&wire));
            journal.expected_signature = Some(signature.to_string());
            journal.phase = StageJournalPhaseV1::SignedNotSubmitted;
            write_terminal_journal_v1(journal_path, journal, false)?;

            authenticate_terminal_cluster_v1(rpc, origin, expected_cluster)?;
            require_terminal_prestate_unchanged_v1(rpc, journal)?;
            let signature =
                submit_terminal_packet_once_v1(journal_path, journal, |signed_packet_base64| {
                    rpc.call_once(
                        "sendTransaction",
                        &json!([signed_packet_base64, {
                            "encoding":"base64",
                            "skipPreflight":false,
                            "preflightCommitment":"finalized",
                            "maxRetries":0
                        }]),
                    )?
                    .as_str()
                    .ok_or_else(|| Error::new("sendTransaction result was not a signature"))?
                    .parse::<Signature>()
                    .map_err(|error| Error::new(format!("terminal returned signature: {error}")))
                })?;
            wait_terminal_signature_v1(rpc, &signature.to_string())?;
            finalize_terminal_signature_v1(rpc, journal, &signature.to_string())?;
            write_terminal_journal_v1(journal_path, journal, false)
        }
    }
}

/// Cross the only terminal send boundary in one durable order.
///
/// `SignedNotSubmitted` proves the exact packet exists, but it does not prove
/// whether a process died before or during its first send. The transition to
/// `Submitted` is therefore fsynced before the one transport attempt. Every
/// failure after that point leaves the exact local signature in a poll-only
/// phase; callers may never infer permission to resend from a missing or
/// hostile RPC response.
fn submit_terminal_packet_once_v1(
    journal_path: &Path,
    journal: &mut DurableTerminalJournalV1,
    send_once: impl FnOnce(&str) -> Result<Signature>,
) -> Result<Signature> {
    authenticate_terminal_journal_v1(journal)?;
    if journal.phase != StageJournalPhaseV1::SignedNotSubmitted {
        return Err(refusal(
            "terminal send requires the exact newly signed, not-yet-crossed boundary",
        ));
    }
    let signed_packet_base64 = journal
        .signed_packet_base64
        .clone()
        .ok_or_else(|| refusal("terminal send omitted its durable signed packet"))?;
    let expected_signature = journal
        .expected_signature
        .as_deref()
        .ok_or_else(|| refusal("terminal send omitted its durable expected signature"))?
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("terminal expected signature: {error}")))?;

    journal.phase = StageJournalPhaseV1::Submitted;
    write_terminal_journal_v1(journal_path, journal, false)?;

    let returned = send_once(&signed_packet_base64)?;
    if returned != expected_signature {
        return Err(refusal(
            "RPC returned a signature different from the locally persisted packet",
        ));
    }
    Ok(expected_signature)
}

/// Retire one ambiguous submission that can never be included.
///
/// Durable proof that a landed packet's PREDICTIONS were re-derived, and that
/// nothing the chain saw was touched.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DurableReconciledEvidenceV1 {
    reason: String,
    landed_signature: String,
    slot: u64,
    /// The Resolution whose rule was applied, and the rule it names.
    deployment_program_id: String,
    deployment_elf_sha256: String,
    rule: String,
    /// The Rent sysvar the execution read, authenticated against this journal's
    /// own recorded prestate digest for that account.
    rent_sysvar_sha256: String,
    receipt_account: String,
    prior_ledger_rent_lamports: u64,
    prior_ledger_lamport_surplus: u64,
    ledger_rent_lamports: u64,
    ledger_lamport_surplus: u64,
    execution_clock_offset: usize,
    declared_return_data: bool,
}

/// Re-derive one landed packet's predictions under the current model.
///
/// `--supersede-unlandable` is the exit for a submission that can never land.
/// This is the other one, and market 1 is why it exists: the first
/// `ResolutionCloseFund` ever to execute LANDED, and its journal could not
/// certify it, because the driver that planned it modelled the receipt wrong in
/// three fields. Superseding refuses that packet by name -- "is known to
/// transaction history and must be reconciled, not retired" -- and there was no
/// reconciliation to send it to.
///
/// What may move and what may not is the whole of this act. A journal's intent
/// holds two kinds of thing: BYTES THE CHAIN SAW -- the message, the packet,
/// the signature, the instruction, the resolved keys, the balance vectors, the
/// prestate -- and PREDICTIONS derived from them. The first kind is never
/// touched here, and the proof is not a promise: every field of the intent is
/// compared before and after against a copy with only the re-derived fields
/// restored, so a reconciliation that moved anything else refuses itself.
///
/// The re-derivation is from RECORDED INPUTS, never from the chain's answer,
/// which would be fitting the prediction to the observation. The rent partition
/// needs the Rent sysvar the program executed against; the journal recorded that
/// account's digest in its prestate, so the live sysvar is read and required to
/// hash to it. A cluster that has moved its rate again fails that check and the
/// reconciliation refuses, correctly: the input can no longer be reconstructed.
///
/// It certifies nothing. Writing the journal reauthenticates it, and the next
/// ordinary pass polls the signature and certifies against the chain exactly as
/// it would for a packet planned right the first time.
fn operate_terminal_reconcile_landed_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    plan: &SuccessorPlan,
    landed: &str,
) -> Result<()> {
    if !arguments.execute {
        return Err(refusal(
            "--reconcile-landed rewrites a durable journal's predictions and requires --execute",
        ));
    }
    let (path, mut journal) =
        find_terminal_journal_by_signature_v1(&arguments.journal_dir, landed)?;
    match journal.phase {
        StageJournalPhaseV1::SignedNotSubmitted | StageJournalPhaseV1::Submitted => {}
        StageJournalPhaseV1::Finalized => {
            return Err(refusal(&format!(
                "terminal packet {landed} is already certified against its own predictions; there \
                 is nothing to reconcile"
            )));
        }
        StageJournalPhaseV1::Planned | StageJournalPhaseV1::Superseded => {
            return Err(refusal(&format!(
                "terminal packet {landed} is in phase {:?}, which carries no landed execution to \
                 reconcile a prediction against",
                journal.phase,
            )));
        }
    }
    if journal.reconciled.is_some() {
        return Err(refusal(&format!(
            "terminal packet {landed} has already been reconciled once; a second rewrite of a \
             durable prediction is not expressible"
        )));
    }

    // IT LANDED, AND IT IS THIS PACKET. Superseding proves the opposite pair.
    let transaction = finalized_terminal_transaction_v1(rpc, landed)?.ok_or_else(|| {
        refusal(&format!(
            "terminal packet {landed} is not finalized on chain, so there is no execution to \
             reconcile its prediction against"
        ))
    })?;
    let meta = transaction
        .get("meta")
        .ok_or_else(|| refusal("finalized terminal transaction omitted meta"))?;
    if !meta.get("err").is_none_or(Value::is_null) {
        return Err(refusal(&format!(
            "terminal packet {landed} landed with an error; a failed execution wrote no poststate \
             to reconcile against"
        )));
    }
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("finalized terminal transaction omitted slot"))?;
    let observed_packet = transaction
        .get("transaction")
        .and_then(Value::as_array)
        .and_then(|wire| wire.first())
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("finalized terminal history omitted base64 packet"))?;
    if Some(observed_packet) != journal.signed_packet_base64.as_deref() {
        return Err(refusal(&format!(
            "the finalized packet for {landed} differs from this journal's signed packet; nothing \
             about a reconciliation may be inferred across two different transactions"
        )));
    }

    let unchanged = journal.intent.clone();
    let resolution = Pubkey::from_str(&journal.intent.program_id)
        .map_err(|error| Error::new(format!("terminal journal program: {error}")))?;
    let rule =
        deployed_closure_rent_rule_v1(&plan.resolution, plan.checked_local_mutable_set.as_ref())?;
    if pubkey(&plan.resolution.program_id)? != resolution {
        return Err(refusal(&format!(
            "this journal's first-party program is {resolution}, which is not the Resolution \
             {} the plan pins, so the plan's deployment rule does not describe it",
            plan.resolution.program_id,
        )));
    }

    // THE RENT SYSVAR THE EXECUTION READ, AUTHENTICATED BY THE JOURNAL'S OWN
    // RECORD OF IT. Reading it live is not circular; adopting the chain's
    // answer for the field being predicted would be.
    let recorded_rent = journal
        .intent
        .prestate
        .get(&sysvar::rent::ID.to_string())
        .ok_or_else(|| {
            refusal("this journal's frame carries no Rent sysvar, so its rent rule cannot be rerun")
        })?
        .clone();
    let live = finalized_snapshot(rpc, &[sysvar::rent::ID])?;
    let live_rent_account = live.account(sysvar::rent::ID)?;
    if durable_observed_state(live_rent_account) != recorded_rent {
        return Err(refusal(&format!(
            "the Rent sysvar has moved since this packet executed (recorded {}, live {}), so the \
             input its rent partition was computed from can no longer be reconstructed",
            recorded_rent.data_sha256,
            durable_observed_state(live_rent_account).data_sha256,
        )));
    }
    let live_rent: Rent = bincode::deserialize(&live_rent_account.data)
        .map_err(|error| Error::new(format!("reconciled Rent sysvar: {error}")))?;

    // THE CLOSURE RECEIPT, IDENTIFIED BY DECODING RATHER THAN BY POSITION.
    let mut receipts = journal
        .intent
        .expected_accounts
        .iter()
        .filter_map(|(address, expected)| {
            let data = BASE64.decode(&expected.data_base64).ok()?;
            let receipt = SourceClosureReceiptV3::decode(&data).ok()?;
            (Pubkey::new_from_array(receipt.receipt_account).to_string() == *address)
                .then_some((address.clone(), receipt))
        })
        .collect::<Vec<_>>();
    let (receipt_address, receipt) = match receipts.len() {
        1 => receipts.remove(0),
        count => {
            return Err(refusal(&format!(
                "this journal predicts {count} self-naming Source closure receipts; a \
                 reconciliation of the closure rent partition needs exactly one"
            )));
        }
    };

    // THE CLOSING LEDGER, IDENTIFIED BY ITS OWN ARITHMETIC. Its width is what
    // the deployed program prices, and the receipt already states the three
    // classes that add up to its balance.
    let ledger_lamports = receipt
        .ledger_remaining_native_principal
        .checked_add(receipt.ledger_rent_lamports)
        .and_then(|value| value.checked_add(receipt.ledger_lamport_surplus))
        .ok_or_else(|| refusal("reconciled closure receipt ledger classes overflowed"))?;
    let mut ledgers = journal
        .intent
        .prestate
        .iter()
        .filter(|(address, state)| {
            state.lamports == ledger_lamports
                && state.data_len > 0
                && journal
                    .intent
                    .expected_accounts
                    .get(*address)
                    .is_some_and(|expected| {
                        expected.lamports_after_protocol == 0 && expected.data_base64.is_empty()
                    })
        })
        .map(|(address, state)| (address.clone(), state.data_len))
        .collect::<Vec<_>>();
    let (ledger_address, ledger_data_len) = match ledgers.len() {
        1 => ledgers.remove(0),
        count => {
            return Err(refusal(&format!(
                "{count} accounts in this frame close from exactly the {ledger_lamports} lamports \
                 the receipt classifies; the closing funding ledger must be exactly one"
            )));
        }
    };

    let partition = project_closure_rent_partition_v1(
        rule,
        ledger_data_len,
        &live_rent,
        ClosureRentPartitionV1 {
            ledger_rent_lamports: receipt.ledger_rent_lamports,
            ledger_lamport_surplus: receipt.ledger_lamport_surplus,
        },
    )?;
    let mut reconciled_receipt = receipt;
    reconciled_receipt.ledger_rent_lamports = partition.ledger_rent_lamports;
    reconciled_receipt.ledger_lamport_surplus = partition.ledger_lamport_surplus;
    let execution_clock = ExecutionClockFieldV1 {
        offset: closure_receipt_closed_at_offset_v1(&reconciled_receipt)?,
        planned_unix_timestamp: u64::try_from(journal.intent.observation_unix_timestamp)
            .map_err(|_| refusal("this journal records a negative observation clock"))?,
        ceiling_unix_timestamp: u64::try_from(journal.intent.observation_unix_timestamp)
            .map_err(|_| refusal("this journal records a negative observation clock"))?
            .checked_add(TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS)
            .ok_or_else(|| refusal("reconciled execution-clock ceiling overflowed"))?,
    };
    require_derived_execution_clock_ceiling_v1(execution_clock)?;
    // The codec re-runs its own arithmetic on the repriced body, so a partition
    // that broke the receipt's exhaustive refund equation refuses here rather
    // than reaching a journal.
    let body = reconciled_receipt
        .to_bytes()
        .map_err(|error| Error::new(format!("reconciled closure receipt: {error:?}")))?
        .to_vec();

    let expected = journal
        .intent
        .expected_accounts
        .get_mut(&receipt_address)
        .ok_or_else(|| refusal("the reconciled receipt poststate disappeared from its own map"))?;
    if expected.owner != resolution.to_string() {
        return Err(refusal(
            "the reconciled closure receipt is not predicted to be Resolution-owned",
        ));
    }
    expected.data_base64 = BASE64.encode(&body);
    expected.data_sha256 = sha256_hex(&body);
    expected.execution_clock = Some(durable_execution_clock_v1(execution_clock));
    journal.intent.expected_return_data = Some(DurableReturnDataV1 {
        producer: resolution.to_string(),
        body_base64: BASE64.encode(&body),
        body_sha256: sha256_hex(&body),
        execution_clock: Some(durable_execution_clock_v1(execution_clock)),
    });

    authenticate_terminal_reconciliation_scope_v1(&unchanged, &journal.intent, &receipt_address)?;

    journal.reconciled = Some(DurableReconciledEvidenceV1 {
        reason: format!(
            "the packet landed and its plan modelled the deployed Resolution's closure receipt \
             with the rate the ledger was FUNDED at; {} prices {ledger_data_len} bytes from {} \
             instead, and the plan declared no return data for a route that always returns the \
             same bytes it writes",
            plan.resolution.program_id,
            match rule {
                DeployedClosureRentRuleV1::LiveRentSysvar => "the live Rent sysvar",
                DeployedClosureRentRuleV1::FundedRate => "the rate the ledger was funded at",
            },
        ),
        landed_signature: landed.into(),
        slot,
        deployment_program_id: plan.resolution.program_id.clone(),
        deployment_elf_sha256: plan.resolution.checked_candidate_elf_sha256.clone(),
        rule: format!("{rule:?}"),
        rent_sysvar_sha256: recorded_rent.data_sha256.clone(),
        receipt_account: receipt_address.clone(),
        prior_ledger_rent_lamports: receipt.ledger_rent_lamports,
        prior_ledger_lamport_surplus: receipt.ledger_lamport_surplus,
        ledger_rent_lamports: partition.ledger_rent_lamports,
        ledger_lamport_surplus: partition.ledger_lamport_surplus,
        execution_clock_offset: execution_clock.offset,
        declared_return_data: unchanged.expected_return_data.is_none(),
    });
    // The writer refreshes the STATE digest and authenticates what it
    // publishes; the intent digest belongs to whoever changed the intent, which
    // is this act. Getting that wrong is caught rather than shipped: the
    // writer's own reauthentication refused the first attempt at this.
    journal.intent_sha256 = sha256_hex(&serde_json::to_vec(&journal.intent)?);
    // The writer owns the state-digest refresh and authenticates what it
    // publishes, so the reconciled intent is proven canonical -- rule shapes,
    // the stated-once return body and all -- before it reaches disk.
    write_terminal_journal_v1(&path, &mut journal, false)?;
    terminal_stdout_v1(json!({
        "status": "reconciled",
        "landedSignature": landed,
        "slot": slot,
        "journal": path.display().to_string(),
        "receipt": receipt_address,
        "fundingLedger": ledger_address,
        "ledgerRentLamports": partition.ledger_rent_lamports,
        "ledgerLamportSurplus": partition.ledger_lamport_surplus,
        "executionClockOffset": execution_clock.offset,
        "message": "The landed packet's predictions were re-derived from its own recorded inputs; nothing signed moved. Resume the sequence to certify it."
    }))
}

/// NOTHING ELSE MOVED, AND THIS IS THE PROOF RATHER THAN THE CLAIM.
///
/// A journal's intent holds bytes the chain saw and predictions derived from
/// them. A reconciliation may re-derive exactly one account poststate and the
/// return data stated from those same bytes; every other field -- the message,
/// the packet, the instruction, the resolved keys, the balance vectors, the
/// prestate, the deltas, every other poststate -- must come back identical.
/// Restoring the two re-derived fields and demanding equality proves that in
/// one comparison, with no field of the intent left unchecked and no list here
/// to fall behind the struct.
fn authenticate_terminal_reconciliation_scope_v1(
    unchanged: &DurableTerminalIntentV1,
    reconciled: &DurableTerminalIntentV1,
    receipt_address: &str,
) -> Result<()> {
    let mut restored = reconciled.clone();
    restored.expected_return_data = unchanged.expected_return_data.clone();
    let before = unchanged
        .expected_accounts
        .get(receipt_address)
        .ok_or_else(|| {
            refusal("a terminal reconciliation names a poststate its journal never declared")
        })?
        .clone();
    restored
        .expected_accounts
        .insert(receipt_address.to_owned(), before);
    if restored != *unchanged {
        return Err(refusal(
            "a terminal reconciliation moved something other than the one poststate it \
             re-derived; nothing the chain saw may change",
        ));
    }
    Ok(())
}

/// The canonical journal in this directory carrying one exact signature.
fn find_terminal_journal_by_signature_v1(
    journal_dir: &Path,
    signature: &str,
) -> Result<(PathBuf, DurableTerminalJournalV1)> {
    let mut candidates = vec![journal_dir.join("12-resolution-receipt-prepay.json")];
    candidates.extend(
        TerminalStageV1::ORDERED
            .into_iter()
            .map(|stage| journal_dir.join(stage_journal_name_v1(stage))),
    );
    for path in candidates {
        if !path.exists() {
            continue;
        }
        let journal = read_terminal_journal_v1(&path)?;
        if journal.expected_signature.as_deref() == Some(signature) {
            return Ok((path, journal));
        }
    }
    Err(refusal(&format!(
        "no canonical terminal journal in this directory carries the signature {signature}"
    )))
}

/// This is the only exit from `Submitted` other than finality, and it is
/// deliberately explicit: the caller names the exact signature it means, so
/// nothing here can retire a packet by inference, by timeout, or in bulk. Two
/// observations have to hold together, and neither alone is enough --
///
///   * the finalized block height has passed the packet's own
///     `lastValidBlockHeight`, after which no validator will accept that
///     blockhash from anyone, so the packet's fate is settled forever; and
///   * `getSignatureStatuses` with `searchTransactionHistory` still does not
///     know the signature, so what it settled on is "never included".
///
/// The retired journal keeps its packet and its signature and gains the two
/// readings, then moves aside under a name that carries the signature. The
/// canonical path is left free for a fresh plan -- which will carry a
/// different blockhash and therefore a different signature, so no resubmission
/// of the retired packet is even expressible.
fn operate_terminal_supersede_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    retired: &str,
) -> Result<()> {
    if !arguments.execute {
        return Err(refusal(
            "--supersede-unlandable changes a durable journal and requires --execute",
        ));
    }
    let (path, mut journal) =
        find_terminal_journal_by_signature_v1(&arguments.journal_dir, retired)?;
    match journal.phase {
        StageJournalPhaseV1::SignedNotSubmitted | StageJournalPhaseV1::Submitted => {}
        StageJournalPhaseV1::Finalized => {
            return Err(refusal(&format!(
                "terminal packet {retired} is finalized on chain and is not unlandable"
            )));
        }
        StageJournalPhaseV1::Planned | StageJournalPhaseV1::Superseded => {
            return Err(refusal(&format!(
                "terminal packet {retired} is in phase {:?} and has no ambiguous submission to retire",
                journal.phase
            )));
        }
    }
    let observed_block_height = rpc
        .call("getBlockHeight", &json!([{"commitment":"finalized"}]))?
        .as_u64()
        .ok_or_else(|| Error::new("getBlockHeight result was not a u64"))?;
    if observed_block_height <= journal.intent.last_valid_block_height {
        return Err(refusal(&format!(
            "terminal packet {retired} may still be included: finalized block height \
             {observed_block_height} has not passed its last valid height {}",
            journal.intent.last_valid_block_height
        )));
    }
    let status = rpc.call(
        "getSignatureStatuses",
        &json!([[retired], {"searchTransactionHistory":true}]),
    )?;
    let known = status
        .get("value")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .is_some_and(|value| !value.is_null());
    if known {
        return Err(refusal(&format!(
            "terminal packet {retired} is known to transaction history and must be reconciled, \
             not retired"
        )));
    }
    let observed_slot = rpc.finalized_slot()?;
    journal.phase = StageJournalPhaseV1::Superseded;
    journal.superseded = Some(DurableSupersededEvidenceV1 {
        reason: format!(
            "the packet exceeded the 200,000-CU default meter and can never land; its blockhash \
             expired at last valid block height {} and the signature is absent from transaction \
             history at finalized block height {observed_block_height}",
            journal.intent.last_valid_block_height
        ),
        retired_signature: retired.into(),
        last_valid_block_height: journal.intent.last_valid_block_height,
        observed_block_height,
        observed_slot,
    });
    // The writer is the one that refreshes the digest, and it refuses an update
    // whose in-memory digest has already moved -- that guard is how a lost
    // update is caught, so refreshing here would defeat it. It authenticates
    // the refreshed journal before it publishes, so the retired shape is still
    // proven canonical before it reaches disk.
    write_terminal_journal_v1(&path, &mut journal, false)?;
    let retired_path = path.with_file_name(format!(
        "{}.superseded.{retired}",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| refusal("terminal journal path had no file name"))?
    ));
    if retired_path.exists() {
        return Err(refusal(
            "a retired terminal journal already exists under that signature",
        ));
    }
    fs::rename(&path, &retired_path)?;
    terminal_stdout_v1(json!({
        "status": "superseded",
        "retiredSignature": retired,
        "journal": retired_path.display().to_string(),
        "lastValidBlockHeight": journal.intent.last_valid_block_height,
        "observedBlockHeight": observed_block_height,
        "observedSlot": observed_slot,
        "message": "The ambiguous submission can never be included and is retired; plan the stage fresh."
    }))
}

/// A route that does not fit the default meter may not be planned or signed
/// without declaring its budget.
///
/// Stated by ROUTE rather than by number, so a later re-pin of the measured
/// figure can never retroactively refuse a journal that already finalized, and
/// checked at the two doors where a durable obligation is created rather than
/// on every read -- market 1's retired packet IS an undeclared CloseFund, and
/// it has to stay readable to be retired.
fn require_declared_budget_before_signing_v1(intent: &DurableTerminalIntentV1) -> Result<()> {
    if terminal_route_requires_declared_budget_v1(&intent.mutation)
        && intent.compute_unit_limit.is_none()
    {
        return Err(refusal(&format!(
            "terminal {:?} durable message declares no ComputeBudget limit and this route does \
             not fit Solana's 200,000-CU default meter; it consumed 200,000 of 200,000 on devnet \
             2026-09-04 and can never land",
            intent.mutation
        )));
    }
    Ok(())
}

fn terminal_retired_refusal_v1(journal: &DurableTerminalJournalV1) -> Error {
    refusal(&format!(
        "terminal packet {} was superseded and may never be signed, submitted or polled again: {}",
        journal.expected_signature.as_deref().unwrap_or("<unknown>"),
        journal
            .superseded
            .as_ref()
            .map_or("<no reason recorded>", |evidence| evidence.reason.as_str())
    ))
}

fn authenticate_terminal_journal_v1(journal: &DurableTerminalJournalV1) -> Result<()> {
    terminal_journal_cluster_v1(journal)?;
    if sha256_hex(&serde_json::to_vec(&journal.intent)?) != journal.intent_sha256
        || terminal_journal_state_digest_v1(journal)? != journal.state_sha256
    {
        return Err(refusal(
            "terminal journal schema, cluster, intent, origin, authorization, or state digest changed",
        ));
    }
    authenticate_terminal_intent_arithmetic_v1(&journal.intent)?;
    match journal.phase {
        StageJournalPhaseV1::Planned
            if journal.signed_packet_base64.is_none()
                && journal.expected_signature.is_none()
                && journal.finalized.is_none()
                && journal.superseded.is_none()
                && journal.reconciled.is_none() => {}
        StageJournalPhaseV1::SignedNotSubmitted | StageJournalPhaseV1::Submitted
            if journal.signed_packet_base64.is_some()
                && journal.expected_signature.is_some()
                && journal.finalized.is_none()
                && journal.superseded.is_none() => {}
        StageJournalPhaseV1::Finalized
            if journal.signed_packet_base64.is_some()
                && journal.expected_signature.is_some()
                && journal.finalized.is_some()
                && journal.superseded.is_none() => {}
        // A retired packet keeps every byte it had -- the packet, the
        // signature, the intent -- and gains the observation that closed it. It
        // may never also carry finalized evidence: those are the two answers to
        // the same question and a journal that claimed both would be claiming a
        // transaction both landed and cannot land.
        StageJournalPhaseV1::Superseded
            if journal.signed_packet_base64.is_some()
                && journal.expected_signature.is_some()
                && journal.finalized.is_none()
                && journal.reconciled.is_none()
                && journal.superseded.as_ref().is_some_and(|evidence| {
                    journal.expected_signature.as_deref()
                        == Some(evidence.retired_signature.as_str())
                        && evidence.observed_block_height > evidence.last_valid_block_height
                        && evidence.last_valid_block_height
                            == journal.intent.last_valid_block_height
                        && !evidence.reason.is_empty()
                }) => {}
        _ => {
            return Err(refusal(
                "terminal journal phase/evidence shape is noncanonical",
            ));
        }
    }
    if let Some(evidence) = &journal.reconciled
        && journal.expected_signature.as_deref() != Some(evidence.landed_signature.as_str())
    {
        return Err(refusal(
            "terminal reconciliation evidence names another signature than the journal it sits in",
        ));
    }
    if journal.phase != StageJournalPhaseV1::Planned {
        if !journal.authorized_mutation {
            return Err(refusal(
                "signed terminal journal omitted durable mutation authorization",
            ));
        }
        authenticate_terminal_signed_packet_v1(journal)?;
    }
    if let Some(finalized) = &journal.finalized {
        let packet = BASE64
            .decode(
                journal
                    .signed_packet_base64
                    .as_deref()
                    .ok_or_else(|| refusal("finalized journal omitted packet"))?,
            )
            .map_err(|error| Error::new(format!("finalized journal packet base64: {error}")))?;
        if finalized.signature != journal.expected_signature.as_deref().unwrap_or_default()
            || finalized.packet_sha256 != sha256_hex(&packet)
            || finalized.fee_lamports != journal.intent.transaction_fee_lamports
        {
            return Err(refusal(
                "finalized terminal evidence signature, packet, or fee binding changed",
            ));
        }
    }
    Ok(())
}

fn authenticate_terminal_intent_arithmetic_v1(intent: &DurableTerminalIntentV1) -> Result<()> {
    authenticate_terminal_message_decompilation_v1(intent)?;
    if checked_terminal_delta_sum_v1(intent.protocol_lamport_deltas.values().copied())? != 0 {
        return Err(refusal(
            "terminal durable protocol/prepay lamport vector was not exactly conserving before fee",
        ));
    }
    let instruction_data = BASE64
        .decode(&intent.instruction_data_base64)
        .map_err(|error| Error::new(format!("terminal instruction base64: {error}")))?;
    if sha256_hex(&instruction_data) != intent.instruction_data_sha256
        || intent.program_class != TerminalAddressClassV1::InlineProgram
    {
        return Err(refusal(
            "terminal instruction bytes or program address class changed",
        ));
    }
    let lookup_addresses = intent
        .lookup_table_addresses
        .iter()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal lookup address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if lookup_addresses
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != lookup_addresses.len()
        || pubkey_vector_sha256(&lookup_addresses) != intent.lookup_table_addresses_sha256
    {
        return Err(refusal(
            "terminal frozen lookup addresses contained a duplicate or digest mismatch",
        ));
    }
    let resolved = intent
        .resolved_account_keys
        .iter()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal resolved address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if resolved.iter().copied().collect::<BTreeSet<_>>().len() != resolved.len()
        || intent.pre_balances.len() != resolved.len()
        || intent.post_balances.len() != resolved.len()
        || intent.resolved_account_keys.first() != Some(&intent.payer)
    {
        return Err(refusal(
            "terminal resolved key or balance vector was duplicate, mis-sized, or omitted payer first",
        ));
    }
    let writable = intent
        .accounts
        .iter()
        .filter(|account| account.writable)
        .map(|account| account.address.as_str())
        .chain(std::iter::once(intent.payer.as_str()))
        .collect::<BTreeSet<_>>();
    let missing_poststate = writable
        .iter()
        .filter(|key| !intent.expected_accounts.contains_key(**key))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if !missing_poststate.is_empty() {
        return Err(refusal(&format!(
            "terminal durable intent omitted a poststate for writable {missing_poststate:?}"
        )));
    }
    // The persisted twin of the semantic report's rule, split the same way and
    // for the same reason: a ZERO delta on a readonly account is the claim that
    // its lamports do not move, which is checked against its own recorded
    // pre/post balances. A NONZERO one is a claim the frame cannot make good.
    let moved_readonly = intent
        .protocol_lamport_deltas
        .iter()
        .filter(|(key, delta)| **delta != 0 && !writable.contains(key.as_str()))
        .map(|(key, delta)| format!("{key}:{delta}"))
        .collect::<Vec<_>>();
    if !moved_readonly.is_empty() {
        return Err(refusal(&format!(
            "terminal durable intent moved lamports on readonly accounts {moved_readonly:?}"
        )));
    }
    for (key, expected) in &intent.expected_accounts {
        let exact_data = match &expected.lookup_table {
            None => {
                let data = BASE64.decode(&expected.data_base64).map_err(|error| {
                    Error::new(format!("terminal expected account base64: {error}"))
                })?;
                if sha256_hex(&data) != expected.data_sha256 {
                    return Err(refusal("terminal expected account byte digest changed"));
                }
                if let Some(clock) = &expected.execution_clock {
                    authenticate_execution_clock_shape_v1(
                        &format!("terminal durable poststate {key}"),
                        clock,
                        data.len(),
                        intent.observation_unix_timestamp,
                    )?;
                }
                true
            }
            Some(table) => {
                let addresses = table
                    .addresses
                    .iter()
                    .map(|key| {
                        Pubkey::from_str(key).map_err(|error| {
                            Error::new(format!("terminal expected ALT address: {error}"))
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                if !expected.data_base64.is_empty()
                    || !expected.data_sha256.is_empty()
                    || expected.execution_clock.is_some()
                    || expected.owner != lookup_table_program::id().to_string()
                    || expected.executable
                    || addresses.iter().copied().collect::<BTreeSet<_>>().len() != addresses.len()
                    || table
                        .authority
                        .as_deref()
                        .map(Pubkey::from_str)
                        .transpose()
                        .is_err()
                {
                    return Err(refusal(
                        "terminal expected ALT rule was malformed or carried parallel byte truth",
                    ));
                }
                false
            }
        };
        if key != &expected.address
            || (key != &intent.payer
                && expected.lamports_after_protocol != expected.lamports_after_fee)
            || (key == &intent.payer
                && expected
                    .lamports_after_protocol
                    .checked_sub(intent.transaction_fee_lamports)
                    != Some(expected.lamports_after_fee))
        {
            return Err(refusal(
                "terminal expected account key, bytes, or fee arithmetic changed",
            ));
        }
        let _ = exact_data;
    }
    for (index, key) in intent.resolved_account_keys.iter().enumerate() {
        let prestate = intent
            .prestate
            .get(key)
            .ok_or_else(|| refusal("terminal resolved key omitted exact prestate"))?;
        if prestate.address != *key || prestate.lamports != intent.pre_balances[index] {
            return Err(refusal(
                "terminal resolved key pre-balance differed from persisted account state",
            ));
        }
        let delta = intent
            .protocol_lamport_deltas
            .get(key)
            .copied()
            .unwrap_or(0);
        let after_protocol = checked_apply_lamport_delta(intent.pre_balances[index], delta)?;
        let expected = if key == &intent.payer {
            after_protocol
                .checked_sub(intent.transaction_fee_lamports)
                .ok_or_else(|| refusal("terminal payer fee arithmetic underflowed"))?
        } else {
            after_protocol
        };
        if intent.post_balances[index] != expected
            || intent
                .expected_accounts
                .get(key)
                .is_some_and(|account| account.lamports_after_fee != intent.post_balances[index])
        {
            return Err(refusal(
                "terminal durable post-balance disagreed with protocol delta or expected account",
            ));
        }
    }
    if let Some(expected) = &intent.expected_return_data {
        let body = BASE64
            .decode(&expected.body_base64)
            .map_err(|error| Error::new(format!("terminal expected return base64: {error}")))?;
        if sha256_hex(&body) != expected.body_sha256 {
            return Err(refusal("terminal expected return-data digest changed"));
        }
        if let Some(clock) = &expected.execution_clock {
            authenticate_execution_clock_shape_v1(
                "terminal durable return data",
                clock,
                body.len(),
                intent.observation_unix_timestamp,
            )?;
            // STATED ONCE AND SPENT TWICE.
            //
            // `b00dcad96` made the closure receipt one statement of bytes that
            // the route both WRITES and RETURNS, because a plan that says them
            // twice can disagree with itself on top of disagreeing with the
            // chain. A field the plan cannot bind is the case where that
            // matters most, so a return body carrying such a field must be the
            // exact bytes of exactly one declared account poststate, and that
            // account's rule must be the same rule.
            let twins = intent
                .expected_accounts
                .iter()
                .filter(|(_, account)| account.body_matches(&body))
                .collect::<Vec<_>>();
            let [(address, account)] = twins.as_slice() else {
                return Err(refusal(&format!(
                    "terminal durable return data declares an execution clock but {} \
                     declared account poststates carry those exact bytes; it must be exactly one",
                    twins.len(),
                )));
            };
            if account.execution_clock.as_ref() != Some(clock) {
                return Err(refusal(&format!(
                    "terminal durable return data and the account {address} it is written \
                     to state different execution-clock rules for one set of bytes"
                )));
            }
        }
    }
    let pre_total = intent
        .pre_balances
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)))
        .ok_or_else(|| refusal("terminal pre-balance total overflowed"))?;
    let post_total = intent
        .post_balances
        .iter()
        .try_fold(0_u128, |sum, value| sum.checked_add(u128::from(*value)))
        .ok_or_else(|| refusal("terminal post-balance total overflowed"))?;
    if pre_total.checked_sub(post_total) != Some(u128::from(intent.transaction_fee_lamports)) {
        return Err(refusal(
            "terminal complete balance vector concealed a lamport delta beyond the exact fee",
        ));
    }
    Ok(())
}

fn authenticate_terminal_message_decompilation_v1(intent: &DurableTerminalIntentV1) -> Result<()> {
    let message_bytes = BASE64
        .decode(&intent.message_base64)
        .map_err(|error| Error::new(format!("terminal durable message base64: {error}")))?;
    if sha256_hex(&message_bytes) != intent.message_sha256 {
        return Err(refusal("terminal durable message digest changed"));
    }
    let versioned: VersionedMessage = bincode::deserialize(&message_bytes)
        .map_err(|error| Error::new(format!("terminal durable versioned message: {error}")))?;
    if versioned.serialize() != message_bytes {
        return Err(refusal(
            "terminal durable message encoding was noncanonical",
        ));
    }
    let VersionedMessage::V0(message) = &versioned else {
        return Err(refusal("terminal durable message was not v0"));
    };
    let payer = Pubkey::from_str(&intent.payer)
        .map_err(|error| Error::new(format!("terminal durable payer: {error}")))?;
    let recent_blockhash = intent
        .recent_blockhash
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("terminal durable blockhash: {error}")))?;
    if message.recent_blockhash != recent_blockhash
        || message.header.num_required_signatures != 1
        || message.header.num_readonly_signed_accounts != 0
        || message.account_keys.first() != Some(&payer)
    {
        return Err(refusal(
            "terminal message blockhash, payer, or signer header differed from intent",
        ));
    }
    // EXACTLY ONE FIRST-PARTY INSTRUCTION, OPTIONALLY PRECEDED BY EXACTLY ONE
    // ComputeBudget LIMIT WHOSE VALUE IS THE ONE THE INTENT RECORDS.
    //
    // The prefix is pinned by PROGRAM and by its exact encoded data -- built
    // here from the same SDK constructor that compiled it, so no second copy of
    // the discriminant or the little-endian width exists to drift -- rather
    // than skipped, because "ignore the instructions before the interesting
    // one" is how a substituted prefix gets past a verifier that reads the
    // interesting one correctly. `direct_trade.rs` pins its two the same way.
    //
    // The ROUTE requirement is deliberately NOT here. This function is what
    // every read of a journal runs, and the journal that most needs reading is
    // the one whose defect is exactly an undeclared CloseFund: market 1's
    // retired packet. Refusing to decode it would be refusing to look at the
    // evidence. What may not happen to such a journal is that it gets PLANNED
    // or SIGNED, and `require_declared_budget_before_signing_v1` stands at both
    // of those doors.
    let program_at = |index: u8| message.account_keys.get(usize::from(index)).copied();
    let instruction = match (message.instructions.as_slice(), intent.compute_unit_limit) {
        ([first_party], None) => first_party,
        ([limit, first_party], Some(declared)) => {
            if program_at(limit.program_id_index) != Some(compute_budget::ID)
                || !limit.accounts.is_empty()
                || limit.data != ComputeBudgetInstruction::set_compute_unit_limit(declared).data
            {
                return Err(refusal(&format!(
                    "terminal durable ComputeBudget prefix was not exactly one \
                     SetComputeUnitLimit for the recorded budget of {declared}"
                )));
            }
            first_party
        }
        _ => {
            return Err(refusal(&format!(
                "terminal durable message carried {} instruction(s) against a recorded budget of \
                 {:?}; the shape is exactly one first-party instruction, optionally preceded by \
                 exactly one ComputeBudget limit",
                message.instructions.len(),
                intent.compute_unit_limit
            )));
        }
    };
    // Which makes a second ComputeBudget in the first-party slot -- two
    // prefixes -- refuse for its own reason rather than incidentally.
    if program_at(instruction.program_id_index) == Some(compute_budget::ID) {
        return Err(refusal(
            "terminal durable first-party instruction was itself a ComputeBudget declaration",
        ));
    }
    let resolved = intent
        .resolved_account_keys
        .iter()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal resolved message key: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let program_id = Pubkey::from_str(&intent.program_id)
        .map_err(|error| Error::new(format!("terminal durable program: {error}")))?;
    if resolved.get(..message.account_keys.len()) != Some(message.account_keys.as_slice())
        || resolved.get(usize::from(instruction.program_id_index)) != Some(&program_id)
        || !message.account_keys.contains(&program_id)
        || intent.program_class != TerminalAddressClassV1::InlineProgram
    {
        return Err(refusal(
            "terminal compiled program index was not the exact inline program identity",
        ));
    }
    let data = BASE64
        .decode(&intent.instruction_data_base64)
        .map_err(|error| Error::new(format!("terminal durable instruction base64: {error}")))?;
    if instruction.data != data || instruction.accounts.len() != intent.accounts.len() {
        return Err(refusal(
            "terminal compiled instruction bytes or account width differed from intent",
        ));
    }
    let static_len = message.account_keys.len();
    let required = usize::from(message.header.num_required_signatures);
    let writable_signed_end = required
        .checked_sub(usize::from(message.header.num_readonly_signed_accounts))
        .ok_or_else(|| refusal("terminal signed account header underflowed"))?;
    let writable_unsigned_end = static_len
        .checked_sub(usize::from(message.header.num_readonly_unsigned_accounts))
        .ok_or_else(|| refusal("terminal unsigned account header underflowed"))?;
    for (compiled_index, expected) in instruction.accounts.iter().zip(&intent.accounts) {
        let index = usize::from(*compiled_index);
        let key = resolved
            .get(index)
            .ok_or_else(|| refusal("terminal compiled account index exceeded resolved keys"))?;
        let expected_key = Pubkey::from_str(&expected.address)
            .map_err(|error| Error::new(format!("terminal intended account: {error}")))?;
        let signer = index < required;
        let writable = if index < required {
            index < writable_signed_end
        } else if index < static_len {
            index < writable_unsigned_end
        } else {
            index < static_len + intent.loaded_writable.len()
        };
        let loaded = index >= static_len;
        let class_placement_ok = match expected.class {
            TerminalAddressClassV1::LookupStable => loaded,
            TerminalAddressClassV1::InlineSigner
            | TerminalAddressClassV1::InlineProgram
            | TerminalAddressClassV1::InlineRequestBound => !loaded,
        };
        if *key != expected_key
            || signer != expected.signer
            || writable != expected.writable
            || !class_placement_ok
        {
            // Four independent conjuncts over every account in the frame. The
            // relation is unchanged; it now says which account and which of the
            // four stopped holding, because "something in this frame differed"
            // is a refusal that cannot be acted on.
            let mut differed = Vec::new();
            if *key != expected_key {
                differed.push(format!("key {key} != intended {expected_key}"));
            }
            if signer != expected.signer {
                differed.push(format!("signer {signer} != intended {}", expected.signer));
            }
            if writable != expected.writable {
                differed.push(format!(
                    "writable {writable} != intended {}",
                    expected.writable
                ));
            }
            if !class_placement_ok {
                differed.push(format!(
                    "class {:?} requires {} placement but this key is {}",
                    expected.class,
                    match expected.class {
                        TerminalAddressClassV1::LookupStable => "lookup-loaded",
                        _ => "static",
                    },
                    if loaded { "lookup-loaded" } else { "static" }
                ));
            }
            return Err(refusal(&format!(
                "terminal {:?} compiled account {key} at frame index {} (resolved index {index} \
                 of {static_len} static) differed from intent: {}",
                intent.mutation,
                instruction
                    .accounts
                    .iter()
                    .position(|value| usize::from(*value) == index)
                    .unwrap_or_default(),
                differed.join("; ")
            )));
        }
    }
    match &intent.lookup_table {
        Some(table) => {
            let table = Pubkey::from_str(table)
                .map_err(|error| Error::new(format!("terminal durable lookup table: {error}")))?;
            if message.address_table_lookups.len() != 1
                || message.address_table_lookups[0].account_key != table
            {
                return Err(refusal(
                    "terminal compiled lookup identity differed from durable intent",
                ));
            }
            let addresses = intent
                .lookup_table_addresses
                .iter()
                .map(|key| {
                    Pubkey::from_str(key).map_err(|error| {
                        Error::new(format!("terminal durable lookup address: {error}"))
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let lookup = &message.address_table_lookups[0];
            let projected_writable = lookup
                .writable_indexes
                .iter()
                .map(|index| addresses.get(usize::from(*index)).map(ToString::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| refusal("terminal writable lookup index exceeded durable table"))?;
            let projected_readonly = lookup
                .readonly_indexes
                .iter()
                .map(|index| addresses.get(usize::from(*index)).map(ToString::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| refusal("terminal readonly lookup index exceeded durable table"))?;
            if projected_writable != intent.loaded_writable
                || projected_readonly != intent.loaded_readonly
            {
                return Err(refusal(
                    "terminal lookup indices did not reproduce durable loaded address order",
                ));
            }
        }
        None => {
            if !message.address_table_lookups.is_empty()
                || !intent.lookup_table_addresses.is_empty()
                || !intent.loaded_writable.is_empty()
                || !intent.loaded_readonly.is_empty()
            {
                return Err(refusal(
                    "inline terminal infrastructure message concealed lookup state",
                ));
            }
        }
    }
    let expected_wire = 1_usize
        .checked_add(64)
        .and_then(|value| value.checked_add(message_bytes.len()))
        .ok_or_else(|| refusal("terminal durable wire geometry overflowed"))?;
    if intent.wire_bytes != expected_wire || intent.wire_bytes > PACKET_DATA_BYTES {
        return Err(refusal(
            "terminal durable wire geometry differed from exact one-signature message",
        ));
    }
    Ok(())
}

fn authenticate_terminal_signed_packet_v1(journal: &DurableTerminalJournalV1) -> Result<()> {
    let wire = BASE64
        .decode(
            journal
                .signed_packet_base64
                .as_deref()
                .ok_or_else(|| refusal("signed terminal journal omitted packet"))?,
        )
        .map_err(|error| Error::new(format!("signed terminal packet base64: {error}")))?;
    if wire.len() != journal.intent.wire_bytes {
        return Err(refusal(
            "signed terminal packet width differed from durable intent",
        ));
    }
    let transaction: VersionedTransaction = bincode::deserialize(&wire)
        .map_err(|error| Error::new(format!("signed terminal packet: {error}")))?;
    if bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("reencode signed terminal packet: {error}")))?
        != wire
    {
        return Err(refusal("signed terminal packet encoding was noncanonical"));
    }
    let message_bytes = BASE64
        .decode(&journal.intent.message_base64)
        .map_err(|error| Error::new(format!("durable terminal message base64: {error}")))?;
    if sha256_hex(&message_bytes) != journal.intent.message_sha256
        || transaction.message.serialize() != message_bytes
    {
        return Err(refusal(
            "signed terminal packet message differed from the durable exact message",
        ));
    }
    let payer = Pubkey::from_str(&journal.intent.payer)
        .map_err(|error| Error::new(format!("durable terminal payer: {error}")))?;
    let VersionedMessage::V0(message) = &transaction.message else {
        return Err(refusal("signed terminal packet was not v0"));
    };
    if message.header.num_required_signatures != 1
        || message.account_keys.first() != Some(&payer)
        || transaction.signatures.len() != 1
    {
        return Err(refusal(
            "signed terminal packet signer set was not exactly the durable payer",
        ));
    }
    let signature = transaction
        .signatures
        .first()
        .ok_or_else(|| refusal("signed terminal packet omitted signature"))?;
    if !signature.verify(payer.as_ref(), &message_bytes)
        || journal.expected_signature.as_deref() != Some(&signature.to_string())
    {
        return Err(refusal(
            "terminal signature was invalid or differed from the durable expected signature",
        ));
    }
    Ok(())
}

fn refresh_terminal_journal_digest_v1(journal: &mut DurableTerminalJournalV1) -> Result<()> {
    journal.state_sha256 = terminal_journal_state_digest_v1(journal)?;
    Ok(())
}

fn terminal_journal_state_digest_v1(journal: &DurableTerminalJournalV1) -> Result<String> {
    let mut canonical = journal.clone();
    canonical.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn reconcile_terminal_signature_v1(
    rpc: &mut Rpc,
    path: &Path,
    journal: &mut DurableTerminalJournalV1,
) -> Result<()> {
    let signature = journal
        .expected_signature
        .clone()
        .ok_or_else(|| refusal("terminal signed phase omitted its expected signature"))?;
    if finalized_terminal_transaction_v1(rpc, &signature)?.is_none() {
        return Err(Error::new(format!(
            "REFUSED: terminal transaction {signature} is not finalized. The durable {:?} journal is ambiguous and will not re-sign or resubmit; reconcile only this exact signature",
            journal.phase
        )));
    }
    finalize_terminal_signature_v1(rpc, journal, &signature)?;
    write_terminal_journal_v1(path, journal, false)
}

fn wait_terminal_signature_v1(rpc: &mut Rpc, signature: &str) -> Result<()> {
    let deadline = Instant::now() + TERMINAL_FINALITY_WAIT;
    while Instant::now() < deadline {
        if finalized_terminal_transaction_v1(rpc, signature)?.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(Error::new(format!(
        "terminal transaction {signature} did not reach finalized history within 300 seconds; durable signature retained and no replay attempted"
    )))
}

fn finalized_terminal_transaction_v1(rpc: &mut Rpc, signature: &str) -> Result<Option<Value>> {
    let status = rpc.call(
        "getSignatureStatuses",
        &json!([[signature], {"searchTransactionHistory":true}]),
    )?;
    let status = status
        .get("value")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .filter(|value| !value.is_null());
    let Some(status) = status else {
        return Ok(None);
    };
    if status.get("confirmationStatus").and_then(Value::as_str) != Some("finalized") {
        return Ok(None);
    }
    let transaction = rpc.call(
        "getTransaction",
        &json!([signature, {
            "encoding":"base64",
            "commitment":"finalized",
            "maxSupportedTransactionVersion":0
        }]),
    )?;
    if transaction.is_null() {
        return Err(refusal(
            "finalized terminal signature omitted finalized transaction history",
        ));
    }
    Ok(Some(transaction))
}

fn finalize_terminal_signature_v1(
    rpc: &mut Rpc,
    journal: &mut DurableTerminalJournalV1,
    signature: &str,
) -> Result<()> {
    let history = authenticate_finalized_terminal_history_v1(rpc, journal, signature)?;
    let (poststate, execution_clocks) =
        terminal_expected_poststate_v1(rpc, &journal.intent.expected_accounts, history.slot)?;
    journal.finalized = Some(DurableFinalizedEvidenceV1 {
        signature: signature.into(),
        slot: history.slot,
        fee_lamports: history.fee_lamports,
        compute_units_consumed: history.compute_units_consumed,
        packet_sha256: history.packet_sha256,
        poststate,
        execution_clocks,
    });
    journal.phase = StageJournalPhaseV1::Finalized;
    Ok(())
}

struct AuthenticatedFinalizedHistoryV1 {
    slot: u64,
    fee_lamports: u64,
    compute_units_consumed: Option<u64>,
    packet_sha256: String,
}

/// Authenticate only immutable transaction history. Live account reads are
/// intentionally excluded: later terminal stages legitimately mutate or close
/// predecessor accounts, so archived finalized journals must not rot.
fn authenticate_finalized_terminal_history_v1(
    rpc: &mut Rpc,
    journal: &DurableTerminalJournalV1,
    signature: &str,
) -> Result<AuthenticatedFinalizedHistoryV1> {
    let transaction = finalized_terminal_transaction_v1(rpc, signature)?.ok_or_else(|| {
        refusal("terminal signature has not reached finalized transaction history")
    })?;
    let meta = transaction
        .get("meta")
        .ok_or_else(|| refusal("finalized terminal transaction omitted meta"))?;
    if meta.get("err").is_some_and(|value| !value.is_null()) {
        return Err(Error::new(format!(
            "finalized terminal transaction failed: {}",
            meta.get("err").unwrap_or(&Value::Null)
        )));
    }
    let transaction_wire = transaction
        .get("transaction")
        .and_then(Value::as_array)
        .ok_or_else(|| refusal("finalized terminal history omitted transaction tuple"))?;
    if transaction_wire.len() != 2
        || transaction_wire.get(1).and_then(Value::as_str) != Some("base64")
    {
        return Err(refusal(
            "finalized terminal transaction tuple was not exactly [body, base64]",
        ));
    }
    let encoded_packet = transaction_wire
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("finalized terminal history omitted base64 packet"))?;
    let packet = BASE64
        .decode(encoded_packet)
        .map_err(|error| Error::new(format!("finalized terminal packet base64: {error}")))?;
    let durable_packet = BASE64
        .decode(
            journal
                .signed_packet_base64
                .as_deref()
                .ok_or_else(|| refusal("terminal journal omitted signed packet"))?,
        )
        .map_err(|error| Error::new(format!("durable terminal packet base64: {error}")))?;
    if packet != durable_packet {
        return Err(refusal(
            "finalized transaction packet differed byte-for-byte from the durable signed packet",
        ));
    }
    let signed: VersionedTransaction = bincode::deserialize(&packet)
        .map_err(|error| Error::new(format!("finalized terminal packet: {error}")))?;
    let packet_signature = signed
        .signatures
        .first()
        .ok_or_else(|| refusal("finalized terminal packet omitted signature"))?;
    if packet_signature.to_string() != signature
        || journal.expected_signature.as_deref() != Some(signature)
    {
        return Err(refusal(
            "finalized packet signature differed from the durable expected signature",
        ));
    }
    let fee = meta
        .get("fee")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("finalized terminal transaction omitted fee"))?;
    if fee != journal.intent.transaction_fee_lamports {
        return Err(refusal(
            "finalized terminal fee differed from the exact planned fee",
        ));
    }
    authenticate_finalized_terminal_balance_vector_v1(meta, &signed, &journal.intent, fee)?;
    authenticate_terminal_return_data_v1(meta, journal.intent.expected_return_data.as_ref())?;
    let slot = transaction
        .get("slot")
        .and_then(Value::as_u64)
        .ok_or_else(|| refusal("finalized terminal transaction omitted slot"))?;
    Ok(AuthenticatedFinalizedHistoryV1 {
        slot,
        fee_lamports: fee,
        compute_units_consumed: meta.get("computeUnitsConsumed").and_then(Value::as_u64),
        packet_sha256: sha256_hex(&packet),
    })
}

fn authenticate_terminal_return_data_v1(
    meta: &Value,
    expected: Option<&DurableReturnDataV1>,
) -> Result<()> {
    let observed = meta.get("returnData").filter(|value| !value.is_null());
    match (expected, observed) {
        (None, None) => Ok(()),
        (None, Some(_)) => Err(refusal(
            "finalized terminal transaction carried unexpected return data",
        )),
        (Some(_), None) => Err(refusal(
            "finalized terminal transaction omitted required return data",
        )),
        (Some(expected), Some(observed)) => {
            let producer = observed
                .get("programId")
                .and_then(Value::as_str)
                .ok_or_else(|| refusal("terminal return data omitted producer"))?;
            let encoded_body = observed
                .get("data")
                .and_then(Value::as_array)
                .ok_or_else(|| refusal("terminal return data omitted body tuple"))?;
            if encoded_body.len() != 2
                || encoded_body.get(1).and_then(Value::as_str) != Some("base64")
            {
                return Err(refusal(
                    "terminal return data tuple was not exactly [body, base64]",
                ));
            }
            let encoded = encoded_body
                .first()
                .and_then(Value::as_str)
                .ok_or_else(|| refusal("terminal return data omitted base64 body"))?;
            let body = BASE64
                .decode(encoded)
                .map_err(|error| Error::new(format!("terminal return data base64: {error}")))?;
            let planned = BASE64
                .decode(&expected.body_base64)
                .map_err(|error| Error::new(format!("terminal expected return base64: {error}")))?;
            if producer != expected.producer || sha256_hex(&planned) != expected.body_sha256 {
                return Err(refusal(
                    "finalized terminal return producer or body differed from semantic prediction",
                ));
            }
            authenticate_execution_clock_bytes_v1(
                "finalized terminal return data",
                &body,
                &planned,
                expected.execution_clock.as_ref(),
            )
            .map(|_| ())
        }
    }
}

fn durable_execution_clock_v1(clock: ExecutionClockFieldV1) -> DurableExecutionClockFieldV1 {
    DurableExecutionClockFieldV1 {
        offset: clock.offset,
        planned_unix_timestamp: clock.planned_unix_timestamp,
        ceiling_unix_timestamp: clock.ceiling_unix_timestamp,
    }
}

/// Compare finalized bytes to a plan that binds every byte but one interval.
///
/// With no rule this is byte equality, which is what five of the six stages'
/// poststates are and stay. With a rule it is byte equality EVERYWHERE ELSE
/// plus an interval on the eight bytes the executing runtime stamps -- so a
/// stage that declares such a field weakens exactly one field of one account
/// and nothing else, and the declaration is in the journal where a reader can
/// see it.
fn authenticate_execution_clock_bytes_v1(
    label: &str,
    observed: &[u8],
    planned: &[u8],
    rule: Option<&DurableExecutionClockFieldV1>,
) -> Result<Option<u64>> {
    let Some(rule) = rule else {
        if observed != planned {
            return Err(refusal(&format!("{label} differed from exact poststate")));
        }
        return Ok(None);
    };
    if observed.len() != planned.len() {
        return Err(refusal(&format!(
            "{label} was {} bytes against the {} its plan predicted",
            observed.len(),
            planned.len(),
        )));
    }
    let end = rule
        .offset
        .checked_add(8)
        .filter(|end| *end <= planned.len())
        .ok_or_else(|| {
            refusal(&format!(
                "{label} is too short to carry the execution clock its plan declares at offset {}",
                rule.offset,
            ))
        })?;
    if observed[..rule.offset] != planned[..rule.offset] || observed[end..] != planned[end..] {
        return Err(refusal(&format!(
            "{label} differed from exact poststate outside the one field its plan cannot bind"
        )));
    }
    let field = |bytes: &[u8]| -> u64 {
        let mut value = [0_u8; 8];
        value.copy_from_slice(&bytes[rule.offset..end]);
        u64::from_le_bytes(value)
    };
    let planned_clock = field(planned);
    if planned_clock != rule.planned_unix_timestamp {
        return Err(refusal(&format!(
            "{label}'s own predicted bytes carry execution clock {planned_clock} while the \
             rule beside them declares the plan observed {}",
            rule.planned_unix_timestamp,
        )));
    }
    let observed_clock = field(observed);
    if observed_clock < rule.planned_unix_timestamp {
        return Err(refusal(&format!(
            "{label} carries execution clock {observed_clock}, BEFORE the {} its plan was \
             built on; an execution cannot precede its own inputs",
            rule.planned_unix_timestamp,
        )));
    }
    if observed_clock > rule.ceiling_unix_timestamp {
        return Err(refusal(&format!(
            "{label} carries execution clock {observed_clock}, past the {} this sequence \
             will certify as its own execution of a packet planned at {}",
            rule.ceiling_unix_timestamp, rule.planned_unix_timestamp,
        )));
    }
    Ok(Some(observed_clock))
}

/// The shape a declared execution-clock field must have to be readable at all.
fn authenticate_execution_clock_shape_v1(
    label: &str,
    rule: &DurableExecutionClockFieldV1,
    data_len: usize,
    observation_unix_timestamp: i64,
) -> Result<()> {
    let planned = u64::try_from(observation_unix_timestamp)
        .map_err(|_| refusal("terminal durable intent carries a negative observation clock"))?;
    if rule.offset.checked_add(8).is_none_or(|end| end > data_len) {
        return Err(refusal(&format!(
            "{label} declares an execution clock at offset {} that does not fit its \
             {data_len} predicted bytes",
            rule.offset,
        )));
    }
    if rule.planned_unix_timestamp != planned {
        return Err(refusal(&format!(
            "{label} declares an execution clock planned at {} while its intent observed {planned}",
            rule.planned_unix_timestamp,
        )));
    }
    if rule.ceiling_unix_timestamp < rule.planned_unix_timestamp {
        return Err(refusal(&format!(
            "{label} declares an execution-clock ceiling {} below the {} it planned at",
            rule.ceiling_unix_timestamp, rule.planned_unix_timestamp,
        )));
    }
    Ok(())
}

/// The ceiling a NEW durable obligation must carry.
///
/// Placed at the doors that create one -- the plan site and the reconciliation
/// that rewrites a prediction -- and never on a read, for the reason
/// `450cc2222` moved the compute-budget requirement there: a journal already
/// written under an earlier derivation must stay readable, or its own remedy
/// becomes unreachable.
fn require_derived_execution_clock_ceiling_v1(clock: ExecutionClockFieldV1) -> Result<()> {
    let derived = clock
        .planned_unix_timestamp
        .checked_add(TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS)
        .ok_or_else(|| refusal("terminal execution-clock ceiling overflowed"))?;
    if clock.ceiling_unix_timestamp != derived {
        return Err(refusal(&format!(
            "a new terminal execution-clock obligation must carry this sequence's derived ceiling              {derived} ({} plus {TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS} seconds), not {}",
            clock.planned_unix_timestamp, clock.ceiling_unix_timestamp,
        )));
    }
    Ok(())
}

fn authenticate_finalized_terminal_balance_vector_v1(
    meta: &Value,
    transaction: &VersionedTransaction,
    intent: &DurableTerminalIntentV1,
    fee: u64,
) -> Result<()> {
    let loaded = meta
        .get("loadedAddresses")
        .and_then(Value::as_object)
        .ok_or_else(|| refusal("finalized terminal meta omitted loadedAddresses"))?;
    if loaded.len() != 2 {
        return Err(refusal(
            "finalized terminal loadedAddresses carried unknown fields",
        ));
    }
    let strings = |value: Option<&Value>, label: &str| -> Result<Vec<String>> {
        value
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("finalized terminal {label} was not an array")))?
            .iter()
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    Error::new(format!("finalized terminal {label} key was not a string"))
                })
            })
            .collect()
    };
    let loaded_writable = strings(loaded.get("writable"), "loaded writable")?;
    let loaded_readonly = strings(loaded.get("readonly"), "loaded readonly")?;
    if loaded_writable != intent.loaded_writable || loaded_readonly != intent.loaded_readonly {
        return Err(refusal(
            "finalized terminal loaded address order differed from durable v0 resolution",
        ));
    }
    let VersionedMessage::V0(message) = &transaction.message else {
        return Err(refusal("finalized terminal message was not v0"));
    };
    let resolved = message
        .account_keys
        .iter()
        .map(ToString::to_string)
        .chain(loaded_writable.iter().cloned())
        .chain(loaded_readonly.iter().cloned())
        .collect::<Vec<_>>();
    if resolved != intent.resolved_account_keys {
        return Err(refusal(
            "finalized terminal account-index vector differed from durable resolution",
        ));
    }
    let balances = |label: &str| -> Result<Vec<u64>> {
        meta.get(label)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(format!("finalized terminal {label} was not an array")))?
            .iter()
            .map(|value| {
                value.as_u64().ok_or_else(|| {
                    Error::new(format!("finalized terminal {label} entry was not u64"))
                })
            })
            .collect()
    };
    let pre = balances("preBalances")?;
    let post = balances("postBalances")?;
    if pre != intent.pre_balances
        || post != intent.post_balances
        || pre.len() != resolved.len()
        || post.len() != resolved.len()
    {
        return Err(refusal(
            "finalized terminal pre/post balance vector differed from exact durable arithmetic",
        ));
    }
    let payer_index = resolved
        .iter()
        .position(|key| key == &intent.payer)
        .ok_or_else(|| refusal("terminal balance vector omitted payer"))?;
    let payer_protocol_delta = intent
        .protocol_lamport_deltas
        .get(&intent.payer)
        .copied()
        .unwrap_or(0);
    let payer_after_protocol = checked_apply_lamport_delta(pre[payer_index], payer_protocol_delta)?;
    if payer_after_protocol.checked_sub(fee) != Some(post[payer_index]) {
        return Err(refusal(
            "terminal payer pre/post balance did not equal protocol delta plus exact fee",
        ));
    }
    Ok(())
}

type TerminalObservedPoststateV1 = (
    BTreeMap<String, DurableAccountStateV1>,
    BTreeMap<String, u64>,
);

fn terminal_expected_poststate_v1(
    rpc: &mut Rpc,
    expected: &BTreeMap<String, DurableExpectedAccountV1>,
    minimum_slot: u64,
) -> Result<TerminalObservedPoststateV1> {
    let keys = expected
        .values()
        .map(|account| {
            Pubkey::from_str(&account.address)
                .map_err(|error| Error::new(format!("terminal expected address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let (slot, values) = rpc.finalized_accounts(&keys, minimum_slot)?;
    if slot < minimum_slot {
        return Err(refusal(
            "terminal poststate RPC context slot was below the finalized transaction slot",
        ));
    }
    let mut states = BTreeMap::new();
    let mut execution_clocks = BTreeMap::new();
    for ((key, value), expected) in keys.into_iter().zip(values.iter()).zip(expected.values()) {
        let state = durable_rpc_state(key, value.as_ref());
        if state.owner != expected.owner
            || state.lamports != expected.lamports_after_fee
            || state.executable != expected.executable
        {
            return Err(refusal(
                "finalized terminal owner/lamports/executable differed from exact poststate",
            ));
        }
        match &expected.lookup_table {
            None => {
                let data = BASE64.decode(&expected.data_base64).map_err(|error| {
                    Error::new(format!("terminal expected data base64: {error}"))
                })?;
                if state.data_len != data.len() || sha256_hex(&data) != expected.data_sha256 {
                    return Err(refusal(
                        "finalized terminal bytes differed from exact poststate",
                    ));
                }
                if let Some(clock) = authenticate_execution_clock_bytes_v1(
                    &format!("finalized terminal account {key}"),
                    value.as_ref().map_or(&[][..], |account| &account.data),
                    &data,
                    expected.execution_clock.as_ref(),
                )? {
                    execution_clocks.insert(key.to_string(), clock);
                }
            }
            Some(rule) => {
                let account = value.as_ref().ok_or_else(|| {
                    refusal("finalized terminal ALT poststate was unexpectedly vacant")
                })?;
                authenticate_lookup_poststate_rule_v1(account, rule, minimum_slot)?;
            }
        }
        states.insert(key.to_string(), state);
    }
    Ok((states, execution_clocks))
}

fn authenticate_lookup_poststate_rule_v1(
    account: &RpcAccount,
    rule: &DurableLookupTablePoststateV1,
    transaction_slot: u64,
) -> Result<()> {
    let decoded = AddressLookupTable::deserialize(&account.data)
        .map_err(|_| refusal("finalized terminal ALT bytes did not decode canonically"))?;
    let authority = rule
        .authority
        .as_deref()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal ALT authority: {error}")))
        })
        .transpose()?;
    let addresses = rule
        .addresses
        .iter()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal ALT address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let expected_last_extended_slot = match rule.last_extended_slot {
        DurableLookupLastExtendedSlotV1::Exact(slot) => slot,
        DurableLookupLastExtendedSlotV1::FinalizedTransaction => transaction_slot,
    };
    if decoded.meta.authority != authority
        || decoded.meta.deactivation_slot != rule.deactivation_slot
        || decoded.meta.last_extended_slot != expected_last_extended_slot
        || decoded.meta.last_extended_slot_start_index != rule.last_extended_slot_start_index
        || decoded.addresses.as_ref() != addresses.as_slice()
        || account.data.len() != lookup_table_data_len(addresses.len())?
    {
        return Err(refusal(
            "finalized terminal ALT authority, slots, extension boundary, addresses, or width differed from exact poststate",
        ));
    }
    Ok(())
}

fn canonical_lookup_poststate_bytes_v1(
    rule: &DurableLookupTablePoststateV1,
    transaction_slot: u64,
) -> Result<Vec<u8>> {
    let authority = rule
        .authority
        .as_deref()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal ALT authority: {error}")))
        })
        .transpose()?;
    let addresses = rule
        .addresses
        .iter()
        .map(|key| {
            Pubkey::from_str(key)
                .map_err(|error| Error::new(format!("terminal ALT address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let last_extended_slot = match rule.last_extended_slot {
        DurableLookupLastExtendedSlotV1::Exact(slot) => slot,
        DurableLookupLastExtendedSlotV1::FinalizedTransaction => transaction_slot,
    };
    let mut data = vec![0_u8; lookup_table_data_len(addresses.len())?];
    AddressLookupTable::overwrite_meta_data(
        &mut data,
        LookupTableMeta {
            deactivation_slot: rule.deactivation_slot,
            last_extended_slot,
            last_extended_slot_start_index: rule.last_extended_slot_start_index,
            authority,
            ..LookupTableMeta::default()
        },
    )
    .map_err(|_| refusal("terminal ALT exact poststate metadata did not serialize"))?;
    let mut offset = LOOKUP_TABLE_META_SIZE;
    for address in addresses {
        let end = offset
            .checked_add(ALT_ADDRESS_BYTES)
            .ok_or_else(|| refusal("terminal ALT exact poststate offset overflowed"))?;
        data.get_mut(offset..end)
            .ok_or_else(|| refusal("terminal ALT exact poststate width underflowed"))?
            .copy_from_slice(address.as_ref());
        offset = end;
    }
    Ok(data)
}

fn authenticate_persisted_terminal_poststate_v1(
    intent: &DurableTerminalIntentV1,
    finalized: &DurableFinalizedEvidenceV1,
) -> Result<()> {
    if finalized.poststate.len() != intent.expected_accounts.len() {
        return Err(refusal(
            "persisted finalized poststate width differed from exact durable intent",
        ));
    }
    for (key, expected) in &intent.expected_accounts {
        let address = Pubkey::from_str(key)
            .map_err(|error| Error::new(format!("terminal persisted address: {error}")))?;
        let owner = Pubkey::from_str(&expected.owner)
            .map_err(|error| Error::new(format!("terminal persisted owner: {error}")))?;
        let mut data = match &expected.lookup_table {
            None => BASE64.decode(&expected.data_base64).map_err(|error| {
                Error::new(format!("terminal expected persisted data base64: {error}"))
            })?,
            Some(rule) => canonical_lookup_poststate_bytes_v1(rule, finalized.slot)?,
        };
        // THE RECORD IS THE STATEMENT; THE INTERVAL IS THE FALLBACK.
        //
        // Certification writes down the clock it admitted, and with that record
        // this is one exact comparison. A journal CERTIFIED BEFORE that record
        // existed carries none -- and must stay verifiable, or a change to the
        // evidence schema retroactively rots journals that already landed,
        // which is the placement mistake `450cc2222` cost a lane. It does stay
        // verifiable, because the declared interval is finite and the other
        // bytes are known: exactly one admissible clock can reproduce the
        // recorded digest, and finding it IS the proof that the observed bytes
        // are the plan's bytes with an admissible clock. The search is bounded
        // by the plan's own ceiling and is never a substitute for the record --
        // a clock outside the interval is not among the candidates.
        let candidates = match (
            &expected.execution_clock,
            finalized.execution_clocks.get(key),
        ) {
            (None, None) => Vec::new(),
            (None, Some(observed)) => {
                return Err(refusal(&format!(
                    "the finalized evidence records execution clock {observed} for {key}, whose \
                     plan declares no such field"
                )));
            }
            (Some(clock), Some(observed)) => {
                if *observed < clock.planned_unix_timestamp
                    || *observed > clock.ceiling_unix_timestamp
                {
                    return Err(refusal(&format!(
                        "the execution clock recorded for {key} is {observed}, outside the [{}, \
                         {}] its plan declared",
                        clock.planned_unix_timestamp, clock.ceiling_unix_timestamp,
                    )));
                }
                vec![*observed]
            }
            (Some(clock), None) => {
                (clock.planned_unix_timestamp..=clock.ceiling_unix_timestamp).collect::<Vec<_>>()
            }
        };
        if let Some(clock) = &expected.execution_clock {
            let end = clock
                .offset
                .checked_add(8)
                .filter(|end| *end <= data.len())
                .ok_or_else(|| {
                    refusal("a persisted execution-clock field does not fit its poststate bytes")
                })?;
            let recorded = finalized.poststate.get(key).ok_or_else(|| {
                refusal("persisted finalized poststate omitted a declared expected account")
            })?;
            let admitted = candidates.into_iter().find(|candidate| {
                let mut probe = data.clone();
                probe[clock.offset..end].copy_from_slice(&candidate.to_le_bytes());
                durable_state(
                    address,
                    owner,
                    expected.lamports_after_fee,
                    expected.executable,
                    &probe,
                ) == *recorded
            });
            let admitted = admitted.ok_or_else(|| {
                refusal(&format!(
                    "no execution clock in [{}, {}] reproduces the poststate recorded for {key}; \
                     the finalized bytes differ from the plan somewhere other than the one field \
                     it could not bind",
                    clock.planned_unix_timestamp, clock.ceiling_unix_timestamp,
                ))
            })?;
            data[clock.offset..end].copy_from_slice(&admitted.to_le_bytes());
        }
        let exact = durable_state(
            address,
            owner,
            expected.lamports_after_fee,
            expected.executable,
            &data,
        );
        if finalized.poststate.get(key) != Some(&exact) {
            return Err(refusal(
                "persisted finalized poststate differed from exact intent bytes, owner, lamports, or executable flag",
            ));
        }
    }
    Ok(())
}

fn verify_persisted_terminal_finalization_v1(
    rpc: &mut Rpc,
    journal: &DurableTerminalJournalV1,
) -> Result<()> {
    let finalized = journal
        .finalized
        .as_ref()
        .ok_or_else(|| refusal("finalized terminal journal omitted evidence"))?;
    let history = authenticate_finalized_terminal_history_v1(rpc, journal, &finalized.signature)?;
    if history.slot != finalized.slot
        || history.fee_lamports != finalized.fee_lamports
        || history.compute_units_consumed != finalized.compute_units_consumed
        || history.packet_sha256 != finalized.packet_sha256
    {
        return Err(refusal(
            "reverified terminal packet, return data, balances, slot, fee, or compute evidence differed from persisted finalized evidence",
        ));
    }
    authenticate_persisted_terminal_poststate_v1(&journal.intent, finalized)
}

fn require_terminal_prestate_unchanged_v1(
    rpc: &mut Rpc,
    journal: &DurableTerminalJournalV1,
) -> Result<()> {
    let keys = journal
        .intent
        .prestate
        .values()
        .map(|account| {
            Pubkey::from_str(&account.address)
                .map_err(|error| Error::new(format!("terminal prestate address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let (slot, values) = rpc.finalized_accounts(&keys, journal.intent.observation_slot)?;
    if slot < journal.intent.observation_slot {
        return Err(refusal(
            "terminal prestate RPC context slot was below durable observation",
        ));
    }
    for ((key, value), expected) in keys
        .into_iter()
        .zip(values.iter())
        .zip(journal.intent.prestate.values())
    {
        // THE CLOCK IS NOT A PRESTATE. IT IS THE CLOCK.
        //
        // `resolution-receipt-prepay` carries the Clock sysvar in its frame,
        // so it lands in the durable prestate map like every other account --
        // and its bytes change every slot BY CONSTRUCTION. Requiring it
        // unchanged between planning and key load makes this stage
        // structurally unreachable: measured on devnet 2026-09-04, eight
        // consecutive plan-then-execute passes refused, and the only account
        // that moved across all eight was `SysvarC1ock…` (observation slot
        // 492,898,149 to 492,900,125, everything else byte-identical).
        //
        // Presence is still required. What is dropped is the equality, because
        // no plan can bind a value the runtime redefines every 400ms, and a
        // guard nothing can satisfy protects nothing.
        if prestate_is_slot_bound_runtime_account_v1(key) {
            if value.is_none() {
                return Err(refusal(
                    "terminal prestate lost a slot-bound runtime account between planning and key load",
                ));
            }
            continue;
        }
        if durable_rpc_state(key, value.as_ref()) != *expected {
            return Err(refusal(
                "terminal finalized prestate changed after durable planning",
            ));
        }
    }
    Ok(())
}

/// Accounts whose CONTENT is a function of the executing slot.
///
/// Exactly one today. The Rent and Instructions sysvars are also in terminal
/// frames and neither belongs here: Rent's bytes are a cluster parameter that
/// does not move per slot, and the Instructions sysvar reads as zero-length
/// off chain. A future addition to this set is a claim that a plan cannot bind
/// that account either, and it should carry the measurement that says so.
fn prestate_is_slot_bound_runtime_account_v1(key: Pubkey) -> bool {
    key == sysvar::clock::ID
}

fn terminal_latest_blockhash(rpc: &mut Rpc) -> Result<(Hash, u64)> {
    let value = rpc.call("getLatestBlockhash", &json!([{"commitment":"finalized"}]))?;
    let value = value
        .get("value")
        .ok_or_else(|| Error::new("getLatestBlockhash omitted value"))?;
    let blockhash = value
        .get("blockhash")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted blockhash"))?
        .parse::<Hash>()
        .map_err(|error| Error::new(format!("terminal blockhash: {error}")))?;
    let last_valid = value
        .get("lastValidBlockHeight")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("getLatestBlockhash omitted lastValidBlockHeight"))?;
    Ok((blockhash, last_valid))
}

fn terminal_fee_for_message(rpc: &mut Rpc, message_base64: &str) -> Result<u64> {
    rpc.call(
        "getFeeForMessage",
        &json!([message_base64, {"commitment":"finalized"}]),
    )?
    .get("value")
    .and_then(Value::as_u64)
    .ok_or_else(|| Error::new("getFeeForMessage omitted exact terminal fee"))
}

fn authenticate_terminal_cluster_v1(
    rpc: &mut Rpc,
    origin: &ClusterOriginV1,
    expected_cluster: ExpectedClusterV1,
) -> Result<String> {
    expected_cluster.authenticate(origin)?;
    let genesis = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| Error::new("getGenesisHash result was not a string"))?
        .to_owned();
    if expected_cluster == ExpectedClusterV1::Devnet && genesis != DEVNET_GENESIS_HASH {
        return Err(refusal(
            "terminal executor observed another genesis than exact Solana devnet",
        ));
    }
    origin.authenticate_genesis(&genesis)?;
    Ok(genesis)
}

fn durable_observed_state(account: &ObservedAccount) -> DurableAccountStateV1 {
    durable_state(
        account.key,
        account.owner,
        account.lamports,
        account.executable,
        &account.data,
    )
}

fn durable_rpc_state(key: Pubkey, account: Option<&RpcAccount>) -> DurableAccountStateV1 {
    match account {
        Some(account) => durable_state(
            key,
            account.owner,
            account.lamports,
            account.executable,
            &account.data,
        ),
        None => durable_state(key, solana_sdk_ids::system_program::ID, 0, false, &[]),
    }
}

fn durable_state(
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    executable: bool,
    data: &[u8],
) -> DurableAccountStateV1 {
    let mut exact = Sha256::new();
    exact.update(owner.as_ref());
    exact.update(lamports.to_le_bytes());
    exact.update([u8::from(executable)]);
    exact.update(u64::try_from(data.len()).unwrap_or(u64::MAX).to_le_bytes());
    exact.update(data);
    DurableAccountStateV1 {
        address: key.to_string(),
        owner: owner.to_string(),
        lamports,
        executable,
        data_len: data.len(),
        data_sha256: sha256_hex(data),
        account_sha256: hex_bytes(&exact.finalize()),
    }
}

fn pubkey_vector_sha256(addresses: &[Pubkey]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/terminal-alt-stable-addresses/v1");
    hasher.update(
        u64::try_from(addresses.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for address in addresses {
        hasher.update(address.as_ref());
    }
    hex_bytes(&hasher.finalize())
}

/// The generation of the Market this sequence retires.
///
/// The founding input names the generation the **founding lane** occupies. Its
/// product — "the DCLTGMF3 Market at generation + 1 … the one that ends Open,
/// which is the product of the whole founding" (`market.rs`,
/// `FoundingTargetsV1::open_market`, derived through
/// `PrestateLaneV1::Founding`, whose offset is 1) — is one past it.
///
/// Every coordinate this sequence retires belongs to that Market: its own
/// identity, its Source state PDA, the Direct BeginRetiring request, and the
/// Resolution closure receipt. Reading the input's own generation for any of
/// them addressed a Market one short — a Source state that does not exist and
/// an identity that never matches. Nothing caught it because no market had ever
/// reached this stage with a life behind it.
///
/// The chain comparison this feeds is therefore a *check* of the derivation and
/// not an assumption: `state.identity.generation` must equal what this returns.
fn retired_market_generation_v1(founding_generation: u64) -> Result<u64> {
    founding_generation
        .checked_add(1)
        .ok_or_else(|| refusal("retired Market generation overflow"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_bytes(&Sha256::digest(bytes))
}

/// The two row shapes are field-identical; only their owning modules differ.
pub(crate) fn terminal_rows_as_model_v1(
    rows: &BTreeMap<String, crate::campaign::CampaignAccountEvidenceV1>,
) -> BTreeMap<String, crate::model::AccountEvidence> {
    rows.iter()
        .map(|(label, row)| {
            (
                label.clone(),
                crate::model::AccountEvidence {
                    address: row.address.clone(),
                    owner: row.owner.clone(),
                    lamports: row.lamports,
                    executable: row.executable,
                    data_len: row.data_len,
                    data_sha256: row.data_sha256.clone(),
                    account_sha256: row.account_sha256.clone(),
                },
            )
        })
        .collect()
}

pub(crate) fn model_rows_as_terminal_v1(
    rows: BTreeMap<String, crate::model::AccountEvidence>,
) -> BTreeMap<String, crate::campaign::CampaignAccountEvidenceV1> {
    rows.into_iter()
        .map(|(label, row)| {
            (
                label,
                crate::campaign::CampaignAccountEvidenceV1 {
                    address: row.address,
                    owner: row.owner,
                    lamports: row.lamports,
                    executable: row.executable,
                    data_len: row.data_len,
                    data_sha256: row.data_sha256,
                    account_sha256: row.account_sha256,
                },
            )
        })
        .collect()
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Stage-two planning is the one protocol stage whose semantic owner admits
/// both Submit and chain-detected Complete.  Complete carries no invented
/// receipt and advances only after the caller authenticates the exact observed
/// Retiring root returned by the operator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DirectBeginRetiringCallerPlanV1 {
    Submit(TerminalSemanticMutationV1),
    Complete {
        observation: Observation,
        root: ExpectedAccountPoststateV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChainDerivedTerminalMutationV1 {
    pub(crate) mutation: TerminalSemanticMutationV1,
    pub(crate) closure: TerminalMetaClosureV1,
    pub(crate) prestate: Vec<ObservedAccount>,
}

fn authenticate_chain_derived_planned_journal_v1(
    journal: &DurableTerminalJournalV1,
    chain: &ChainDerivedTerminalMutationV1,
) -> Result<AuthenticatedPlannedTerminalIntentV1> {
    authenticate_planned_protocol_owner_v1(
        journal,
        DurableTerminalMutationV1::Protocol {
            stage: chain.mutation.stage,
        },
        &chain.mutation,
        &chain.closure,
        &chain.prestate,
    )
}

/// Observe the derived failure escrow of one Market, when the aggregate has a
/// width that seats one at all.
///
/// A Market too narrow to seat an escrow returns `None` rather than an error:
/// there is no such account to look for, and the preflight's ordinary
/// unpaid-holder sentence is the correct one there.
fn failure_escrow_observation_v1(
    rpc: &mut Rpc,
    claims: Pubkey,
    aggregate: Pubkey,
    aggregate_account: &ObservedAccount,
) -> Result<Option<ObservedAccount>> {
    let view = match dclutch_claims::liability_basis_state_v2::LiabilityBasisMarketViewV2::decode(
        &aggregate_account.data,
    ) {
        Ok(view) => view,
        Err(_) => return Ok(None),
    };
    let Ok(escrow) = dclutch_operator::failure_escrow_v1::failure_escrow_v1(
        claims,
        view.logical_market,
        aggregate,
        view.claim_count,
    ) else {
        return Ok(None);
    };
    let snapshot = finalized_snapshot(rpc, &[escrow.position])?;
    snapshot
        .account(escrow.position)
        .map(|account| Some(account.clone()))
        .map_err(|error| Error::new(format!("failure escrow Position: {error}")))
}

pub(crate) fn plan_core_begin_retiring_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    market_key: Pubkey,
    additional_keys: &[Pubkey],
) -> Result<ChainDerivedTerminalMutationV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let core_programdata = pubkey(&plan.core.programdata_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let claims_programdata = pubkey(&plan.claims.programdata_id)?;
    let activation = pubkey(&plan.activation)?;
    let aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, market_key.as_ref()],
        &claims,
    )
    .0;
    if aggregate != evidence_pubkey(evidence, "claims_aggregate")? {
        return Err(refusal(
            "Core BeginRetiring Claims aggregate evidence was not the canonical Market PDA",
        ));
    }
    let mut keys = vec![
        market_key,
        aggregate,
        registry,
        activation,
        core,
        core_programdata,
        claims,
        claims_programdata,
    ];
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let market_account = account(market_key, "Core Market")?;
    let market = decode_routed_market(&market_account, core, plan)?;
    if market.phase != Phase::Terminal || market.readiness != Readiness::Consumed {
        return Err(refusal(
            "Core BeginRetiring requires the exact Terminal/Consumed Market",
        ));
    }
    authenticate_role(
        &account(registry, "Registry program")?,
        &account(activation, "activation cache")?,
        market.identity.selected_release_set.to_bytes(),
        ExecutionRoleV1::Core,
        &account(core, "Core program")?,
        &account(core_programdata, "Core ProgramData")?,
    )?;
    authenticate_role(
        &account(registry, "Registry program")?,
        &account(activation, "activation cache")?,
        market.identity.selected_release_set.to_bytes(),
        ExecutionRoleV1::Claims,
        &account(claims, "Claims program")?,
        &account(claims_programdata, "Claims ProgramData")?,
    )?;
    // The escrow's observation is DIAGNOSIS, not authority: it decides which of
    // two sentences the preflight prints about a nonzero failure column, and the
    // snapshot of record above is untouched by it. Its address is derived off
    // the aggregate the snapshot already carries, so it cannot be pointed at
    // another Market's escrow, and it costs one bounded read on a path that runs
    // once per retirement.
    let aggregate_account = account(aggregate, "Claims aggregate")?;
    let escrow_observation =
        failure_escrow_observation_v1(rpc, claims, aggregate, &aggregate_account)?;
    authenticate_zero_claims(
        &aggregate_account,
        aggregate,
        claims,
        market,
        hex32(&evidence.founding_custody_context)?,
        escrow_observation.as_ref(),
    )?;
    let request = Request::administrative(
        Action::BeginRetiring,
        market.identity.generation,
        market.identity.market_id,
    );
    let instruction = Instruction {
        program_id: core,
        accounts: core_begin_retiring_meta_closure_v1(
            market_key,
            activation,
            registry,
            core,
            core_programdata,
        )
        .accounts,
        data: request
            .encode()
            .map_err(|error| Error::new(format!("Core BeginRetiring request: {error:?}")))?
            .to_vec(),
    };
    let closure = core_begin_retiring_meta_closure_v1(
        market_key,
        activation,
        registry,
        core,
        core_programdata,
    );
    closure.authenticate_instruction(&instruction)?;
    let mut expected_market = market;
    begin_retiring(
        request,
        &mut expected_market,
        core_admission_from_plan_v1(plan, market)?,
    )
    .map_err(|error| Error::new(format!("Core BeginRetiring semantic owner: {error:?}")))?;
    let expected_market_data = expected_market
        .encode()
        .map_err(|error| Error::new(format!("Core Retiring Market bytes: {error:?}")))?
        .to_vec();
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    Ok(ChainDerivedTerminalMutationV1 {
        mutation: TerminalSemanticMutationV1 {
            stage: TerminalStageV1::CoreBeginRetiring,
            observation: snapshot.observation,
            instruction,
            expected_return_data: None,
            expected_accounts: vec![ExpectedAccountPoststateV1::exact(
                market_key,
                core,
                market_account.lamports,
                false,
                expected_market_data,
            )],
            protocol_lamport_deltas: BTreeMap::from([(market_key, 0)]),
        },
        closure,
        prestate,
    })
}

fn core_admission_from_plan_v1(plan: &SuccessorPlan, market: CoreState) -> Result<Admission> {
    let binding = |pin: &crate::model::ProgramPin| -> Result<Binding> {
        Ok(Binding {
            program: Identity::new(pubkey(&pin.program_id)?.to_bytes())
                .map_err(|_| refusal("Core release Program identity was zero"))?,
            artifact_release: Identity::new(hex32(&pin.artifact_release_id)?)
                .map_err(|_| refusal("Core release artifact identity was zero"))?,
            semantic_release: Identity::new(hex32(&pin.semantic_release_id)?)
                .map_err(|_| refusal("Core release semantic identity was zero"))?,
        })
    };
    let selected = ReleaseSet {
        release_set_id: market.identity.selected_release_set,
        bindings: [
            binding(&plan.core)?,
            binding(&plan.claims)?,
            binding(&plan.trading)?,
            binding(&plan.resolution)?,
            binding(&plan.custody)?,
        ],
    };
    let observed = selected.bindings[Role::Core as usize];
    Ok(Admission {
        market_registry_program: market.identity.registry_program,
        market_release_set_id: market.identity.selected_release_set,
        selected,
        receipt: ReleaseReceipt {
            registry_program: market.identity.registry_program,
            release_set_id: market.identity.selected_release_set,
            role: Role::Core,
            observed,
            activation_cache_authenticated: true,
            current_deployment_reauthenticated: true,
        },
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChainDirectBeginRetiringV1 {
    Submit(ChainDerivedTerminalMutationV1),
    Complete {
        observation: Observation,
        root: ExpectedAccountPoststateV1,
    },
}

/// Discover and authenticate the complete stage-two graph directly from the
/// plan/evidence identities. No caller PDA or artifact table is supplied: the
/// DCLTDBR1 semantic owner derives all 20 coordinates, and its fresh finalized
/// report must reproduce the initial closure before a journal can be built.
pub(crate) fn plan_direct_begin_retiring_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
    additional_keys: &[Pubkey],
) -> Result<ChainDirectBeginRetiringV1> {
    let direct = market_input
        .direct_capability
        .as_ref()
        .ok_or_else(|| refusal("successor plan omitted its Direct capability payload"))?;
    let root = evidence_pubkey(evidence, "direct_capability_root")?;
    let release_set = hex32(&plan.release_set_id)?;
    let manifest = evidence_digest(evidence, "capability_manifest_record")?;
    let program_set = evidence_digest(evidence, "direct_program_set_record")?;
    let config = evidence_digest(evidence, "direct_execution_config_record")?;
    let context = direct_begin_retiring_context_v1(
        release_set,
        market.to_bytes(),
        root.to_bytes(),
        manifest,
        program_set,
        config,
        retired_market_generation_v1(market_input.generation)?,
        evidence.direct_selected_manifest_entry_index,
    );
    let placeholder_request = DirectBeginRetiringRequestV1 {
        release_set,
        market: market.to_bytes(),
        context,
        root: root.to_bytes(),
        manifest,
        program_set,
        config,
        // Neither digest derives an account coordinate. Nonzero placeholders
        // let the semantic owner project the stable frame before Core stage
        // one changes Market bytes; the fresh stage-two report still commits
        // and authenticates the real finalized digests.
        expected_market_digest: [1; 32],
        expected_root_digest: [2; 32],
        generation: retired_market_generation_v1(market_input.generation)?,
        entry_index: evidence.direct_selected_manifest_entry_index,
    };
    let closure = direct_begin_retiring_meta_closure_v1(DirectBeginRetiringCoordinateInputV1 {
        request: placeholder_request,
        descriptor: evidence_digest(evidence, "direct_begin_retiring_descriptor_record")?,
        account_profile: evidence_digest(evidence, "direct_begin_retiring_account_profile_record")?,
        effect: evidence_digest(evidence, "direct_begin_retiring_effect_record")?,
        registry_program: pubkey(&plan.registry.program_id)?,
        core_program: pubkey(&plan.core.program_id)?,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        trading_program: pubkey(&plan.trading.program_id)?,
        trading_programdata: pubkey(&plan.trading.programdata_id)?,
    })?;
    let mut keys = closure
        .accounts
        .iter()
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |index: usize, label: &str| -> Result<ObservedAccount> {
        let key = closure
            .accounts
            .get(index)
            .ok_or_else(|| refusal("DCLTDBR1 closure omitted a canonical role"))?
            .pubkey;
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let operator = DirectBeginRetiringSnapshotV1 {
        genesis_hash: SOLANA_DEVNET_GENESIS_HASH_V1,
        ordinary_release_witness: direct_ordinary_witness_v1(direct)?,
        root: account(0, "Direct root")?,
        market: account(1, "Core Market")?,
        capability_manifest: account(2, "capability manifest")?,
        program_set: account(3, "Direct ProgramSet")?,
        program_set_staging: account(4, "Direct ProgramSet staging")?,
        descriptor: account(5, "Direct BeginRetiring descriptor")?,
        descriptor_staging: account(6, "Direct BeginRetiring descriptor staging")?,
        config: account(7, "Direct config")?,
        config_staging: account(8, "Direct config staging")?,
        account_profile: account(9, "Direct BeginRetiring profile")?,
        account_profile_staging: account(10, "Direct BeginRetiring profile staging")?,
        effect: account(11, "Direct BeginRetiring effect")?,
        effect_staging: account(12, "Direct BeginRetiring effect staging")?,
        activation_cache: account(13, "Registry activation cache")?,
        core_program: account(14, "Core program")?,
        core_programdata: account(15, "Core ProgramData")?,
        trading_program: account(16, "Trading program")?,
        trading_programdata: account(17, "Trading ProgramData")?,
        registry_program: account(18, "Registry program")?,
        rent_sysvar: account(19, "Rent sysvar")?,
    };
    match plan_direct_begin_retiring_caller_v1(&operator, &closure)? {
        DirectBeginRetiringCallerPlanV1::Submit(mutation) => {
            let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
            prestate.sort_unstable_by_key(|account| account.key);
            Ok(ChainDirectBeginRetiringV1::Submit(
                ChainDerivedTerminalMutationV1 {
                    mutation,
                    closure,
                    prestate,
                },
            ))
        }
        DirectBeginRetiringCallerPlanV1::Complete { observation, root } => {
            Ok(ChainDirectBeginRetiringV1::Complete { observation, root })
        }
    }
}

fn direct_ordinary_witness_v1(
    direct: &crate::model::DirectMarketCapabilityV1,
) -> Result<DirectInlineOrdinaryHotBundleV4> {
    Ok(DirectInlineOrdinaryHotBundleV4 {
        account_profile: decoded_exact_array_v1(
            &direct.ordinary_account_profile_hex,
            "Direct ordinary AccountProfile",
        )?,
        lifecycle_policy: decoded_exact_array_v1(
            &direct.ordinary_lifecycle_policy_hex,
            "Direct ordinary LifecyclePolicy",
        )?,
        request_profile: decoded_exact_array_v1(
            &direct.ordinary_request_profile_hex,
            "Direct ordinary RequestProfile",
        )?,
        transition: decoded_exact_array_v1(
            &direct.ordinary_transition_hex,
            "Direct ordinary Transition",
        )?,
        strategy: decoded_exact_array_v1(
            &direct.ordinary_strategy_hex,
            "Direct ordinary Strategy",
        )?,
        effect: decoded_exact_array_v1(&direct.ordinary_effect_hex, "Direct ordinary Effect")?,
        descriptor: decoded_exact_array_v1(
            &direct.ordinary_descriptor_hex,
            "Direct ordinary descriptor",
        )?,
    })
}

fn decoded_exact_array_v1<const N: usize>(value: &str, label: &str) -> Result<[u8; N]> {
    decode_hex(value)?.try_into().map_err(|bytes: Vec<u8>| {
        Error::new(format!("{label} is {} bytes, expected {N}", bytes.len()))
    })
}

fn evidence_pubkey(evidence: &CampaignTerminalEvidenceV1, label: &str) -> Result<Pubkey> {
    pubkey(&required_account(evidence, label)?.address)
}

fn evidence_digest(evidence: &CampaignTerminalEvidenceV1, label: &str) -> Result<[u8; 32]> {
    hex32(&required_account(evidence, label)?.data_sha256)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChainResolutionCloseV1 {
    NeedsReceiptPrepay {
        observation: Observation,
        payer: ObservedAccount,
        receipt: ObservedAccount,
        exact_receipt_rent: u64,
        prestate: Vec<ObservedAccount>,
    },
    Submit {
        stage: ChainDerivedTerminalMutationV1,
        snapshot: Box<ResolutionCloseFundSnapshotV3>,
    },
}

/// Build Resolution CloseFund (or its exact receipt-rent prerequisite) from
/// one complete finalized snapshot. The role request and request-bound caller
/// remain owned by Resolution's existing semantic operator.
pub(crate) fn plan_resolution_close_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    market_key: Pubkey,
    payer: Pubkey,
    additional_keys: &[Pubkey],
    session_funded_rent_rate: u32,
) -> Result<ChainResolutionCloseV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let preliminary = finalized_snapshot(rpc, &[market_key])?;
    let preliminary_market = decode_routed_market(preliminary.account(market_key)?, core, plan)?;
    if preliminary_market.phase != Phase::Retiring {
        return Err(refusal(
            "Resolution CloseFund requires the exact Retiring Core Market",
        ));
    }
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market_key.as_ref(),
            &preliminary_market.identity.generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let source_material = routed_record(
        evidence,
        "source_material_record",
        registry,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    )?;
    let capability_manifest = routed_record(
        evidence,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let explicit_recovery = evidence.accounts.contains_key("recovery_policy_record");
    let recovery_policy = if explicit_recovery {
        routed_record(
            evidence,
            "recovery_policy_record",
            registry,
            RECOVERY_POLICY_SCHEMA_ID_V2,
        )?
    } else {
        routed_record(
            evidence,
            "source_material_record",
            registry,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        )?
    };
    let source_route = finalized_snapshot(rpc, &[source_state])?;
    let routed_source = source_route.account(source_state)?;
    let source = SourceResolutionStateV2::decode(&routed_source.data)
        .map_err(|error| Error::new(format!("Resolution Source state: {error:?}")))?;
    if !matches!(
        source.phase(),
        SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
    ) {
        return Err(refusal(
            "Resolution Source is not resolved/failure-committed for CloseFund",
        ));
    }
    let terminal = source
        .terminal_projection()
        .map_err(|error| Error::new(format!("Resolution terminal projection: {error:?}")))?;
    let closure_sequence = terminal
        .terminal_sequence()
        .checked_add(1)
        .ok_or_else(|| refusal("Resolution closure sequence overflowed"))?;
    let closure_receipt = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &closure_sequence.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let certificate = preliminary_market
        .terminal_receipt
        .ok_or_else(|| refusal("Retiring Market omitted terminal receipt"))?;
    let certificate = Pubkey::new_from_array(certificate.to_bytes());
    let beneficiary = Pubkey::new_from_array(preliminary_market.rent_beneficiary.to_bytes());
    let funding_ledger = evidence_pubkey(evidence, "resolution_funding_ledger")?;
    let core_programdata = pubkey(&plan.core.programdata_id)?;
    let resolution_programdata = pubkey(&plan.resolution.programdata_id)?;
    let activation = pubkey(&plan.activation)?;
    let mut keys = vec![
        payer,
        market_key,
        activation,
        registry,
        core,
        core_programdata,
        resolution,
        resolution_programdata,
        source_material.raw,
        source_material.staging,
        capability_manifest.raw,
        capability_manifest.staging,
        source_state,
        funding_ledger,
        certificate,
        closure_receipt,
        beneficiary,
        sysvar::clock::ID,
        sysvar::rent::ID,
        system_program::ID,
        recovery_policy.raw,
        recovery_policy.staging,
    ];
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let market_account = account(market_key, "Resolution close Market")?;
    let market = decode_routed_market(&market_account, core, plan)?;
    if market != preliminary_market || market.phase != Phase::Retiring {
        return Err(refusal(
            "Resolution close Market changed between coordinate discovery and full snapshot",
        ));
    }
    let close_snapshot = ResolutionCloseFundSnapshotV3 {
        market: market_account,
        activation_cache: account(activation, "Resolution close activation cache")?,
        registry_program: account(registry, "Resolution close Registry")?,
        core_program: account(core, "Resolution close Core")?,
        core_programdata: account(core_programdata, "Resolution close Core ProgramData")?,
        resolution_program: account(resolution, "Resolution program")?,
        resolution_programdata: account(resolution_programdata, "Resolution ProgramData")?,
        source_material: account(source_material.raw, "SourceMaterial")?,
        source_material_staging: account(source_material.staging, "SourceMaterial staging")?,
        capability_manifest: account(capability_manifest.raw, "capability manifest")?,
        capability_manifest_staging: account(
            capability_manifest.staging,
            "capability manifest staging",
        )?,
        source_state: account(source_state, "Resolution Source")?,
        funding_ledger: account(funding_ledger, "Resolution FundingLedger")?,
        certificate: account(certificate, "Resolution certificate")?,
        closure_destination: account(closure_receipt, "Resolution closure receipt")?,
        beneficiary: account(beneficiary, "Resolution beneficiary")?,
        clock_sysvar: account(sysvar::clock::ID, "Clock sysvar")?,
        rent_sysvar: account(sysvar::rent::ID, "Rent sysvar")?,
        system_program: account(system_program::ID, "System Program")?,
        recovery_policy: account(recovery_policy.raw, "RecoveryPolicy")?,
        recovery_policy_staging: account(recovery_policy.staging, "RecoveryPolicy staging")?,
    };
    // The seat was prepaid at the rate this session recorded, and the deployed
    // program's own conjunct (`require_prepaid_output`,
    // `programs/dclutch-resolution-proof-sbf/src/core_effect.rs:2843`) is a
    // FLOOR -- `lamports < minimum` refuses. So a seat holding what it was
    // prepaid is admissible on chain even after a rate drop, and this host
    // bound must be the same figure the prepay finalized with rather than a
    // rederivation that turns the session's own correct arithmetic into a
    // surplus. It stays an upper bound: a seat carrying more than it was
    // prepaid still refuses.
    let exact_receipt_rent =
        funded_rent_minimum_v2(session_funded_rent_rate, SOURCE_CLOSURE_RECEIPT_BYTES_V3)
            .map_err(|error| Error::new(format!("closure receipt funded rent rate: {error:?}")))?;
    if close_snapshot.closure_destination.owner != system_program::ID
        || close_snapshot.closure_destination.executable
        || !close_snapshot.closure_destination.data.is_empty()
        || close_snapshot.closure_destination.lamports > exact_receipt_rent
    {
        return Err(refusal(
            "Resolution closure receipt was not the exact vacant at-most-rent destination",
        ));
    }
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    if close_snapshot.closure_destination.lamports < exact_receipt_rent {
        return Ok(ChainResolutionCloseV1::NeedsReceiptPrepay {
            observation: snapshot.observation,
            payer: account(payer, "Resolution prepay payer")?,
            receipt: close_snapshot.closure_destination,
            exact_receipt_rent,
            prestate,
        });
    }
    let closure = resolution_close_meta_closure_v2(&ResolutionCloseMetaCoordinatesV2 {
        market: market_key,
        activation_cache: activation,
        registry_program: registry,
        core_program: core,
        core_programdata,
        resolution_program: resolution,
        resolution_programdata,
        source_material: source_material.raw,
        source_material_staging: source_material.staging,
        capability_manifest: capability_manifest.raw,
        capability_manifest_staging: capability_manifest.staging,
        source_state,
        funding_ledger,
        certificate,
        closure_receipt,
        beneficiary,
        clock_sysvar: sysvar::clock::ID,
        rent_sysvar: sysvar::rent::ID,
        system_program: system_program::ID,
        recovery_policy: explicit_recovery
            .then_some((recovery_policy.raw, recovery_policy.staging)),
    })?;
    let mutation = plan_resolution_close_caller_v1(
        &close_snapshot,
        &closure,
        deployed_closure_rent_rule_v1(&plan.resolution, plan.checked_local_mutable_set.as_ref())?,
    )?;
    Ok(ChainResolutionCloseV1::Submit {
        stage: ChainDerivedTerminalMutationV1 {
            mutation,
            closure,
            prestate,
        },
        snapshot: Box::new(close_snapshot),
    })
}

pub(crate) fn plan_direct_begin_retiring_caller_v1(
    snapshot: &DirectBeginRetiringSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
) -> Result<DirectBeginRetiringCallerPlanV1> {
    match plan_direct_begin_retiring_v1(snapshot)
        .map_err(|error| Error::new(format!("Direct BeginRetiring: {error:?}")))?
    {
        DirectBeginRetiringPlanV1::Submit(report) => {
            let fresh_closure = TerminalMetaClosureV1 {
                stage: TerminalStageV1::DirectBeginRetiring,
                program_id: report.meta_closure.program_id,
                program_class: TerminalAddressClassV1::InlineProgram,
                accounts: report.meta_closure.accounts.to_vec(),
                classes: report
                    .meta_closure
                    .classes
                    .into_iter()
                    .map(direct_begin_class)
                    .collect(),
            };
            fresh_closure.authenticate_instruction(&report.instruction)?;
            persisted_closure.authenticate_fresh_closure(&fresh_closure)?;
            let root = ExpectedAccountPoststateV1::exact(
                snapshot.root.key,
                snapshot.root.owner,
                snapshot.root.lamports,
                snapshot.root.executable,
                report.expected_post_root_data.clone(),
            );
            if root.data_digest != report.expected_post_root_digest {
                return Err(refusal(
                    "Direct BeginRetiring report post-root digest disagreed with its bytes",
                ));
            }
            Ok(DirectBeginRetiringCallerPlanV1::Submit(
                TerminalSemanticMutationV1 {
                    stage: TerminalStageV1::DirectBeginRetiring,
                    observation: report.observation,
                    instruction: report.instruction.clone(),
                    expected_return_data: Some(ExpectedReturnDataV1 {
                        producer: report.expected_receipt_producer,
                        body: report.expected_receipt_body.to_vec(),
                        execution_clock: None,
                    }),
                    expected_accounts: vec![root],
                    protocol_lamport_deltas: BTreeMap::new(),
                },
            ))
        }
        DirectBeginRetiringPlanV1::Complete(report) => {
            let root = ExpectedAccountPoststateV1::exact(
                report.root,
                snapshot.root.owner,
                snapshot.root.lamports,
                snapshot.root.executable,
                report.observed_post_root_data.clone(),
            );
            if root.data_digest != report.observed_post_root_digest {
                return Err(refusal(
                    "Direct BeginRetiring complete digest disagreed with observed root bytes",
                ));
            }
            Ok(DirectBeginRetiringCallerPlanV1::Complete {
                observation: report.observation,
                root,
            })
        }
    }
}

/// Consume Resolution's existing CloseFund semantic owner and project every
/// writable account exactly. The closure receipt bytes are encoded by their
/// protocol contract from the report's authenticated typed facts.
/// The Resolution closure receipt is written to its account AND returned by
/// the same instruction, and they are the same 416 bytes -- measured on devnet
/// 2026-09-04, where `getTransaction`'s `returnData` was byte-identical to the
/// closure destination's account data. The plan states those bytes ONCE and
/// spends them in both places, so a defect in the prediction can never make
/// the two halves of the plan disagree with each other on top of disagreeing
/// with the chain.
///
/// It used to declare only the account, which is why the first CloseFund ever
/// to execute could not be certified: `authenticate_terminal_return_data_v1`
/// read `(None, Some(_))` -- "finalized terminal transaction carried
/// unexpected return data" -- against a receipt the plan had in fact already
/// written down, in the other field.
fn resolution_close_receipt_poststate_v1(
    resolution_program: Pubkey,
    destination: &ObservedAccount,
    receipt: Vec<u8>,
    execution_clock: ExecutionClockFieldV1,
) -> (ExpectedReturnDataV1, ExpectedAccountPoststateV1) {
    (
        ExpectedReturnDataV1 {
            producer: resolution_program,
            body: receipt.clone(),
            execution_clock: Some(execution_clock),
        },
        ExpectedAccountPoststateV1::exact(
            destination.key,
            resolution_program,
            destination.lamports,
            false,
            receipt,
        )
        .with_execution_clock(execution_clock),
    )
}

/// Where `closed_at` sits inside an encoded closure receipt.
///
/// Never written here as a number. `source_closure_receipt_v3.rs` owns the
/// layout and keeps its offsets private, so this asks the encoder instead: two
/// receipts identical except for `closed_at` differ in exactly one contiguous
/// eight-byte window, and that window reads back the little-endian value. A
/// layout that moves the field moves this answer with it; a layout where the
/// field is not one contiguous little-endian u64 refuses rather than letting a
/// poststate rule bind the wrong bytes.
fn closure_receipt_closed_at_offset_v1(receipt: &SourceClosureReceiptV3) -> Result<usize> {
    let encode = |closed_at: u64| -> Result<Vec<u8>> {
        let mut probe = *receipt;
        probe.closed_at = closed_at;
        Ok(probe
            .to_bytes()
            .map_err(|error| Error::new(format!("closure receipt probe: {error:?}")))?
            .to_vec())
    };
    // 1 and u64::MAX, not 0 and u64::MAX: the codec refuses a zero coordinate,
    // and these two still differ in every one of the eight bytes.
    let low = encode(1)?;
    let high = encode(u64::MAX)?;
    if low.len() != high.len() {
        return Err(refusal(
            "closure receipt encoding changed width with its closed_at",
        ));
    }
    let differing = (0..low.len())
        .filter(|index| low[*index] != high[*index])
        .collect::<Vec<_>>();
    let offset = match differing.as_slice() {
        [first, ..] if differing.len() == 8 && differing.last() == Some(&(first + 7)) => *first,
        _ => {
            return Err(refusal(&format!(
                "closure receipt closed_at is not one contiguous eight-byte window: it moves                  {} bytes",
                differing.len(),
            )));
        }
    };
    let exact = encode(receipt.closed_at)?;
    if low[offset..offset + 8] != 1_u64.to_le_bytes()
        || high[offset..offset + 8] != u64::MAX.to_le_bytes()
        || exact[offset..offset + 8] != receipt.closed_at.to_le_bytes()
    {
        return Err(refusal(
            "closure receipt closed_at window is not the little-endian value it encodes",
        ));
    }
    Ok(offset)
}

pub(crate) fn plan_resolution_close_caller_v1(
    snapshot: &ResolutionCloseFundSnapshotV3,
    persisted_closure: &TerminalMetaClosureV1,
    rent_rule: DeployedClosureRentRuleV1,
) -> Result<TerminalSemanticMutationV1> {
    let report = build_resolution_direct_close_fund_v1(snapshot)
        .map_err(|error| Error::new(format!("Resolution CloseFund: {error:?}")))?;
    let facts = report.expected_retirement_facts;
    // THE PARTITION IS THE DEPLOYED PROGRAM'S, THE SUM IS EVERYONE'S.
    //
    // `facts` answers "what rent did this ledger already pay" with the rate it
    // was FUNDED at, which is right for every guard on an account a founding
    // already bought. What the receipt will SAY is a different question, and
    // its answer belongs to the Resolution that will execute.
    let live_rent: Rent = bincode::deserialize(&snapshot.rent_sysvar.data)
        .map_err(|error| Error::new(format!("Resolution close Rent sysvar: {error}")))?;
    let partition = project_closure_rent_partition_v1(
        rent_rule,
        snapshot.funding_ledger.data.len(),
        &live_rent,
        ClosureRentPartitionV1 {
            ledger_rent_lamports: facts.ledger_rent_lamports,
            ledger_lamport_surplus: facts.ledger_lamport_surplus,
        },
    )?;
    // The plan's own recorded observation clock, which is the number the
    // durable intent states and therefore the one the rule's lower bound must
    // be built from. `facts.closed_at` reads the Clock sysvar of the same
    // finalized snapshot; either is a plan-time reading of a field the
    // execution redefines, and one of them has to be the single author.
    let planned_unix_timestamp = u64::try_from(report.observation.unix_timestamp)
        .map_err(|_| refusal("Resolution close observed a negative clock"))?;
    let receipt = SourceClosureReceiptV3 {
        market: facts.market,
        source_state: facts.source_state,
        source_material: facts.source_material,
        capability_manifest: facts.capability_manifest,
        terminal_certificate: facts.terminal_certificate,
        receipt_account: facts.resolution_closure_receipt,
        beneficiary: facts.beneficiary,
        source_state_digest: facts.source_state_digest,
        terminal_certificate_digest: facts.terminal_certificate_digest,
        funding_set_digest: facts.funding_set_digest,
        generation: facts.generation,
        terminal_sequence: facts.terminal_sequence,
        selector: facts.selector,
        source_refund_lamports: facts.source_refund_lamports,
        ledger_remaining_native_principal: facts.ledger_remaining_native_principal,
        ledger_rent_lamports: partition.ledger_rent_lamports,
        ledger_lamport_surplus: partition.ledger_lamport_surplus,
        refund_lamports: facts.refund_lamports,
        closed_at: planned_unix_timestamp,
    };
    let execution_clock = ExecutionClockFieldV1 {
        offset: closure_receipt_closed_at_offset_v1(&receipt)?,
        planned_unix_timestamp,
        ceiling_unix_timestamp: planned_unix_timestamp
            .checked_add(TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS)
            .ok_or_else(|| refusal("Resolution close execution-clock ceiling overflowed"))?,
    };
    require_derived_execution_clock_ceiling_v1(execution_clock)?;
    let receipt = receipt
        .to_bytes()
        .map_err(|error| Error::new(format!("Resolution closure receipt: {error:?}")))?
        .to_vec();
    if report.closure_receipt != snapshot.closure_destination.key
        || snapshot
            .beneficiary
            .lamports
            .checked_add(facts.refund_lamports)
            .is_none()
    {
        return Err(refusal(
            "Resolution close receipt coordinate or refund arithmetic disagreed",
        ));
    }
    let expected_beneficiary_lamports = snapshot
        .beneficiary
        .lamports
        .checked_add(facts.refund_lamports)
        .ok_or_else(|| refusal("Resolution close beneficiary overflowed"))?;
    let fresh_closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        program_id: report.instruction.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        classes: match report.instruction.accounts.len() {
            19 => resolution_close_meta_classes_v2(false),
            21 => resolution_close_meta_classes_v2(true),
            _ => {
                return Err(refusal(
                    "Resolution close fresh frame has another width than 19 or 21",
                ));
            }
        },
        accounts: report.instruction.accounts.clone(),
    };
    persisted_closure.authenticate_fresh_closure(&fresh_closure)?;
    fresh_closure.authenticate_instruction(&report.instruction)?;
    let (expected_return_data, expected_receipt_account) = resolution_close_receipt_poststate_v1(
        snapshot.resolution_program.key,
        &snapshot.closure_destination,
        receipt,
        execution_clock,
    );
    Ok(TerminalSemanticMutationV1 {
        stage: TerminalStageV1::ResolutionCloseFund,
        observation: report.observation,
        instruction: report.instruction,
        expected_return_data: Some(expected_return_data),
        expected_accounts: vec![
            ExpectedAccountPoststateV1::exact(
                snapshot.market.key,
                snapshot.market.owner,
                snapshot.market.lamports,
                snapshot.market.executable,
                snapshot.market.data.clone(),
            ),
            ExpectedAccountPoststateV1::exact(
                snapshot.source_state.key,
                solana_sdk_ids::system_program::ID,
                0,
                false,
                Vec::new(),
            ),
            ExpectedAccountPoststateV1::exact(
                snapshot.funding_ledger.key,
                solana_sdk_ids::system_program::ID,
                0,
                false,
                Vec::new(),
            ),
            expected_receipt_account,
            ExpectedAccountPoststateV1::exact(
                snapshot.beneficiary.key,
                snapshot.beneficiary.owner,
                expected_beneficiary_lamports,
                snapshot.beneficiary.executable,
                snapshot.beneficiary.data.clone(),
            ),
        ],
        protocol_lamport_deltas: BTreeMap::from([
            (snapshot.market.key, 0),
            (
                snapshot.source_state.key,
                -i128::from(snapshot.source_state.lamports),
            ),
            (
                snapshot.funding_ledger.key,
                -i128::from(snapshot.funding_ledger.lamports),
            ),
            (snapshot.closure_destination.key, 0),
            (snapshot.beneficiary.key, i128::from(facts.refund_lamports)),
        ]),
    })
}

/// Consume the production F=2 Direct close semantic owner. The selected
/// Trading ledger and root close, the Resolution dependency ledger is preserved
/// exactly, and only the two refunded accounts credit RentCredit.
pub(crate) fn plan_direct_native_close_caller_v1(
    snapshot: &DirectNativeCloseSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
) -> Result<TerminalSemanticMutationV1> {
    let report = build_direct_native_close_v1(snapshot)
        .map_err(|error| Error::new(format!("Direct native CloseCapability: {error:?}")))?;
    let fresh_closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::DirectCloseCapability,
        program_id: report.meta_closure.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: report.meta_closure.accounts.clone(),
        classes: report
            .meta_closure
            .classes
            .iter()
            .copied()
            .map(terminal_owner_class)
            .collect(),
    };
    persisted_closure.authenticate_fresh_closure(&fresh_closure)?;
    fresh_closure.authenticate_instruction(&report.instruction)?;
    let market = ExpectedAccountPoststateV1::exact(
        snapshot.market.key,
        report.expected_market_owner,
        report.expected_market_lamports,
        snapshot.market.executable,
        report.expected_market_data.clone(),
    );
    if market.data_digest != report.expected_market_digest
        || report.expected_outstanding_capabilities != 0
        || report.rent_credit_delta_lamports
            != report
                .root_refund_lamports
                .checked_add(report.funding_refund_lamports)
                .ok_or_else(|| refusal("Direct close refund arithmetic overflowed"))?
        || report.expected_rent_credit_lamports
            != report
                .rent_credit_pre_lamports
                .checked_add(report.rent_credit_delta_lamports)
                .ok_or_else(|| refusal("Direct close RentCredit arithmetic overflowed"))?
        || report.preserved_dependency_ledgers.len() != 1
    {
        return Err(refusal(
            "production Direct close byte, F=2 dependency, capability, or refund arithmetic disagreed",
        ));
    }
    let preserved = report
        .preserved_dependency_ledgers
        .into_iter()
        .map(|ledger| {
            let expected = ExpectedAccountPoststateV1::exact(
                ledger.key,
                ledger.owner,
                ledger.lamports,
                ledger.executable,
                ledger.data,
            );
            if expected.data_digest != ledger.data_digest {
                return Err(refusal(
                    "Direct close preserved dependency digest disagreed with its bytes",
                ));
            }
            Ok(expected)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut expected_accounts = vec![
        market,
        ExpectedAccountPoststateV1::exact(
            report.closed_root,
            report.expected_root_owner,
            report.expected_root_lamports,
            false,
            report.expected_root_data,
        ),
        ExpectedAccountPoststateV1::exact(
            report.closed_funding_ledger,
            report.expected_funding_owner,
            report.expected_funding_lamports,
            false,
            report.expected_funding_data,
        ),
        ExpectedAccountPoststateV1::exact(
            report.rent_credit,
            report.expected_rent_credit_owner,
            report.expected_rent_credit_lamports,
            snapshot.rent_credit.executable,
            report.expected_rent_credit_data,
        ),
    ];
    expected_accounts.extend(preserved);
    Ok(TerminalSemanticMutationV1 {
        stage: TerminalStageV1::DirectCloseCapability,
        observation: report.observation,
        instruction: report.instruction,
        expected_return_data: None,
        expected_accounts,
        protocol_lamport_deltas: BTreeMap::from([
            (snapshot.market.key, 0),
            (report.closed_root, -i128::from(report.root_refund_lamports)),
            (
                report.closed_funding_ledger,
                -i128::from(report.funding_refund_lamports),
            ),
            (
                report.rent_credit,
                i128::from(report.rent_credit_delta_lamports),
            ),
        ]),
    })
}

/// Discover the request-bound Core caller from a fully authenticated preflight,
/// then reacquire every role plus that exact PDA in one newer finalized
/// snapshot. No fabricated vacant account reaches the submission builder.
pub(crate) fn plan_direct_native_close_two_pass_from_chain_v1(
    rpc: &mut Rpc,
    discovery: &DirectNativeCloseSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
    additional_keys: &[Pubkey],
) -> Result<ChainDerivedTerminalMutationV1> {
    let preflight = preflight_direct_native_close_caller_v1(discovery)
        .map_err(|error| Error::new(format!("Direct close caller preflight: {error:?}")))?;
    let mut keys = direct_native_close_snapshot_keys_v1(discovery);
    keys.push(preflight.caller_authority);
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let full = DirectNativeCloseSnapshotV1 {
        market: account(discovery.market.key, "Direct close Market")?,
        realm: account(discovery.realm.key, "Direct close Realm")?,
        realm_staging: account(discovery.realm_staging.key, "Direct close Realm staging")?,
        manifest: account(discovery.manifest.key, "Direct close manifest")?,
        manifest_staging: account(
            discovery.manifest_staging.key,
            "Direct close manifest staging",
        )?,
        funding_ledgers: discovery
            .funding_ledgers
            .iter()
            .map(|ledger| account(ledger.key, "Direct close funding ledger"))
            .collect::<Result<Vec<_>>>()?,
        root: account(discovery.root.key, "Direct close root")?,
        activation_cache: account(
            discovery.activation_cache.key,
            "Direct close activation cache",
        )?,
        core_program: account(discovery.core_program.key, "Direct close Core program")?,
        core_programdata: account(
            discovery.core_programdata.key,
            "Direct close Core ProgramData",
        )?,
        trading_program: account(
            discovery.trading_program.key,
            "Direct close Trading program",
        )?,
        trading_programdata: account(
            discovery.trading_programdata.key,
            "Direct close Trading ProgramData",
        )?,
        resolution_program: account(
            discovery.resolution_program.key,
            "Direct close Resolution program",
        )?,
        resolution_programdata: account(
            discovery.resolution_programdata.key,
            "Direct close Resolution ProgramData",
        )?,
        registry_program: account(
            discovery.registry_program.key,
            "Direct close Registry program",
        )?,
        rent_sysvar: account(discovery.rent_sysvar.key, "Direct close Rent sysvar")?,
        caller_authority: Some(account(
            preflight.caller_authority,
            "Direct close request-bound caller",
        )?),
        program_set: account(discovery.program_set.key, "Direct close ProgramSet")?,
        program_set_staging: account(
            discovery.program_set_staging.key,
            "Direct close ProgramSet staging",
        )?,
        config: account(discovery.config.key, "Direct close config")?,
        config_staging: account(discovery.config_staging.key, "Direct close config staging")?,
        close_profile: account(discovery.close_profile.key, "Direct close AccountProfile")?,
        close_profile_staging: account(
            discovery.close_profile_staging.key,
            "Direct close AccountProfile staging",
        )?,
        close_effect: account(discovery.close_effect.key, "Direct close Effect")?,
        close_effect_staging: account(
            discovery.close_effect_staging.key,
            "Direct close Effect staging",
        )?,
        system_program: account(discovery.system_program.key, "System Program")?,
        close_descriptor: account(discovery.close_descriptor.key, "Direct close descriptor")?,
        close_descriptor_staging: account(
            discovery.close_descriptor_staging.key,
            "Direct close descriptor staging",
        )?,
        rent_program: account(discovery.rent_program.key, "Rent program")?,
        rent_credit: account(discovery.rent_credit.key, "Direct close RentCredit")?,
    };
    let mutation = plan_direct_native_close_caller_v1(&full, persisted_closure)?;
    let report = build_direct_native_close_v1(&full)
        .map_err(|error| Error::new(format!("Direct close full caller replay: {error:?}")))?;
    if report.caller_authority != preflight.caller_authority
        || report.role_request_digest != preflight.request_digest
        || report.observation != snapshot.observation
    {
        return Err(refusal(
            "Direct close full snapshot changed the preflight request-bound caller identity",
        ));
    }
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    Ok(ChainDerivedTerminalMutationV1 {
        mutation,
        closure: persisted_closure.clone(),
        prestate,
    })
}

pub(crate) fn direct_native_close_discovery_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
) -> Result<DirectNativeCloseSnapshotV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let realm = routed_record(
        evidence,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let manifest = routed_record(
        evidence,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let program_set = routed_record(
        evidence,
        "direct_program_set_record",
        registry,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    )?;
    let config = routed_record(
        evidence,
        "direct_execution_config_record",
        registry,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
    )?;
    let close_profile = routed_record(
        evidence,
        "direct_native_close_account_profile_record",
        registry,
        direct_native_close_account_profile_schema_v1(),
    )?;
    let close_effect = routed_record(
        evidence,
        "direct_native_close_effect_record",
        registry,
        direct_native_close_effect_schema_v1(),
    )?;
    let close_descriptor = routed_record(
        evidence,
        "direct_native_close_descriptor_record",
        registry,
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
    )?;
    let coordinates = [
        market,
        realm.raw,
        realm.staging,
        manifest.raw,
        manifest.staging,
        evidence_pubkey(evidence, "resolution_funding_ledger")?,
        evidence_pubkey(evidence, "direct_trading_funding_ledger")?,
        evidence_pubkey(evidence, "direct_capability_root")?,
        pubkey(&plan.activation)?,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.core.programdata_id)?,
        pubkey(&plan.trading.program_id)?,
        pubkey(&plan.trading.programdata_id)?,
        pubkey(&plan.resolution.program_id)?,
        pubkey(&plan.resolution.programdata_id)?,
        registry,
        sysvar::rent::ID,
        program_set.raw,
        program_set.staging,
        config.raw,
        config.staging,
        close_profile.raw,
        close_profile.staging,
        close_effect.raw,
        close_effect.staging,
        system_program::ID,
        close_descriptor.raw,
        close_descriptor.staging,
        pubkey(&plan.rent_credit.program_id)?,
        evidence_pubkey(evidence, "founding_lifecycle_rent_credit")?,
    ];
    let snapshot = finalized_snapshot(rpc, &coordinates)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    Ok(DirectNativeCloseSnapshotV1 {
        market: account(market, "Direct close Market")?,
        realm: account(realm.raw, "Direct close Realm")?,
        realm_staging: account(realm.staging, "Direct close Realm staging")?,
        manifest: account(manifest.raw, "Direct close manifest")?,
        manifest_staging: account(manifest.staging, "Direct close manifest staging")?,
        // Canonical mask order, from the one author of that rule. The
        // discovery is what the caller preflight builds its masks from, so an
        // order typed here rather than derived is an order the preflight
        // refuses.
        funding_ledgers: crate::market::ordered_funding_ledger_slice_v1(
            evidence.direct_selected_manifest_entry_index,
            (coordinates[6], "Trading selected FundingLedger"),
            (coordinates[5], "Resolution dependency FundingLedger"),
        )
        .into_iter()
        .map(|(key, label)| account(key, label))
        .collect::<Result<Vec<_>>>()?,
        root: account(coordinates[7], "Direct root")?,
        activation_cache: account(coordinates[8], "activation cache")?,
        core_program: account(coordinates[9], "Core program")?,
        core_programdata: account(coordinates[10], "Core ProgramData")?,
        trading_program: account(coordinates[11], "Trading program")?,
        trading_programdata: account(coordinates[12], "Trading ProgramData")?,
        resolution_program: account(coordinates[13], "Resolution program")?,
        resolution_programdata: account(coordinates[14], "Resolution ProgramData")?,
        registry_program: account(registry, "Registry program")?,
        rent_sysvar: account(sysvar::rent::ID, "Rent sysvar")?,
        caller_authority: None,
        program_set: account(program_set.raw, "Direct ProgramSet")?,
        program_set_staging: account(program_set.staging, "Direct ProgramSet staging")?,
        config: account(config.raw, "Direct config")?,
        config_staging: account(config.staging, "Direct config staging")?,
        close_profile: account(close_profile.raw, "Direct close profile")?,
        close_profile_staging: account(close_profile.staging, "Direct close profile staging")?,
        close_effect: account(close_effect.raw, "Direct close effect")?,
        close_effect_staging: account(close_effect.staging, "Direct close effect staging")?,
        system_program: account(system_program::ID, "System Program")?,
        close_descriptor: account(close_descriptor.raw, "Direct close descriptor")?,
        close_descriptor_staging: account(
            close_descriptor.staging,
            "Direct close descriptor staging",
        )?,
        rent_program: account(coordinates[28], "Rent program")?,
        rent_credit: account(coordinates[29], "RentCredit")?,
    })
}

fn direct_native_close_snapshot_keys_v1(snapshot: &DirectNativeCloseSnapshotV1) -> Vec<Pubkey> {
    let mut keys = vec![
        snapshot.market.key,
        snapshot.realm.key,
        snapshot.realm_staging.key,
        snapshot.manifest.key,
        snapshot.manifest_staging.key,
        snapshot.root.key,
        snapshot.activation_cache.key,
        snapshot.core_program.key,
        snapshot.core_programdata.key,
        snapshot.trading_program.key,
        snapshot.trading_programdata.key,
        snapshot.resolution_program.key,
        snapshot.resolution_programdata.key,
        snapshot.registry_program.key,
        snapshot.rent_sysvar.key,
        snapshot.program_set.key,
        snapshot.program_set_staging.key,
        snapshot.config.key,
        snapshot.config_staging.key,
        snapshot.close_profile.key,
        snapshot.close_profile_staging.key,
        snapshot.close_effect.key,
        snapshot.close_effect_staging.key,
        snapshot.system_program.key,
        snapshot.close_descriptor.key,
        snapshot.close_descriptor_staging.key,
        snapshot.rent_program.key,
        snapshot.rent_credit.key,
    ];
    keys.extend(snapshot.funding_ledgers.iter().map(|ledger| ledger.key));
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Consume the Core/Custody handoff semantic owner without reconstructing its
/// 23-role frame, 208-byte request, or 512-byte receipt.
pub(crate) fn plan_retirement_replay_handoff_caller_v1(
    snapshot: &RetirementReplayHandoffSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
) -> Result<TerminalSemanticMutationV1> {
    let report = build_retirement_replay_handoff_v1(snapshot)
        .map_err(|error| Error::new(format!("retirement replay handoff: {error:?}")))?;
    let fresh_closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::RetirementReplayHandoff,
        program_id: report.meta_closure.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: report.meta_closure.accounts.clone(),
        classes: report
            .meta_closure
            .classes
            .iter()
            .copied()
            .map(terminal_owner_class)
            .collect(),
    };
    persisted_closure.authenticate_fresh_closure(&fresh_closure)?;
    fresh_closure.authenticate_instruction(&report.instruction)?;
    let replay_rent = report.expected_core_replay_lamports;
    let expected_payer = snapshot
        .payer
        .lamports
        .checked_sub(replay_rent)
        .ok_or_else(|| refusal("replay handoff payer rent debit underflowed"))?;
    let expected_credit = snapshot
        .rent_credit
        .lamports
        .checked_add(report.trading_replay_refund_lamports)
        .ok_or_else(|| refusal("replay handoff RentCredit refund overflowed"))?;
    if expected_payer != report.expected_payer_lamports
        || expected_credit != report.expected_rent_credit_lamports
        || hash(&report.expected_core_replay_data).to_bytes() != report.expected_core_replay_digest
    {
        return Err(refusal(
            "replay handoff byte, payer-rent, or RentCredit arithmetic disagreed",
        ));
    }
    Ok(TerminalSemanticMutationV1 {
        stage: TerminalStageV1::RetirementReplayHandoff,
        observation: report.observation,
        instruction: report.instruction,
        expected_return_data: Some(ExpectedReturnDataV1 {
            producer: snapshot.custody_program.key,
            body: report.expected_receipt_body.to_vec(),
            execution_clock: None,
        }),
        expected_accounts: vec![
            ExpectedAccountPoststateV1::exact(
                report.core_replay,
                report.expected_core_replay_owner,
                report.expected_core_replay_lamports,
                report.expected_core_replay_executable,
                report.expected_core_replay_data,
            ),
            ExpectedAccountPoststateV1::exact(
                report.trading_replay,
                report.expected_trading_replay_owner,
                report.expected_trading_replay_lamports,
                false,
                report.expected_trading_replay_data,
            ),
            ExpectedAccountPoststateV1::exact(
                snapshot.payer.key,
                report.expected_payer_owner,
                report.expected_payer_lamports,
                snapshot.payer.executable,
                report.expected_payer_data,
            ),
            ExpectedAccountPoststateV1::exact(
                snapshot.rent_credit.key,
                report.expected_rent_credit_owner,
                report.expected_rent_credit_lamports,
                snapshot.rent_credit.executable,
                report.expected_rent_credit_data,
            ),
            ExpectedAccountPoststateV1::exact(
                report.hoard,
                report.expected_hoard_owner,
                report.expected_hoard_lamports,
                snapshot.hoard.executable,
                report.expected_hoard_data,
            ),
        ],
        protocol_lamport_deltas: BTreeMap::from([
            (snapshot.payer.key, -i128::from(replay_rent)),
            (report.core_replay, i128::from(replay_rent)),
            (
                report.trading_replay,
                -i128::from(report.trading_replay_refund_lamports),
            ),
            (
                snapshot.rent_credit.key,
                i128::from(report.trading_replay_refund_lamports),
            ),
            (report.hoard, 0),
        ]),
    })
}

pub(crate) fn plan_retirement_replay_handoff_two_pass_from_chain_v1(
    rpc: &mut Rpc,
    discovery: &RetirementReplayHandoffSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
    additional_keys: &[Pubkey],
) -> Result<ChainDerivedTerminalMutationV1> {
    let preflight = preflight_retirement_replay_handoff_caller_v1(discovery)
        .map_err(|error| Error::new(format!("replay-handoff caller preflight: {error:?}")))?;
    let mut keys = retirement_replay_handoff_snapshot_keys_v1(discovery);
    keys.push(preflight.caller_authority);
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    let full = RetirementReplayHandoffSnapshotV1 {
        payer: account(discovery.payer.key, "handoff payer")?,
        market: account(discovery.market.key, "handoff Market")?,
        activation_cache: account(discovery.activation_cache.key, "handoff activation cache")?,
        registry_program: account(discovery.registry_program.key, "handoff Registry program")?,
        core_program: account(discovery.core_program.key, "handoff Core program")?,
        core_programdata: account(discovery.core_programdata.key, "handoff Core ProgramData")?,
        trading_program: account(discovery.trading_program.key, "handoff Trading program")?,
        trading_programdata: account(
            discovery.trading_programdata.key,
            "handoff Trading ProgramData",
        )?,
        custody_program: account(discovery.custody_program.key, "handoff Custody program")?,
        custody_programdata: account(
            discovery.custody_programdata.key,
            "handoff Custody ProgramData",
        )?,
        caller_authority: Some(account(
            preflight.caller_authority,
            "handoff request-bound caller",
        )?),
        claims_aggregate: account(discovery.claims_aggregate.key, "handoff Claims aggregate")?,
        realm: account(discovery.realm.key, "handoff Realm")?,
        realm_staging: account(discovery.realm_staging.key, "handoff Realm staging")?,
        rent_sysvar: account(discovery.rent_sysvar.key, "handoff Rent sysvar")?,
        rent_credit: account(discovery.rent_credit.key, "handoff RentCredit")?,
        trading_replay: account(discovery.trading_replay.key, "handoff Trading replay")?,
        core_replay: account(discovery.core_replay.key, "handoff Core replay")?,
        hoard: account(discovery.hoard.key, "handoff Hoard")?,
        system_program: account(discovery.system_program.key, "handoff System Program")?,
        mint: account(discovery.mint.key, "handoff collateral Mint")?,
        token_program: account(discovery.token_program.key, "handoff token program")?,
        custody_authority: account(discovery.custody_authority.key, "handoff Custody authority")?,
    };
    let mutation = plan_retirement_replay_handoff_caller_v1(&full, persisted_closure)?;
    let report = build_retirement_replay_handoff_v1(&full)
        .map_err(|error| Error::new(format!("replay-handoff full caller replay: {error:?}")))?;
    if report.caller_authority != preflight.caller_authority
        || report.request_digest != preflight.request_digest
        || report.observation != snapshot.observation
    {
        return Err(refusal(
            "replay-handoff full snapshot changed the preflight request-bound caller identity",
        ));
    }
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    Ok(ChainDerivedTerminalMutationV1 {
        mutation,
        closure: persisted_closure.clone(),
        prestate,
    })
}

pub(crate) fn retirement_replay_handoff_discovery_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
    payer: Pubkey,
) -> Result<RetirementReplayHandoffSnapshotV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let release = hex32(&plan.release_set_id)?;
    let context = hex32(&evidence.founding_custody_context)?;
    let realm = routed_record(
        evidence,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let claims_aggregate = evidence_pubkey(evidence, "claims_aggregate")?;
    let trading_replay = evidence_pubkey(evidence, "founding_normal_custody_replay")?;
    let core_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(market.to_bytes(), release, ExecutionRoleV1::Core, context)
            .as_slices(),
        &custody,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market.to_bytes(),
            release,
            context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &custody,
    )
    .0;
    if hoard != evidence_pubkey(evidence, "founding_hoard_vault_open")? {
        return Err(refusal(
            "replay-handoff Hoard evidence was not canonical for the authenticated custody context",
        ));
    }
    let authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market.to_bytes(), release).as_slices(),
        &custody,
    )
    .0;
    let mint = evidence_pubkey(evidence, "collateral_mint")?;
    let token_program = pubkey(&required_account(evidence, "collateral_mint")?.owner)?;
    let keys = [
        payer,
        market,
        pubkey(&plan.activation)?,
        registry,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.core.programdata_id)?,
        pubkey(&plan.trading.program_id)?,
        pubkey(&plan.trading.programdata_id)?,
        custody,
        pubkey(&plan.custody.programdata_id)?,
        claims_aggregate,
        realm.raw,
        realm.staging,
        sysvar::rent::ID,
        evidence_pubkey(evidence, "founding_lifecycle_rent_credit")?,
        trading_replay,
        core_replay,
        hoard,
        system_program::ID,
        mint,
        token_program,
        authority,
    ];
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |key: Pubkey, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(key)
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    Ok(RetirementReplayHandoffSnapshotV1 {
        payer: account(payer, "handoff payer")?,
        market: account(market, "handoff Market")?,
        activation_cache: account(keys[2], "handoff activation cache")?,
        registry_program: account(registry, "handoff Registry program")?,
        core_program: account(keys[4], "handoff Core program")?,
        core_programdata: account(keys[5], "handoff Core ProgramData")?,
        trading_program: account(keys[6], "handoff Trading program")?,
        trading_programdata: account(keys[7], "handoff Trading ProgramData")?,
        custody_program: account(custody, "handoff Custody program")?,
        custody_programdata: account(keys[9], "handoff Custody ProgramData")?,
        caller_authority: None,
        claims_aggregate: account(claims_aggregate, "handoff Claims aggregate")?,
        realm: account(realm.raw, "handoff Realm")?,
        realm_staging: account(realm.staging, "handoff Realm staging")?,
        rent_sysvar: account(sysvar::rent::ID, "handoff Rent sysvar")?,
        rent_credit: account(keys[14], "handoff RentCredit")?,
        trading_replay: account(trading_replay, "handoff Trading replay")?,
        core_replay: account(core_replay, "handoff Core replay")?,
        hoard: account(hoard, "handoff Hoard")?,
        system_program: account(system_program::ID, "handoff System Program")?,
        mint: account(mint, "handoff collateral Mint")?,
        token_program: account(token_program, "handoff token program")?,
        custody_authority: account(authority, "handoff Custody authority")?,
    })
}

fn retirement_replay_handoff_snapshot_keys_v1(
    snapshot: &RetirementReplayHandoffSnapshotV1,
) -> Vec<Pubkey> {
    let mut keys = vec![
        snapshot.payer.key,
        snapshot.market.key,
        snapshot.activation_cache.key,
        snapshot.registry_program.key,
        snapshot.core_program.key,
        snapshot.core_programdata.key,
        snapshot.trading_program.key,
        snapshot.trading_programdata.key,
        snapshot.custody_program.key,
        snapshot.custody_programdata.key,
        snapshot.claims_aggregate.key,
        snapshot.realm.key,
        snapshot.realm_staging.key,
        snapshot.rent_sysvar.key,
        snapshot.rent_credit.key,
        snapshot.trading_replay.key,
        snapshot.core_replay.key,
        snapshot.hoard.key,
        snapshot.system_program.key,
        snapshot.mint.key,
        snapshot.token_program.key,
        snapshot.custody_authority.key,
    ];
    if let Some(caller) = &snapshot.caller_authority {
        keys.push(caller.key);
    }
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Consume the existing aggregate-retirement semantic owner and project the
/// complete physical closure/refund poststate. The persisted initial closure
/// must still equal the fresh Registry-wrapped 46-meta report exactly.
pub(crate) fn plan_aggregate_retirement_caller_v1(
    snapshot: &MarketRetirementSnapshotV1,
    persisted_closure: &TerminalMetaClosureV1,
) -> Result<TerminalSemanticMutationV1> {
    let report = build_market_retirement_v1(snapshot)
        .map_err(|error| Error::new(format!("aggregate Market retirement: {error:?}")))?;
    let fresh_closure = TerminalMetaClosureV1 {
        stage: TerminalStageV1::AggregateRetirement,
        program_id: report.instruction.program_id,
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: report.instruction.accounts.clone(),
        classes: aggregate_retirement_meta_classes_v1(),
    };
    persisted_closure.authenticate_fresh_closure(&fresh_closure)?;
    fresh_closure.authenticate_instruction(&report.instruction)?;
    let expected_refund_wallet_lamports = snapshot
        .refund_wallet
        .lamports
        .checked_add(report.expected_refund_delta)
        .ok_or_else(|| refusal("aggregate retirement refund wallet overflowed"))?;
    let closed = |account: &ObservedAccount| {
        ExpectedAccountPoststateV1::exact(
            account.key,
            solana_sdk_ids::system_program::ID,
            0,
            false,
            Vec::new(),
        )
    };
    Ok(TerminalSemanticMutationV1 {
        stage: TerminalStageV1::AggregateRetirement,
        observation: report.observation,
        instruction: report.instruction,
        expected_return_data: None,
        expected_accounts: vec![
            closed(&snapshot.market),
            closed(&snapshot.rent_credit),
            closed(&snapshot.claims_aggregate),
            closed(&snapshot.custody_replay),
            closed(&snapshot.hoard_vault),
            ExpectedAccountPoststateV1::exact(
                snapshot.refund_wallet.key,
                snapshot.refund_wallet.owner,
                expected_refund_wallet_lamports,
                snapshot.refund_wallet.executable,
                snapshot.refund_wallet.data.clone(),
            ),
        ],
        protocol_lamport_deltas: BTreeMap::from([
            (snapshot.market.key, -i128::from(snapshot.market.lamports)),
            (
                snapshot.rent_credit.key,
                -i128::from(snapshot.rent_credit.lamports),
            ),
            (
                snapshot.claims_aggregate.key,
                -i128::from(snapshot.claims_aggregate.lamports),
            ),
            (
                snapshot.custody_replay.key,
                -i128::from(snapshot.custody_replay.lamports),
            ),
            (
                snapshot.hoard_vault.key,
                -i128::from(snapshot.hoard_vault.lamports),
            ),
            (
                snapshot.refund_wallet.key,
                i128::from(report.expected_refund_delta),
            ),
        ]),
    })
}

/// Derive this Market's failure escrow off the Claims aggregate itself.
///
/// Every input is the aggregate account's own: the Claims program is its owner,
/// the logical Market and the runtime width are its header fields. So a
/// substituted document cannot point this at another Market's escrow, and the
/// derivation has the one author every other host consumer uses
/// (`dclutch_operator::failure_escrow_v1`, re-exporting
/// `dclutch_claims::protocol_position_v2::failure_escrow_v1`).
///
/// `None` means there is no escrow to look for at all -- the account is not the
/// Claims aggregate this snapshot expects, or its width seats no failure
/// coordinate. It is never an assertion that the escrow is empty: that is a
/// question about the account at the derived address, which the caller reads.
fn derived_failure_escrow_v1(
    preliminary: &FinalizedSnapshotV1,
    aggregate: Pubkey,
    claims: Pubkey,
) -> Result<Option<dclutch_operator::failure_escrow_v1::FailureEscrowV1>> {
    let account = preliminary
        .account(aggregate)
        .map_err(|error| Error::new(format!("Claims aggregate: {error}")))?;
    if account.owner != claims || account.lamports == 0 {
        return Ok(None);
    }
    let Ok(view) =
        dclutch_claims::liability_basis_state_v2::LiabilityBasisMarketViewV2::decode(&account.data)
    else {
        return Ok(None);
    };
    Ok(dclutch_operator::failure_escrow_v1::failure_escrow_v1(
        claims,
        view.logical_market,
        aggregate,
        view.claim_count,
    )
    .ok())
}

pub(crate) fn aggregate_retirement_snapshot_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
    source_receipt: Pubkey,
    additional_keys: &[Pubkey],
) -> Result<(MarketRetirementSnapshotV1, Vec<ObservedAccount>)> {
    let registry = pubkey(&plan.registry.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let release = hex32(&plan.release_set_id)?;
    let context = hex32(&evidence.founding_custody_context)?;
    let rent_credit = evidence_pubkey(evidence, "founding_lifecycle_rent_credit")?;
    let claims_aggregate = evidence_pubkey(evidence, "claims_aggregate")?;
    let preliminary = finalized_snapshot(rpc, &[rent_credit, claims_aggregate])?;
    let credit_account = preliminary.account(rent_credit)?.clone();
    let credit = LifecycleRentCreditV2::decode(&credit_account.data)
        .map_err(|error| Error::new(format!("aggregate RentCredit: {error:?}")))?;
    let refund_wallet = Pubkey::new_from_array(credit.refund_wallet().to_bytes());
    // THE ESCROW TAIL'S THREE ADDRESSES, EACH FROM ITS ONE AUTHOR.
    //
    // The pair is DERIVED off the aggregate this snapshot already reads --
    // program, logical Market and runtime width are the aggregate's own fields,
    // so this cannot be pointed at another Market's escrow. The linked basis
    // record is NOT derivable from the aggregate, and it is not re-derived by a
    // second hand either: it is a content-addressed Registry record whose
    // digest the FOUNDING PRODUCER stored under `linked_liability_basis_record`
    // (`market.rs`'s `publish_market_records`, published under
    // `GRADED_BASIS_RECORD_SCHEMA_ID_V3`), and `routed_record` recomputes the
    // address from that stored digest and refuses a report whose row does not
    // reproduce it. The producer's digest is the author; this path is a reader.
    let escrow = derived_failure_escrow_v1(&preliminary, claims_aggregate, claims)?;
    // Resolved leniently on purpose. A founding evidence document written before
    // the label existed retires exactly as it did, and a Market that turns out to
    // have a SEATED column without one is refused by name below rather than
    // built into a plan the chain refuses.
    let basis_record = routed_record(
        evidence,
        "linked_liability_basis_record",
        registry,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    )
    .ok();
    let realm = routed_record(
        evidence,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let registry_artifact = plan_record_pair_v1(plan, "registry_artifact_release")?;
    let rent_artifact = plan_record_pair_v1(plan, "rent_artifact_release")?;
    let core_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(market.to_bytes(), release, ExecutionRoleV1::Core, context)
            .as_slices(),
        &custody,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market.to_bytes(),
            release,
            context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &custody,
    )
    .0;
    let authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market.to_bytes(), release).as_slices(),
        &custody,
    )
    .0;
    let mint = evidence_pubkey(evidence, "collateral_mint")?;
    let token_program = pubkey(&required_account(evidence, "collateral_mint")?.owner)?;
    let mut keys = vec![
        market,
        rent_credit,
        pubkey(&plan.activation)?,
        registry,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.core.programdata_id)?,
        claims,
        pubkey(&plan.claims.programdata_id)?,
        pubkey(&plan.resolution.program_id)?,
        pubkey(&plan.resolution.programdata_id)?,
        custody,
        pubkey(&plan.custody.programdata_id)?,
        pubkey(&plan.rent_credit.program_id)?,
        source_receipt,
        claims_aggregate,
        core_replay,
        hoard,
        authority,
        mint,
        token_program,
        realm.raw,
        realm.staging,
        // The V2 domain. Core authenticates the succession profile and nothing
        // else since `2951b226`; the sealed V1 is lineage evidence and is never
        // an account in a live instruction. The founding path reaches this
        // address through Found's own selection, and this path reads it from
        // the plan, so the two must name the same account or a market founded
        // on a born-at-V2 cohort could not be retired.
        pubkey(&plan.genesis_infrastructure_profile.address)?,
        registry_artifact.0,
        registry_artifact.1,
        pubkey(&plan.registry.programdata_id)?,
        rent_artifact.0,
        rent_artifact.1,
        pubkey(&plan.rent_credit.programdata_id)?,
        sysvar::rent::ID,
        refund_wallet,
    ];
    // The escrow tail sits at fixed indexes AFTER the thirty-one and BEFORE the
    // caller's own additional keys, so the frame's shape is a property of this
    // function rather than of what a caller appended.
    let mut escrow_index = None;
    if let Some(escrow) = escrow {
        escrow_index = Some(keys.len());
        keys.push(escrow.position);
        keys.push(escrow.admission);
    }
    let mut basis_index = None;
    if let Some(basis) = basis_record {
        basis_index = Some(keys.len());
        keys.push(basis.raw);
    }
    keys.extend_from_slice(additional_keys);
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let account = |index: usize, label: &str| -> Result<ObservedAccount> {
        snapshot
            .account(keys[index])
            .map_err(|error| Error::new(format!("{label}: {error}")))
            .cloned()
    };
    // SEATED OR VACANT, decided by the chain rather than by which read returned.
    // A Market whose derived escrow holds nothing is the exact thirty-five-account
    // retirement that shipped; one whose escrow is live carries all three trailing
    // accounts, and if the founding evidence names no basis record the tail is
    // refused BY NAME here rather than built into a plan the chain refuses.
    let tail = match escrow_index {
        Some(base) => {
            let position = account(base, "aggregate failure-escrow Position")?;
            if position.lamports == 0 || position.data.is_empty() {
                None
            } else {
                let admission = account(base + 1, "aggregate failure-escrow admission")?;
                let basis = basis_index
                    .ok_or_else(|| {
                        Error::new(format!(
                            "this Market's failure column is seated in escrow Position {} and its \
                             founding evidence carries no canonical linked_liability_basis_record; \
                             the closure's burn needs that record in frame and no second hand may \
                             re-derive its address",
                            position.key
                        ))
                    })
                    .and_then(|index| account(index, "aggregate linked basis record"))?;
                Some((position, admission, basis))
            }
        }
        None => None,
    };
    let retirement = MarketRetirementSnapshotV1 {
        market: account(0, "aggregate Market")?,
        rent_credit: account(1, "aggregate RentCredit")?,
        activation_cache: account(2, "aggregate activation cache")?,
        registry_program: account(3, "aggregate Registry program")?,
        core_program: account(4, "aggregate Core program")?,
        core_programdata: account(5, "aggregate Core ProgramData")?,
        claims_program: account(6, "aggregate Claims program")?,
        claims_programdata: account(7, "aggregate Claims ProgramData")?,
        resolution_program: account(8, "aggregate Resolution program")?,
        resolution_programdata: account(9, "aggregate Resolution ProgramData")?,
        custody_program: account(10, "aggregate Custody program")?,
        custody_programdata: account(11, "aggregate Custody ProgramData")?,
        rent_program: account(12, "aggregate Rent program")?,
        source_receipt: account(13, "aggregate Source receipt")?,
        claims_aggregate: account(14, "aggregate Claims aggregate")?,
        custody_replay: account(15, "aggregate Core Custody replay")?,
        hoard_vault: account(16, "aggregate Hoard vault")?,
        custody_authority: account(17, "aggregate Custody authority")?,
        collateral_mint: account(18, "aggregate collateral Mint")?,
        collateral_token_program: account(19, "aggregate token program")?,
        realm_raw: account(20, "aggregate Realm")?,
        realm_staging: account(21, "aggregate Realm staging")?,
        infrastructure_profile: account(22, "aggregate infrastructure profile")?,
        registry_artifact_raw: account(23, "aggregate Registry ArtifactRelease")?,
        registry_artifact_staging: account(24, "aggregate Registry ArtifactRelease staging")?,
        registry_programdata: account(25, "aggregate Registry ProgramData")?,
        rent_artifact_raw: account(26, "aggregate Rent ArtifactRelease")?,
        rent_artifact_staging: account(27, "aggregate Rent ArtifactRelease staging")?,
        rent_programdata: account(28, "aggregate Rent ProgramData")?,
        rent_sysvar: account(29, "aggregate Rent sysvar")?,
        refund_wallet: account(30, "aggregate refund wallet")?,
        // Decision 0025's escrow tail, threaded. All three or none: half a tail
        // is a shape neither program accepts, and the builder refuses it.
        failure_escrow_position: tail.as_ref().map(|(position, _, _)| position.clone()),
        failure_escrow_admission: tail.as_ref().map(|(_, admission, _)| admission.clone()),
        linked_basis_record: tail.as_ref().map(|(_, _, basis)| basis.clone()),
    };
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    Ok((retirement, prestate))
}

fn plan_record_pair_v1(plan: &SuccessorPlan, label: &str) -> Result<(Pubkey, Pubkey)> {
    let pair = plan
        .records
        .get(label)
        .ok_or_else(|| Error::new(format!("successor plan omitted {label}")))?;
    Ok((pubkey(&pair.raw)?, pubkey(&pair.staging)?))
}

/// Project the six coordinate closures used only to derive the immutable ALT
/// stable-key union. Request-bound coordinates use nonzero planning
/// commitments and are deliberately excluded from the table; every eventual
/// stage still supplies and authenticates its fresh exact request-bound frame.
pub(crate) fn project_terminal_lookup_closures_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
    payer: Pubkey,
    pinned_source_receipt: Option<Pubkey>,
) -> Result<(Vec<TerminalMetaClosureV1>, Pubkey)> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let activation = pubkey(&plan.activation)?;
    let release_set = hex32(&plan.release_set_id)?;
    let context = hex32(&evidence.founding_custody_context)?;
    let root = evidence_pubkey(evidence, "direct_capability_root")?;
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &retired_market_generation_v1(market_input.generation)?.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let rent_credit = evidence_pubkey(evidence, "founding_lifecycle_rent_credit")?;
    let initial = finalized_snapshot(rpc, &[market, source_state, rent_credit])?;
    let state = decode_routed_market(initial.account(market)?, core, plan)?;
    if state.identity.generation != retired_market_generation_v1(market_input.generation)?
        || !matches!(state.phase, Phase::Terminal | Phase::Retiring)
    {
        return Err(refusal(
            "terminal ALT projection requires the exact terminal/retiring founding Market generation",
        ));
    }
    let source_receipt = match pinned_source_receipt {
        Some(receipt) => receipt,
        None => {
            let source = SourceResolutionStateV2::decode(&initial.account(source_state)?.data)
                .map_err(|error| Error::new(format!("terminal ALT Source state: {error:?}")))?;
            let terminal = source.terminal_projection().map_err(|error| {
                Error::new(format!("terminal ALT Source projection: {error:?}"))
            })?;
            let closure_sequence = terminal
                .terminal_sequence()
                .checked_add(1)
                .ok_or_else(|| refusal("terminal ALT Resolution closure sequence overflowed"))?;
            Pubkey::find_program_address(
                &[
                    SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
                    source_state.as_ref(),
                    &closure_sequence.to_le_bytes(),
                ],
                &resolution,
            )
            .0
        }
    };
    let certificate = Pubkey::new_from_array(
        state
            .terminal_receipt
            .ok_or_else(|| refusal("terminal ALT Market omitted terminal receipt"))?
            .to_bytes(),
    );
    let beneficiary = Pubkey::new_from_array(state.rent_beneficiary.to_bytes());
    let source_material = routed_record(
        evidence,
        "source_material_record",
        registry,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    )?;
    let manifest = routed_record(
        evidence,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let recovery = evidence
        .accounts
        .contains_key("recovery_policy_record")
        .then(|| {
            routed_record(
                evidence,
                "recovery_policy_record",
                registry,
                RECOVERY_POLICY_SCHEMA_ID_V2,
            )
        })
        .transpose()?;
    if market_input.direct_capability.is_none() {
        return Err(refusal(
            "terminal ALT projection omitted Direct capability payload",
        ));
    }
    let direct_manifest = evidence_digest(evidence, "capability_manifest_record")?;
    let program_set_digest = evidence_digest(evidence, "direct_program_set_record")?;
    let config_digest = evidence_digest(evidence, "direct_execution_config_record")?;
    let begin_context = direct_begin_retiring_context_v1(
        release_set,
        market.to_bytes(),
        root.to_bytes(),
        direct_manifest,
        program_set_digest,
        config_digest,
        retired_market_generation_v1(market_input.generation)?,
        evidence.direct_selected_manifest_entry_index,
    );
    let begin = direct_begin_retiring_meta_closure_v1(DirectBeginRetiringCoordinateInputV1 {
        request: DirectBeginRetiringRequestV1 {
            release_set,
            market: market.to_bytes(),
            context: begin_context,
            root: root.to_bytes(),
            manifest: direct_manifest,
            program_set: program_set_digest,
            config: config_digest,
            expected_market_digest: [1; 32],
            expected_root_digest: [2; 32],
            generation: retired_market_generation_v1(market_input.generation)?,
            entry_index: evidence.direct_selected_manifest_entry_index,
        },
        descriptor: evidence_digest(evidence, "direct_begin_retiring_descriptor_record")?,
        account_profile: evidence_digest(evidence, "direct_begin_retiring_account_profile_record")?,
        effect: evidence_digest(evidence, "direct_begin_retiring_effect_record")?,
        registry_program: registry,
        core_program: core,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        trading_program: trading,
        trading_programdata: pubkey(&plan.trading.programdata_id)?,
    })?;
    let close_profile = routed_record(
        evidence,
        "direct_native_close_account_profile_record",
        registry,
        direct_native_close_account_profile_schema_v1(),
    )?;
    let close_effect = routed_record(
        evidence,
        "direct_native_close_effect_record",
        registry,
        direct_native_close_effect_schema_v1(),
    )?;
    let close_descriptor = routed_record(
        evidence,
        "direct_native_close_descriptor_record",
        registry,
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
    )?;
    let program_set = routed_record(
        evidence,
        "direct_program_set_record",
        registry,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    )?;
    let config = routed_record(
        evidence,
        "direct_execution_config_record",
        registry,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
    )?;
    let realm = routed_record(
        evidence,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let record = |pair: &crate::wallet_terminal::RecordPairV1| TerminalRecordCoordinatesV1 {
        raw: pair.raw,
        staging: pair.staging,
    };
    let deployment = |program: &crate::model::ProgramPin| -> Result<_> {
        Ok(TerminalDeploymentCoordinatesV1 {
            program: pubkey(&program.program_id)?,
            programdata: pubkey(&program.programdata_id)?,
        })
    };
    let resolution_funding = evidence_pubkey(evidence, "resolution_funding_ledger")?;
    let trading_funding = evidence_pubkey(evidence, "direct_trading_funding_ledger")?;
    // Canonical mask order, which is a fact about the selected entry's manifest
    // POSITION and never about which controller owns which ledger.
    // `selected_funding_ledger_leads_v1` is the one author of that rule.
    let funding_ledgers = crate::market::ordered_funding_ledger_slice_v1(
        evidence.direct_selected_manifest_entry_index,
        trading_funding,
        resolution_funding,
    );
    let selected_funding_position = crate::market::selected_funding_ledger_position_v1(
        evidence.direct_selected_manifest_entry_index,
    );
    let close = direct_native_close_meta_closure_v1(&DirectNativeCloseCoordinateInputV1 {
        release_set,
        role_request_digest: [4; 32],
        market,
        realm: record(&realm),
        manifest: record(&manifest),
        funding_ledgers,
        selected_funding_position,
        root,
        activation_cache: activation,
        core: deployment(&plan.core)?,
        trading: deployment(&plan.trading)?,
        resolution: deployment(&plan.resolution)?,
        registry_program: registry,
        rent_sysvar: sysvar::rent::ID,
        program_set: record(&program_set),
        config: record(&config),
        close_profile: record(&close_profile),
        close_effect: record(&close_effect),
        system_program: system_program::ID,
        close_descriptor: record(&close_descriptor),
        rent_program,
        rent_credit,
    })?;
    let trading_replay = evidence_pubkey(evidence, "founding_normal_custody_replay")?;
    let core_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            market.to_bytes(),
            release_set,
            ExecutionRoleV1::Core,
            context,
        )
        .as_slices(),
        &custody,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market.to_bytes(),
            release_set,
            context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &custody,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market.to_bytes(), release_set).as_slices(),
        &custody,
    )
    .0;
    let collateral_mint = evidence_pubkey(evidence, "collateral_mint")?;
    let token_program = pubkey(&required_account(evidence, "collateral_mint")?.owner)?;
    let handoff =
        retirement_replay_handoff_meta_closure_v1(&RetirementReplayHandoffCoordinateInputV1 {
            release_set,
            context,
            request_digest: [5; 32],
            payer,
            market,
            activation_cache: activation,
            registry_program: registry,
            core: deployment(&plan.core)?,
            trading: deployment(&plan.trading)?,
            custody: deployment(&plan.custody)?,
            claims_aggregate: evidence_pubkey(evidence, "claims_aggregate")?,
            realm: record(&realm),
            rent_sysvar: sysvar::rent::ID,
            rent_credit,
            trading_replay,
            core_replay,
            hoard,
            system_program: system_program::ID,
            mint: collateral_mint,
            token_program,
            custody_authority,
        })?;
    let credit = LifecycleRentCreditV2::decode(&initial.account(rent_credit)?.data)
        .map_err(|error| Error::new(format!("terminal ALT RentCredit: {error:?}")))?;
    let refund_wallet = Pubkey::new_from_array(credit.refund_wallet().to_bytes());
    let registry_artifact = plan_record_pair_v1(plan, "registry_artifact_release")?;
    let rent_artifact = plan_record_pair_v1(plan, "rent_artifact_release")?;
    let continuation = RegistryContinuationRequestV1::new(
        ContentId::new(release_set)
            .map_err(|error| Error::new(format!("terminal ALT release identity: {error:?}")))?,
        ContentId::new([6; 32])
            .map_err(|error| Error::new(format!("terminal ALT cache digest: {error:?}")))?,
        ContentId::new([7; 32])
            .map_err(|error| Error::new(format!("terminal ALT instruction digest: {error:?}")))?,
        1,
        ExecutionRoleV1::Core,
        &[
            ExecutionRoleV1::Core,
            ExecutionRoleV1::Claims,
            ExecutionRoleV1::Resolution,
            ExecutionRoleV1::Custody,
        ],
    )
    .map_err(|error| Error::new(format!("terminal ALT continuation: {error:?}")))?;
    let aggregate = aggregate_retirement_meta_closure_v1(&AggregateRetirementMetaCoordinatesV1 {
        release_set,
        parent_request_digest: [8; 32],
        claims_request_body: vec![1],
        custody_context: context,
        close_vault_request_body: vec![2],
        close_replay_request_body: vec![3],
        rent_post_resource_digest: [9; 32],
        continuation,
        market,
        rent_credit,
        activation_cache: activation,
        registry_program: registry,
        core_program: core,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        claims_program: claims,
        claims_programdata: pubkey(&plan.claims.programdata_id)?,
        resolution_program: resolution,
        resolution_programdata: pubkey(&plan.resolution.programdata_id)?,
        custody_program: custody,
        custody_programdata: pubkey(&plan.custody.programdata_id)?,
        rent_program,
        source_receipt,
        claims_aggregate: evidence_pubkey(evidence, "claims_aggregate")?,
        custody_replay: core_replay,
        hoard_vault: hoard,
        custody_authority,
        collateral_mint,
        collateral_token_program: token_program,
        realm_raw: realm.raw,
        realm_staging: realm.staging,
        // The V2 domain, for the same reason as the aggregate graph above.
        infrastructure_profile: pubkey(&plan.genesis_infrastructure_profile.address)?,
        registry_artifact_raw: registry_artifact.0,
        registry_artifact_staging: registry_artifact.1,
        registry_programdata: pubkey(&plan.registry.programdata_id)?,
        rent_artifact_raw: rent_artifact.0,
        rent_artifact_staging: rent_artifact.1,
        rent_programdata: pubkey(&plan.rent_credit.programdata_id)?,
        rent_sysvar: sysvar::rent::ID,
        refund_wallet,
    })?;
    let resolution_close = resolution_close_meta_closure_v2(&ResolutionCloseMetaCoordinatesV2 {
        market,
        activation_cache: activation,
        registry_program: registry,
        core_program: core,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        resolution_program: resolution,
        resolution_programdata: pubkey(&plan.resolution.programdata_id)?,
        source_material: source_material.raw,
        source_material_staging: source_material.staging,
        capability_manifest: manifest.raw,
        capability_manifest_staging: manifest.staging,
        source_state,
        funding_ledger: resolution_funding,
        certificate,
        closure_receipt: source_receipt,
        beneficiary,
        clock_sysvar: sysvar::clock::ID,
        rent_sysvar: sysvar::rent::ID,
        system_program: system_program::ID,
        recovery_policy: recovery.map(|pair| (pair.raw, pair.staging)),
    })?;
    // THE FOURTH RESTATEMENT OF THE ORDER, and the one PROGRAMS-18A's sweep did
    // not reach. `ea9174135` gave the six mutations one author
    // (`TerminalStageV1::ORDERED`) and reversed `DirectCloseCapability` and
    // `ResolutionCloseFund`; it found and fixed the `#[cfg(test)]` router, the
    // completion verifier's `protocol_order` and that verifier's fixture. This
    // list is the fourth, and no test in this file could have caught it: it is
    // built only by a function that needs a live chain, and every host test
    // constructs its closures by hand. From the reversal until 2026-09-06 every
    // real terminal sequence therefore refused at
    // `terminal_lookup_union_from_closures_v1` with "ALT coordinate closure set
    // is not the exact ordered six-stage sequence" -- measured by the journey
    // tier on hbox `20260906T152908Z`, the first run of any tier to reach a
    // Terminal Market since the reorder.
    //
    // The order here is the ALT union's declaration order and not a run order;
    // `canonical_union_addresses` sorts the addresses anyway. What it has to be
    // is the same order, because that is the check.
    Ok((
        vec![
            core_begin_retiring_meta_closure_v1(
                market,
                activation,
                registry,
                core,
                pubkey(&plan.core.programdata_id)?,
            ),
            begin,
            close,
            resolution_close,
            handoff,
            aggregate,
        ],
        source_receipt,
    ))
}

fn direct_native_close_closure_from_discovery_v1(
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    snapshot: &DirectNativeCloseSnapshotV1,
    request_digest: [u8; 32],
) -> Result<TerminalMetaClosureV1> {
    let records = |raw: Pubkey, staging: Pubkey| TerminalRecordCoordinatesV1 { raw, staging };
    let deployments = |program: Pubkey, programdata: Pubkey| TerminalDeploymentCoordinatesV1 {
        program,
        programdata,
    };
    if snapshot.funding_ledgers.len() != DIRECT_NATIVE_CLOSE_FUNDING_LEDGERS_V1 {
        return Err(refusal(
            "Direct close discovery did not contain exact F=2 funding ledgers",
        ));
    }
    // Discovery already ordered the slice canonically; which of the two the
    // close WRITES follows the selected entry's position, not a controller.
    let selected_funding_position = crate::market::selected_funding_ledger_position_v1(
        evidence.direct_selected_manifest_entry_index,
    );
    direct_native_close_meta_closure_v1(&DirectNativeCloseCoordinateInputV1 {
        release_set: hex32(&plan.release_set_id)?,
        role_request_digest: request_digest,
        market: snapshot.market.key,
        realm: records(snapshot.realm.key, snapshot.realm_staging.key),
        manifest: records(snapshot.manifest.key, snapshot.manifest_staging.key),
        funding_ledgers: [
            snapshot.funding_ledgers[0].key,
            snapshot.funding_ledgers[1].key,
        ],
        selected_funding_position,
        root: snapshot.root.key,
        activation_cache: snapshot.activation_cache.key,
        core: deployments(snapshot.core_program.key, snapshot.core_programdata.key),
        trading: deployments(
            snapshot.trading_program.key,
            snapshot.trading_programdata.key,
        ),
        resolution: deployments(
            snapshot.resolution_program.key,
            snapshot.resolution_programdata.key,
        ),
        registry_program: snapshot.registry_program.key,
        rent_sysvar: snapshot.rent_sysvar.key,
        program_set: records(snapshot.program_set.key, snapshot.program_set_staging.key),
        config: records(snapshot.config.key, snapshot.config_staging.key),
        close_profile: records(
            snapshot.close_profile.key,
            snapshot.close_profile_staging.key,
        ),
        close_effect: records(snapshot.close_effect.key, snapshot.close_effect_staging.key),
        system_program: snapshot.system_program.key,
        close_descriptor: records(
            snapshot.close_descriptor.key,
            snapshot.close_descriptor_staging.key,
        ),
        rent_program: snapshot.rent_program.key,
        rent_credit: snapshot.rent_credit.key,
    })
    .map_err(|error| Error::new(format!("Direct close coordinate projection: {error:?}")))
    .and_then(|closure| {
        if evidence_pubkey(evidence, "direct_capability_root")? != snapshot.root.key {
            return Err(refusal(
                "Direct close discovery root differed from persisted campaign evidence",
            ));
        }
        Ok(closure)
    })
}

fn retirement_handoff_closure_from_discovery_v1(
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    snapshot: &RetirementReplayHandoffSnapshotV1,
    request_digest: [u8; 32],
) -> Result<TerminalMetaClosureV1> {
    retirement_replay_handoff_meta_closure_v1(&RetirementReplayHandoffCoordinateInputV1 {
        release_set: hex32(&plan.release_set_id)?,
        context: hex32(&evidence.founding_custody_context)?,
        request_digest,
        payer: snapshot.payer.key,
        market: snapshot.market.key,
        activation_cache: snapshot.activation_cache.key,
        registry_program: snapshot.registry_program.key,
        core: TerminalDeploymentCoordinatesV1 {
            program: snapshot.core_program.key,
            programdata: snapshot.core_programdata.key,
        },
        trading: TerminalDeploymentCoordinatesV1 {
            program: snapshot.trading_program.key,
            programdata: snapshot.trading_programdata.key,
        },
        custody: TerminalDeploymentCoordinatesV1 {
            program: snapshot.custody_program.key,
            programdata: snapshot.custody_programdata.key,
        },
        claims_aggregate: snapshot.claims_aggregate.key,
        realm: TerminalRecordCoordinatesV1 {
            raw: snapshot.realm.key,
            staging: snapshot.realm_staging.key,
        },
        rent_sysvar: snapshot.rent_sysvar.key,
        rent_credit: snapshot.rent_credit.key,
        trading_replay: snapshot.trading_replay.key,
        core_replay: snapshot.core_replay.key,
        hoard: snapshot.hoard.key,
        system_program: snapshot.system_program.key,
        mint: snapshot.mint.key,
        token_program: snapshot.token_program.key,
        custody_authority: snapshot.custody_authority.key,
    })
    .map_err(|error| Error::new(format!("replay-handoff coordinate projection: {error:?}")))
}

#[allow(clippy::too_many_arguments)]
fn fresh_protocol_stage_from_chain_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
    market: Pubkey,
    payer: Pubkey,
    table: Pubkey,
    source_receipt: Pubkey,
    stage: TerminalStageV1,
    funded_rent_rate: u32,
    compute_unit_limit: Option<u32>,
) -> Result<ChainDerivedTerminalMutationV1> {
    // A message that declares a limit carries the ComputeBudget program as a
    // static key, which makes it a resolved account with a pre- and
    // post-balance; it has to be snapshotted at the SAME finalized observation
    // as the rest of the frame, not read separately afterwards.
    let mut extra = vec![payer, table];
    let mut table_only = vec![table];
    if compute_unit_limit.is_some() {
        extra.push(compute_budget::ID);
        table_only.push(compute_budget::ID);
    }
    match stage {
        TerminalStageV1::CoreBeginRetiring => {
            plan_core_begin_retiring_from_chain_v1(rpc, plan, evidence, market, &extra)
        }
        TerminalStageV1::DirectBeginRetiring => {
            match plan_direct_begin_retiring_from_chain_v1(
                rpc,
                plan,
                market_input,
                evidence,
                market,
                &extra,
            )? {
                ChainDirectBeginRetiringV1::Submit(stage) => Ok(stage),
                ChainDirectBeginRetiringV1::Complete { .. } => Err(refusal(
                    "Direct BeginRetiring is complete on chain but has no exact finalized journal to reconcile",
                )),
            }
        }
        TerminalStageV1::ResolutionCloseFund => {
            match plan_resolution_close_from_chain_v1(
                rpc,
                plan,
                evidence,
                market,
                payer,
                &table_only,
                funded_rent_rate,
            )? {
                ChainResolutionCloseV1::Submit { stage, .. } => Ok(stage),
                ChainResolutionCloseV1::NeedsReceiptPrepay { .. } => Err(refusal(
                    "Resolution CloseFund remains blocked on its durable receipt prepayment",
                )),
            }
        }
        TerminalStageV1::DirectCloseCapability => {
            let discovery =
                direct_native_close_discovery_from_chain_v1(rpc, plan, evidence, market)?;
            let preflight = preflight_direct_native_close_caller_v1(&discovery)
                .map_err(|error| Error::new(format!("Direct close caller preflight: {error:?}")))?;
            let closure = direct_native_close_closure_from_discovery_v1(
                plan,
                evidence,
                &discovery,
                preflight.request_digest,
            )?;
            plan_direct_native_close_two_pass_from_chain_v1(rpc, &discovery, &closure, &extra)
        }
        TerminalStageV1::RetirementReplayHandoff => {
            let discovery = retirement_replay_handoff_discovery_from_chain_v1(
                rpc, plan, evidence, market, payer,
            )?;
            let preflight =
                preflight_retirement_replay_handoff_caller_v1(&discovery).map_err(|error| {
                    Error::new(format!("replay-handoff caller preflight: {error:?}"))
                })?;
            let closure = retirement_handoff_closure_from_discovery_v1(
                plan,
                evidence,
                &discovery,
                preflight.request_digest,
            )?;
            plan_retirement_replay_handoff_two_pass_from_chain_v1(
                rpc,
                &discovery,
                &closure,
                &table_only,
            )
        }
        TerminalStageV1::AggregateRetirement => {
            let (snapshot, prestate) = aggregate_retirement_snapshot_from_chain_v1(
                rpc,
                plan,
                evidence,
                market,
                source_receipt,
                &extra,
            )?;
            let report = build_market_retirement_v1(&snapshot)
                .map_err(|error| Error::new(format!("aggregate retirement: {error:?}")))?;
            // The one-shot route's Core frame is fixed at thirty-five accounts,
            // so it carries no escrow tail and its Claims closure reaches the
            // supply loop with the failure column still standing. Now that this
            // path threads the tail it can say so before a submission rather
            // than after one: the checkpointed route is where a refunding
            // Market retires.
            if report.failure_escrow_seated {
                return Err(Error::new(
                    "this Market's failure column is seated in its derived escrow, and the                      one-shot AggregateRetirement frame is fixed at thirty-five accounts, so its                      Claims closure would refuse the retirement by name (0x5503). Retire it                      through the checkpointed route, whose four packets carry the escrow pair and                      the linked basis record and whose prepare burns the column (decision 0025,                      shape A)",
                ));
            }
            let closure = TerminalMetaClosureV1 {
                stage,
                program_id: report.instruction.program_id,
                program_class: TerminalAddressClassV1::InlineProgram,
                accounts: report.instruction.accounts.clone(),
                classes: aggregate_retirement_meta_classes_v1(),
            };
            let mutation = plan_aggregate_retirement_caller_v1(&snapshot, &closure)?;
            Ok(ChainDerivedTerminalMutationV1 {
                mutation,
                closure,
                prestate,
            })
        }
    }
}

/// Canonical immutable ALT plan shared by every large terminal stage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalLookupTablePlanV1 {
    pub(crate) lookup_table: Pubkey,
    pub(crate) payer: Pubkey,
    pub(crate) authority: Pubkey,
    pub(crate) recent_slot: u64,
    pub(crate) addresses: Vec<Pubkey>,
    pub(crate) create: Instruction,
    pub(crate) extensions: Vec<Instruction>,
    pub(crate) freeze: Instruction,
    pub(crate) final_data_len: usize,
    pub(crate) final_rent_lamports: u64,
    pub(crate) maximum_preflight_wire_bytes: usize,
}

/// Build a deterministic, packet-safe create/extend/freeze preflight without
/// reading a key.  The existing versioned-message operator remains the sole
/// owner of address ordering, extension chunking, and message geometry.
pub(crate) fn plan_terminal_lookup_table_v1(
    payer: Pubkey,
    recent_slot: u64,
    union: &[Pubkey],
    funded_rent_rate: u32,
) -> Result<TerminalLookupTablePlanV1> {
    let plan = build_lookup_table_creation_v1(payer, payer, recent_slot, union)
        .map_err(|error| Error::new(format!("terminal ALT creation plan: {error:?}")))?;
    if plan.addresses.contains(&payer) {
        return Err(Error::new(
            "terminal ALT union must omit the inline transaction fee payer",
        ));
    }
    let freeze = build_lookup_table_freeze(plan.lookup_table, payer);
    let final_data_len = LOOKUP_TABLE_META_SIZE
        .checked_add(
            ALT_ADDRESS_BYTES
                .checked_mul(plan.addresses.len())
                .ok_or_else(|| Error::new("terminal ALT address width overflow"))?,
        )
        .ok_or_else(|| Error::new("terminal ALT data width overflow"))?;
    let final_rent_lamports = funded_rent_minimum_v2(funded_rent_rate, final_data_len)
        .map_err(|error| Error::new(format!("terminal ALT funded rent rate: {error:?}")))?;
    let observation = Observation {
        slot: recent_slot
            .checked_add(1)
            .ok_or_else(|| Error::new("terminal ALT geometry slot overflow"))?,
        unix_timestamp: 1,
        finality: Finality::Finalized,
    };
    let maximum_preflight_wire_bytes = std::iter::once(&plan.create)
        .chain(plan.extensions.iter())
        .chain(std::iter::once(&freeze))
        .map(|instruction| {
            compile_v0_message_with_optional_tables(
                payer,
                std::slice::from_ref(instruction),
                Hash::new_from_array(ALT_GEOMETRY_BLOCKHASH),
                observation,
                &[],
            )
            .map(|message| message.wire_bytes)
            .map_err(|error| Error::new(format!("terminal ALT packet geometry: {error:?}")))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max()
        .ok_or_else(|| Error::new("terminal ALT preflight has no instructions"))?;
    if maximum_preflight_wire_bytes > PACKET_DATA_BYTES {
        return Err(Error::new("terminal ALT preflight exceeds packet limit"));
    }
    Ok(terminal_lookup_plan(
        plan,
        payer,
        recent_slot,
        freeze,
        final_data_len,
        final_rent_lamports,
        maximum_preflight_wire_bytes,
    ))
}

fn terminal_lookup_plan(
    plan: LookupTableCreationPlanV1,
    payer: Pubkey,
    recent_slot: u64,
    freeze: Instruction,
    final_data_len: usize,
    final_rent_lamports: u64,
    maximum_preflight_wire_bytes: usize,
) -> TerminalLookupTablePlanV1 {
    TerminalLookupTablePlanV1 {
        lookup_table: plan.lookup_table,
        payer,
        authority: payer,
        recent_slot,
        addresses: plan.addresses,
        create: plan.create,
        extensions: plan.extensions,
        freeze,
        final_data_len,
        final_rent_lamports,
        maximum_preflight_wire_bytes,
    }
}

/// Require a caller-supplied table to be the exact, frozen canonical terminal
/// union.  A mutable superset is not immutable routing evidence.
pub(crate) fn authenticate_supplied_terminal_lookup_table_v1(
    expected_addresses: &[Pubkey],
    table: &ObservedAccount,
    funded_rent_rate: u32,
) -> Result<()> {
    let canonical = canonical_union_addresses(expected_addresses)?;
    if table.owner != lookup_table_program::id()
        || table.executable
        || table.observation.finality != Finality::Finalized
    {
        return Err(Error::new(
            "supplied terminal ALT owner/executable/finality refused",
        ));
    }
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|_| Error::new("supplied terminal ALT bytes refused"))?;
    let final_data_len = lookup_table_data_len(canonical.len())?;
    // Six conjuncts, one sentence, and the sixth used to be the cluster's rent
    // at the moment of the read rather than the rate the table was funded at.
    // A frozen table's lamports never change; the number it was compared
    // against did, and that is what refused market 1's retirement with
    // 11,703,384 on a table whose rederived minimum had become 9,387,840.
    // Still EXACT -- one extra lamport on the table still refuses, which is
    // what `terminal_alt_refuses_divergence_partial_freeze_surplus_and_wrong_boundary`
    // is written about.
    let final_rent_minimum = funded_rent_minimum_v2(funded_rent_rate, final_data_len)
        .map_err(|error| Error::new(format!("terminal ALT funded rent rate: {error:?}")))?;
    if table.lamports != final_rent_minimum {
        return Err(Error::new(format!(
            "supplied terminal ALT holds {} against the {final_rent_minimum} its funded rate of \
             {funded_rent_rate} lamports per byte prices {final_data_len} bytes at",
            table.lamports
        )));
    }
    if decoded.meta.authority.is_some()
        || decoded.meta.deactivation_slot != u64::MAX
        || decoded.meta.last_extended_slot >= table.observation.slot
        || decoded.addresses.as_ref() != canonical.as_slice()
        || table.data.len() != final_data_len
    {
        return Err(Error::new(format!(
            "supplied terminal ALT is not the exact activated frozen union              (authority {}, deactivating {}, extended at {} against observation {},              addresses {} of {}, width {} against {final_data_len})",
            decoded.meta.authority.is_some(),
            decoded.meta.deactivation_slot != u64::MAX,
            decoded.meta.last_extended_slot,
            table.observation.slot,
            decoded.addresses.as_ref().len(),
            canonical.len(),
            table.data.len(),
        )));
    }
    Ok(())
}

/// Resume create/append/freeze from exact finalized state.  Existing bytes may
/// equal only a complete extension prefix; divergence, overfill, a foreign
/// authority, surplus lamports, or premature freeze all refuse.
pub(crate) fn route_terminal_lookup_table_v1(
    plan: &TerminalLookupTablePlanV1,
    table: Option<&ObservedAccount>,
    funded_rent_rate: u32,
) -> Result<TerminalLookupTableRouteV1> {
    let Some(table) = table.filter(|account| account.lamports != 0) else {
        return Ok(TerminalLookupTableRouteV1::Create(plan.create.clone()));
    };
    if table.key != plan.lookup_table
        || table.owner != lookup_table_program::id()
        || table.executable
        || table.observation.finality != Finality::Finalized
    {
        return Err(Error::new(
            "terminal ALT address/owner/executable/finality refused",
        ));
    }
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|_| Error::new("terminal ALT bytes refused"))?;
    if decoded.meta.deactivation_slot != u64::MAX {
        return Err(Error::new("terminal ALT is deactivating"));
    }
    let addresses = decoded.addresses.as_ref();
    let expected_data_len = lookup_table_data_len(addresses.len())?;
    // Four conjuncts that used to share one sentence, and three of them are
    // properties of the table while the rent one is a property of the CLUSTER at
    // the moment of the read. The surplus case is refused ON PURPOSE --
    // `terminal_alt_refuses_divergence_partial_freeze_surplus_and_wrong_boundary`
    // is written about one extra lamport -- so nothing here is loosened; a
    // reader is only told which of the four it hit and what the two numbers are.
    let table_rent_minimum = funded_rent_minimum_v2(funded_rent_rate, expected_data_len)
        .map_err(|error| Error::new(format!("terminal ALT funded rent rate: {error:?}")))?;
    if table.data.len() != expected_data_len {
        return Err(Error::new(format!(
            "terminal ALT data width {} is not the {expected_data_len} its {} addresses need",
            table.data.len(),
            addresses.len()
        )));
    }
    if table.lamports != table_rent_minimum {
        return Err(Error::new(format!(
            "terminal ALT holds {} against the {table_rent_minimum} its funded rate of \
             {funded_rent_rate} lamports per byte prices {expected_data_len} bytes at",
            table.lamports
        )));
    }
    if addresses.len() > plan.addresses.len() || addresses != &plan.addresses[..addresses.len()] {
        return Err(Error::new(format!(
            "terminal ALT carries {} addresses that are not a canonical prefix of the plan's {}",
            addresses.len(),
            plan.addresses.len()
        )));
    }
    let complete = addresses.len() == plan.addresses.len();
    if !addresses.is_empty() {
        let expected_start = ((addresses.len() - 1) / EXTEND_ADDRESSES_PER_TRANSACTION_V1)
            * EXTEND_ADDRESSES_PER_TRANSACTION_V1;
        if usize::from(decoded.meta.last_extended_slot_start_index) != expected_start
            || decoded.meta.last_extended_slot >= table.observation.slot
        {
            return Err(Error::new(
                "terminal ALT extension boundary is not canonical or active",
            ));
        }
    }
    match decoded.meta.authority {
        None if complete => {
            // Same split, same verdicts: the width is the table's and the rent
            // minimum is the cluster's, and only one of them can move under an
            // account that is already frozen.
            if table.data.len() != plan.final_data_len {
                return Err(Error::new(format!(
                    "frozen terminal ALT width {} is not the planned {}",
                    table.data.len(),
                    plan.final_data_len
                )));
            }
            if table.lamports != plan.final_rent_lamports {
                return Err(Error::new(format!(
                    "frozen terminal ALT holds {} against the planned rent minimum {}",
                    table.lamports, plan.final_rent_lamports
                )));
            }
            Ok(TerminalLookupTableRouteV1::Complete)
        }
        None => Err(Error::new(
            "terminal ALT froze before the union was complete",
        )),
        Some(authority) if authority != plan.authority => {
            Err(Error::new("terminal ALT authority was substituted"))
        }
        Some(_) if complete => Ok(TerminalLookupTableRouteV1::Freeze(plan.freeze.clone())),
        Some(_) => {
            let starts = extension_prefix_lengths(plan);
            let extension_index = starts
                .iter()
                .position(|prefix| *prefix == addresses.len())
                .ok_or_else(|| {
                    Error::new("terminal ALT prefix ends inside a canonical extension page")
                })?;
            let instruction = plan
                .extensions
                .get(extension_index)
                .ok_or_else(|| Error::new("terminal ALT extension prefix overflow"))?
                .clone();
            Ok(TerminalLookupTableRouteV1::Extend {
                prefix_len: addresses.len(),
                instruction,
            })
        }
    }
}

/// Build one ALT create/extend/freeze action through the same durable message,
/// signature, balance-vector, submission, and finalized verifier as protocol
/// stages. Extend poststate commits an exact slot-relative rule because the ALT
/// program writes the eventual transaction slot, which does not exist yet at
/// plan time.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_lookup_infrastructure_journal_v1(
    rpc: &mut Rpc,
    origin: &ClusterOriginV1,
    expected_cluster: ExpectedClusterV1,
    plan: &TerminalLookupTablePlanV1,
    route: &TerminalLookupTableRouteV1,
    payer: &ObservedAccount,
    table: &ObservedAccount,
    rent: &Rent,
    prestate: &[ObservedAccount],
    authorized_mutation: bool,
) -> Result<DurableTerminalJournalV1> {
    expected_cluster.authenticate(origin)?;
    if payer.key != plan.payer
        || plan.authority != plan.payer
        || table.key != plan.lookup_table
        || payer.observation != table.observation
        || payer.observation.finality != Finality::Finalized
    {
        return Err(refusal(
            "terminal ALT journal mixed cluster, payer, table, authority, or observation",
        ));
    }
    let (mutation, instruction, authority, addresses, last_slot, start_index) = match route {
        TerminalLookupTableRouteV1::Create(instruction) => {
            if table.owner != solana_sdk_ids::system_program::ID
                || table.lamports != 0
                || table.executable
                || !table.data.is_empty()
                || instruction != &plan.create
            {
                return Err(refusal(
                    "terminal ALT create prestate or instruction differed from exact vacant plan",
                ));
            }
            (
                DurableTerminalMutationV1::LookupCreate,
                instruction.clone(),
                Some(plan.authority),
                Vec::new(),
                DurableLookupLastExtendedSlotV1::Exact(0),
                0,
            )
        }
        TerminalLookupTableRouteV1::Extend {
            prefix_len,
            instruction,
        } => {
            let current = AddressLookupTable::deserialize(&table.data)
                .map_err(|_| refusal("terminal ALT extend prestate did not decode"))?;
            let prefixes = extension_prefix_lengths(plan);
            let extension_index = prefixes
                .iter()
                .position(|prefix| prefix == prefix_len)
                .ok_or_else(|| refusal("terminal ALT extend prefix was not canonical"))?;
            if current.meta.authority != Some(plan.authority)
                || current.meta.deactivation_slot != u64::MAX
                || current.addresses.as_ref() != &plan.addresses[..*prefix_len]
                || plan.extensions.get(extension_index) != Some(instruction)
            {
                return Err(refusal(
                    "terminal ALT extend prestate, prefix, authority, or instruction diverged",
                ));
            }
            let next_len = prefix_len
                .checked_add(EXTEND_ADDRESSES_PER_TRANSACTION_V1)
                .map(|value| value.min(plan.addresses.len()))
                .ok_or_else(|| refusal("terminal ALT extension prefix overflowed"))?;
            (
                DurableTerminalMutationV1::LookupExtend {
                    prefix_len: *prefix_len,
                },
                instruction.clone(),
                Some(plan.authority),
                plan.addresses[..next_len].to_vec(),
                DurableLookupLastExtendedSlotV1::FinalizedTransaction,
                u8::try_from(*prefix_len)
                    .map_err(|_| refusal("terminal ALT extension prefix exceeded u8"))?,
            )
        }
        TerminalLookupTableRouteV1::Freeze(instruction) => {
            let current = AddressLookupTable::deserialize(&table.data)
                .map_err(|_| refusal("terminal ALT freeze prestate did not decode"))?;
            if current.meta.authority != Some(plan.authority)
                || current.meta.deactivation_slot != u64::MAX
                || current.addresses.as_ref() != plan.addresses.as_slice()
                || instruction != &plan.freeze
            {
                return Err(refusal(
                    "terminal ALT freeze prestate, addresses, authority, or instruction diverged",
                ));
            }
            (
                DurableTerminalMutationV1::LookupFreeze,
                instruction.clone(),
                None,
                plan.addresses.clone(),
                DurableLookupLastExtendedSlotV1::Exact(current.meta.last_extended_slot),
                current.meta.last_extended_slot_start_index,
            )
        }
        TerminalLookupTableRouteV1::Complete => {
            return Err(refusal(
                "complete frozen ALT has no infrastructure mutation to journal",
            ));
        }
    };
    let mut prestate_by_key = BTreeMap::new();
    for account in prestate {
        if account.observation != payer.observation
            || prestate_by_key.insert(account.key, account).is_some()
        {
            return Err(refusal(
                "terminal ALT prestate mixed observations or duplicate accounts",
            ));
        }
    }
    for key in std::iter::once(payer.key)
        .chain(instruction.accounts.iter().map(|meta| meta.pubkey))
        .chain(std::iter::once(instruction.program_id))
    {
        if !prestate_by_key.contains_key(&key) {
            return Err(refusal(
                "terminal ALT prestate omitted payer, program, or instruction account",
            ));
        }
    }
    let expected_table_lamports = rent.minimum_balance(lookup_table_data_len(addresses.len())?);
    let table_top_up = expected_table_lamports
        .checked_sub(table.lamports)
        .ok_or_else(|| refusal("terminal ALT action would require a rent refund"))?;
    let payer_after_protocol = payer
        .lamports
        .checked_sub(table_top_up)
        .ok_or_else(|| refusal("terminal ALT payer cannot cover exact rent delta"))?;
    let (recent_blockhash, last_valid_block_height) = terminal_latest_blockhash(rpc)?;
    let compiled = compile_v0_message_with_optional_tables(
        payer.key,
        std::slice::from_ref(&instruction),
        recent_blockhash,
        payer.observation,
        &[],
    )
    .map_err(|error| Error::new(format!("terminal ALT durable message: {error:?}")))?;
    let VersionedMessage::V0(message) = &compiled.message else {
        return Err(refusal("terminal ALT action did not compile as v0"));
    };
    if compiled.loaded_addresses != 0 || !compiled.lookup_tables.is_empty() {
        return Err(refusal(
            "terminal ALT infrastructure mutation unexpectedly used a lookup table",
        ));
    }
    let resolved_account_keys = message.account_keys.clone();
    let pre_balances = resolved_account_keys
        .iter()
        .map(|key| {
            prestate_by_key
                .get(key)
                .map(|account| account.lamports)
                .ok_or_else(|| refusal("terminal ALT resolved key omitted prestate"))
        })
        .collect::<Result<Vec<_>>>()?;
    let message_bytes = compiled.message.serialize();
    let message_base64 = BASE64.encode(&message_bytes);
    let transaction_fee_lamports = terminal_fee_for_message(rpc, &message_base64)?;
    let payer_after_fee = payer_after_protocol
        .checked_sub(transaction_fee_lamports)
        .ok_or_else(|| refusal("terminal ALT payer cannot cover exact rent plus fee"))?;
    let deltas = BTreeMap::from([
        (payer.key, -i128::from(table_top_up)),
        (table.key, i128::from(table_top_up)),
    ]);
    if checked_terminal_delta_sum_v1(deltas.values().copied())? != 0 {
        return Err(refusal("terminal ALT rent transfer did not conserve"));
    }
    let post_balances = resolved_account_keys
        .iter()
        .zip(pre_balances.iter().copied())
        .map(|(key, pre)| {
            let after = checked_apply_lamport_delta(pre, deltas.get(key).copied().unwrap_or(0))?;
            if *key == payer.key {
                after
                    .checked_sub(transaction_fee_lamports)
                    .ok_or_else(|| refusal("terminal ALT fee debit underflowed"))
            } else {
                Ok(after)
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let expected_accounts = BTreeMap::from([
        (
            payer.key.to_string(),
            DurableExpectedAccountV1 {
                address: payer.key.to_string(),
                owner: payer.owner.to_string(),
                lamports_after_protocol: payer_after_protocol,
                lamports_after_fee: payer_after_fee,
                executable: payer.executable,
                data_base64: BASE64.encode(&payer.data),
                data_sha256: sha256_hex(&payer.data),
                lookup_table: None,
                execution_clock: None,
            },
        ),
        (
            table.key.to_string(),
            DurableExpectedAccountV1 {
                address: table.key.to_string(),
                owner: lookup_table_program::id().to_string(),
                lamports_after_protocol: expected_table_lamports,
                lamports_after_fee: expected_table_lamports,
                executable: false,
                data_base64: String::new(),
                data_sha256: String::new(),
                lookup_table: Some(DurableLookupTablePoststateV1 {
                    authority: authority.map(|key| key.to_string()),
                    addresses: addresses.iter().map(ToString::to_string).collect(),
                    deactivation_slot: u64::MAX,
                    last_extended_slot: last_slot,
                    last_extended_slot_start_index: start_index,
                }),
                execution_clock: None,
            },
        ),
    ]);
    let classes = instruction
        .accounts
        .iter()
        .map(|meta| {
            if meta.is_signer {
                TerminalAddressClassV1::InlineSigner
            } else if meta.pubkey == lookup_table_program::id()
                || meta.pubkey == solana_sdk_ids::system_program::ID
            {
                TerminalAddressClassV1::InlineProgram
            } else {
                TerminalAddressClassV1::InlineRequestBound
            }
        })
        .collect::<Vec<_>>();
    let intent = DurableTerminalIntentV1 {
        mutation,
        observation_slot: payer.observation.slot,
        observation_unix_timestamp: payer.observation.unix_timestamp,
        payer: payer.key.to_string(),
        program_id: instruction.program_id.to_string(),
        program_class: TerminalAddressClassV1::InlineProgram,
        accounts: instruction
            .accounts
            .iter()
            .zip(classes)
            .map(|(meta, class)| durable_instruction_account_v1(meta, class, payer.key))
            .collect(),
        instruction_data_base64: BASE64.encode(&instruction.data),
        instruction_data_sha256: sha256_hex(&instruction.data),
        // ALT create/extend/freeze: measured on devnet 2026-09-04 at 10,508,
        // 1,500-odd and 1,517 CU, all inside the default meter.
        compute_unit_limit: None,
        lookup_table: None,
        lookup_table_addresses: Vec::new(),
        lookup_table_addresses_sha256: pubkey_vector_sha256(&[]),
        loaded_writable: Vec::new(),
        loaded_readonly: Vec::new(),
        resolved_account_keys: resolved_account_keys
            .iter()
            .map(ToString::to_string)
            .collect(),
        pre_balances,
        post_balances,
        recent_blockhash: recent_blockhash.to_string(),
        last_valid_block_height,
        transaction_fee_lamports,
        wire_bytes: compiled.wire_bytes,
        message_base64,
        message_sha256: sha256_hex(&message_bytes),
        prestate: prestate_by_key
            .into_iter()
            .map(|(key, account)| (key.to_string(), durable_observed_state(account)))
            .collect(),
        expected_accounts,
        expected_return_data: None,
        protocol_lamport_deltas: deltas
            .into_iter()
            .map(|(key, delta)| (key.to_string(), delta))
            .collect(),
    };
    let intent_sha256 = sha256_hex(&serde_json::to_vec(&intent)?);
    let mut journal = DurableTerminalJournalV1 {
        schema: terminal_journal_schema_v1(expected_cluster).into(),
        cluster: expected_cluster.evidence_label().into(),
        rpc_url: origin.redacted_url(),
        authorized_mutation,
        state_sha256: String::new(),
        phase: StageJournalPhaseV1::Planned,
        intent_sha256,
        intent,
        signed_packet_base64: None,
        expected_signature: None,
        finalized: None,
        superseded: None,
        reconciled: None,
    };
    refresh_terminal_journal_digest_v1(&mut journal)?;
    authenticate_terminal_journal_v1(&journal)?;
    Ok(journal)
}

pub(crate) fn authenticate_lookup_infrastructure_planned_journal_v1(
    journal: &DurableTerminalJournalV1,
    plan: &TerminalLookupTablePlanV1,
    route: &TerminalLookupTableRouteV1,
    payer: &ObservedAccount,
    table: &ObservedAccount,
    funded_rent_rate: u32,
    exact_execution_prestate: &[ObservedAccount],
) -> Result<AuthenticatedPlannedTerminalIntentV1> {
    authenticate_terminal_journal_v1(journal)?;
    if journal.phase != StageJournalPhaseV1::Planned
        || payer.key != plan.payer
        || table.key != plan.lookup_table
        || payer.observation != table.observation
    {
        return Err(refusal(
            "terminal ALT semantic-owner authorization mixed journal, payer, table, or observation",
        ));
    }
    let rerouted = route_terminal_lookup_table_v1(plan, Some(table), funded_rent_rate)?;
    if &rerouted != route {
        return Err(refusal(
            "terminal ALT durable route differed from the fresh canonical chain route",
        ));
    }
    let (mutation, instruction, authority, addresses, last_slot, start_index) = match route {
        TerminalLookupTableRouteV1::Create(instruction) => (
            DurableTerminalMutationV1::LookupCreate,
            instruction,
            Some(plan.authority),
            Vec::new(),
            DurableLookupLastExtendedSlotV1::Exact(0),
            0,
        ),
        TerminalLookupTableRouteV1::Extend {
            prefix_len,
            instruction,
        } => {
            let next_len = prefix_len
                .checked_add(EXTEND_ADDRESSES_PER_TRANSACTION_V1)
                .map(|value| value.min(plan.addresses.len()))
                .ok_or_else(|| refusal("terminal ALT authorization prefix overflowed"))?;
            (
                DurableTerminalMutationV1::LookupExtend {
                    prefix_len: *prefix_len,
                },
                instruction,
                Some(plan.authority),
                plan.addresses[..next_len].to_vec(),
                DurableLookupLastExtendedSlotV1::FinalizedTransaction,
                u8::try_from(*prefix_len)
                    .map_err(|_| refusal("terminal ALT authorization prefix exceeded u8"))?,
            )
        }
        TerminalLookupTableRouteV1::Freeze(instruction) => {
            let current = AddressLookupTable::deserialize(&table.data)
                .map_err(|_| refusal("terminal ALT authorization freeze state did not decode"))?;
            (
                DurableTerminalMutationV1::LookupFreeze,
                instruction,
                None,
                plan.addresses.clone(),
                DurableLookupLastExtendedSlotV1::Exact(current.meta.last_extended_slot),
                current.meta.last_extended_slot_start_index,
            )
        }
        TerminalLookupTableRouteV1::Complete => {
            return Err(refusal(
                "complete terminal ALT has no Planned mutation to authorize",
            ));
        }
    };
    let classes = instruction
        .accounts
        .iter()
        .map(|meta| {
            if meta.is_signer {
                TerminalAddressClassV1::InlineSigner
            } else if meta.pubkey == lookup_table_program::id()
                || meta.pubkey == solana_sdk_ids::system_program::ID
            {
                TerminalAddressClassV1::InlineProgram
            } else {
                TerminalAddressClassV1::InlineRequestBound
            }
        })
        .collect::<Vec<_>>();
    let accounts = instruction
        .accounts
        .iter()
        .zip(classes)
        .map(|(meta, class)| durable_instruction_account_v1(meta, class, payer.key))
        .collect::<Vec<_>>();
    if journal.intent.mutation != mutation
        || payer.observation.slot < journal.intent.observation_slot
        || journal.intent.payer != payer.key.to_string()
        || journal.intent.program_id != instruction.program_id.to_string()
        || journal.intent.program_class != TerminalAddressClassV1::InlineProgram
        || journal.intent.accounts != accounts
        || journal.intent.instruction_data_base64 != BASE64.encode(&instruction.data)
        || journal.intent.instruction_data_sha256 != sha256_hex(&instruction.data)
        || journal.intent.lookup_table.is_some()
    {
        return Err(refusal(
            "terminal ALT durable message differed from the canonical infrastructure semantic owner",
        ));
    }
    let observed_prestate = exact_execution_prestate
        .iter()
        .map(|account| (account.key.to_string(), durable_observed_state(account)))
        .collect::<BTreeMap<_, _>>();
    if observed_prestate.len() != exact_execution_prestate.len()
        || exact_execution_prestate
            .iter()
            .any(|account| account.observation != payer.observation)
        || observed_prestate != journal.intent.prestate
    {
        return Err(refusal(
            "terminal ALT durable prestate differed from the fresh finalized canonical route",
        ));
    }
    let table_lamports =
        funded_rent_minimum_v2(funded_rent_rate, lookup_table_data_len(addresses.len())?)
            .map_err(|error| refusal(&format!("terminal ALT funded rent rate: {error:?}")))?;
    let top_up = table_lamports
        .checked_sub(table.lamports)
        .ok_or_else(|| refusal("terminal ALT authorization required a rent refund"))?;
    let payer_after_protocol = payer
        .lamports
        .checked_sub(top_up)
        .ok_or_else(|| refusal("terminal ALT authorization payer underflowed"))?;
    let payer_after_fee = payer_after_protocol
        .checked_sub(journal.intent.transaction_fee_lamports)
        .ok_or_else(|| refusal("terminal ALT authorization fee underflowed"))?;
    let expected_accounts = BTreeMap::from([
        (
            payer.key.to_string(),
            DurableExpectedAccountV1 {
                address: payer.key.to_string(),
                owner: payer.owner.to_string(),
                lamports_after_protocol: payer_after_protocol,
                lamports_after_fee: payer_after_fee,
                executable: payer.executable,
                data_base64: BASE64.encode(&payer.data),
                data_sha256: sha256_hex(&payer.data),
                lookup_table: None,
                execution_clock: None,
            },
        ),
        (
            table.key.to_string(),
            DurableExpectedAccountV1 {
                address: table.key.to_string(),
                owner: lookup_table_program::id().to_string(),
                lamports_after_protocol: table_lamports,
                lamports_after_fee: table_lamports,
                executable: false,
                data_base64: String::new(),
                data_sha256: String::new(),
                lookup_table: Some(DurableLookupTablePoststateV1 {
                    authority: authority.map(|key| key.to_string()),
                    addresses: addresses.iter().map(ToString::to_string).collect(),
                    deactivation_slot: u64::MAX,
                    last_extended_slot: last_slot,
                    last_extended_slot_start_index: start_index,
                }),
                execution_clock: None,
            },
        ),
    ]);
    let deltas = BTreeMap::from([
        (payer.key.to_string(), -i128::from(top_up)),
        (table.key.to_string(), i128::from(top_up)),
    ]);
    if journal.intent.expected_accounts != expected_accounts
        || journal.intent.protocol_lamport_deltas != deltas
        || journal.intent.expected_return_data.is_some()
    {
        return Err(refusal(
            "terminal ALT poststate, rent, fee, or lamport vector differed from the canonical route",
        ));
    }
    Ok(AuthenticatedPlannedTerminalIntentV1 {
        intent_sha256: journal.intent_sha256.clone(),
    })
}

fn extension_prefix_lengths(plan: &TerminalLookupTablePlanV1) -> Vec<usize> {
    let mut prefixes = Vec::with_capacity(plan.extensions.len());
    let mut count = 0_usize;
    for _instruction in &plan.extensions {
        prefixes.push(count);
        let remaining_addresses = plan.addresses.len().saturating_sub(count);
        let page = remaining_addresses.min(EXTEND_ADDRESSES_PER_TRANSACTION_V1);
        count = count.saturating_add(page);
    }
    prefixes
}

fn lookup_table_data_len(address_count: usize) -> Result<usize> {
    LOOKUP_TABLE_META_SIZE
        .checked_add(
            ALT_ADDRESS_BYTES
                .checked_mul(address_count)
                .ok_or_else(|| Error::new("terminal ALT address width overflow"))?,
        )
        .ok_or_else(|| Error::new("terminal ALT data width overflow"))
}

fn canonical_union_addresses(addresses: &[Pubkey]) -> Result<Vec<Pubkey>> {
    let mut canonical = addresses.to_vec();
    canonical.sort_unstable_by_key(Pubkey::to_bytes);
    if canonical.is_empty() {
        return Err(Error::new("terminal ALT union is empty"));
    }
    if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::new("terminal ALT union contains a duplicate"));
    }
    Ok(canonical)
}

/// Build the only admitted terminal ALT union from all six typed coordinate
/// closures. Only semantic-owner-assigned `LookupStable` identities enter the
/// table. Signers, program identities, and request/balance-bound coordinates
/// remain inline even when their public keys happen to look durable. This runs
/// before any protocol stage and therefore consumes no projected account state.
pub(crate) fn terminal_lookup_union_from_closures_v1(
    payer: Pubkey,
    closures: &[TerminalMetaClosureV1],
) -> Result<Vec<Pubkey>> {
    if closures.len() != TerminalStageV1::ORDERED.len()
        || closures
            .iter()
            .zip(TerminalStageV1::ORDERED)
            .any(|(closure, expected)| closure.stage != expected)
    {
        return Err(refusal(
            "ALT coordinate closure set is not the exact ordered six-stage sequence",
        ));
    }
    let mut assigned = BTreeMap::<Pubkey, TerminalAddressClassV1>::new();
    let mut stable = BTreeSet::new();
    for closure in closures {
        if closure.program_id == Pubkey::default()
            || closure.program_class != TerminalAddressClassV1::InlineProgram
            || closure.accounts.is_empty()
            || closure.classes.len() != closure.accounts.len()
        {
            return Err(refusal(
                "ALT coordinate closure has a vacant program, missing class, or non-inline program identity",
            ));
        }
        assign_terminal_address_class(
            &mut assigned,
            closure.program_id,
            TerminalAddressClassV1::InlineProgram,
        )?;
        for (index, (account, class)) in closure
            .accounts
            .iter()
            .zip(closure.classes.iter().copied())
            .enumerate()
        {
            // The System Program's id IS the all-zero pubkey, so a frame that
            // legitimately names it is byte-indistinguishable from one that
            // left a coordinate unset. Every closure here names it as the
            // `system_program::ID` constant and classes it InlineProgram — a
            // class no derived coordinate carries, since those are LookupStable
            // — so exempting exactly that position keeps the vacancy refusal
            // meaningful everywhere it can still mean anything.
            //
            // Without this, `ResolutionCloseFund` refuses at frame index 18 of
            // 19 on its own System Program, which is why no market had ever
            // reached the stage behind it.
            let names_system_program = class == TerminalAddressClassV1::InlineProgram
                && !account.is_signer
                && !account.is_writable;
            if account.pubkey == Pubkey::default() && !names_system_program {
                // Which stage, and which index within its frame. A closure set
                // spanning six stages and a couple of hundred metas behind one
                // string is a refusal nobody can act on.
                return Err(refusal(&format!(
                    "ALT coordinate closure for {:?} carries a vacant account identity at frame \
                     index {index} of {}",
                    closure.stage,
                    closure.accounts.len()
                )));
            }
            match class {
                TerminalAddressClassV1::LookupStable => {
                    if account.is_signer || account.pubkey == payer {
                        return Err(refusal(
                            "ALT LookupStable role was a signer or the inline fee payer",
                        ));
                    }
                    stable.insert(account.pubkey);
                }
                TerminalAddressClassV1::InlineSigner => {
                    if !account.is_signer || account.pubkey != payer {
                        return Err(refusal(
                            "terminal InlineSigner role was not the exact fee payer signer",
                        ));
                    }
                }
                TerminalAddressClassV1::InlineProgram => {
                    if account.is_signer || account.is_writable {
                        return Err(refusal(
                            "terminal InlineProgram role carried signer or writable privilege",
                        ));
                    }
                }
                TerminalAddressClassV1::InlineRequestBound => {
                    if account.is_signer {
                        return Err(refusal(
                            "terminal request-bound invocation coordinate cannot sign",
                        ));
                    }
                }
            }
            assign_terminal_address_class(&mut assigned, account.pubkey, class)?;
        }
    }
    if stable.is_empty() {
        return Err(Error::new("terminal ALT stable union is empty"));
    }
    Ok(stable.into_iter().collect())
}

fn assign_terminal_address_class(
    assigned: &mut BTreeMap<Pubkey, TerminalAddressClassV1>,
    key: Pubkey,
    class: TerminalAddressClassV1,
) -> Result<()> {
    if assigned
        .insert(key, class)
        .is_some_and(|existing| existing != class)
    {
        return Err(refusal(
            "one terminal identity was assigned conflicting ALT placement classes",
        ));
    }
    Ok(())
}

/// Prove the v0 compiler respected every semantic-owner address class. Stable
/// identities must be loaded from the one frozen dedicated table; payer,
/// programs, signers, and request-bound identities must all remain static.
pub(crate) fn authenticate_terminal_v0_placement_v1(
    payer: Pubkey,
    closure: &TerminalMetaClosureV1,
    lookup_table: Pubkey,
    frozen_addresses: &[Pubkey],
    compiled: &VersionedMessagePlanV0,
) -> Result<()> {
    let VersionedMessage::V0(message) = &compiled.message else {
        return Err(refusal("terminal stage did not compile as a v0 message"));
    };
    if compiled.lookup_tables != [lookup_table]
        || message.address_table_lookups.len() != 1
        || message.address_table_lookups[0].account_key != lookup_table
    {
        return Err(refusal(
            "terminal stage did not use exactly the dedicated frozen lookup table",
        ));
    }
    let mut expected_stable = BTreeMap::<Pubkey, bool>::new();
    let mut expected_inline = BTreeSet::<Pubkey>::from([payer, closure.program_id]);
    for (meta, class) in closure.accounts.iter().zip(closure.classes.iter().copied()) {
        match class {
            TerminalAddressClassV1::LookupStable => {
                expected_stable
                    .entry(meta.pubkey)
                    .and_modify(|writable| *writable |= meta.is_writable)
                    .or_insert(meta.is_writable);
            }
            TerminalAddressClassV1::InlineSigner
            | TerminalAddressClassV1::InlineProgram
            | TerminalAddressClassV1::InlineRequestBound => {
                expected_inline.insert(meta.pubkey);
            }
        }
    }
    if expected_stable
        .keys()
        .any(|stable| expected_inline.contains(stable))
    {
        return Err(refusal(
            "terminal stage assigned one identity both stable and inline",
        ));
    }
    let lookup = &message.address_table_lookups[0];
    let loaded_writable = lookup
        .writable_indexes
        .iter()
        .map(|index| {
            frozen_addresses
                .get(usize::from(*index))
                .copied()
                .ok_or_else(|| refusal("terminal v0 writable lookup index exceeded frozen ALT"))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let loaded_readonly = lookup
        .readonly_indexes
        .iter()
        .map(|index| {
            frozen_addresses
                .get(usize::from(*index))
                .copied()
                .ok_or_else(|| refusal("terminal v0 readonly lookup index exceeded frozen ALT"))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    if !loaded_writable.is_disjoint(&loaded_readonly)
        || loaded_writable.len() + loaded_readonly.len() != compiled.loaded_addresses
    {
        return Err(refusal(
            "terminal v0 lookup indices were duplicated or its loaded count disagreed",
        ));
    }
    let expected_writable = expected_stable
        .iter()
        .filter_map(|(key, writable)| writable.then_some(*key))
        .collect::<BTreeSet<_>>();
    let expected_readonly = expected_stable
        .iter()
        .filter_map(|(key, writable)| (!writable).then_some(*key))
        .collect::<BTreeSet<_>>();
    if loaded_writable != expected_writable || loaded_readonly != expected_readonly {
        return Err(refusal(
            "terminal v0 loaded keys or privileges differed from LookupStable roles",
        ));
    }
    let static_keys = message
        .account_keys
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if expected_stable
        .keys()
        .any(|stable| static_keys.contains(stable))
        || expected_inline
            .iter()
            .any(|inline| !static_keys.contains(inline))
    {
        return Err(refusal(
            "terminal v0 static/lookup placement differed from owner-assigned address classes",
        ));
    }
    Ok(())
}

fn resolved_terminal_v0_keys_v1(
    compiled: &VersionedMessagePlanV0,
    lookup_table: Pubkey,
    frozen_addresses: &[Pubkey],
) -> Result<(Vec<Pubkey>, Vec<Pubkey>, Vec<Pubkey>)> {
    let VersionedMessage::V0(message) = &compiled.message else {
        return Err(refusal("terminal message was not v0"));
    };
    if message.address_table_lookups.len() != 1
        || message.address_table_lookups[0].account_key != lookup_table
    {
        return Err(refusal(
            "terminal message did not carry its one dedicated lookup",
        ));
    }
    let lookup = &message.address_table_lookups[0];
    let resolve = |index: &u8| -> Result<Pubkey> {
        frozen_addresses
            .get(usize::from(*index))
            .copied()
            .ok_or_else(|| refusal("terminal lookup index exceeded frozen address vector"))
    };
    let writable = lookup
        .writable_indexes
        .iter()
        .map(resolve)
        .collect::<Result<Vec<_>>>()?;
    let readonly = lookup
        .readonly_indexes
        .iter()
        .map(resolve)
        .collect::<Result<Vec<_>>>()?;
    let mut resolved = message.account_keys.clone();
    resolved.extend(writable.iter().copied());
    resolved.extend(readonly.iter().copied());
    if resolved.len() != message.account_keys.len() + compiled.loaded_addresses
        || resolved.iter().copied().collect::<BTreeSet<_>>().len() != resolved.len()
    {
        return Err(refusal(
            "terminal resolved v0 key vector contained a duplicate or wrong width",
        ));
    }
    Ok((writable, readonly, resolved))
}

#[cfg(test)]
impl TerminalRouteV1 {
    const fn ordinal(self) -> Option<u8> {
        match self {
            Self::PrepayResolutionReceipt => Some(TerminalStageV1::ResolutionCloseFund.ordinal()),
            Self::Execute(stage) => Some(stage.ordinal()),
            Self::Complete => None,
        }
    }
}

/// Protocol facts after their respective semantic owners authenticated one
/// same-finalized observation.  Booleans here are conclusions, never caller
/// assertions or persisted routing hints.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedTerminalProgressV1 {
    pub(crate) core: CoreTerminalStateV1,
    pub(crate) direct: DirectTerminalStateV1,
    pub(crate) resolution: ResolutionTerminalStateV1,
    pub(crate) replay: RetirementReplayStateV1,
    pub(crate) outstanding_capabilities: u64,
    pub(crate) all_claim_supplies_zero: bool,
    pub(crate) claims_aggregate_live: bool,
    pub(crate) rent_credit_live: bool,
    pub(crate) hoard_vault_live: bool,
}

/// Select exactly one next mutation from one authenticated finalized graph.
#[cfg(test)]
pub(crate) fn route_terminal_progress_v1(
    progress: AuthenticatedTerminalProgressV1,
) -> Result<TerminalRouteV1> {
    if progress.claims_aggregate_live && !progress.all_claim_supplies_zero {
        return Err(refusal(
            "Claims supply became nonzero during terminal retirement",
        ));
    }
    match progress.core {
        CoreTerminalStateV1::Terminal => route_terminal_market(progress),
        CoreTerminalStateV1::Retiring => route_retiring_market(progress),
        CoreTerminalStateV1::Closed => route_closed_market(progress),
    }
}

#[cfg(test)]
fn route_terminal_market(progress: AuthenticatedTerminalProgressV1) -> Result<TerminalRouteV1> {
    if progress.direct != DirectTerminalStateV1::Open
        || progress.resolution == ResolutionTerminalStateV1::Closed
        || progress.replay != RetirementReplayStateV1::Trading
        || progress.outstanding_capabilities != 1
        || !progress.claims_aggregate_live
        || !progress.rent_credit_live
        || !progress.hoard_vault_live
    {
        return Err(refusal(
            "a downstream terminal resource advanced before Core BeginRetiring",
        ));
    }
    Ok(TerminalRouteV1::Execute(TerminalStageV1::CoreBeginRetiring))
}

#[cfg(test)]
fn route_retiring_market(progress: AuthenticatedTerminalProgressV1) -> Result<TerminalRouteV1> {
    if !progress.claims_aggregate_live || !progress.rent_credit_live || !progress.hoard_vault_live {
        return Err(refusal(
            "aggregate resources partially closed while the Core Market remains Retiring",
        ));
    }
    match progress.direct {
        DirectTerminalStateV1::Open => {
            if progress.resolution == ResolutionTerminalStateV1::Closed
                || progress.replay != RetirementReplayStateV1::Trading
                || progress.outstanding_capabilities != 1
            {
                return Err(refusal(
                    "a downstream stage advanced before Trading BeginDirectRetiring",
                ));
            }
            Ok(TerminalRouteV1::Execute(
                TerminalStageV1::DirectBeginRetiring,
            ))
        }
        DirectTerminalStateV1::Retiring => route_retiring_direct(progress),
        DirectTerminalStateV1::Closed => route_closed_direct(progress),
    }
}

/// The Direct root is retiring: its capability close is the only next act.
///
/// This used to read the RESOLUTION state here and send a market whose fund was
/// still open to `ResolutionCloseFund` -- a second author of the stage order,
/// agreeing with the wrong `ORDERED` because both were written in this file.
/// Under the ruling the Resolution fund is IRRELEVANT at this point except that
/// it must still be open: the Direct close decodes the dependency ledger, so a
/// market whose fund has already closed cannot be routed forward at all.
#[cfg(test)]
fn route_retiring_direct(progress: AuthenticatedTerminalProgressV1) -> Result<TerminalRouteV1> {
    if progress.replay != RetirementReplayStateV1::Trading || progress.outstanding_capabilities != 1
    {
        return Err(refusal(
            "Direct close or replay handoff partially committed before the Direct capability close",
        ));
    }
    if progress.resolution == ResolutionTerminalStateV1::Closed {
        return Err(refusal(
            TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose.message(),
        ));
    }
    Ok(TerminalRouteV1::Execute(
        TerminalStageV1::DirectCloseCapability,
    ))
}

/// The Direct root is closed and Core's capability count is zero: the
/// Resolution fund closes next, then the replay handoff, then the aggregate.
#[cfg(test)]
fn route_closed_direct(progress: AuthenticatedTerminalProgressV1) -> Result<TerminalRouteV1> {
    if progress.outstanding_capabilities != 0 {
        return Err(refusal(
            "Direct root closure lacks the exact Core capability decrement",
        ));
    }
    match progress.resolution {
        ResolutionTerminalStateV1::NeedsReceiptPrepayment => {
            Ok(TerminalRouteV1::PrepayResolutionReceipt)
        }
        ResolutionTerminalStateV1::ReadyToClose => Ok(TerminalRouteV1::Execute(
            TerminalStageV1::ResolutionCloseFund,
        )),
        ResolutionTerminalStateV1::Closed => match progress.replay {
            RetirementReplayStateV1::Trading => Ok(TerminalRouteV1::Execute(
                TerminalStageV1::RetirementReplayHandoff,
            )),
            RetirementReplayStateV1::Core => Ok(TerminalRouteV1::Execute(
                TerminalStageV1::AggregateRetirement,
            )),
            RetirementReplayStateV1::Closed => Err(refusal(
                "Custody replay closed outside the atomic aggregate-retirement poststate",
            )),
        },
    }
}

#[cfg(test)]
fn route_closed_market(progress: AuthenticatedTerminalProgressV1) -> Result<TerminalRouteV1> {
    if progress.direct != DirectTerminalStateV1::Closed
        || progress.resolution != ResolutionTerminalStateV1::Closed
        || progress.replay != RetirementReplayStateV1::Closed
        || progress.outstanding_capabilities != 0
        || progress.claims_aggregate_live
        || progress.rent_credit_live
        || progress.hoard_vault_live
    {
        return Err(refusal(
            "closed Core Market has a substituted or partial aggregate-retirement poststate",
        ));
    }
    Ok(TerminalRouteV1::Complete)
}

fn refusal(reason: &str) -> Error {
    Error::new(format!("REFUSED terminal sequence: {reason}"))
}

/// Authenticate that one durable stage journal cannot be replayed or silently
/// retargeted after a later chain stage has finalized.
#[cfg(test)]
pub(crate) fn authenticate_journal_route_v1(
    journal_mutation: TerminalRouteV1,
    journal_phase: StageJournalPhaseV1,
    current: TerminalRouteV1,
) -> Result<()> {
    if journal_mutation == TerminalRouteV1::Complete {
        return Err(refusal("a durable journal cannot plan Complete"));
    }
    match (journal_mutation, current) {
        (journal, observed) if journal == observed => Ok(()),
        (
            TerminalRouteV1::PrepayResolutionReceipt,
            TerminalRouteV1::Execute(TerminalStageV1::ResolutionCloseFund),
        ) => Err(refusal(
            "receipt prepayment appeared but its exact journal is not yet reconciled",
        )),
        (journal, observed)
            if journal
                .ordinal()
                .zip(observed.ordinal())
                .is_some_and(|(left, right)| right == left.saturating_add(1)) =>
        {
            // The caller must still resolve and authenticate the journal's
            // exact finalized signature/receipt before opening the next stage.
            let _ = journal_phase;
            Err(refusal(
                "chain poststate advanced but the prior journal is not yet reconciled",
            ))
        }
        (
            TerminalRouteV1::Execute(TerminalStageV1::AggregateRetirement),
            TerminalRouteV1::Complete,
        ) => Err(refusal(
            "aggregate poststate exists but its exact journal is not yet reconciled",
        )),
        _ => Err(refusal(
            "durable journal stage does not equal the one next stage selected by finalized state",
        )),
    }
}

pub(crate) fn run_terminal_sequence(arguments: Vec<String>) -> Result<()> {
    run_terminal_sequence_with_expected_cluster_v1(arguments, ExpectedClusterV1::Devnet)
}

pub(crate) fn run_terminal_sequence_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run_terminal_sequence_with_expected_cluster_v1(arguments, ExpectedClusterV1::OwnedLoopback)
}

fn run_terminal_sequence_with_expected_cluster_v1(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    let arguments = parse_terminal_sequence_arguments_v1(arguments, expected_cluster)?;
    expected_cluster.authenticate(&arguments.origin)?;
    if !arguments.journal_dir.is_dir() {
        return Err(refusal(
            "--journal-dir must be an existing absolute directory",
        ));
    }
    let plan_source = fs::read(&arguments.plan)?;
    let market_source = fs::read(&arguments.market_input)?;
    let evidence_source = fs::read(&arguments.evidence)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let market_input: MarketRunInput = serde_json::from_slice(&market_source)?;
    let founding_evidence = if expected_cluster == ExpectedClusterV1::Devnet {
        parse_campaign_terminal_evidence_v1(&evidence_source)?
    } else {
        parse_campaign_terminal_evidence_with_expected_cluster_v1(
            &evidence_source,
            expected_cluster,
        )?
    };
    authenticate_plan_source(&plan_source, &founding_evidence.plan_sha256)?;
    let refreshed_source = arguments
        .refreshed_evidence
        .as_ref()
        .map(fs::read)
        .transpose()?;
    let mut rpc = Rpc::connect_cluster(
        &arguments.origin,
        if arguments.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;
    authenticate_terminal_cluster_v1(&mut rpc, &arguments.origin, expected_cluster)?;
    // ONE DURABLE ACT, AND IT DOES NOT DEPEND ON THE SESSION.
    //
    // Retiring an unlandable packet is what a sequence needs precisely when its
    // session cannot be loaded -- a schema that moved under it, an input digest
    // that changed -- so it runs before the session is touched and returns
    // rather than continuing into a pass that would plan a stage.
    if let Some(retired) = arguments.supersede_unlandable.as_deref() {
        if arguments.reconcile_landed.is_some() {
            return Err(refusal(
                "--supersede-unlandable and --reconcile-landed are the two opposite answers to \
                 one question and may not be asked together",
            ));
        }
        return operate_terminal_supersede_v1(&mut rpc, &arguments, retired);
    }
    // The other exit from Submitted, and it needs the session no more than the
    // first does: a prediction that cannot certify is exactly what stops a
    // sequence from loading past the stage that holds it.
    if let Some(landed) = arguments.reconcile_landed.as_deref() {
        return operate_terminal_reconcile_landed_v1(&mut rpc, &arguments, &plan, landed);
    }
    // The refresh widens *which document may carry a row*, never what is then
    // demanded of the row: `require_direct_retirement_evidence` and
    // `authenticate_campaign_market_v1` below run unchanged, against the
    // effective map. `docs/design/EVIDENCE_REFRESH_V1.md` §2, and §3 for why
    // this stage in particular needs it — `direct_capability_root` names two
    // different addresses, the founding checkpoint's founding-permit root (at
    // which no account can ever exist) and the execution root this stage means,
    // and the refresh is the document that emits the second under that label.
    //
    // These two checks moved below the cluster connect because the refresh must
    // be admitted against a finalized slot before the evidence it produces can
    // be judged. Nothing between them was reordered, and with no refresh
    // supplied the sequence is byte-for-byte what it was.
    let evidence = match refreshed_source.as_deref() {
        None => founding_evidence,
        Some(bytes) => {
            let refresh = crate::evidence_refresh::parse_refresh_v1(bytes)?;
            let effective = crate::evidence_refresh::effective_accounts_v1(
                &refresh,
                &evidence_source,
                &terminal_rows_as_model_v1(&founding_evidence.accounts),
                &founding_evidence.plan_sha256,
                expected_cluster,
                rpc.finalized_slot()?,
            )?;
            // §3's remedy, applied to the second label that names two values.
            // `founding_custody_context` records the founding's own action
            // pre-image; Custody addresses the Hoard and every post-founding
            // role replay under its projected-hoard digest, and every consumer
            // below means that one. The refresh selects which, and cannot
            // introduce a third: the equality is re-derived here.
            let founding_custody_context = crate::evidence_refresh::effective_custody_context_v1(
                Some(&refresh),
                &founding_evidence.founding_custody_context,
            )?;
            CampaignTerminalEvidenceV1 {
                accounts: model_rows_as_terminal_v1(effective),
                founding_custody_context,
                ..founding_evidence
            }
        }
    };
    require_direct_retirement_evidence(&evidence)?;
    authenticate_campaign_market_v1(&evidence, arguments.market)?;
    let input_digests = (
        sha256_hex(&plan_source),
        sha256_hex(&market_source),
        sha256_hex(&evidence_source),
        refreshed_source.as_deref().map(sha256_hex),
    );
    let session = load_or_create_terminal_session_v1(
        &mut rpc,
        &arguments,
        &plan,
        &market_input,
        &evidence,
        &input_digests,
    )?;
    let source_receipt = Pubkey::from_str(&session.source_receipt)
        .map_err(|error| Error::new(format!("terminal session Source receipt: {error}")))?;
    let lookup_table = Pubkey::from_str(&session.lookup_table)
        .map_err(|error| Error::new(format!("terminal session lookup table: {error}")))?;
    let lookup_addresses = authenticate_terminal_session_semantics_v1(
        &mut rpc,
        &arguments,
        &session,
        &plan,
        &market_input,
        &evidence,
    )?;
    // THE SEQUENCE NO LONGER READS THE RENT SYSVAR AT ALL after its session
    // exists. Every account this run touches was funded before it started, and
    // the session records the rate they were funded at; a fresh reading here is
    // exactly the reading that refused market 1's retirement five guards deep.
    if !operate_terminal_lookup_preflight_v1(
        &mut rpc,
        &arguments,
        &session,
        lookup_table,
        &lookup_addresses,
    )? {
        return Ok(());
    }
    if !operate_terminal_protocol_journals_v1(
        &mut rpc,
        &arguments,
        &session,
        &plan,
        &market_input,
        &evidence,
        lookup_table,
        &lookup_addresses,
        source_receipt,
    )? {
        return Ok(());
    }
    let completion = build_terminal_sequence_completion_v1(
        &mut rpc,
        &arguments,
        &session,
        lookup_table,
        &lookup_addresses,
    )?;
    write_or_authenticate_terminal_completion_v1(&arguments.completion, &completion)?;
    terminal_stdout_v1(json!({
        "status": "complete",
        "market": arguments.market.to_string(),
        "lookupTable": lookup_table.to_string(),
        "journalDirectory": arguments.journal_dir.display().to_string(),
        "completion": arguments.completion.display().to_string(),
        "completionSha256": sha256_hex(&fs::read(&arguments.completion)?),
        "message": "Every exact terminal journal reverified at finalized and the aggregate Market account is closed."
    }))
}

fn completion_mutation_v1(mutation: &DurableTerminalMutationV1) -> TerminalCompletionMutationV1 {
    match mutation {
        DurableTerminalMutationV1::LookupCreate => TerminalCompletionMutationV1::LookupCreate,
        DurableTerminalMutationV1::LookupExtend { prefix_len } => {
            TerminalCompletionMutationV1::LookupExtend {
                prefix_len: prefix_len.to_string(),
            }
        }
        DurableTerminalMutationV1::LookupFreeze => TerminalCompletionMutationV1::LookupFreeze,
        DurableTerminalMutationV1::ResolutionReceiptPrepay => {
            TerminalCompletionMutationV1::ResolutionReceiptPrepay
        }
        DurableTerminalMutationV1::Protocol { stage } => match stage {
            TerminalStageV1::CoreBeginRetiring => TerminalCompletionMutationV1::CoreBeginRetiring,
            TerminalStageV1::DirectBeginRetiring => {
                TerminalCompletionMutationV1::DirectBeginRetiring
            }
            TerminalStageV1::ResolutionCloseFund => {
                TerminalCompletionMutationV1::ResolutionCloseFund
            }
            TerminalStageV1::DirectCloseCapability => {
                TerminalCompletionMutationV1::DirectCloseCapability
            }
            TerminalStageV1::RetirementReplayHandoff => {
                TerminalCompletionMutationV1::RetirementReplayHandoff
            }
            TerminalStageV1::AggregateRetirement => {
                TerminalCompletionMutationV1::AggregateRetirement
            }
        },
    }
}

fn canonical_completion_relative_path_v1(
    root: &Path,
    path: &Path,
    directory: bool,
) -> Result<String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        Error::new(format!(
            "terminal completion evidence {}: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(refusal(
            "terminal completion evidence path was not an ordinary file/directory of its exact kind",
        ));
    }
    let root = fs::canonicalize(root)
        .map_err(|error| Error::new(format!("terminal completion evidence root: {error}")))?;
    let path = fs::canonicalize(path)
        .map_err(|error| Error::new(format!("canonical terminal completion evidence: {error}")))?;
    let relative = path.strip_prefix(&root).map_err(|_| {
        refusal("terminal completion evidence escaped its completion evidence root")
    })?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(refusal(
            "terminal completion evidence path was not a nonempty canonical relative path",
        ));
    }
    let segments = relative
        .components()
        .map(|component| match component {
            std::path::Component::Normal(value) => value
                .to_str()
                .ok_or_else(|| refusal("terminal completion evidence path was not UTF-8")),
            _ => Err(refusal(
                "terminal completion evidence path was not canonical",
            )),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(segments.join("/"))
}

fn terminal_completion_expected_journals_v1(
    arguments: &TerminalSequenceArgumentsV1,
    session: &TerminalSequenceSessionV1,
    lookup_addresses: &[Pubkey],
) -> Result<Vec<(PathBuf, DurableTerminalMutationV1)>> {
    let mut expected = Vec::new();
    if !session.supplied_lookup_table {
        let plan = plan_terminal_lookup_table_v1(
            arguments.payer,
            session.lookup_recent_slot,
            lookup_addresses,
            session.funded_rent_rate,
        )?;
        expected.extend(lookup_journal_sequence_v1(&plan, &arguments.journal_dir));
    }
    if session.receipt_initial_lamports < session.receipt_rent_lamports {
        expected.push((
            arguments
                .journal_dir
                .join("12-resolution-receipt-prepay.json"),
            DurableTerminalMutationV1::ResolutionReceiptPrepay,
        ));
    }
    expected.extend(TerminalStageV1::ORDERED.into_iter().map(|stage| {
        (
            arguments.journal_dir.join(stage_journal_name_v1(stage)),
            DurableTerminalMutationV1::Protocol { stage },
        )
    }));
    Ok(expected)
}

fn build_terminal_sequence_completion_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    session: &TerminalSequenceSessionV1,
    lookup_table: Pubkey,
    lookup_addresses: &[Pubkey],
) -> Result<TerminalSequenceCompletionV1> {
    if !arguments.execute {
        return Err(refusal(
            "terminal completion must be finalized by an explicit --execute reconciliation run",
        ));
    }
    let evidence_root = arguments
        .completion
        .parent()
        .ok_or_else(|| refusal("terminal completion needs an evidence-root parent"))?;
    let session_source = fs::read(&arguments.session)?;
    let journal_directory =
        canonical_completion_relative_path_v1(evidence_root, &arguments.journal_dir, true)?;
    let session_path =
        canonical_completion_relative_path_v1(evidence_root, &arguments.session, false)?;
    let expected = terminal_completion_expected_journals_v1(arguments, session, lookup_addresses)?;
    let expected_paths = expected
        .iter()
        .map(|(path, _)| fs::canonicalize(path))
        .collect::<std::io::Result<BTreeSet<_>>>()?;
    let observed_paths = fs::read_dir(&arguments.journal_dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .map(|path| {
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(std::io::Error::other(
                    "terminal journal directory contained a non-ordinary entry",
                ));
            }
            fs::canonicalize(path)
        })
        .collect::<std::io::Result<BTreeSet<_>>>()?;
    if observed_paths != expected_paths {
        return Err(refusal(
            "terminal completion journal directory was not the exact canonical mutating set",
        ));
    }

    let mut journals = Vec::with_capacity(expected.len());
    let mut finalized_slot = 0u64;
    let mut total_fees = 0u64;
    let mut total_compute_units = 0u64;
    for (path, expected_mutation) in expected {
        let source = fs::read(&path)?;
        let journal = read_terminal_journal_v1(&path)?;
        if journal.phase != StageJournalPhaseV1::Finalized
            || journal.intent.mutation != expected_mutation
            || terminal_journal_cluster_v1(&journal)? != arguments.expected_cluster
        {
            return Err(refusal(
                "terminal completion journal was not the exact finalized typed predecessor",
            ));
        }
        verify_persisted_terminal_finalization_v1(rpc, &journal)?;
        let finalized = journal
            .finalized
            .as_ref()
            .ok_or_else(|| refusal("terminal completion journal omitted finalization"))?;
        let compute_units = finalized.compute_units_consumed.ok_or_else(|| {
            refusal("terminal completion requires exact compute units for every mutation")
        })?;
        if finalized.slot == 0 {
            return Err(refusal(
                "terminal completion mutation finalized at the sentinel zero slot",
            ));
        }
        finalized_slot = finalized_slot.max(finalized.slot);
        total_fees = total_fees
            .checked_add(journal.intent.transaction_fee_lamports)
            .ok_or_else(|| refusal("terminal completion fee total overflowed"))?;
        total_compute_units = total_compute_units
            .checked_add(compute_units)
            .ok_or_else(|| refusal("terminal completion compute total overflowed"))?;
        let protocol_lamport_deltas = journal
            .intent
            .protocol_lamport_deltas
            .iter()
            .map(|(address, delta)| TerminalCompletionLamportDeltaV1 {
                account_address: address.clone(),
                delta_lamports: delta.to_string(),
            })
            .collect();
        journals.push(TerminalCompletionJournalV1 {
            path: canonical_completion_relative_path_v1(evidence_root, &path, false)?,
            sha256: sha256_hex(&source),
            schema: terminal_journal_schema_v1(arguments.expected_cluster).into(),
            mutation: completion_mutation_v1(&journal.intent.mutation),
            phase: journal.phase,
            fee_payer: journal.intent.payer.clone(),
            signature: finalized.signature.clone(),
            finalized_slot: finalized.slot.to_string(),
            compute_units_consumed: compute_units.to_string(),
            transaction_fee_lamports: journal.intent.transaction_fee_lamports.to_string(),
            protocol_lamport_deltas,
        });
    }
    let genesis_hash = match arguments.expected_cluster {
        ExpectedClusterV1::Devnet => DEVNET_GENESIS_HASH.into(),
        ExpectedClusterV1::OwnedLoopback => session
            .owned_loopback_genesis_hash
            .clone()
            .ok_or_else(|| refusal("owned-loopback terminal session omitted genesis hash"))?,
    };
    Ok(TerminalSequenceCompletionV1 {
        schema: terminal_completion_schema_v1(arguments.expected_cluster).into(),
        status: "finalized".into(),
        cluster: match arguments.expected_cluster {
            ExpectedClusterV1::Devnet => "devnet",
            ExpectedClusterV1::OwnedLoopback => "owned-loopback",
        }
        .into(),
        genesis_hash,
        invocation: TerminalCompletionInvocationV1 {
            command: terminal_sequence_command_v1(arguments.expected_cluster).into(),
            rpc_url: arguments.origin.redacted_url(),
            plan_path: arguments.plan.display().to_string(),
            market_input_path: arguments.market_input.display().to_string(),
            evidence_path: arguments.evidence.display().to_string(),
            market: arguments.market.to_string(),
            fee_payer: arguments.payer.to_string(),
            fee_payer_keypair_path: arguments.payer_keypair.display().to_string(),
            session_path: arguments.session.display().to_string(),
            journal_directory: arguments.journal_dir.display().to_string(),
            completion_path: arguments.completion.display().to_string(),
            supplied_lookup_table: arguments
                .supplied_lookup_table
                .map(|value| value.to_string()),
            execute: true,
        },
        session: TerminalCompletionSessionV1 {
            path: session_path,
            sha256: sha256_hex(&session_source),
            schema: terminal_session_schema_v1(arguments.expected_cluster).into(),
            session_sha256: session.session_sha256.clone(),
        },
        journal_directory,
        market: arguments.market.to_string(),
        payer: arguments.payer.to_string(),
        lookup_table: lookup_table.to_string(),
        journals,
        finalized_slot: finalized_slot.to_string(),
        transaction_fees_lamports: total_fees.to_string(),
        compute_units_consumed: total_compute_units.to_string(),
    })
}

fn write_or_authenticate_terminal_completion_v1(
    path: &Path,
    completion: &TerminalSequenceCompletionV1,
) -> Result<()> {
    authenticate_terminal_completion_v1(completion)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(refusal(
                "terminal completion output was not an ordinary nonsymlink file",
            ));
        }
        let source = fs::read(path)?;
        let _: UniqueJsonV1 = serde_json::from_slice(&source).map_err(|error| {
            Error::new(format!(
                "terminal completion JSON contains a duplicate key or malformed value: {error}"
            ))
        })?;
        let existing: TerminalSequenceCompletionV1 = serde_json::from_slice(&source)?;
        authenticate_terminal_completion_v1(&existing)?;
        if &existing != completion {
            return Err(refusal(
                "persisted terminal completion differed from freshly reauthenticated final evidence",
            ));
        }
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| refusal("terminal completion needs a parent directory"))?;
    if !parent.is_dir() {
        return Err(refusal(
            "terminal completion parent must be an existing evidence directory",
        ));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| refusal("terminal completion needs a UTF-8 file name"))?;
    let lock = path.with_file_name(format!(".{name}.terminal-completion.lock"));
    let lock_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)?;
    lock_file.sync_all()?;
    if path.exists() {
        let _ = fs::remove_file(&lock);
        return Err(refusal(
            "terminal completion appeared concurrently; rerun and authenticate it",
        ));
    }
    let temporary = path.with_file_name(format!(
        ".{name}.terminal-completion-{}.tmp",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(completion)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    fs::hard_link(&temporary, path).map_err(|error| {
        Error::new(format!(
            "atomically publish terminal completion {} without clobber: {error}",
            path.display()
        ))
    })?;
    fs::remove_file(&temporary)?;
    OpenOptions::new()
        .read(true)
        .open(parent)
        .and_then(|directory| directory.sync_all())?;
    fs::remove_file(&lock)?;
    Ok(())
}

fn is_lower_hex_digest_v1(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_relative_evidence_path_v1(value: &str) -> bool {
    if value.is_empty() || !value.is_ascii() || value.contains('\\') {
        return false;
    }
    let segments = Path::new(value)
        .components()
        .map(|component| match component {
            std::path::Component::Normal(segment) => segment.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>();
    segments.is_some_and(|segments| segments.join("/") == value)
}

fn canonical_u64_v1(value: &str, allow_zero: bool) -> Result<u64> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| refusal("terminal completion quantity was not canonical decimal u64"))?;
    if parsed.to_string() != value || (!allow_zero && parsed == 0) {
        return Err(refusal(
            "terminal completion quantity was not canonical decimal u64",
        ));
    }
    Ok(parsed)
}

fn canonical_i128_v1(value: &str) -> Result<i128> {
    let parsed = value
        .parse::<i128>()
        .map_err(|_| refusal("terminal completion delta was not canonical decimal i128"))?;
    if parsed.to_string() != value {
        return Err(refusal(
            "terminal completion delta was not canonical decimal i128",
        ));
    }
    Ok(parsed)
}

fn authenticate_terminal_completion_v1(completion: &TerminalSequenceCompletionV1) -> Result<()> {
    let expected_cluster = match (completion.schema.as_str(), completion.cluster.as_str()) {
        (TERMINAL_COMPLETION_SCHEMA_V1, "devnet")
            if completion.genesis_hash == DEVNET_GENESIS_HASH =>
        {
            ExpectedClusterV1::Devnet
        }
        (OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA_V1, "owned-loopback") => {
            let genesis = Hash::from_str(&completion.genesis_hash)
                .map_err(|error| Error::new(format!("terminal completion genesis: {error}")))?;
            if genesis == Hash::default() || completion.genesis_hash == DEVNET_GENESIS_HASH {
                return Err(refusal(
                    "owned-loopback terminal completion used a public or sentinel genesis",
                ));
            }
            ExpectedClusterV1::OwnedLoopback
        }
        _ => {
            return Err(refusal(
                "terminal completion schema, cluster, and genesis were not one admitted tuple",
            ));
        }
    };
    if completion.status != "finalized"
        || completion.invocation.command != terminal_sequence_command_v1(expected_cluster)
        || completion.invocation.market != completion.market
        || completion.invocation.fee_payer != completion.payer
        || !completion.invocation.execute
        || !Path::new(&completion.invocation.plan_path).is_absolute()
        || !Path::new(&completion.invocation.market_input_path).is_absolute()
        || !Path::new(&completion.invocation.evidence_path).is_absolute()
        || !Path::new(&completion.invocation.fee_payer_keypair_path).is_absolute()
        || !Path::new(&completion.invocation.session_path).is_absolute()
        || !Path::new(&completion.invocation.journal_directory).is_absolute()
        || !Path::new(&completion.invocation.completion_path).is_absolute()
        || completion.invocation.rpc_url.is_empty()
        || completion.session.schema != terminal_session_schema_v1(expected_cluster)
        || !is_lower_hex_digest_v1(&completion.session.sha256)
        || !is_lower_hex_digest_v1(&completion.session.session_sha256)
        || !canonical_relative_evidence_path_v1(&completion.session.path)
        || !canonical_relative_evidence_path_v1(&completion.journal_directory)
        || Pubkey::from_str(&completion.market).is_err()
        || Pubkey::from_str(&completion.payer).is_err()
        || Pubkey::from_str(&completion.lookup_table).is_err()
        || completion
            .invocation
            .supplied_lookup_table
            .as_deref()
            .is_some_and(|value| value != completion.lookup_table)
    {
        return Err(refusal(
            "terminal completion invocation, owner references, or identities were noncanonical",
        ));
    }
    let journal_root = Path::new(&completion.journal_directory);
    let mut seen_paths = BTreeSet::new();
    let mut seen_signatures = BTreeSet::new();
    let mut max_slot = 0u64;
    let mut prior_slot = 0u64;
    let mut total_fees = 0u64;
    let mut total_compute = 0u64;
    for journal in &completion.journals {
        if journal.schema != terminal_journal_schema_v1(expected_cluster)
            || journal.phase != StageJournalPhaseV1::Finalized
            || journal.fee_payer != completion.payer
            || !canonical_relative_evidence_path_v1(&journal.path)
            || Path::new(&journal.path).strip_prefix(journal_root).is_err()
            || !is_lower_hex_digest_v1(&journal.sha256)
            || Pubkey::from_str(&journal.fee_payer).is_err()
            || Signature::from_str(&journal.signature).is_err()
            || !seen_paths.insert(journal.path.clone())
            || !seen_signatures.insert(journal.signature.clone())
        {
            return Err(refusal(
                "terminal completion journal identity, signature, path, or provenance changed",
            ));
        }
        let slot = canonical_u64_v1(&journal.finalized_slot, false)?;
        let compute = canonical_u64_v1(&journal.compute_units_consumed, false)?;
        let fee = canonical_u64_v1(&journal.transaction_fee_lamports, false)?;
        if slot < prior_slot {
            return Err(refusal(
                "terminal completion finalized slots regressed across predecessor order",
            ));
        }
        prior_slot = slot;
        max_slot = max_slot.max(slot);
        total_fees = total_fees
            .checked_add(fee)
            .ok_or_else(|| refusal("terminal completion fee total overflowed"))?;
        total_compute = total_compute
            .checked_add(compute)
            .ok_or_else(|| refusal("terminal completion compute total overflowed"))?;
        let mut prior = None;
        let mut delta_sum = 0i128;
        for delta in &journal.protocol_lamport_deltas {
            if Pubkey::from_str(&delta.account_address).is_err()
                || prior
                    .as_deref()
                    .is_some_and(|value| value >= delta.account_address.as_str())
            {
                return Err(refusal(
                    "terminal completion lamport deltas were not unique canonical address order",
                ));
            }
            let amount = canonical_i128_v1(&delta.delta_lamports)?;
            delta_sum = delta_sum
                .checked_add(amount)
                .ok_or_else(|| refusal("terminal completion delta total overflowed"))?;
            prior = Some(delta.account_address.clone());
        }
        if delta_sum != 0 {
            return Err(refusal(
                "terminal completion protocol deltas did not conserve before fee",
            ));
        }
    }

    let mut index = 0usize;
    if completion.invocation.supplied_lookup_table.is_none() {
        if completion.journals.get(index).map(|row| &row.mutation)
            != Some(&TerminalCompletionMutationV1::LookupCreate)
        {
            return Err(refusal(
                "terminal completion omitted its first lookup-table create mutation",
            ));
        }
        index += 1;
        let mut prior_prefix = 0u64;
        while let Some(TerminalCompletionMutationV1::LookupExtend { prefix_len }) =
            completion.journals.get(index).map(|row| &row.mutation)
        {
            let prefix = canonical_u64_v1(prefix_len, false)?;
            if prefix <= prior_prefix {
                return Err(refusal(
                    "terminal completion lookup extensions were not strict prefix order",
                ));
            }
            prior_prefix = prefix;
            index += 1;
        }
        if prior_prefix == 0
            || completion.journals.get(index).map(|row| &row.mutation)
                != Some(&TerminalCompletionMutationV1::LookupFreeze)
        {
            return Err(refusal(
                "terminal completion omitted its ordered lookup-table extension/freeze mutations",
            ));
        }
        index += 1;
    } else if completion.journals.iter().any(|row| {
        matches!(
            row.mutation,
            TerminalCompletionMutationV1::LookupCreate
                | TerminalCompletionMutationV1::LookupExtend { .. }
                | TerminalCompletionMutationV1::LookupFreeze
        )
    }) {
        return Err(refusal(
            "terminal completion created lookup infrastructure despite a supplied frozen table",
        ));
    }
    if completion.journals.get(index).map(|row| &row.mutation)
        == Some(&TerminalCompletionMutationV1::ResolutionReceiptPrepay)
    {
        index += 1;
    }
    // DERIVED from the one declaration, not retyped. This array was a second
    // copy of the stage order living four thousand lines from the first, and a
    // reorder that moved only the first would have made every honest completion
    // document unverifiable while reading like a tampering refusal.
    let protocol_order = TerminalStageV1::ORDERED
        .map(|stage| completion_mutation_v1(&DurableTerminalMutationV1::Protocol { stage }));
    if completion.journals.len() != index + protocol_order.len()
        || completion.journals[index..]
            .iter()
            .map(|row| &row.mutation)
            .ne(protocol_order.iter())
    {
        return Err(refusal(
            "terminal completion journal mutations were not the exact predecessor sequence",
        ));
    }
    if canonical_u64_v1(&completion.finalized_slot, false)? != max_slot
        || canonical_u64_v1(&completion.transaction_fees_lamports, false)? != total_fees
        || canonical_u64_v1(&completion.compute_units_consumed, false)? != total_compute
    {
        return Err(refusal(
            "terminal completion aggregate slot, fee, or compute arithmetic changed",
        ));
    }
    Ok(())
}

fn load_or_create_terminal_session_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
    input_digests: &(String, String, String, Option<String>),
) -> Result<TerminalSequenceSessionV1> {
    if arguments.session.exists() {
        amend_terminal_session_compute_budgets_v1(&arguments.session, &arguments.journal_dir)?;
        let session = read_terminal_session_v1(&arguments.session)?;
        authenticate_terminal_session_inputs_v1(&session, arguments, input_digests)?;
        return Ok(session);
    }
    let observed_genesis =
        authenticate_terminal_cluster_v1(rpc, &arguments.origin, arguments.expected_cluster)?;
    let (closures, source_receipt) = project_terminal_lookup_closures_from_chain_v1(
        rpc,
        plan,
        market_input,
        evidence,
        arguments.market,
        arguments.payer,
        None,
    )?;
    let lookup_addresses = terminal_lookup_union_from_closures_v1(arguments.payer, &closures)?;
    let snapshot = finalized_snapshot(rpc, &[source_receipt, arguments.market])?;
    let receipt = snapshot.account(source_receipt)?;
    let market_account = snapshot.account(arguments.market)?;
    // THE RATE THE ACCOUNTS THIS SEQUENCE TOUCHES WERE FUNDED AT, AND NOT THE
    // CLUSTER'S RATE OF THE MOMENT.
    //
    // This read the Rent sysvar, which answers what an account created NOW
    // would cost -- and every account a terminal sequence prices was created by
    // a founding that may long predate the reading. Devnet dropped its
    // rent-exempt rate from 6,333 to 5,080 lamports per byte at the epoch-1141
    // boundary with cohort-15 live on it, and all five of this sequence's rent
    // guards then refused accounts nobody had touched.
    //
    // So the rate is RECOVERED from accounts of the founding itself, through
    // `one_rate_prices_every_length` read backwards. The Market is the primary
    // reading: it holds exactly the rent-exempt minimum its founding paid for
    // its own width and parks no principal. A closure receipt already prepaid
    // by an earlier session is a second reading at a SECOND WIDTH -- prepaid
    // for the width it will become, not the zero bytes it currently occupies --
    // and the two must agree, which is `the_whole_cohort_is_one_rate`. The
    // division is exact or the recovery refuses; a donated lamport is never
    // rounded away.
    let mut readings = vec![FundedRentReadingV2 {
        account_bytes: market_account.data.len(),
        account_lamports: market_account.lamports,
        remaining_native_principal: 0,
    }];
    if receipt.lamports != 0 {
        readings.push(FundedRentReadingV2 {
            account_bytes: SOURCE_CLOSURE_RECEIPT_BYTES_V3,
            account_lamports: receipt.lamports,
            remaining_native_principal: 0,
        });
    }
    let funded_rent_rate = recover_funded_rent_rate_v2(&readings).map_err(|error| {
        refusal(&format!(
            "terminal session cannot recover one funded rent rate from this Market              ({} bytes holding {} lamports) and its {} prepaid closure receipt: {error:?}",
            market_account.data.len(),
            market_account.lamports,
            receipt.lamports,
        ))
    })?;
    let receipt_rent_lamports =
        funded_rent_minimum_v2(funded_rent_rate, SOURCE_CLOSURE_RECEIPT_BYTES_V3).map_err(
            |error| refusal(&format!("terminal session closure receipt rent: {error:?}")),
        )?;
    if receipt.owner != system_program::ID
        || receipt.executable
        || !receipt.data.is_empty()
        || receipt.lamports > receipt_rent_lamports
    {
        return Err(refusal(
            "new terminal session requires the canonical vacant at-most-rent Resolution receipt",
        ));
    }
    let (supplied_lookup_table, lookup_table, lookup_recent_slot) =
        match arguments.supplied_lookup_table {
            Some(table) => {
                let supplied = finalized_snapshot(rpc, &[table])?;
                authenticate_supplied_terminal_lookup_table_v1(
                    &lookup_addresses,
                    supplied.account(table)?,
                    funded_rent_rate,
                )?;
                (true, table, 0)
            }
            None => {
                let recent_slot = rpc.finalized_slot()?;
                let plan = plan_terminal_lookup_table_v1(
                    arguments.payer,
                    recent_slot,
                    &lookup_addresses,
                    funded_rent_rate,
                )?;
                (false, plan.lookup_table, recent_slot)
            }
        };
    let mut session = TerminalSequenceSessionV1 {
        schema: terminal_session_schema_v1(arguments.expected_cluster).into(),
        devnet_genesis_hash: (arguments.expected_cluster == ExpectedClusterV1::Devnet)
            .then(|| DEVNET_GENESIS_HASH.into()),
        owned_loopback_genesis_hash: (arguments.expected_cluster
            == ExpectedClusterV1::OwnedLoopback)
            .then_some(observed_genesis),
        rpc_url: arguments.origin.redacted_url(),
        plan_sha256: input_digests.0.clone(),
        market_input_sha256: input_digests.1.clone(),
        evidence_sha256: input_digests.2.clone(),
        refreshed_evidence_sha256: input_digests.3.clone(),
        market: arguments.market.to_string(),
        payer: arguments.payer.to_string(),
        source_receipt: source_receipt.to_string(),
        receipt_initial_lamports: receipt.lamports,
        receipt_rent_lamports,
        funded_rent_rate,
        declared_compute_unit_limits: declared_terminal_compute_budgets_v1(),
        supplied_lookup_table,
        lookup_table: lookup_table.to_string(),
        lookup_recent_slot,
        lookup_addresses: lookup_addresses.iter().map(ToString::to_string).collect(),
        lookup_addresses_sha256: pubkey_vector_sha256(&lookup_addresses),
        session_sha256: String::new(),
    };
    refresh_terminal_session_digest_v1(&mut session)?;
    write_new_terminal_session_v1(&arguments.session, &session)?;
    Ok(session)
}

fn authenticate_terminal_session_inputs_v1(
    session: &TerminalSequenceSessionV1,
    arguments: &TerminalSequenceArgumentsV1,
    input_digests: &(String, String, String, Option<String>),
) -> Result<()> {
    authenticate_terminal_session_v1(session)?;
    if terminal_session_cluster_v1(session)? != arguments.expected_cluster
        || session.rpc_url != arguments.origin.redacted_url()
        || session.plan_sha256 != input_digests.0
        || session.market_input_sha256 != input_digests.1
        || session.evidence_sha256 != input_digests.2
        // A resumable sequence must not change which refresh carried its rows
        // between invocations, exactly as it must not change the founding bytes.
        || session.refreshed_evidence_sha256 != input_digests.3
        || session.market != arguments.market.to_string()
        || session.payer != arguments.payer.to_string()
        || session.supplied_lookup_table != arguments.supplied_lookup_table.is_some()
        || arguments
            .supplied_lookup_table
            .is_some_and(|table| session.lookup_table != table.to_string())
    {
        return Err(refusal(
            "terminal session input digests, origin, Market, payer, or supplied ALT changed",
        ));
    }
    Ok(())
}

fn terminal_protocol_closure_from_journal_v1(
    journal: &DurableTerminalJournalV1,
    stage: TerminalStageV1,
    session: &TerminalSequenceSessionV1,
) -> Result<TerminalMetaClosureV1> {
    if journal.intent.mutation != (DurableTerminalMutationV1::Protocol { stage })
        || journal.intent.lookup_table.as_deref() != Some(session.lookup_table.as_str())
        || journal.intent.lookup_table_addresses != session.lookup_addresses
        || journal.intent.lookup_table_addresses_sha256 != session.lookup_addresses_sha256
    {
        return Err(refusal(
            "terminal protocol journal did not bind its exact stage and immutable session ALT",
        ));
    }
    let program_id = Pubkey::from_str(&journal.intent.program_id)
        .map_err(|error| Error::new(format!("terminal journal program: {error}")))?;
    let accounts = journal
        .intent
        .accounts
        .iter()
        .map(|account| {
            let pubkey = Pubkey::from_str(&account.address)
                .map_err(|error| Error::new(format!("terminal journal account: {error}")))?;
            Ok(AccountMeta {
                pubkey,
                is_signer: account.signer,
                is_writable: account.writable,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let closure = TerminalMetaClosureV1 {
        stage,
        program_id,
        program_class: journal.intent.program_class,
        accounts,
        classes: journal
            .intent
            .accounts
            .iter()
            .map(|account| account.class)
            .collect(),
    };
    let instruction = Instruction {
        program_id,
        accounts: closure.accounts.clone(),
        data: BASE64
            .decode(&journal.intent.instruction_data_base64)
            .map_err(|error| Error::new(format!("terminal journal instruction: {error}")))?,
    };
    closure.authenticate_instruction(&instruction)?;
    Ok(closure)
}

fn authenticate_source_receipt_journal_v1(
    journal: &DurableTerminalJournalV1,
    session: &TerminalSequenceSessionV1,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    market: Pubkey,
) -> Result<Pubkey> {
    if journal.intent.mutation
        != (DurableTerminalMutationV1::Protocol {
            stage: TerminalStageV1::ResolutionCloseFund,
        })
    {
        return Err(refusal(
            "Resolution receipt evidence came from another terminal mutation",
        ));
    }
    let receipt = Pubkey::from_str(&session.source_receipt)
        .map_err(|error| Error::new(format!("terminal session Source receipt: {error}")))?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let expected = journal
        .intent
        .expected_accounts
        .get(&receipt.to_string())
        .ok_or_else(|| {
            refusal("Resolution CloseFund journal omitted its exact receipt poststate")
        })?;
    let data = BASE64
        .decode(&expected.data_base64)
        .map_err(|error| Error::new(format!("Resolution receipt journal base64: {error}")))?;
    let decoded = SourceClosureReceiptV3::decode(&data)
        .map_err(|error| Error::new(format!("Resolution receipt journal: {error:?}")))?;
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &retired_market_generation_v1(market_input.generation)?.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    if expected.address != receipt.to_string()
        || expected.owner != resolution.to_string()
        || expected.executable
        || expected.lookup_table.is_some()
        || expected.lamports_after_protocol != session.receipt_rent_lamports
        || expected.lamports_after_fee != session.receipt_rent_lamports
        || expected.data_sha256 != sha256_hex(&data)
        || data.len() != SOURCE_CLOSURE_RECEIPT_BYTES_V3
        || Pubkey::new_from_array(decoded.receipt_account) != receipt
        || Pubkey::new_from_array(decoded.market) != market
        || Pubkey::new_from_array(decoded.source_state) != source_state
        || decoded.generation != retired_market_generation_v1(market_input.generation)?
    {
        return Err(refusal(
            "Resolution CloseFund journal carried a substituted receipt identity, bytes, owner, rent, or generation",
        ));
    }
    Ok(receipt)
}

/// Whether a receipt-prepay journal can account for the balance a session
/// recorded as its receipt's starting lamports.
///
/// Two shapes are admissible and one fact tells them apart -- whether the
/// session started BELOW its own rent figure:
///
/// - the session that wrote the prepay started below, and the journal is the
///   action it took, in any phase, because a planned entry is exactly the
///   durability record that stops it being signed twice; and
/// - a SUCCESSOR session started at or above, having observed the poststate of
///   a prepay that had already landed -- and nothing but a FINALIZED journal is
///   an account of that. A planned or submitted prepay puts no lamports on a
///   seat, so it cannot explain a seat that has them.
fn receipt_prepay_journal_accounts_for_session_v1(
    session_initial_lamports: u64,
    session_rent_lamports: u64,
    journal_finalized: bool,
) -> Result<()> {
    if session_initial_lamports >= session_rent_lamports && !journal_finalized {
        return Err(refusal(&format!(
            "a receipt-prepay journal that has not finalized cannot account for a session that \
             began exactly funded ({session_initial_lamports} lamports against its own rent \
             figure of {session_rent_lamports})"
        )));
    }
    Ok(())
}

fn authenticate_terminal_receipt_funding_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    session: &TerminalSequenceSessionV1,
    stage_three: Option<&DurableTerminalJournalV1>,
) -> Result<()> {
    let receipt = Pubkey::from_str(&session.source_receipt)
        .map_err(|error| Error::new(format!("terminal session Source receipt: {error}")))?;
    // THE OWED ANSWER, GIVEN. This used to rederive the receipt's rent from the
    // Rent sysvar of the moment and refuse if it had moved -- which is how a
    // session that had correctly prepaid a seat at 6,333 lamports per byte
    // refused itself the instant devnet dropped to 5,080 at the epoch-1141
    // boundary (slot 492,912,000, 2026-09-04 07:31:40 UTC). The rent an account
    // was funded at is a fact fixed when it was funded. The session records the
    // rate it paid; the check is that the session's own two numbers agree, and
    // it is still EXACT -- a seat carrying one lamport nobody can account for
    // still refuses, at the arithmetic below and on chain.
    let recorded =
        funded_rent_minimum_v2(session.funded_rent_rate, SOURCE_CLOSURE_RECEIPT_BYTES_V3)
            .map_err(|error| refusal(&format!("terminal session funded rent rate: {error:?}")))?;
    if recorded != session.receipt_rent_lamports {
        return Err(refusal(&format!(
            "terminal session receipt rent {} is not what its own recorded rate of {} lamports \
             per byte prices {SOURCE_CLOSURE_RECEIPT_BYTES_V3} bytes at ({recorded})",
            session.receipt_rent_lamports, session.funded_rent_rate
        )));
    }
    let prepay_path = arguments
        .journal_dir
        .join("12-resolution-receipt-prepay.json");
    if prepay_path.exists() {
        let journal = read_terminal_journal_v1(&prepay_path)?;
        if journal.intent.mutation != DurableTerminalMutationV1::ResolutionReceiptPrepay {
            return Err(refusal("receipt-prepay path carried another mutation"));
        }
        // A JOURNAL OUTLIVES THE SESSION THAT WROTE IT, AND MAY EXPLAIN THE
        // BALANCE A SUCCESSOR SESSION STARTS FROM.
        //
        // This used to refuse the pair outright when the session began exactly
        // funded, on the reading that a prepay journal contradicts a receipt
        // that needed no prepay. It does not contradict it when the journal is
        // the FINALIZED prepay that put the lamports there: the journal is the
        // durability record of the sequence, not a field of one session, and a
        // session opened after its predecessor's prepay landed observes exactly
        // that prepay's poststate as its own starting balance. Cohort-15 met
        // this the moment a session had to be reopened at all -- market 1's
        // seat was prepaid at 03:42 UTC by a session the funded-rate schema
        // then superseded.
        //
        // So the pair is admitted only when the journal ACCOUNTS for the
        // balance, which is a stricter question than the one it replaces:
        // an unfinalized prepay explains nothing and is still refused, and a
        // finalized one must land on exactly the lamports the session recorded.
        receipt_prepay_journal_accounts_for_session_v1(
            session.receipt_initial_lamports,
            session.receipt_rent_lamports,
            journal.phase == StageJournalPhaseV1::Finalized,
        )?;
        let inherited = session.receipt_initial_lamports >= session.receipt_rent_lamports;
        let key = receipt.to_string();
        let before = journal
            .intent
            .prestate
            .get(&key)
            .ok_or_else(|| refusal("receipt-prepay journal omitted receipt prestate"))?;
        let after = journal
            .intent
            .expected_accounts
            .get(&key)
            .ok_or_else(|| refusal("receipt-prepay journal omitted receipt poststate"))?;
        // The prestate this journal names is the session's own starting balance
        // when the session predates it, and a strictly smaller balance when the
        // session inherited its result. Both are exact; neither is a range.
        let prestate_accounted = if inherited {
            before.lamports < session.receipt_rent_lamports
        } else {
            before.lamports == session.receipt_initial_lamports
        };
        if before.owner != system_program::ID.to_string()
            || before.executable
            || before.data_len != 0
            || before.data_sha256 != sha256_hex(&[])
            || !prestate_accounted
            || after.owner != system_program::ID.to_string()
            || after.executable
            || !after.data_base64.is_empty()
            || after.data_sha256 != sha256_hex(&[])
            || after.lamports_after_protocol != session.receipt_rent_lamports
            || after.lamports_after_fee != session.receipt_rent_lamports
        {
            return Err(refusal(
                "receipt-prepay journal did not reproduce the session's exact vacant receipt and rent arithmetic",
            ));
        }
        if journal.phase == StageJournalPhaseV1::Finalized {
            verify_persisted_terminal_finalization_v1(rpc, &journal)?;
        }
        return Ok(());
    }
    if session.receipt_initial_lamports < session.receipt_rent_lamports
        && stage_three.is_some_and(|journal| journal.phase != StageJournalPhaseV1::Planned)
    {
        return Err(refusal(
            "Resolution CloseFund was signed without the required durable receipt-prepay journal",
        ));
    }
    if let Some(journal) =
        stage_three.filter(|journal| journal.phase != StageJournalPhaseV1::Planned)
    {
        let before = journal
            .intent
            .prestate
            .get(&receipt.to_string())
            .ok_or_else(|| refusal("Resolution CloseFund journal omitted receipt prestate"))?;
        if before.owner != system_program::ID.to_string()
            || before.executable
            || before.data_len != 0
            || before.data_sha256 != sha256_hex(&[])
            || before.lamports != session.receipt_initial_lamports
            || session.receipt_initial_lamports != session.receipt_rent_lamports
        {
            return Err(refusal(
                "Resolution CloseFund journal did not reproduce the exactly funded initial receipt",
            ));
        }
        return Ok(());
    }
    let receipt_snapshot = finalized_snapshot(rpc, &[receipt])?;
    let account = receipt_snapshot.account(receipt)?;
    if account.owner != system_program::ID
        || account.executable
        || !account.data.is_empty()
        || account.lamports != session.receipt_initial_lamports
    {
        return Err(refusal(
            "vacant Resolution receipt changed without its durable prepay or CloseFund journal",
        ));
    }
    Ok(())
}

fn authenticate_terminal_session_semantics_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    session: &TerminalSequenceSessionV1,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
) -> Result<Vec<Pubkey>> {
    // `ResolutionCloseFund` is stage FOUR under the ruled order; the local used
    // to be called `stage_three` and the number is not what identifies it.
    let close_fund_path = arguments
        .journal_dir
        .join(stage_journal_name_v1(TerminalStageV1::ResolutionCloseFund));
    let close_fund = close_fund_path
        .exists()
        .then(|| read_terminal_journal_v1(&close_fund_path))
        .transpose()?;
    authenticate_terminal_receipt_funding_v1(rpc, arguments, session, close_fund.as_ref())?;
    let pinned_receipt = close_fund
        .as_ref()
        .filter(|journal| journal.phase != StageJournalPhaseV1::Planned)
        .map(|journal| {
            authenticate_source_receipt_journal_v1(
                journal,
                session,
                plan,
                market_input,
                arguments.market,
            )
        })
        .transpose()?;

    let mut finalized_closures = Vec::with_capacity(TerminalStageV1::ORDERED.len());
    let mut all_finalized = true;
    for stage in TerminalStageV1::ORDERED {
        let path = arguments.journal_dir.join(stage_journal_name_v1(stage));
        if !path.exists() {
            all_finalized = false;
            break;
        }
        let journal = read_terminal_journal_v1(&path)?;
        if journal.phase != StageJournalPhaseV1::Finalized {
            all_finalized = false;
            break;
        }
        verify_persisted_terminal_finalization_v1(rpc, &journal)?;
        finalized_closures.push(terminal_protocol_closure_from_journal_v1(
            &journal, stage, session,
        )?);
    }
    let (closures, source_receipt) = if all_finalized {
        let receipt = pinned_receipt.ok_or_else(|| {
            refusal("finalized terminal sequence omitted finalized Resolution receipt evidence")
        })?;
        (finalized_closures, receipt)
    } else {
        project_terminal_lookup_closures_from_chain_v1(
            rpc,
            plan,
            market_input,
            evidence,
            arguments.market,
            arguments.payer,
            pinned_receipt,
        )?
    };
    let union = terminal_lookup_union_from_closures_v1(arguments.payer, &closures)?;
    authenticate_terminal_session_union_v1(session, source_receipt, &union)?;
    Ok(union)
}

fn authenticate_terminal_session_union_v1(
    session: &TerminalSequenceSessionV1,
    source_receipt: Pubkey,
    union: &[Pubkey],
) -> Result<()> {
    let expected_receipt = Pubkey::from_str(&session.source_receipt)
        .map_err(|error| Error::new(format!("terminal session Source receipt: {error}")))?;
    let durable = session
        .lookup_addresses
        .iter()
        .map(|value| {
            Pubkey::from_str(value)
                .map_err(|error| Error::new(format!("terminal session ALT address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if source_receipt != expected_receipt
        || union != durable.as_slice()
        || pubkey_vector_sha256(union) != session.lookup_addresses_sha256
    {
        return Err(refusal(
            "terminal session receipt or ALT union did not rederive from protocol semantic owners",
        ));
    }
    Ok(())
}

fn refresh_terminal_session_digest_v1(session: &mut TerminalSequenceSessionV1) -> Result<()> {
    session.session_sha256.clear();
    session.session_sha256 = sha256_hex(&serde_json::to_vec(session)?);
    Ok(())
}

fn authenticate_terminal_session_v1(session: &TerminalSequenceSessionV1) -> Result<()> {
    terminal_session_cluster_v1(session)?;
    let mut material = session.clone();
    let digest = material.session_sha256.clone();
    material.session_sha256.clear();
    let addresses = session
        .lookup_addresses
        .iter()
        .map(|value| {
            Pubkey::from_str(value)
                .map_err(|error| Error::new(format!("terminal session ALT address: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    if session.rpc_url.is_empty()
        || digest != sha256_hex(&serde_json::to_vec(&material)?)
        || addresses != canonical_union_addresses(&addresses)?
        || session.lookup_addresses_sha256 != pubkey_vector_sha256(&addresses)
        || Pubkey::from_str(&session.lookup_table).is_err()
        || Pubkey::from_str(&session.source_receipt).is_err()
        || session.receipt_initial_lamports > session.receipt_rent_lamports
        || (!session.supplied_lookup_table && session.lookup_recent_slot == 0)
    {
        return Err(refusal(
            "terminal session schema, digest, exact devnet, ALT union, receipt, or lookup identity changed",
        ));
    }
    if session.declared_compute_unit_limits != declared_terminal_compute_budgets_v1() {
        return Err(refusal(&format!(
            "terminal session declares ComputeBudget limits {:?} and this driver declares {:?}; \
             a sequence may not change the budget its later stages compile against after its \
             earlier stages were signed",
            session.declared_compute_unit_limits,
            declared_terminal_compute_budgets_v1()
        )));
    }
    Ok(())
}

/// Whether a persisted table may be amended to `declared` by ADDING rows only.
///
/// Every stage the session already declares must keep the exact number it
/// declares -- a re-pin under a live sequence is the drift the guard exists for
/// -- and the difference must be additions alone. Whether those additions are
/// legal for THIS sequence is a second question, about journals, answered by
/// the caller.
fn terminal_compute_budget_additions_v1(
    persisted: &[TerminalStageComputeBudgetV1],
    declared: &[TerminalStageComputeBudgetV1],
) -> Option<Vec<TerminalStageV1>> {
    let mut additions = Vec::new();
    for row in declared {
        match persisted.iter().find(|held| held.stage == row.stage) {
            Some(held) if held.compute_unit_limit == row.compute_unit_limit => {}
            Some(_) => return None,
            None => additions.push(row.stage),
        }
    }
    if persisted
        .iter()
        .any(|held| !declared.iter().any(|row| row.stage == held.stage))
    {
        return None;
    }
    (!additions.is_empty()).then_some(additions)
}

/// Amend a live session that predates a stage's MEASURED ComputeBudget row.
///
/// `authenticate_terminal_session_v1` holds the session's table and the
/// driver's exactly equal, so a sequence cannot change the budget a signed
/// stage compiled against. A stage that has never been planned compiled
/// nothing: its canonical journal path does not exist, and a packet that was
/// planned and then retired as unlandable moved aside under its own signature,
/// which is what frees that path. So a table that only GAINS such a stage is
/// not a change to any commitment this sequence has made, and refusing it would
/// force a market already at Retiring -- with a frozen ALT and finalized stages
/// on chain -- to abandon both over a number no signed packet has ever read.
///
/// Cohort-17's market 2 is why this exists: `DirectCloseCapability` met the
/// 200,000-CU default meter on the first market ever to reach that stage, and
/// its budget could not be declared without this.
///
/// Everything else stays refused by the guard, which runs immediately after:
/// a changed value, a removed row, or an addition for a stage whose canonical
/// journal already exists in any phase.
fn amend_terminal_session_compute_budgets_v1(
    session_path: &Path,
    journal_dir: &Path,
) -> Result<()> {
    let source = fs::read(session_path)?;
    let Ok(mut session) = serde_json::from_slice::<TerminalSequenceSessionV1>(&source) else {
        return Ok(());
    };
    let declared = declared_terminal_compute_budgets_v1();
    let Some(additions) =
        terminal_compute_budget_additions_v1(&session.declared_compute_unit_limits, &declared)
    else {
        return Ok(());
    };
    for stage in &additions {
        if journal_dir.join(stage_journal_name_v1(*stage)).exists() {
            return Ok(());
        }
    }
    session.declared_compute_unit_limits = declared;
    refresh_terminal_session_digest_v1(&mut session)?;
    let temporary = session_path.with_file_name(format!(
        ".{}.terminal-session-amend-{}.tmp",
        session_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| refusal("terminal session needs a UTF-8 file name"))?,
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(&session)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, session_path)?;
    println!(
        "amended terminal session ComputeBudget table with unplanned stages {additions:?}; \
         every stage it already declared kept its exact number"
    );
    Ok(())
}

fn read_terminal_session_v1(path: &Path) -> Result<TerminalSequenceSessionV1> {
    if !path.is_absolute() {
        return Err(refusal("terminal session path must be absolute"));
    }
    let source = fs::read(path)?;
    let _: UniqueJsonV1 = serde_json::from_slice(&source).map_err(|error| {
        Error::new(format!(
            "terminal session JSON contains a duplicate key or malformed value: {error}"
        ))
    })?;
    let session: TerminalSequenceSessionV1 = serde_json::from_slice(&source)?;
    authenticate_terminal_session_v1(&session)?;
    Ok(session)
}

fn write_new_terminal_session_v1(path: &Path, session: &TerminalSequenceSessionV1) -> Result<()> {
    if !path.is_absolute() {
        return Err(refusal("--session must be absolute"));
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| refusal("terminal session needs a UTF-8 file name"))?;
    let lock = path.with_file_name(format!(".{name}.terminal-session.lock"));
    let lock_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|error| {
            Error::new(format!(
                "REFUSED: acquire exclusive terminal session lock {}: {error}",
                lock.display()
            ))
        })?;
    lock_file.sync_all()?;
    if path.exists() {
        let _ = fs::remove_file(&lock);
        return Err(refusal(
            "terminal session appeared concurrently; rerun and authenticate it",
        ));
    }
    let temporary = path.with_file_name(format!(
        ".{name}.terminal-session-{}.tmp",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(session)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    fs::hard_link(&temporary, path).map_err(|error| {
        Error::new(format!(
            "atomically publish terminal session {} without clobber: {error}",
            path.display()
        ))
    })?;
    fs::remove_file(&temporary)?;
    if let Some(parent) = path.parent() {
        OpenOptions::new()
            .read(true)
            .open(parent)
            .and_then(|directory| directory.sync_all())?;
    }
    fs::remove_file(&lock)?;
    Ok(())
}

fn operate_terminal_lookup_preflight_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    session: &TerminalSequenceSessionV1,
    lookup_table: Pubkey,
    lookup_addresses: &[Pubkey],
) -> Result<bool> {
    if session.supplied_lookup_table {
        let snapshot = finalized_snapshot(rpc, &[lookup_table])?;
        authenticate_supplied_terminal_lookup_table_v1(
            lookup_addresses,
            snapshot.account(lookup_table)?,
            session.funded_rent_rate,
        )?;
        return Ok(true);
    }
    let plan = plan_terminal_lookup_table_v1(
        arguments.payer,
        session.lookup_recent_slot,
        lookup_addresses,
        session.funded_rent_rate,
    )?;
    if plan.lookup_table != lookup_table || plan.addresses != lookup_addresses {
        return Err(refusal(
            "terminal session ALT plan no longer reproduced its exact table or union",
        ));
    }
    let expected = lookup_journal_sequence_v1(&plan, &arguments.journal_dir);
    let mut gap = false;
    for (path, mutation) in &expected {
        if !path.exists() {
            gap = true;
            continue;
        }
        if gap {
            return Err(refusal(
                "terminal ALT journals contained a later action after a missing prefix",
            ));
        }
        let mut journal = read_terminal_journal_v1(path)?;
        if journal.intent.mutation != *mutation {
            return Err(refusal(
                "terminal ALT journal path carried another infrastructure mutation",
            ));
        }
        if journal.phase == StageJournalPhaseV1::Finalized {
            resume_terminal_journal_v1(
                rpc,
                &arguments.origin,
                arguments.expected_cluster,
                path,
                &arguments.payer_keypair,
                false,
                &mut journal,
                None,
            )?;
            continue;
        }
        let authorization = if journal.phase == StageJournalPhaseV1::Planned {
            let route = current_lookup_route_v1(rpc, &plan, session.funded_rent_rate)?;
            if lookup_route_mutation_v1(&route) != Some(mutation.clone()) {
                return Err(refusal(
                    "current ALT route differed from the unresolved Planned journal",
                ));
            }
            let (payer, table, _observed_rent, prestate) =
                lookup_execution_snapshot_v1(rpc, &plan, &route, session.funded_rent_rate)?;
            Some(authenticate_lookup_infrastructure_planned_journal_v1(
                &journal,
                &plan,
                &route,
                &payer,
                &table,
                session.funded_rent_rate,
                &prestate,
            )?)
        } else {
            None
        };
        resume_terminal_journal_v1(
            rpc,
            &arguments.origin,
            arguments.expected_cluster,
            path,
            &arguments.payer_keypair,
            arguments.execute,
            &mut journal,
            authorization.as_ref(),
        )?;
        terminal_stdout_v1(json!({
            "status": "lookup-action",
            "journal": path.display().to_string(),
            "phase": journal.phase,
            "lookupTable": lookup_table.to_string()
        }))?;
        return Ok(false);
    }
    let route = current_lookup_route_v1(rpc, &plan, session.funded_rent_rate)?;
    if route == TerminalLookupTableRouteV1::Complete {
        let snapshot = finalized_snapshot(rpc, &[lookup_table])?;
        authenticate_supplied_terminal_lookup_table_v1(
            lookup_addresses,
            snapshot.account(lookup_table)?,
            session.funded_rent_rate,
        )?;
        return Ok(true);
    }
    let mutation = lookup_route_mutation_v1(&route)
        .ok_or_else(|| refusal("terminal ALT route omitted its infrastructure mutation"))?;
    let path = expected
        .iter()
        .find_map(|(path, expected)| (*expected == mutation).then_some(path.clone()))
        .ok_or_else(|| refusal("terminal ALT route was outside its canonical journal sequence"))?;
    if path.exists() {
        return Err(refusal(
            "terminal ALT next journal appeared after the authenticated prefix scan",
        ));
    }
    let (payer, table, observed_rent, prestate) =
        lookup_execution_snapshot_v1(rpc, &plan, &route, session.funded_rent_rate)?;
    let mut journal = build_lookup_infrastructure_journal_v1(
        rpc,
        &arguments.origin,
        arguments.expected_cluster,
        &plan,
        &route,
        &payer,
        &table,
        &observed_rent,
        &prestate,
        arguments.execute,
    )?;
    write_terminal_journal_v1(&path, &mut journal, true)?;
    let authorization = authenticate_lookup_infrastructure_planned_journal_v1(
        &journal,
        &plan,
        &route,
        &payer,
        &table,
        session.funded_rent_rate,
        &prestate,
    )?;
    resume_terminal_journal_v1(
        rpc,
        &arguments.origin,
        arguments.expected_cluster,
        &path,
        &arguments.payer_keypair,
        arguments.execute,
        &mut journal,
        Some(&authorization),
    )?;
    terminal_stdout_v1(json!({
        "status": "lookup-action",
        "journal": path.display().to_string(),
        "phase": journal.phase,
        "lookupTable": lookup_table.to_string(),
        "addresses": lookup_addresses.len(),
        "maximumWireBytes": plan.maximum_preflight_wire_bytes,
        "finalRentLamports": plan.final_rent_lamports
    }))?;
    Ok(false)
}

fn lookup_journal_sequence_v1(
    plan: &TerminalLookupTablePlanV1,
    directory: &Path,
) -> Vec<(PathBuf, DurableTerminalMutationV1)> {
    let mut sequence = vec![(
        directory.join("00-alt-create.json"),
        DurableTerminalMutationV1::LookupCreate,
    )];
    sequence.extend(
        extension_prefix_lengths(plan)
            .into_iter()
            .map(|prefix_len| {
                (
                    directory.join(format!("01-alt-extend-{prefix_len:03}.json")),
                    DurableTerminalMutationV1::LookupExtend { prefix_len },
                )
            }),
    );
    sequence.push((
        directory.join("02-alt-freeze.json"),
        DurableTerminalMutationV1::LookupFreeze,
    ));
    sequence
}

fn lookup_route_mutation_v1(
    route: &TerminalLookupTableRouteV1,
) -> Option<DurableTerminalMutationV1> {
    match route {
        TerminalLookupTableRouteV1::Create(_) => Some(DurableTerminalMutationV1::LookupCreate),
        TerminalLookupTableRouteV1::Extend { prefix_len, .. } => {
            Some(DurableTerminalMutationV1::LookupExtend {
                prefix_len: *prefix_len,
            })
        }
        TerminalLookupTableRouteV1::Freeze(_) => Some(DurableTerminalMutationV1::LookupFreeze),
        TerminalLookupTableRouteV1::Complete => None,
    }
}

fn current_lookup_route_v1(
    rpc: &mut Rpc,
    plan: &TerminalLookupTablePlanV1,
    funded_rent_rate: u32,
) -> Result<TerminalLookupTableRouteV1> {
    let snapshot = finalized_snapshot(rpc, &[plan.lookup_table])?;
    route_terminal_lookup_table_v1(
        plan,
        Some(snapshot.account(plan.lookup_table)?),
        funded_rent_rate,
    )
}

fn lookup_execution_snapshot_v1(
    rpc: &mut Rpc,
    plan: &TerminalLookupTablePlanV1,
    expected_route: &TerminalLookupTableRouteV1,
    funded_rent_rate: u32,
) -> Result<(ObservedAccount, ObservedAccount, Rent, Vec<ObservedAccount>)> {
    let instruction = match expected_route {
        TerminalLookupTableRouteV1::Create(instruction)
        | TerminalLookupTableRouteV1::Freeze(instruction) => instruction,
        TerminalLookupTableRouteV1::Extend { instruction, .. } => instruction,
        TerminalLookupTableRouteV1::Complete => {
            return Err(refusal("complete terminal ALT has no execution snapshot"));
        }
    };
    let mut keys = vec![
        plan.payer,
        plan.lookup_table,
        instruction.program_id,
        sysvar::rent::ID,
    ];
    keys.extend(instruction.accounts.iter().map(|meta| meta.pubkey));
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let payer = snapshot.account(plan.payer)?.clone();
    let table = snapshot.account(plan.lookup_table)?.clone();
    let rent: Rent = bincode::deserialize(&snapshot.account(sysvar::rent::ID)?.data)
        .map_err(|error| Error::new(format!("terminal ALT Rent sysvar: {error}")))?;
    let rerouted = route_terminal_lookup_table_v1(plan, Some(&table), funded_rent_rate)?;
    if &rerouted != expected_route {
        return Err(refusal(
            "terminal ALT route changed while acquiring its complete execution snapshot",
        ));
    }
    let mut prestate = snapshot.accounts.values().cloned().collect::<Vec<_>>();
    prestate.sort_unstable_by_key(|account| account.key);
    Ok((payer, table, rent, prestate))
}

#[allow(clippy::too_many_arguments)]
fn operate_terminal_protocol_journals_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    session: &TerminalSequenceSessionV1,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &CampaignTerminalEvidenceV1,
    lookup_table: Pubkey,
    lookup_addresses: &[Pubkey],
    source_receipt: Pubkey,
) -> Result<bool> {
    let prepay_required = session.receipt_initial_lamports < session.receipt_rent_lamports;
    let prepay_path = arguments
        .journal_dir
        .join("12-resolution-receipt-prepay.json");
    // The same pairing `authenticate_terminal_receipt_funding_v1` has already
    // ruled on, one layer down and for the ORDER of the journals rather than
    // their arithmetic: a session that began exactly funded may be resuming the
    // journal of the prepay that funded it. That journal is still part of the
    // durable prefix -- a later action after a missing earlier one must go on
    // refusing -- so it is counted in, not skipped.
    let prepay_inherited = !prepay_required && prepay_path.exists();
    authenticate_terminal_journal_prefix_v1(
        &arguments.journal_dir,
        prepay_required || prepay_inherited,
    )?;
    if prepay_inherited
        && read_terminal_journal_v1(&prepay_path)?.phase != StageJournalPhaseV1::Finalized
    {
        return Err(refusal(
            "terminal session began with an exactly funded receipt beside a prepay journal that              never finalized, so nothing accounts for the lamports on the seat",
        ));
    }
    for stage in TerminalStageV1::ORDERED {
        if stage == TerminalStageV1::ResolutionCloseFund
            && prepay_required
            && !operate_resolution_prepay_journal_v1(
                rpc,
                arguments,
                plan,
                evidence,
                lookup_table,
                lookup_addresses,
                session,
                &prepay_path,
            )?
        {
            return Ok(false);
        }
        let path = arguments.journal_dir.join(stage_journal_name_v1(stage));
        if path.exists() {
            let mut journal = read_terminal_journal_v1(&path)?;
            if journal.intent.mutation != (DurableTerminalMutationV1::Protocol { stage }) {
                return Err(refusal(
                    "terminal protocol journal path carried another stage mutation",
                ));
            }
            if journal.phase == StageJournalPhaseV1::Finalized {
                resume_terminal_journal_v1(
                    rpc,
                    &arguments.origin,
                    arguments.expected_cluster,
                    &path,
                    &arguments.payer_keypair,
                    false,
                    &mut journal,
                    None,
                )?;
                continue;
            }
            let authorization = if journal.phase == StageJournalPhaseV1::Planned {
                let fresh = fresh_protocol_stage_from_chain_v1(
                    rpc,
                    plan,
                    market_input,
                    evidence,
                    arguments.market,
                    arguments.payer,
                    lookup_table,
                    source_receipt,
                    stage,
                    session.funded_rent_rate,
                    session_stage_compute_unit_limit_v1(session, stage),
                )?;
                Some(authenticate_chain_derived_planned_journal_v1(
                    &journal, &fresh,
                )?)
            } else {
                None
            };
            resume_terminal_journal_v1(
                rpc,
                &arguments.origin,
                arguments.expected_cluster,
                &path,
                &arguments.payer_keypair,
                arguments.execute,
                &mut journal,
                authorization.as_ref(),
            )?;
            terminal_stdout_v1(json!({
                "status": "protocol-stage",
                "stage": stage,
                "journal": path.display().to_string(),
                "phase": journal.phase
            }))?;
            return Ok(false);
        }
        let later = TerminalStageV1::ORDERED
            .iter()
            .copied()
            .filter(|candidate| candidate.ordinal() > stage.ordinal())
            .map(|candidate| arguments.journal_dir.join(stage_journal_name_v1(candidate)))
            .find(|candidate| candidate.exists());
        if later.is_some() {
            return Err(refusal(
                "terminal protocol journals contained a later stage after a missing prefix",
            ));
        }
        let fresh = fresh_protocol_stage_from_chain_v1(
            rpc,
            plan,
            market_input,
            evidence,
            arguments.market,
            arguments.payer,
            lookup_table,
            source_receipt,
            stage,
            session.funded_rent_rate,
            session_stage_compute_unit_limit_v1(session, stage),
        )?;
        let table = fresh
            .prestate
            .iter()
            .find(|account| account.key == lookup_table)
            .ok_or_else(|| refusal("fresh terminal stage omitted frozen ALT prestate"))?;
        authenticate_supplied_terminal_lookup_table_v1(
            lookup_addresses,
            table,
            session.funded_rent_rate,
        )?;
        let mut journal = build_protocol_stage_journal_v1(
            rpc,
            &arguments.origin,
            arguments.expected_cluster,
            arguments.payer,
            &fresh.mutation,
            &fresh.closure,
            table,
            lookup_addresses,
            session.funded_rent_rate,
            session_stage_compute_unit_limit_v1(session, stage),
            &fresh.prestate,
            arguments.execute,
        )?;
        require_declared_budget_before_signing_v1(&journal.intent)?;
        write_terminal_journal_v1(&path, &mut journal, true)?;
        let authorization = authenticate_chain_derived_planned_journal_v1(&journal, &fresh)?;
        resume_terminal_journal_v1(
            rpc,
            &arguments.origin,
            arguments.expected_cluster,
            &path,
            &arguments.payer_keypair,
            arguments.execute,
            &mut journal,
            Some(&authorization),
        )?;
        terminal_stdout_v1(json!({
            "status": "protocol-stage",
            "stage": stage,
            "journal": path.display().to_string(),
            "phase": journal.phase,
            "feeLamports": journal.intent.transaction_fee_lamports,
            "wireBytes": journal.intent.wire_bytes
        }))?;
        return Ok(false);
    }
    let market = finalized_snapshot(rpc, &[arguments.market])?;
    let account = market.account(arguments.market)?;
    if account.owner != system_program::ID
        || account.lamports != 0
        || account.executable
        || !account.data.is_empty()
    {
        return Err(refusal(
            "all six journals finalized but the aggregate Market account was not exactly closed",
        ));
    }
    Ok(true)
}

fn authenticate_terminal_journal_prefix_v1(
    journal_dir: &Path,
    prepay_required: bool,
) -> Result<()> {
    // THE NAMED CAUSE FIRST. A directory holding `ResolutionCloseFund` without
    // `DirectCloseCapability` in front of it is not a generic hole: it is the
    // exact fault cohort-17 met on devnet, where stage three closed the
    // Resolution dependency funding ledger the Direct close decodes and
    // preserves, and market `9e8fTH75...` can never close its capability again.
    // The declaration owns that accusation so every reader gets one page.
    let present: Vec<TerminalStageV1> = TerminalStageV1::ORDERED
        .into_iter()
        .filter(|stage| journal_dir.join(stage_journal_name_v1(*stage)).exists())
        .collect();
    if let Err(error @ TerminalStageOrderErrorV1::ResolutionCloseFundBeforeDirectClose) =
        authenticate_terminal_stage_prefix_v1(&present)
    {
        return Err(refusal(error.message()));
    }
    let mut ordered = Vec::with_capacity(7);
    for stage in TerminalStageV1::ORDERED {
        if stage == TerminalStageV1::ResolutionCloseFund && prepay_required {
            ordered.push(journal_dir.join("12-resolution-receipt-prepay.json"));
        }
        ordered.push(journal_dir.join(stage_journal_name_v1(stage)));
    }
    let mut missing = false;
    for path in ordered {
        if path.exists() {
            if missing {
                return Err(refusal(
                    "terminal journals contained a later action after a missing durable prefix",
                ));
            }
        } else {
            missing = true;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn operate_resolution_prepay_journal_v1(
    rpc: &mut Rpc,
    arguments: &TerminalSequenceArgumentsV1,
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    lookup_table: Pubkey,
    lookup_addresses: &[Pubkey],
    session: &TerminalSequenceSessionV1,
    path: &Path,
) -> Result<bool> {
    if path.exists() {
        let mut journal = read_terminal_journal_v1(path)?;
        if journal.intent.mutation != DurableTerminalMutationV1::ResolutionReceiptPrepay {
            return Err(refusal(
                "Resolution receipt-prepay journal carried another mutation",
            ));
        }
        if journal.phase == StageJournalPhaseV1::Finalized {
            resume_terminal_journal_v1(
                rpc,
                &arguments.origin,
                arguments.expected_cluster,
                path,
                &arguments.payer_keypair,
                false,
                &mut journal,
                None,
            )?;
            return Ok(true);
        }
        let authorization = if journal.phase == StageJournalPhaseV1::Planned {
            let prepay = plan_resolution_close_from_chain_v1(
                rpc,
                plan,
                evidence,
                arguments.market,
                arguments.payer,
                &[lookup_table],
                session.funded_rent_rate,
            )?;
            let ChainResolutionCloseV1::NeedsReceiptPrepay {
                payer,
                receipt,
                exact_receipt_rent,
                prestate,
                ..
            } = prepay
            else {
                return Err(refusal(
                    "receipt became funded but its unresolved prepay journal was not reconciled",
                ));
            };
            Some(authenticate_resolution_receipt_prepay_planned_journal_v1(
                &journal,
                &payer,
                &receipt,
                exact_receipt_rent,
                &prestate,
            )?)
        } else {
            None
        };
        resume_terminal_journal_v1(
            rpc,
            &arguments.origin,
            arguments.expected_cluster,
            path,
            &arguments.payer_keypair,
            arguments.execute,
            &mut journal,
            authorization.as_ref(),
        )?;
        terminal_stdout_v1(json!({
            "status": "receipt-prepay",
            "journal": path.display().to_string(),
            "phase": journal.phase
        }))?;
        return Ok(false);
    }
    let prepay = plan_resolution_close_from_chain_v1(
        rpc,
        plan,
        evidence,
        arguments.market,
        arguments.payer,
        &[lookup_table],
        session.funded_rent_rate,
    )?;
    let ChainResolutionCloseV1::NeedsReceiptPrepay {
        payer,
        receipt,
        exact_receipt_rent,
        prestate,
        ..
    } = prepay
    else {
        return Err(refusal(
            "receipt is funded but the session-required durable prepay journal is missing",
        ));
    };
    let table = prestate
        .iter()
        .find(|account| account.key == lookup_table)
        .ok_or_else(|| refusal("Resolution prepay snapshot omitted frozen ALT"))?;
    authenticate_supplied_terminal_lookup_table_v1(
        lookup_addresses,
        table,
        session.funded_rent_rate,
    )?;
    let mut journal = build_resolution_receipt_prepay_journal_v1(
        rpc,
        &arguments.origin,
        arguments.expected_cluster,
        &payer,
        &receipt,
        exact_receipt_rent,
        table,
        lookup_addresses,
        session.funded_rent_rate,
        &prestate,
        arguments.execute,
    )?;
    write_terminal_journal_v1(path, &mut journal, true)?;
    let authorization = authenticate_resolution_receipt_prepay_planned_journal_v1(
        &journal,
        &payer,
        &receipt,
        exact_receipt_rent,
        &prestate,
    )?;
    resume_terminal_journal_v1(
        rpc,
        &arguments.origin,
        arguments.expected_cluster,
        path,
        &arguments.payer_keypair,
        arguments.execute,
        &mut journal,
        Some(&authorization),
    )?;
    terminal_stdout_v1(json!({
        "status": "receipt-prepay",
        "journal": path.display().to_string(),
        "phase": journal.phase,
        "topUpLamports": exact_receipt_rent.saturating_sub(receipt.lamports),
        "receiptRentLamports": exact_receipt_rent
    }))?;
    Ok(false)
}

fn stage_journal_name_v1(stage: TerminalStageV1) -> &'static str {
    match stage {
        TerminalStageV1::CoreBeginRetiring => "10-core-begin-retiring.json",
        TerminalStageV1::DirectBeginRetiring => "11-direct-begin-retiring.json",
        TerminalStageV1::ResolutionCloseFund => "13-resolution-close-fund.json",
        TerminalStageV1::DirectCloseCapability => "14-direct-close-capability.json",
        TerminalStageV1::RetirementReplayHandoff => "15-retirement-replay-handoff.json",
        TerminalStageV1::AggregateRetirement => "16-aggregate-retirement.json",
    }
}

fn parse_terminal_sequence_arguments_v1(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<TerminalSequenceArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut market_input = None;
    let mut evidence = None;
    let mut refreshed_evidence = None;
    let mut market = None;
    let mut payer = None;
    let mut payer_keypair = None;
    let mut session = None;
    let mut journal_dir = None;
    let mut completion = None;
    let mut supplied_lookup_table = None;
    let mut supersede_unlandable = None;
    let mut reconcile_landed = None;
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--execute" {
            if execute {
                return Err(Error::new("--execute may be supplied only once"));
            }
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--plan" => &mut plan,
            "--market-input" => &mut market_input,
            "--evidence" => &mut evidence,
            "--refreshed-evidence" => &mut refreshed_evidence,
            "--market" => &mut market,
            "--fee-payer" => &mut payer,
            "--fee-payer-keypair" => &mut payer_keypair,
            "--session" => &mut session,
            "--journal-dir" => &mut journal_dir,
            "--completion" => &mut completion,
            "--lookup-table" => &mut supplied_lookup_table,
            "--supersede-unlandable" => &mut supersede_unlandable,
            "--reconcile-landed" => &mut reconcile_landed,
            _ => {
                return Err(Error::new(format!(
                    "unknown {} argument: {argument}",
                    terminal_sequence_command_v1(expected_cluster)
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let required = |value: Option<String>, label: &str| {
        value.ok_or_else(|| Error::new(format!("{label} is required")))
    };
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let absolute = |value: Option<String>, label: &str| -> Result<PathBuf> {
        let path = PathBuf::from(required(value, label)?);
        if !path.is_absolute() {
            return Err(Error::new(format!("{label} must be absolute")));
        }
        Ok(path)
    };
    Ok(TerminalSequenceArgumentsV1 {
        expected_cluster,
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: absolute(plan, "--plan")?,
        market_input: absolute(market_input, "--market-input")?,
        evidence: absolute(evidence, "--evidence")?,
        refreshed_evidence: refreshed_evidence
            .map(|value| absolute(Some(value), "--refreshed-evidence"))
            .transpose()?,
        market: Pubkey::from_str(&required(market, "--market")?)
            .map_err(|error| Error::new(format!("--market: {error}")))?,
        payer: Pubkey::from_str(&required(payer, "--fee-payer")?)
            .map_err(|error| Error::new(format!("--fee-payer: {error}")))?,
        payer_keypair: absolute(payer_keypair, "--fee-payer-keypair")?,
        session: absolute(session, "--session")?,
        journal_dir: absolute(journal_dir, "--journal-dir")?,
        completion: absolute(completion, "--completion")?,
        supplied_lookup_table: supplied_lookup_table
            .map(|value| {
                Pubkey::from_str(&value)
                    .map_err(|error| Error::new(format!("--lookup-table: {error}")))
            })
            .transpose()?,
        supersede_unlandable: supersede_unlandable
            .map(|value| {
                value
                    .parse::<Signature>()
                    .map(|signature| signature.to_string())
                    .map_err(|error| Error::new(format!("--supersede-unlandable: {error}")))
            })
            .transpose()?,
        reconcile_landed: reconcile_landed
            .map(|value| {
                value
                    .parse::<Signature>()
                    .map(|signature| signature.to_string())
                    .map_err(|error| Error::new(format!("--reconcile-landed: {error}")))
            })
            .transpose()?,
        execute,
    })
}

fn terminal_stdout_v1(value: Value) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap devnet-terminal-sequence-v1 --rpc-url URL \\
     [--i-mean-devnet DEVNET_GENESIS_HASH] --plan ABSOLUTE_JSON \\
     --market-input ABSOLUTE_JSON --evidence ABSOLUTE_JSON \\
     [--refreshed-evidence ABSOLUTE_JSON] --market PUBKEY \\
     --fee-payer PUBKEY --fee-payer-keypair ABSOLUTE_KEYPAIR \\
     --session ABSOLUTE_JSON --journal-dir ABSOLUTE_DIRECTORY \\
     --completion ABSOLUTE_JSON \\
     [--lookup-table PUBKEY] [--supersede-unlandable SIGNATURE] \
     [--reconcile-landed SIGNATURE] [--execute]\n\nWithout --execute \
     this command performs bounded \
     finalized devnet reads and persists exactly one unsigned durable next action before any key \
     can be opened. With --execute it reauthenticates that Planned intent through its stage's \
     semantic owner, reads only the named fee-payer key, persists the signed packet and local \
     signature before first send, and accepts only its exact finalized transaction, balances, \
     return data, and account poststate. Rerun to advance. If --lookup-table is absent, the same \
     journal machinery creates, extends, activates, and freezes a dedicated exact-union ALT before \
     protocol stage one. --supersede-unlandable retires ONE named ambiguous submission, and only \
     after proving on chain that its blockhash has expired and that transaction history still \
     does not know the signature; it performs that one act and returns. --reconcile-landed is its \
     opposite: for a packet that DID land whose journal predicted its poststate under a model \
     since corrected, it re-derives that prediction from the journal's own recorded inputs, \
     proves nothing the chain saw moved, and returns without certifying. Mainnet-beta is refused \
     unconditionally."
}

const fn terminal_sequence_command_v1(expected_cluster: ExpectedClusterV1) -> &'static str {
    match expected_cluster {
        ExpectedClusterV1::Devnet => "devnet-terminal-sequence-v1",
        ExpectedClusterV1::OwnedLoopback => "local-private-validator-terminal-sequence-v1",
    }
}

pub(crate) fn owned_loopback_usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap \
     local-private-validator-terminal-sequence-v1 \
     --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON \
     --market-input ABSOLUTE_JSON --evidence ABSOLUTE_JSON --market PUBKEY \
     --fee-payer PUBKEY --fee-payer-keypair ABSOLUTE_KEYPAIR \
     --session ABSOLUTE_JSON --journal-dir ABSOLUTE_DIRECTORY \
     --completion ABSOLUTE_JSON \\
     [--lookup-table PUBKEY] [--supersede-unlandable SIGNATURE] \
     [--reconcile-landed SIGNATURE] [--execute]\n\nWithout --execute \
     this command performs bounded \
     finalized reads from a validator launched and owned by the private lifecycle runner, then \
     persists exactly one unsigned durable next action before any key can be opened. With \
     --execute it uses the same crash-safe signed-packet journal and exact finalized-poststate \
     checks as the devnet command, but writes distinct owned-loopback session and journal domains. \
     It accepts only 127.0.0.1 with an explicit permitted port and refuses every external origin, \
     including devnet and mainnet-beta."
}

#[cfg(test)]
mod tests {
    /// A ZERO DELTA ON A READONLY ACCOUNT IS A CLAIM, NOT A CONTRADICTION.
    ///
    /// The semantic report and the durable intent each carried one refusal over
    /// two accusations: a writable account with no poststate, and a lamport
    /// delta on an account the frame cannot write. The second is only a
    /// contradiction when the delta is NONZERO -- a zero delta says the
    /// account's lamports do not move, which is exactly what a readonly account
    /// supports and which is checked against its own recorded pre/post
    /// balances. `ResolutionCloseFund` says precisely that about the Market,
    /// correctly, and was undrivable for it.
    ///
    /// The predicate below is the rule both sites now apply, stated once.
    #[test]
    fn a_readonly_account_may_be_declared_unmoved_and_may_not_be_moved() {
        let unmoved = |delta: i128, writable: bool| delta != 0 && !writable;
        assert!(!unmoved(0, false), "a readonly account declared unmoved");
        assert!(!unmoved(0, true), "a writable account declared unmoved");
        assert!(!unmoved(-1, true), "a writable account that moves");
        assert!(
            unmoved(1, false),
            "a readonly account credited is a contradiction"
        );
        assert!(unmoved(-1, false), "and so is a readonly account debited");
    }

    /// A JOURNAL OUTLIVES THE SESSION THAT WROTE IT.
    ///
    /// The prepay journal and the session's receipt balance used to be refused
    /// as a pair whenever the session began exactly funded, which made a
    /// terminal sequence unresumable by any successor session -- and cohort-15
    /// needed exactly that, because the funded-rate schema superseded the
    /// session that had prepaid market 1's seat at 03:42 UTC while the seat
    /// stayed prepaid on chain. The replacement is stricter about the case that
    /// matters and admits the one that is sound.
    #[test]
    fn only_a_finalized_prepay_accounts_for_a_session_that_began_exactly_funded() {
        use super::receipt_prepay_journal_accounts_for_session_v1 as accounts_for;
        // Market 1's own figures: 544 * 6,333 on a 416-byte closure receipt.
        const RENT: u64 = 3_445_152;

        // The session that DID the prepay started below its rent figure, and
        // its journal is admissible in any phase -- a planned entry is the
        // record that stops the prepay being signed twice.
        accounts_for(0, RENT, false).expect("a planned prepay for a receipt that needs one");
        accounts_for(0, RENT, true).expect("and the same prepay once it finalizes");

        // A SUCCESSOR session observed the poststate. Only a landed prepay put
        // those lamports there, so only a finalized journal accounts for them.
        accounts_for(RENT, RENT, true).expect("a finalized prepay explains a funded seat");
        let refused = accounts_for(RENT, RENT, false)
            .expect_err("a prepay that has not landed cannot have funded the seat it is beside");
        let refused = format!("{refused}");
        assert!(
            refused.contains("began exactly funded") && refused.contains(&RENT.to_string()),
            "the refusal names the condition and both figures: {refused}"
        );
    }

    /// ONLY THE CLOCK, AND THE OTHER TWO SYSVARS IN THE SAME FRAMES ARE NOT IT.
    ///
    /// The exemption is a hole in a durability guard, so its shape is worth
    /// pinning: widening it to the Rent or Instructions sysvar would drop a
    /// prestate a plan CAN bind, and the next stage to refuse would then refuse
    /// somewhere else entirely.
    #[test]
    fn only_the_clock_is_exempt_from_the_durable_prestate_equality() {
        assert!(super::prestate_is_slot_bound_runtime_account_v1(
            super::sysvar::clock::ID
        ));
        for other in [
            super::sysvar::rent::ID,
            super::sysvar::instructions::ID,
            super::Pubkey::new_from_array([7_u8; 32]),
        ] {
            assert!(
                !super::prestate_is_slot_bound_runtime_account_v1(other),
                "{other} must still be bound by its durable prestate"
            );
        }
    }

    use std::{
        borrow::Cow,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use solana_address_lookup_table_interface::state::LookupTableMeta;

    use super::*;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn test_observation() -> Observation {
        Observation {
            slot: 101,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn test_account(
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
    ) -> ObservedAccount {
        ObservedAccount {
            observation: test_observation(),
            key,
            owner,
            lamports,
            executable,
            data: Vec::new(),
        }
    }

    fn synthetic_system_transfer_journal_for_payer(
        payer: Pubkey,
    ) -> (
        DurableTerminalJournalV1,
        TerminalSemanticMutationV1,
        TerminalMetaClosureV1,
        Vec<ObservedAccount>,
    ) {
        let recipient = key(2);
        let instruction = solana_system_interface::instruction::transfer(&payer, &recipient, 10);
        let closure = TerminalMetaClosureV1 {
            stage: TerminalStageV1::CoreBeginRetiring,
            program_id: instruction.program_id,
            program_class: TerminalAddressClassV1::InlineProgram,
            accounts: instruction.accounts.clone(),
            classes: vec![
                TerminalAddressClassV1::InlineSigner,
                TerminalAddressClassV1::InlineRequestBound,
            ],
        };
        let message = compile_v0_message_with_optional_tables(
            payer,
            std::slice::from_ref(&instruction),
            Hash::new_from_array([7; 32]),
            test_observation(),
            &[],
        )
        .expect("synthetic v0 message");
        let VersionedMessage::V0(compiled) = &message.message else {
            panic!("synthetic v0")
        };
        let resolved = compiled.account_keys.clone();
        let prestate = resolved
            .iter()
            .map(|address| {
                if *address == payer {
                    test_account(*address, solana_sdk_ids::system_program::ID, 100, false)
                } else if *address == recipient {
                    test_account(*address, solana_sdk_ids::system_program::ID, 0, false)
                } else {
                    test_account(*address, solana_sdk_ids::native_loader::ID, 1, true)
                }
            })
            .collect::<Vec<_>>();
        let pre_balances = prestate
            .iter()
            .map(|account| account.lamports)
            .collect::<Vec<_>>();
        let post_balances = resolved
            .iter()
            .zip(pre_balances.iter().copied())
            .map(|(address, balance)| {
                if *address == payer {
                    balance - 15
                } else if *address == recipient {
                    balance + 10
                } else {
                    balance
                }
            })
            .collect::<Vec<_>>();
        let expected_accounts = BTreeMap::from([
            (
                payer.to_string(),
                DurableExpectedAccountV1 {
                    address: payer.to_string(),
                    owner: solana_sdk_ids::system_program::ID.to_string(),
                    lamports_after_protocol: 90,
                    lamports_after_fee: 85,
                    executable: false,
                    data_base64: String::new(),
                    data_sha256: sha256_hex(&[]),
                    lookup_table: None,
                    execution_clock: None,
                },
            ),
            (
                recipient.to_string(),
                DurableExpectedAccountV1 {
                    address: recipient.to_string(),
                    owner: solana_sdk_ids::system_program::ID.to_string(),
                    lamports_after_protocol: 10,
                    lamports_after_fee: 10,
                    executable: false,
                    data_base64: String::new(),
                    data_sha256: sha256_hex(&[]),
                    lookup_table: None,
                    execution_clock: None,
                },
            ),
        ]);
        let message_bytes = message.message.serialize();
        let intent = DurableTerminalIntentV1 {
            mutation: DurableTerminalMutationV1::Protocol {
                stage: TerminalStageV1::CoreBeginRetiring,
            },
            observation_slot: test_observation().slot,
            observation_unix_timestamp: test_observation().unix_timestamp,
            payer: payer.to_string(),
            program_id: instruction.program_id.to_string(),
            program_class: TerminalAddressClassV1::InlineProgram,
            accounts: closure
                .accounts
                .iter()
                .zip(closure.classes.iter().copied())
                .map(|(meta, class)| DurableInstructionAccountV1 {
                    address: meta.pubkey.to_string(),
                    signer: meta.is_signer,
                    writable: meta.is_writable,
                    class,
                })
                .collect(),
            instruction_data_base64: BASE64.encode(&instruction.data),
            instruction_data_sha256: sha256_hex(&instruction.data),
            compute_unit_limit: None,
            lookup_table: None,
            lookup_table_addresses: Vec::new(),
            lookup_table_addresses_sha256: pubkey_vector_sha256(&[]),
            loaded_writable: Vec::new(),
            loaded_readonly: Vec::new(),
            resolved_account_keys: resolved.iter().map(ToString::to_string).collect(),
            pre_balances,
            post_balances,
            recent_blockhash: Hash::new_from_array([7; 32]).to_string(),
            last_valid_block_height: 1_000,
            transaction_fee_lamports: 5,
            wire_bytes: message.wire_bytes,
            message_base64: BASE64.encode(&message_bytes),
            message_sha256: sha256_hex(&message_bytes),
            prestate: prestate
                .iter()
                .map(|account| (account.key.to_string(), durable_observed_state(account)))
                .collect(),
            expected_accounts,
            expected_return_data: None,
            protocol_lamport_deltas: BTreeMap::from([
                (payer.to_string(), -10),
                (recipient.to_string(), 10),
            ]),
        };
        let mutation = TerminalSemanticMutationV1 {
            stage: TerminalStageV1::CoreBeginRetiring,
            observation: test_observation(),
            instruction,
            expected_return_data: None,
            expected_accounts: vec![
                ExpectedAccountPoststateV1::exact(
                    payer,
                    solana_sdk_ids::system_program::ID,
                    90,
                    false,
                    Vec::new(),
                ),
                ExpectedAccountPoststateV1::exact(
                    recipient,
                    solana_sdk_ids::system_program::ID,
                    10,
                    false,
                    Vec::new(),
                ),
            ],
            protocol_lamport_deltas: BTreeMap::from([(payer, -10), (recipient, 10)]),
        };
        let mut journal = DurableTerminalJournalV1 {
            schema: TERMINAL_JOURNAL_SCHEMA_V1.into(),
            cluster: "devnet".into(),
            rpc_url: "https://example.invalid".into(),
            authorized_mutation: false,
            state_sha256: String::new(),
            phase: StageJournalPhaseV1::Planned,
            intent_sha256: sha256_hex(&serde_json::to_vec(&intent).expect("intent")),
            intent,
            signed_packet_base64: None,
            expected_signature: None,
            finalized: None,
            superseded: None,
            reconciled: None,
        };
        refresh_terminal_journal_digest_v1(&mut journal).expect("journal digest");
        (journal, mutation, closure, prestate)
    }

    fn synthetic_system_transfer_journal() -> (
        DurableTerminalJournalV1,
        TerminalSemanticMutationV1,
        TerminalMetaClosureV1,
        Vec<ObservedAccount>,
    ) {
        synthetic_system_transfer_journal_for_payer(key(1))
    }

    /// A synthetic `ResolutionCloseFund` journal whose COMPILED message and
    /// whose RECORDED budget are supplied separately, so the two can be driven
    /// apart on purpose. Everything else is the system-transfer fixture above.
    fn synthetic_close_fund_journal_v1(
        compiled_instructions: &[Instruction],
        recorded_limit: Option<u32>,
    ) -> DurableTerminalJournalV1 {
        let payer = key(1);
        let recipient = key(2);
        let first_party = solana_system_interface::instruction::transfer(&payer, &recipient, 10);
        let message = compile_v0_message_with_optional_tables(
            payer,
            compiled_instructions,
            Hash::new_from_array([7; 32]),
            test_observation(),
            &[],
        )
        .expect("synthetic v0 message");
        let VersionedMessage::V0(compiled) = &message.message else {
            panic!("synthetic v0")
        };
        let resolved = compiled.account_keys.clone();
        let prestate = resolved
            .iter()
            .map(|address| {
                if *address == payer {
                    test_account(*address, solana_sdk_ids::system_program::ID, 100, false)
                } else if *address == recipient {
                    test_account(*address, solana_sdk_ids::system_program::ID, 0, false)
                } else {
                    test_account(*address, solana_sdk_ids::native_loader::ID, 1, true)
                }
            })
            .collect::<Vec<_>>();
        let pre_balances = prestate
            .iter()
            .map(|account| account.lamports)
            .collect::<Vec<_>>();
        let post_balances = resolved
            .iter()
            .zip(pre_balances.iter().copied())
            .map(|(address, balance)| {
                if *address == payer {
                    balance - 15
                } else if *address == recipient {
                    balance + 10
                } else {
                    balance
                }
            })
            .collect::<Vec<_>>();
        let expected_accounts = BTreeMap::from([
            (
                payer.to_string(),
                DurableExpectedAccountV1 {
                    address: payer.to_string(),
                    owner: solana_sdk_ids::system_program::ID.to_string(),
                    lamports_after_protocol: 90,
                    lamports_after_fee: 85,
                    executable: false,
                    data_base64: String::new(),
                    data_sha256: sha256_hex(&[]),
                    lookup_table: None,
                    execution_clock: None,
                },
            ),
            (
                recipient.to_string(),
                DurableExpectedAccountV1 {
                    address: recipient.to_string(),
                    owner: solana_sdk_ids::system_program::ID.to_string(),
                    lamports_after_protocol: 10,
                    lamports_after_fee: 10,
                    executable: false,
                    data_base64: String::new(),
                    data_sha256: sha256_hex(&[]),
                    lookup_table: None,
                    execution_clock: None,
                },
            ),
        ]);
        let message_bytes = message.message.serialize();
        let intent = DurableTerminalIntentV1 {
            mutation: DurableTerminalMutationV1::Protocol {
                stage: TerminalStageV1::ResolutionCloseFund,
            },
            observation_slot: test_observation().slot,
            observation_unix_timestamp: test_observation().unix_timestamp,
            payer: payer.to_string(),
            program_id: first_party.program_id.to_string(),
            program_class: TerminalAddressClassV1::InlineProgram,
            accounts: first_party
                .accounts
                .iter()
                .zip([
                    TerminalAddressClassV1::InlineSigner,
                    TerminalAddressClassV1::InlineRequestBound,
                ])
                .map(|(meta, class)| DurableInstructionAccountV1 {
                    address: meta.pubkey.to_string(),
                    signer: meta.is_signer,
                    writable: meta.is_writable,
                    class,
                })
                .collect(),
            instruction_data_base64: BASE64.encode(&first_party.data),
            instruction_data_sha256: sha256_hex(&first_party.data),
            compute_unit_limit: recorded_limit,
            lookup_table: None,
            lookup_table_addresses: Vec::new(),
            lookup_table_addresses_sha256: pubkey_vector_sha256(&[]),
            loaded_writable: Vec::new(),
            loaded_readonly: Vec::new(),
            resolved_account_keys: resolved.iter().map(ToString::to_string).collect(),
            pre_balances,
            post_balances,
            recent_blockhash: Hash::new_from_array([7; 32]).to_string(),
            last_valid_block_height: 1_000,
            transaction_fee_lamports: 5,
            wire_bytes: message.wire_bytes,
            message_base64: BASE64.encode(&message_bytes),
            message_sha256: sha256_hex(&message_bytes),
            prestate: prestate
                .iter()
                .map(|account| (account.key.to_string(), durable_observed_state(account)))
                .collect(),
            expected_accounts,
            expected_return_data: None,
            protocol_lamport_deltas: BTreeMap::from([
                (payer.to_string(), -10),
                (recipient.to_string(), 10),
            ]),
        };
        let mut journal = DurableTerminalJournalV1 {
            schema: TERMINAL_JOURNAL_SCHEMA_V1.into(),
            cluster: "devnet".into(),
            rpc_url: "https://example.invalid".into(),
            authorized_mutation: false,
            state_sha256: String::new(),
            phase: StageJournalPhaseV1::Planned,
            intent_sha256: sha256_hex(&serde_json::to_vec(&intent).expect("intent")),
            intent,
            signed_packet_base64: None,
            expected_signature: None,
            finalized: None,
            superseded: None,
            reconciled: None,
        };
        refresh_terminal_journal_digest_v1(&mut journal).expect("journal digest");
        journal
    }

    fn close_fund_first_party_v1() -> Instruction {
        solana_system_interface::instruction::transfer(&key(1), &key(2), 10)
    }

    /// THE NUMBER IS DERIVED, NOT CHOSEN.
    ///
    /// `tools/gauntlet/CU_BUDGETS.md`: `tolerance = roundup(band, 10_000) +
    /// 10_000`, floor 15,000; `budget = measured + tolerance`; `measured` is
    /// the highest draw. Market 1's exact durable message, simulated on devnet
    /// 2026-09-04 under a 1,400,000-CU probe, drew 252,518 three times out of
    /// three -- 252,368 inside Resolution plus the 150 the ComputeBudget
    /// instruction costs itself -- so the band is 0 and the tolerance is its
    /// floor.
    #[test]
    fn the_close_fund_budget_is_its_measured_draw_plus_the_trees_tolerance() {
        const MEASURED: u32 = 252_518;
        const BAND: u32 = 0;
        let tolerance = (BAND.div_ceil(10_000) * 10_000 + 10_000).max(15_000);
        assert_eq!(tolerance, 15_000, "a zero band bottoms out at the floor");
        assert_eq!(
            RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1,
            MEASURED + tolerance
        );
        assert!(
            RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1 > 200_000,
            "a budget inside the default meter would not need declaring"
        );
        assert!(
            RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1 < 1_400_000,
            "and one above the ceiling would not fit at all"
        );
        // Exactly the routes that met the meter declare, in stage order.
        assert_eq!(
            declared_terminal_compute_budgets_v1(),
            vec![
                TerminalStageComputeBudgetV1 {
                    stage: TerminalStageV1::DirectCloseCapability,
                    compute_unit_limit: DIRECT_CLOSE_CAPABILITY_COMPUTE_UNIT_LIMIT_V1,
                },
                TerminalStageComputeBudgetV1 {
                    stage: TerminalStageV1::ResolutionCloseFund,
                    compute_unit_limit: RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1,
                },
            ]
        );
    }

    /// THE SECOND NUMBER IS DERIVED, NOT CHOSEN, BY THE SAME RULE.
    ///
    /// Cohort-17's market 2 was the first market on any chain to reach
    /// `DirectCloseCapability`, and it consumed 200,000 of 200,000 inside Core.
    /// Its own unlandable durable message, rebuilt under a 1,400,000-CU probe
    /// and simulated on devnet 2026-09-06, drew 500,929 three times out of
    /// three -- 500,779 in Core, of which 211,558 is the Trading CPI, plus the
    /// ComputeBudget instruction's own 150 -- so the band is 0 and the
    /// tolerance is its floor.
    #[test]
    fn the_direct_close_budget_is_its_measured_draw_plus_the_trees_tolerance() {
        const MEASURED: u32 = 500_929;
        const BAND: u32 = 0;
        let tolerance = (BAND.div_ceil(10_000) * 10_000 + 10_000).max(15_000);
        assert_eq!(tolerance, 15_000, "a zero band bottoms out at the floor");
        assert_eq!(
            DIRECT_CLOSE_CAPABILITY_COMPUTE_UNIT_LIMIT_V1,
            MEASURED + tolerance
        );
        assert!(
            DIRECT_CLOSE_CAPABILITY_COMPUTE_UNIT_LIMIT_V1 > 200_000,
            "a budget inside the default meter would not need declaring"
        );
        assert!(
            DIRECT_CLOSE_CAPABILITY_COMPUTE_UNIT_LIMIT_V1 < 1_400_000,
            "and one above the ceiling would not fit at all"
        );
        // The route's obligation is a fact about the route, so the message it
        // signs may never omit the declaration.
        assert!(terminal_route_requires_declared_budget_v1(
            &DurableTerminalMutationV1::Protocol {
                stage: TerminalStageV1::DirectCloseCapability,
            }
        ));
    }

    /// A live session may GAIN a measured row and may never lose or move one.
    ///
    /// The guard this serves refuses a table that changed under a sequence
    /// whose earlier stages are already signed. An addition for a stage that
    /// has never been planned is not that: cohort-17's market 2 sat at Retiring
    /// with a frozen ALT and two finalized stages when `DirectCloseCapability`
    /// met the meter, and the number could not be declared without amending its
    /// session.
    #[test]
    fn a_session_budget_table_may_only_gain_unplanned_rows() {
        let row = |stage, compute_unit_limit| TerminalStageComputeBudgetV1 {
            stage,
            compute_unit_limit,
        };
        let close_fund = row(TerminalStageV1::ResolutionCloseFund, 267_518);
        let direct = row(TerminalStageV1::DirectCloseCapability, 515_929);

        assert_eq!(
            terminal_compute_budget_additions_v1(
                std::slice::from_ref(&close_fund),
                &[direct.clone(), close_fund.clone()]
            ),
            Some(vec![TerminalStageV1::DirectCloseCapability]),
            "an addition beside an unchanged row is the amendable case"
        );
        assert_eq!(
            terminal_compute_budget_additions_v1(
                std::slice::from_ref(&close_fund),
                std::slice::from_ref(&close_fund)
            ),
            None,
            "an identical table is not an amendment"
        );
        assert_eq!(
            terminal_compute_budget_additions_v1(
                std::slice::from_ref(&close_fund),
                &[
                    direct.clone(),
                    row(TerminalStageV1::ResolutionCloseFund, 267_519)
                ]
            ),
            None,
            "a re-pinned value under a live sequence is the drift the guard is for"
        );
        assert_eq!(
            terminal_compute_budget_additions_v1(
                &[direct.clone(), close_fund.clone()],
                std::slice::from_ref(&close_fund)
            ),
            None,
            "and a removal is never an addition"
        );
    }

    /// The shape, stated positively: one ComputeBudget limit carrying the
    /// recorded budget, then exactly one first-party instruction.
    #[test]
    fn a_close_fund_message_authenticates_with_its_declared_compute_budget() {
        let limit = RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1;
        let journal = synthetic_close_fund_journal_v1(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(limit),
                close_fund_first_party_v1(),
            ],
            Some(limit),
        );
        authenticate_terminal_journal_v1(&journal)
            .expect("a declared budget matching the message authenticates");

        // And the declaration is really in the signed bytes, not merely in the
        // journal's own JSON: decode the message back out and read it.
        let message_bytes = BASE64
            .decode(&journal.intent.message_base64)
            .expect("durable message base64");
        let message: VersionedMessage =
            bincode::deserialize(&message_bytes).expect("durable versioned message");
        let VersionedMessage::V0(message) = message else {
            panic!("durable message is v0")
        };
        assert_eq!(message.instructions.len(), 2);
        assert_eq!(
            message.account_keys[usize::from(message.instructions[0].program_id_index)],
            compute_budget::ID
        );
        assert_eq!(
            message.instructions[0].data,
            ComputeBudgetInstruction::set_compute_unit_limit(limit).data
        );
    }

    /// A ROUTE UNDER THE DEFAULT METER IS STILL VALID WITH NO PREFIX AT ALL.
    ///
    /// Which is the whole reason the field is optional rather than the schema
    /// being rewritten: `00-alt-create` (10,508 CU), `01-alt-extend`,
    /// `02-alt-freeze` (1,517), `10-core-begin-retiring` (23,106),
    /// `11-direct-begin-retiring` (92,137) and `12-resolution-receipt-prepay`
    /// (150) all landed on devnet inside 200,000, and their finalized journals
    /// must go on reverifying byte-for-byte under this code.
    #[test]
    fn a_route_inside_the_default_meter_authenticates_with_no_prefix() {
        let (journal, _, _, _) = synthetic_system_transfer_journal();
        assert_eq!(
            journal.intent.mutation,
            DurableTerminalMutationV1::Protocol {
                stage: TerminalStageV1::CoreBeginRetiring
            }
        );
        assert_eq!(journal.intent.compute_unit_limit, None);
        assert!(!terminal_route_requires_declared_budget_v1(
            &journal.intent.mutation
        ));
        authenticate_terminal_journal_v1(&journal)
            .expect("an undeclared route inside the meter is unchanged by this schema");
        // And a journal that carries no `computeUnitLimit` key at all -- every
        // journal written before this change -- still parses and still hashes
        // to the digest it was written with.
        let encoded = serde_json::to_string(&journal.intent).expect("intent json");
        assert!(!encoded.contains("computeUnitLimit"), "{encoded}");
        let round_trip: DurableTerminalIntentV1 =
            serde_json::from_str(&encoded).expect("an intent with no budget key");
        assert_eq!(round_trip, journal.intent);
    }

    /// THE WALL THAT STOPPED MARKET 1, TURNED INTO A REFUSAL WITH A NAME.
    ///
    /// `13-resolution-close-fund.json` declared nothing, was signed, and hit
    /// `consumed 200000 of 200000`. Under this code it cannot be planned.
    #[test]
    fn close_fund_without_a_declared_budget_refuses_by_route() {
        let journal = synthetic_close_fund_journal_v1(&[close_fund_first_party_v1()], None);
        // It still DECODES, and it has to: the packet that hit the meter is an
        // undeclared CloseFund, and a journal that cannot be read cannot be
        // retired either.
        authenticate_terminal_journal_v1(&journal)
            .expect("a historical undeclared journal stays readable");
        let refused = format!(
            "{}",
            require_declared_budget_before_signing_v1(&journal.intent)
                .expect_err("CloseFund does not fit the default meter")
        );
        assert!(
            refused.contains("ResolutionCloseFund") && refused.contains("200,000-CU default meter"),
            "the refusal names the route and the meter: {refused}"
        );
        // And every route that does fit passes the same door untouched.
        let (inside, _, _, _) = synthetic_system_transfer_journal();
        require_declared_budget_before_signing_v1(&inside.intent)
            .expect("a route inside the meter needs no declaration");
        let declared = synthetic_close_fund_journal_v1(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(
                    RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1,
                ),
                close_fund_first_party_v1(),
            ],
            Some(RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1),
        );
        require_declared_budget_before_signing_v1(&declared.intent)
            .expect("a declared CloseFund passes");
    }

    /// The three hostiles a prefix invites, each refused for its own reason.
    #[test]
    fn a_substituted_doubled_or_trailing_compute_budget_prefix_refuses() {
        let limit = RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1;

        // (1) A prefix whose VALUE is not the recorded budget. This is the one
        // that a verifier which merely skips the prefix would admit.
        let substituted = synthetic_close_fund_journal_v1(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                close_fund_first_party_v1(),
            ],
            Some(limit),
        );
        let refused = format!(
            "{}",
            authenticate_terminal_journal_v1(&substituted)
                .expect_err("a substituted budget must refuse")
        );
        assert!(
            refused.contains("SetComputeUnitLimit") && refused.contains(&limit.to_string()),
            "the refusal names the prefix and the recorded figure: {refused}"
        );

        // (2) TWO prefixes. The second one lands in the first-party slot, and
        // it is refused for being a ComputeBudget declaration rather than
        // incidentally, by the program-index conjunct further down.
        let doubled = synthetic_close_fund_journal_v1(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(limit),
                ComputeBudgetInstruction::set_compute_unit_limit(limit),
            ],
            Some(limit),
        );
        let refused = format!(
            "{}",
            authenticate_terminal_journal_v1(&doubled)
                .expect_err("two ComputeBudget declarations must refuse")
        );
        assert!(
            refused.contains("first-party instruction was itself a ComputeBudget"),
            "the refusal names the doubled declaration: {refused}"
        );

        // (3) THREE instructions -- two prefixes and the first-party one --
        // refuse on width, which is the shape rule stated in its own words.
        let widened = synthetic_close_fund_journal_v1(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(limit),
                ComputeBudgetInstruction::set_compute_unit_limit(limit),
                close_fund_first_party_v1(),
            ],
            Some(limit),
        );
        let refused = format!(
            "{}",
            authenticate_terminal_journal_v1(&widened)
                .expect_err("a third instruction must refuse")
        );
        assert!(
            refused.contains("carried 3 instruction(s)"),
            "the refusal names the width it found: {refused}"
        );

        // (4) The prefix AFTER the instruction, which is a declaration the
        // runtime honours and a reader skimming for "the interesting one"
        // would not notice had moved.
        let trailing = synthetic_close_fund_journal_v1(
            &[
                close_fund_first_party_v1(),
                ComputeBudgetInstruction::set_compute_unit_limit(limit),
            ],
            Some(limit),
        );
        let refused = format!(
            "{}",
            authenticate_terminal_journal_v1(&trailing)
                .expect_err("a trailing declaration must refuse")
        );
        assert!(
            refused.contains("SetComputeUnitLimit"),
            "the refusal names the prefix it did not find in front: {refused}"
        );

        // (5) And a prefix nobody recorded, on a route that needs none.
        let undeclared = synthetic_close_fund_journal_v1(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(limit),
                close_fund_first_party_v1(),
            ],
            None,
        );
        let refused = format!(
            "{}",
            authenticate_terminal_journal_v1(&undeclared)
                .expect_err("an unrecorded prefix must refuse")
        );
        assert!(
            refused.contains("carried 2 instruction(s)"),
            "the refusal names the width it found against a recorded None: {refused}"
        );
    }

    /// A DRIVER REBUILT MID-SEQUENCE MAY NOT MOVE THE BUDGET UNDER A SESSION
    /// WHOSE EARLIER STAGES ARE ALREADY SIGNED.
    #[test]
    fn a_session_declaring_another_budget_than_this_driver_refuses() {
        let mut session = test_session(&[key(10), key(11)]);
        authenticate_terminal_session_v1(&session).expect("the driver's own table");
        session.declared_compute_unit_limits = vec![TerminalStageComputeBudgetV1 {
            stage: TerminalStageV1::ResolutionCloseFund,
            compute_unit_limit: RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1 + 1,
        }];
        refresh_terminal_session_digest_v1(&mut session).expect("session digest");
        let refused = format!(
            "{}",
            authenticate_terminal_session_v1(&session)
                .expect_err("a re-pinned budget mid-sequence must refuse")
        );
        assert!(
            refused.contains("may not change the budget"),
            "the refusal names the rule: {refused}"
        );
        // An empty table is the claim that every route fits the default meter,
        // which is exactly what a v2 session said and what CloseFund refuted.
        let mut emptied = test_session(&[key(10), key(11)]);
        emptied.declared_compute_unit_limits = Vec::new();
        refresh_terminal_session_digest_v1(&mut emptied).expect("session digest");
        authenticate_terminal_session_v1(&emptied)
            .expect_err("a session declaring nothing must refuse");
    }

    fn signed_synthetic_system_transfer_journal() -> (DurableTerminalJournalV1, Signature) {
        let payer = Keypair::new();
        let (mut journal, _, _, _) = synthetic_system_transfer_journal_for_payer(payer.pubkey());
        let message_bytes = BASE64
            .decode(&journal.intent.message_base64)
            .expect("synthetic message base64");
        let message: VersionedMessage =
            bincode::deserialize(&message_bytes).expect("synthetic versioned message");
        let transaction =
            VersionedTransaction::try_new(message, &[&payer]).expect("sign synthetic packet");
        let signature = transaction.signatures[0];
        let packet = bincode::serialize(&transaction).expect("synthetic signed packet");
        assert_eq!(packet.len(), journal.intent.wire_bytes);
        journal.authorized_mutation = true;
        journal.phase = StageJournalPhaseV1::SignedNotSubmitted;
        journal.signed_packet_base64 = Some(BASE64.encode(packet));
        journal.expected_signature = Some(signature.to_string());
        refresh_terminal_journal_digest_v1(&mut journal).expect("signed journal digest");
        authenticate_terminal_journal_v1(&journal).expect("canonical signed journal");
        (journal, signature)
    }

    /// THE ONLY EXIT FROM AN AMBIGUOUS SUBMISSION OTHER THAN FINALITY.
    ///
    /// Market 1's `13-resolution-close-fund.json` was signed and submitted and
    /// then could never land, because the packet exceeded the default meter.
    /// `Submitted` had no exit for that, so the sequence was wedged: polling a
    /// signature that will never appear, and forbidden -- correctly -- from
    /// re-signing. The retired phase is that exit, and it is reachable only
    /// with the two readings that together settle the packet's fate.
    #[test]
    fn a_superseded_journal_carries_its_proof_and_refuses_every_resume() {
        let (signed, signature) = signed_synthetic_system_transfer_journal();
        let retire = |journal: &DurableTerminalJournalV1,
                      evidence: Option<DurableSupersededEvidenceV1>,
                      phase: StageJournalPhaseV1| {
            let mut retired = journal.clone();
            retired.phase = phase;
            retired.superseded = evidence;
            refresh_terminal_journal_digest_v1(&mut retired).expect("digest");
            retired
        };
        let sound = DurableSupersededEvidenceV1 {
            reason: "the packet exceeded the default meter and can never land".into(),
            retired_signature: signature.to_string(),
            last_valid_block_height: signed.intent.last_valid_block_height,
            observed_block_height: signed.intent.last_valid_block_height + 1,
            observed_slot: 500_000,
        };

        let retired = retire(
            &signed,
            Some(sound.clone()),
            StageJournalPhaseV1::Superseded,
        );
        authenticate_terminal_journal_v1(&retired)
            .expect("a retired packet with both readings is canonical");
        assert_eq!(
            terminal_resume_route_v1(retired.phase, true),
            TerminalResumeRouteV1::RefuseRetired,
            "and --execute does not open a door that read-only leaves shut"
        );
        assert_eq!(
            terminal_resume_route_v1(retired.phase, false),
            TerminalResumeRouteV1::RefuseRetired
        );
        let refused = format!("{}", terminal_retired_refusal_v1(&retired));
        assert!(
            refused.contains(&signature.to_string()) && refused.contains("can never land"),
            "the journal refuses its own resubmission by signature: {refused}"
        );

        // The blockhash has NOT expired: the packet may still be included and
        // retiring it would be a guess dressed as a fact.
        let premature = retire(
            &signed,
            Some(DurableSupersededEvidenceV1 {
                observed_block_height: signed.intent.last_valid_block_height,
                ..sound.clone()
            }),
            StageJournalPhaseV1::Superseded,
        );
        authenticate_terminal_journal_v1(&premature)
            .expect_err("a packet whose blockhash may still be accepted is not retirable");

        // Evidence that names a different signature retires nothing.
        let mismatched = retire(
            &signed,
            Some(DurableSupersededEvidenceV1 {
                retired_signature: Signature::default().to_string(),
                ..sound.clone()
            }),
            StageJournalPhaseV1::Superseded,
        );
        authenticate_terminal_journal_v1(&mismatched)
            .expect_err("retirement evidence must name this journal's own signature");

        // A retired phase with no proof at all, and proof attached to a phase
        // that is still live. Both are the same defect from opposite sides.
        authenticate_terminal_journal_v1(&retire(&signed, None, StageJournalPhaseV1::Superseded))
            .expect_err("a retired phase without its readings is noncanonical");
        authenticate_terminal_journal_v1(&retire(
            &signed,
            Some(sound.clone()),
            StageJournalPhaseV1::Submitted,
        ))
        .expect_err("a submitted packet may not also carry retirement evidence");
        authenticate_terminal_journal_v1(&retire(
            &signed,
            Some(sound),
            StageJournalPhaseV1::Planned,
        ))
        .expect_err("a planned entry was never signed and has nothing to retire");
    }

    /// THE RECEIPT IS WRITTEN AND RETURNED, AND IT IS THE SAME BYTES.
    ///
    /// Measured on devnet 2026-09-04 from the first `ResolutionCloseFund` ever
    /// to execute -- signature `3rDH7V5X...`, slot 493,003,631 -- where the
    /// transaction's `returnData` was byte-identical to the closure
    /// destination's 416-byte `DCSRCLS3` account data. The plan had declared
    /// the account and not the return, so the certification refused with
    /// "finalized terminal transaction carried unexpected return data" against
    /// bytes it had already written down.
    #[test]
    fn the_resolution_closure_receipt_is_declared_once_and_spent_twice() {
        let program = key(15);
        let destination = test_account(key(24), program, 3_445_152, false);
        let receipt = b"DCSRCLS3 and then four hundred and eight more".to_vec();
        let clock = ExecutionClockFieldV1 {
            offset: 8,
            planned_unix_timestamp: 1_788_522_293,
            ceiling_unix_timestamp: 1_788_522_293 + TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS,
        };
        let (returned, account) =
            resolution_close_receipt_poststate_v1(program, &destination, receipt.clone(), clock);
        assert_eq!(
            (returned.execution_clock, account.execution_clock),
            (Some(clock), Some(clock)),
            "one statement of the unbindable field, spent in both halves"
        );
        assert_eq!(returned.producer, program, "the producer is Resolution");
        assert_eq!(returned.body, receipt);
        assert_eq!(
            account.data, receipt,
            "the account and the return carry ONE statement of the receipt"
        );
        assert_eq!(account.key, destination.key);
        assert_eq!(account.owner, program);
        assert_eq!(account.lamports, destination.lamports);
        assert!(!account.executable);
        // And the pair the certification compares: a plan that declares the
        // return data at all is the only kind that can be certified, because
        // the route always produces some.
        let observed = json!({"returnData": {
            "programId": program.to_string(),
            "data": [BASE64.encode(&receipt), "base64"],
        }});
        authenticate_terminal_return_data_v1(&observed, None)
            .expect_err("an undeclared return is exactly what refused market 1");
        authenticate_terminal_return_data_v1(
            &observed,
            Some(&DurableReturnDataV1 {
                producer: program.to_string(),
                body_base64: BASE64.encode(&receipt),
                body_sha256: sha256_hex(&receipt),
                execution_clock: None,
            }),
        )
        .expect("the declared receipt certifies");
    }

    /// A closure receipt whose only interesting field is its clock.
    fn clock_test_receipt(closed_at: u64) -> SourceClosureReceiptV3 {
        SourceClosureReceiptV3 {
            market: [1; 32],
            source_state: [2; 32],
            source_material: [3; 32],
            capability_manifest: [4; 32],
            terminal_certificate: [5; 32],
            receipt_account: [6; 32],
            beneficiary: [7; 32],
            source_state_digest: [8; 32],
            terminal_certificate_digest: [9; 32],
            funding_set_digest: [10; 32],
            generation: 2,
            terminal_sequence: 1,
            selector: 1,
            source_refund_lamports: 2_763_520,
            ledger_remaining_native_principal: 3,
            ledger_rent_lamports: 1_991_360,
            ledger_lamport_surplus: 491_176,
            refund_lamports: 2_763_520 + 3 + 1_991_360 + 491_176,
            closed_at,
        }
    }

    fn clock_test_bytes(closed_at: u64) -> Vec<u8> {
        clock_test_receipt(closed_at)
            .to_bytes()
            .expect("the fixture receipt encodes")
            .to_vec()
    }

    /// THE OFFSET IS ASKED OF THE CODEC, NEVER WRITTEN DOWN.
    ///
    /// The recovered window has to be the one the encoder actually uses, and
    /// the proof is that writing a value through the typed field puts it there.
    #[test]
    fn the_closure_receipts_execution_clock_offset_is_derived_from_its_encoder() {
        let receipt = clock_test_receipt(1_788_522_302);
        let offset = closure_receipt_closed_at_offset_v1(&receipt)
            .expect("closed_at is one contiguous little-endian window");
        let bytes = clock_test_bytes(1_788_522_302);
        assert_eq!(
            &bytes[offset..offset + 8],
            &1_788_522_302_u64.to_le_bytes(),
            "the derived window reads back the value the field was set to"
        );
        assert!(offset + 8 <= SOURCE_CLOSURE_RECEIPT_BYTES_V3);
    }

    fn clock_rule(planned: u64) -> DurableExecutionClockFieldV1 {
        DurableExecutionClockFieldV1 {
            offset: closure_receipt_closed_at_offset_v1(&clock_test_receipt(planned))
                .expect("the fixture has a derivable clock offset"),
            planned_unix_timestamp: planned,
            ceiling_unix_timestamp: planned + TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS,
        }
    }

    /// MARKET 1'S RECEIPT, NINE SECONDS LATE, CERTIFIES -- AND NOTHING ELSE
    /// ABOUT IT IS WEAKENED.
    #[test]
    fn an_execution_clock_inside_the_interval_certifies_and_only_that_field_moves() {
        let planned = 1_788_522_293;
        let rule = clock_rule(planned);
        let plan = clock_test_bytes(planned);

        authenticate_execution_clock_bytes_v1(
            "receipt",
            &clock_test_bytes(planned + 9),
            &plan,
            Some(&rule),
        )
        .expect("the clock devnet actually stamped is inside the interval");
        authenticate_execution_clock_bytes_v1("receipt", &plan, &plan, Some(&rule))
            .expect("an execution at exactly the planned clock is admissible");

        // A byte OUTSIDE the declared field still refuses, which is what makes
        // this a weakening of one field rather than of the poststate.
        let mut elsewhere = clock_test_bytes(planned + 9);
        let rent_offset = elsewhere
            .windows(8)
            .position(|window| window == 1_991_360_u64.to_le_bytes())
            .expect("the fixture's rent reserve is findable");
        elsewhere[rent_offset..rent_offset + 8].copy_from_slice(&1_991_361_u64.to_le_bytes());
        let error =
            authenticate_execution_clock_bytes_v1("receipt", &elsewhere, &plan, Some(&rule))
                .expect_err("one lamport elsewhere is still a refusal");
        assert!(
            error
                .to_string()
                .contains("outside the one field its plan cannot bind"),
            "{error}"
        );
    }

    /// A RECEIPT WITH A CLOCK BEFORE THE PLAN REFUSES.
    #[test]
    fn an_execution_clock_before_its_plan_refuses() {
        let planned = 1_788_522_293;
        let error = authenticate_execution_clock_bytes_v1(
            "receipt",
            &clock_test_bytes(planned - 1),
            &clock_test_bytes(planned),
            Some(&clock_rule(planned)),
        )
        .expect_err("an execution cannot precede its own inputs");
        assert!(
            error.to_string().contains("an execution cannot precede"),
            "{error}"
        );
    }

    /// And one past the sequence's own deadline is not this sequence's.
    #[test]
    fn an_execution_clock_past_the_sequence_deadline_refuses() {
        let planned = 1_788_522_293;
        let rule = clock_rule(planned);
        let late = planned + TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS + 1;
        let error = authenticate_execution_clock_bytes_v1(
            "receipt",
            &clock_test_bytes(late),
            &clock_test_bytes(planned),
            Some(&rule),
        )
        .expect_err("past the ceiling is a refusal");
        assert!(
            error.to_string().contains("as its own execution"),
            "{error}"
        );
        authenticate_execution_clock_bytes_v1(
            "receipt",
            &clock_test_bytes(late - 1),
            &clock_test_bytes(planned),
            Some(&rule),
        )
        .expect("the last admissible second still certifies");
    }

    /// With NO rule the comparison is byte equality, which is what the other
    /// five poststates of this sequence are and stay.
    #[test]
    fn a_poststate_with_no_declared_clock_is_still_exact() {
        let plan = clock_test_bytes(1_788_522_293);
        authenticate_execution_clock_bytes_v1("receipt", &plan, &plan, None)
            .expect("identical bytes certify");
        authenticate_execution_clock_bytes_v1(
            "receipt",
            &clock_test_bytes(1_788_522_294),
            &plan,
            None,
        )
        .expect_err("one second's difference is a refusal with no rule");
    }

    /// A NEW OBLIGATION CARRIES THE SEQUENCE'S DERIVED CEILING AND NO OTHER.
    #[test]
    fn a_new_execution_clock_obligation_must_carry_the_derived_ceiling() {
        let planned = 1_788_522_293;
        require_derived_execution_clock_ceiling_v1(ExecutionClockFieldV1 {
            offset: 400,
            planned_unix_timestamp: planned,
            ceiling_unix_timestamp: planned + TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS,
        })
        .expect("the derived ceiling is admissible");
        let error = require_derived_execution_clock_ceiling_v1(ExecutionClockFieldV1 {
            offset: 400,
            planned_unix_timestamp: planned,
            ceiling_unix_timestamp: planned + TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS + 1,
        })
        .expect_err("a widened ceiling is not this sequence's");
        assert!(error.to_string().contains("derived ceiling"), "{error}");
    }

    /// A LANDED PACKET'S PREDICTION MAY BE RE-DERIVED; NOTHING SIGNED MAY MOVE.
    ///
    /// Market 1's `13-resolution-close-fund.json` is the case: the packet
    /// landed, and its journal predicted the closure receipt under a model
    /// since corrected in three fields. `--supersede-unlandable` refuses it by
    /// name, correctly -- it is not unlandable -- so the scope of the other
    /// exit is what has to be exact.
    #[test]
    fn a_reconciliation_may_move_only_the_poststate_it_rederived() {
        let journal = synthetic_close_fund_journal_v1(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(
                    RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1,
                ),
                close_fund_first_party_v1(),
            ],
            Some(RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1),
        );
        let unchanged = journal.intent.clone();
        let receipt = key(2).to_string();

        // The permitted move: one poststate's bytes and the return data stated
        // from them.
        let mut reconciled = unchanged.clone();
        let body = b"a re-derived closure receipt".to_vec();
        let entry = reconciled
            .expected_accounts
            .get_mut(&receipt)
            .expect("the fixture declares that poststate");
        entry.data_base64 = BASE64.encode(&body);
        entry.data_sha256 = sha256_hex(&body);
        reconciled.expected_return_data = Some(DurableReturnDataV1 {
            producer: key(9).to_string(),
            body_base64: BASE64.encode(&body),
            body_sha256: sha256_hex(&body),
            execution_clock: None,
        });
        authenticate_terminal_reconciliation_scope_v1(&unchanged, &reconciled, &receipt)
            .expect("re-deriving one poststate and its return is the whole permitted act");

        // A SECOND poststate moved, and it refuses -- this is the guard that
        // separates a reconciliation from an edit.
        let mut widened = reconciled.clone();
        widened
            .expected_accounts
            .get_mut(&key(1).to_string())
            .expect("the fixture declares the payer poststate")
            .lamports_after_fee -= 1;
        let error = authenticate_terminal_reconciliation_scope_v1(&unchanged, &widened, &receipt)
            .expect_err("a second poststate is outside the act");
        assert!(
            error
                .to_string()
                .contains("nothing the chain saw may change"),
            "{error}"
        );

        // And so does a message byte, which is the half that was signed.
        let mut resigned = reconciled;
        resigned.message_sha256 = sha256_hex(b"another message");
        authenticate_terminal_reconciliation_scope_v1(&unchanged, &resigned, &receipt)
            .expect_err("the signed message is never a prediction");
    }

    /// Reconciliation evidence belongs to a signed journal, names that
    /// journal's own signature, and never sits beside a retirement: a packet
    /// cannot both have landed and be unlandable.
    #[test]
    fn reconciliation_evidence_has_exactly_one_admissible_shape() {
        let (signed, _) = signed_synthetic_system_transfer_journal();
        let evidence = |signature: &str| DurableReconciledEvidenceV1 {
            reason: "the plan modelled the receipt with the wrong rate".into(),
            landed_signature: signature.into(),
            slot: 493_003_631,
            deployment_program_id: key(3).to_string(),
            deployment_elf_sha256: "24af8504".into(),
            rule: "LiveRentSysvar".into(),
            rent_sysvar_sha256: "f00d".into(),
            receipt_account: key(4).to_string(),
            prior_ledger_rent_lamports: 2_482_536,
            prior_ledger_lamport_surplus: 0,
            ledger_rent_lamports: 1_991_360,
            ledger_lamport_surplus: 491_176,
            execution_clock_offset: 400,
            declared_return_data: true,
        };
        let own = signed
            .expected_signature
            .clone()
            .expect("the fixture is signed");

        let mut sound = signed.clone();
        sound.reconciled = Some(evidence(&own));
        refresh_terminal_journal_digest_v1(&mut sound).expect("digest");
        authenticate_terminal_journal_v1(&sound)
            .expect("a signed journal may carry its own reconciliation");

        let mut mismatched = signed.clone();
        mismatched.reconciled = Some(evidence(&Signature::default().to_string()));
        refresh_terminal_journal_digest_v1(&mut mismatched).expect("digest");
        authenticate_terminal_journal_v1(&mismatched)
            .expect_err("reconciliation evidence must name this journal's own signature");

        let mut planned = signed;
        planned.phase = StageJournalPhaseV1::Planned;
        planned.signed_packet_base64 = None;
        planned.expected_signature = None;
        planned.reconciled = Some(evidence(&own));
        refresh_terminal_journal_digest_v1(&mut planned).expect("digest");
        authenticate_terminal_journal_v1(&planned)
            .expect_err("an entry that was never signed has no landed execution to reconcile");
    }

    /// A PERSISTED POSTSTATE HOLDS DIGESTS, NOT BYTES, SO CERTIFICATION HAS TO
    /// WRITE DOWN THE EXECUTION FACT IT ADMITTED.
    ///
    /// Measured on devnet 2026-09-04: with the interval checked and the value
    /// discarded, the very next pass over market 1's certified CloseFund
    /// refused with "persisted finalized poststate differed from exact intent
    /// bytes" -- the plan's prediction and the chain's receipt differ by nine
    /// seconds forever, and nothing in an archived journal could bridge them.
    #[test]
    fn a_certified_execution_clock_is_recorded_and_rechecked_on_every_reverification() {
        let journal = synthetic_close_fund_journal_v1(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(
                    RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1,
                ),
                close_fund_first_party_v1(),
            ],
            Some(RESOLUTION_CLOSE_FUND_COMPUTE_UNIT_LIMIT_V1),
        );
        let planned = 1_788_522_293_u64;
        let executed = planned + 9;
        let address = key(2);
        let body = |clock: u64| {
            let mut data = vec![0_u8; 24];
            data[8..16].copy_from_slice(&clock.to_le_bytes());
            data
        };
        let mut intent = journal.intent;
        intent.observation_unix_timestamp = i64::try_from(planned).expect("fixture clock");
        let expected = intent
            .expected_accounts
            .get_mut(&address.to_string())
            .expect("the fixture declares that poststate");
        expected.data_base64 = BASE64.encode(body(planned));
        expected.data_sha256 = sha256_hex(&body(planned));
        expected.execution_clock = Some(DurableExecutionClockFieldV1 {
            offset: 8,
            planned_unix_timestamp: planned,
            ceiling_unix_timestamp: planned + TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS,
        });
        let owner = Pubkey::from_str(&expected.owner).expect("fixture owner");
        let lamports = expected.lamports_after_fee;
        let executable = expected.executable;
        let mut poststate: BTreeMap<String, DurableAccountStateV1> = BTreeMap::new();
        for (key, account) in &intent.expected_accounts {
            let data = if *key == address.to_string() {
                body(executed)
            } else {
                BASE64.decode(&account.data_base64).expect("fixture bytes")
            };
            poststate.insert(
                key.clone(),
                durable_state(
                    Pubkey::from_str(key).expect("fixture address"),
                    Pubkey::from_str(&account.owner).expect("fixture owner"),
                    account.lamports_after_fee,
                    account.executable,
                    &data,
                ),
            );
        }
        let evidence = |clocks: BTreeMap<String, u64>| DurableFinalizedEvidenceV1 {
            signature: Signature::default().to_string(),
            slot: 493_003_631,
            fee_lamports: 5_000,
            compute_units_consumed: Some(252_368),
            packet_sha256: sha256_hex(b"packet"),
            poststate: poststate.clone(),
            execution_clocks: clocks,
        };

        authenticate_persisted_terminal_poststate_v1(
            &intent,
            &evidence(BTreeMap::from([(address.to_string(), executed)])),
        )
        .expect("the recorded execution fact reconstructs the exact poststate");

        // A JOURNAL CERTIFIED BEFORE THE RECORD EXISTED STAYS VERIFIABLE.
        //
        // The declared interval is finite and the other bytes are known, so
        // exactly one admissible clock reproduces the recorded digest. That is
        // the fallback, and it is why an evidence-schema change does not
        // retroactively rot journals that already landed.
        authenticate_persisted_terminal_poststate_v1(&intent, &evidence(BTreeMap::new()))
            .expect("the bounded interval recovers a clock no record states");

        // And a poststate that differs somewhere OTHER than that one field is
        // recoverable by no clock in the interval.
        let mut elsewhere = evidence(BTreeMap::new());
        let mut other = body(executed);
        other[0] = 1;
        elsewhere.poststate.insert(
            address.to_string(),
            durable_state(address, owner, lamports, executable, &other),
        );
        let error = authenticate_persisted_terminal_poststate_v1(&intent, &elsewhere)
            .expect_err("only the declared field may move");
        assert!(
            error
                .to_string()
                .contains("reproduces the poststate recorded for"),
            "{error}"
        );

        // The search is bounded BY THE PLAN'S CEILING, not by convenience: a
        // poststate stamped past it is recoverable by no candidate.
        let mut late = evidence(BTreeMap::new());
        late.poststate.insert(
            address.to_string(),
            durable_state(
                address,
                owner,
                lamports,
                executable,
                &body(planned + TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS + 1),
            ),
        );
        let error = authenticate_persisted_terminal_poststate_v1(&intent, &late)
            .expect_err("the fallback never reaches past the declared interval");
        assert!(
            error
                .to_string()
                .contains("reproduces the poststate recorded for"),
            "{error}"
        );

        // Evidence that records a clock for a poststate declaring none.
        let mut undeclared = intent.clone();
        undeclared
            .expected_accounts
            .get_mut(&address.to_string())
            .expect("the fixture declares that poststate")
            .execution_clock = None;
        let error = authenticate_persisted_terminal_poststate_v1(
            &undeclared,
            &evidence(BTreeMap::from([(address.to_string(), executed)])),
        )
        .expect_err("a record without a rule is a claim the plan never made");
        assert!(
            error
                .to_string()
                .contains("whose plan declares no such field"),
            "{error}"
        );

        // A recorded value outside the declared interval, and one that simply
        // is not what the chain wrote.
        let outside = executed + TERMINAL_EXECUTION_CLOCK_CEILING_SECONDS;
        let error = authenticate_persisted_terminal_poststate_v1(
            &intent,
            &evidence(BTreeMap::from([(address.to_string(), outside)])),
        )
        .expect_err("the interval is checked again on every re-verification");
        assert!(error.to_string().contains("outside the ["), "{error}");
        authenticate_persisted_terminal_poststate_v1(
            &intent,
            &evidence(BTreeMap::from([(address.to_string(), executed + 1)])),
        )
        .expect_err(
            "a value inside the interval that is not the one the chain wrote still refuses",
        );
    }

    fn unique_test_path(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "dclutch-terminal-{label}-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// `Rent::default()`'s exemption-scaled rate: 3,480 lamports per byte-year
    /// times an exemption threshold of 2.0. Derived here rather than written as
    /// a literal, so a change in the SDK's default is a red test rather than a
    /// silent disagreement between these fixtures and the bank they model.
    fn default_scaled_rent_rate() -> u32 {
        // Imported here rather than at the module head: the session no longer
        // derives a rate from a cluster reading at all, so this is the only
        // caller left and it is a fixture.
        use dclutch_market::capability_manifest::derive_funded_rent_rate_v2;
        derive_funded_rent_rate_v2(
            Rent::default().minimum_balance(0),
            1,
            Rent::default().minimum_balance(1),
        )
        .expect("Rent::default() is affine in the account length")
    }

    fn test_session(addresses: &[Pubkey]) -> TerminalSequenceSessionV1 {
        let mut session = TerminalSequenceSessionV1 {
            schema: TERMINAL_SESSION_SCHEMA_V1.into(),
            devnet_genesis_hash: Some(DEVNET_GENESIS_HASH.into()),
            owned_loopback_genesis_hash: None,
            rpc_url: "https://example.invalid/".into(),
            plan_sha256: "11".repeat(32),
            market_input_sha256: "22".repeat(32),
            evidence_sha256: "33".repeat(32),
            refreshed_evidence_sha256: None,
            market: key(3).to_string(),
            payer: key(1).to_string(),
            source_receipt: key(4).to_string(),
            receipt_initial_lamports: 7,
            receipt_rent_lamports: 9,
            funded_rent_rate: 1,
            declared_compute_unit_limits: declared_terminal_compute_budgets_v1(),
            supplied_lookup_table: false,
            lookup_table: key(5).to_string(),
            lookup_recent_slot: 99,
            lookup_addresses: addresses.iter().map(ToString::to_string).collect(),
            lookup_addresses_sha256: pubkey_vector_sha256(addresses),
            session_sha256: String::new(),
        };
        refresh_terminal_session_digest_v1(&mut session).expect("session digest");
        session
    }

    fn terminal_cli_arguments() -> Vec<String> {
        vec![
            "--rpc-url".into(),
            "http://127.0.0.1:20890/".into(),
            "--plan".into(),
            "/tmp/plan.json".into(),
            "--market-input".into(),
            "/tmp/market.json".into(),
            "--evidence".into(),
            "/tmp/evidence.json".into(),
            "--market".into(),
            key(3).to_string(),
            "--fee-payer".into(),
            key(1).to_string(),
            "--fee-payer-keypair".into(),
            "/tmp/payer.json".into(),
            "--session".into(),
            "/tmp/session.json".into(),
            "--journal-dir".into(),
            "/tmp/journals".into(),
            "--completion".into(),
            "/tmp/completion.json".into(),
        ]
    }

    fn test_terminal_completion() -> TerminalSequenceCompletionV1 {
        // The fixture's six rows come from the ONE declaration too: a hand-typed
        // ladder here would go on agreeing with whatever this file used to say.
        let mutations = TerminalStageV1::ORDERED
            .map(|stage| completion_mutation_v1(&DurableTerminalMutationV1::Protocol { stage }));
        let journals = mutations
            .into_iter()
            .enumerate()
            .map(|(index, mutation)| TerminalCompletionJournalV1 {
                path: format!("journals/{index:02}.json"),
                sha256: format!("{:02x}", index + 1).repeat(32),
                schema: OWNED_LOOPBACK_TERMINAL_JOURNAL_SCHEMA_V1.into(),
                mutation,
                phase: StageJournalPhaseV1::Finalized,
                fee_payer: key(1).to_string(),
                signature: Signature::from([index as u8 + 1; 64]).to_string(),
                finalized_slot: (100 + index as u64).to_string(),
                compute_units_consumed: (10 + index as u64).to_string(),
                transaction_fee_lamports: "5".into(),
                protocol_lamport_deltas: Vec::new(),
            })
            .collect::<Vec<_>>();
        TerminalSequenceCompletionV1 {
            schema: OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA_V1.into(),
            status: "finalized".into(),
            cluster: "owned-loopback".into(),
            genesis_hash: key(88).to_string(),
            invocation: TerminalCompletionInvocationV1 {
                command: "local-private-validator-terminal-sequence-v1".into(),
                rpc_url: "http://127.0.0.1:20890/".into(),
                plan_path: "/tmp/evidence/plan.json".into(),
                market_input_path: "/tmp/evidence/market.json".into(),
                evidence_path: "/tmp/evidence/campaign.json".into(),
                market: key(3).to_string(),
                fee_payer: key(1).to_string(),
                fee_payer_keypair_path: "/tmp/evidence/payer.json".into(),
                session_path: "/tmp/evidence/run/session.json".into(),
                journal_directory: "/tmp/evidence/journals".into(),
                completion_path: "/tmp/evidence/run/retirement.json".into(),
                supplied_lookup_table: Some(key(5).to_string()),
                execute: true,
            },
            session: TerminalCompletionSessionV1 {
                path: "run/session.json".into(),
                sha256: "11".repeat(32),
                schema: OWNED_LOOPBACK_TERMINAL_SESSION_SCHEMA_V1.into(),
                session_sha256: "22".repeat(32),
            },
            journal_directory: "journals".into(),
            market: key(3).to_string(),
            payer: key(1).to_string(),
            lookup_table: key(5).to_string(),
            journals,
            finalized_slot: "105".into(),
            transaction_fees_lamports: "30".into(),
            compute_units_consumed: "75".into(),
        }
    }

    fn observed_table(
        plan: &TerminalLookupTablePlanV1,
        rent: &Rent,
        addresses: Vec<Pubkey>,
        authority: Option<Pubkey>,
        last_extended_slot_start_index: u8,
        lamport_surplus: u64,
    ) -> ObservedAccount {
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority,
                last_extended_slot: 100,
                last_extended_slot_start_index,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        let data = table.serialize_for_tests().expect("table bytes");
        ObservedAccount {
            observation: Observation {
                slot: 101,
                unix_timestamp: 1_800_000_000,
                finality: Finality::Finalized,
            },
            key: plan.lookup_table,
            owner: lookup_table_program::id(),
            lamports: rent
                .minimum_balance(data.len())
                .checked_add(lamport_surplus)
                .expect("lamports"),
            executable: false,
            data,
        }
    }

    fn initial() -> AuthenticatedTerminalProgressV1 {
        AuthenticatedTerminalProgressV1 {
            core: CoreTerminalStateV1::Terminal,
            direct: DirectTerminalStateV1::Open,
            resolution: ResolutionTerminalStateV1::NeedsReceiptPrepayment,
            replay: RetirementReplayStateV1::Trading,
            outstanding_capabilities: 1,
            all_claim_supplies_zero: true,
            claims_aggregate_live: true,
            rent_credit_live: true,
            hoard_vault_live: true,
        }
    }

    /// The router walks the ONE declared order, and the walk is DERIVED from
    /// `TerminalStageV1::ORDERED` rather than retyped -- a hand-written ladder
    /// is how this file came to hold a second, disagreeing author of the order.
    #[test]
    fn exact_six_stage_route_and_operational_prepay_are_ordered() {
        let mut value = initial();
        let mut walked = Vec::new();
        // The prepay is an operational act, not a protocol stage: it appears
        // once, immediately before `ResolutionCloseFund`, and it is the only
        // route this walk sees that `ORDERED` does not name.
        for stage in TerminalStageV1::ORDERED {
            if stage == TerminalStageV1::ResolutionCloseFund {
                assert_eq!(
                    route_terminal_progress_v1(value).unwrap(),
                    TerminalRouteV1::PrepayResolutionReceipt,
                    "the receipt prepay stands immediately before its stage"
                );
                value.resolution = ResolutionTerminalStateV1::ReadyToClose;
            }
            let route = route_terminal_progress_v1(value).unwrap();
            assert_eq!(route, TerminalRouteV1::Execute(stage));
            walked.push(stage);
            match stage {
                TerminalStageV1::CoreBeginRetiring => value.core = CoreTerminalStateV1::Retiring,
                TerminalStageV1::DirectBeginRetiring => {
                    value.direct = DirectTerminalStateV1::Retiring;
                }
                TerminalStageV1::DirectCloseCapability => {
                    value.direct = DirectTerminalStateV1::Closed;
                    value.outstanding_capabilities = 0;
                }
                TerminalStageV1::ResolutionCloseFund => {
                    value.resolution = ResolutionTerminalStateV1::Closed;
                }
                TerminalStageV1::RetirementReplayHandoff => {
                    value.replay = RetirementReplayStateV1::Core;
                }
                TerminalStageV1::AggregateRetirement => {
                    value.core = CoreTerminalStateV1::Closed;
                    value.replay = RetirementReplayStateV1::Closed;
                    value.claims_aggregate_live = false;
                    value.rent_credit_live = false;
                    value.hoard_vault_live = false;
                }
            }
        }
        assert_eq!(walked, TerminalStageV1::ORDERED);
        assert_eq!(
            route_terminal_progress_v1(value).unwrap(),
            TerminalRouteV1::Complete
        );
    }

    /// THE HOSTILE THE RULING EXISTS FOR, at the router: a Retiring Direct root
    /// whose Resolution fund has ALREADY closed. That is cohort-17's market, and
    /// the router names the cause rather than routing it into a close whose
    /// dependency ledger no longer exists.
    #[test]
    fn a_closed_resolution_fund_before_the_direct_close_refuses_by_name() {
        let mut hostile = initial();
        hostile.core = CoreTerminalStateV1::Retiring;
        hostile.direct = DirectTerminalStateV1::Retiring;
        hostile.resolution = ResolutionTerminalStateV1::Closed;
        let error = route_terminal_progress_v1(hostile).unwrap_err().to_string();
        assert!(
            error.contains("destroys the Direct close's own input"),
            "the router must name the lost dependency, not the shape: {error}"
        );
        // The control: the same state with the fund still open routes forward.
        let mut honest = hostile;
        honest.resolution = ResolutionTerminalStateV1::ReadyToClose;
        assert_eq!(
            route_terminal_progress_v1(honest).unwrap(),
            TerminalRouteV1::Execute(TerminalStageV1::DirectCloseCapability)
        );
    }

    /// The same accusation off a journal DIRECTORY, which is what a resumed run
    /// actually holds: cohort-17's `retire-1/terminal/journal` in one line.
    #[test]
    fn a_journal_directory_that_closed_the_fund_first_refuses_by_name() {
        let directory = unique_test_path("close-fund-before-direct-close");
        fs::create_dir(&directory).expect("journal directory");
        for stage in [
            TerminalStageV1::CoreBeginRetiring,
            TerminalStageV1::DirectBeginRetiring,
            TerminalStageV1::ResolutionCloseFund,
        ] {
            fs::write(directory.join(stage_journal_name_v1(stage)), b"{}").expect("journal");
        }
        let error = authenticate_terminal_journal_prefix_v1(&directory, false)
            .expect_err("a closed dependency ledger is not a generic hole")
            .to_string();
        assert!(
            error.contains("destroys the Direct close's own input"),
            "the prefix check must name the lost dependency: {error}"
        );
        // The control: the ruled prefix, same three counts, is admitted.
        let ruled = unique_test_path("direct-close-before-close-fund");
        fs::create_dir(&ruled).expect("journal directory");
        for stage in [
            TerminalStageV1::CoreBeginRetiring,
            TerminalStageV1::DirectBeginRetiring,
            TerminalStageV1::DirectCloseCapability,
        ] {
            fs::write(ruled.join(stage_journal_name_v1(stage)), b"{}").expect("journal");
        }
        authenticate_terminal_journal_prefix_v1(&ruled, false).expect("the ruled prefix");
        fs::remove_dir_all(directory).expect("remove journal directory");
        fs::remove_dir_all(ruled).expect("remove journal directory");
    }

    #[test]
    fn every_skipped_or_partial_shape_refuses() {
        let mut cases = Vec::new();
        let mut skipped_core = initial();
        skipped_core.direct = DirectTerminalStateV1::Retiring;
        cases.push(skipped_core);
        let mut skipped_direct = initial();
        skipped_direct.core = CoreTerminalStateV1::Retiring;
        skipped_direct.resolution = ResolutionTerminalStateV1::Closed;
        cases.push(skipped_direct);
        let mut close_without_decrement = initial();
        close_without_decrement.core = CoreTerminalStateV1::Retiring;
        close_without_decrement.direct = DirectTerminalStateV1::Closed;
        close_without_decrement.resolution = ResolutionTerminalStateV1::Closed;
        cases.push(close_without_decrement);
        let mut early_handoff = initial();
        early_handoff.core = CoreTerminalStateV1::Retiring;
        early_handoff.direct = DirectTerminalStateV1::Retiring;
        early_handoff.replay = RetirementReplayStateV1::Core;
        cases.push(early_handoff);
        let mut partial_aggregate = initial();
        partial_aggregate.core = CoreTerminalStateV1::Closed;
        partial_aggregate.direct = DirectTerminalStateV1::Closed;
        partial_aggregate.resolution = ResolutionTerminalStateV1::Closed;
        partial_aggregate.outstanding_capabilities = 0;
        partial_aggregate.replay = RetirementReplayStateV1::Closed;
        partial_aggregate.claims_aggregate_live = false;
        partial_aggregate.rent_credit_live = true;
        partial_aggregate.hoard_vault_live = false;
        cases.push(partial_aggregate);
        let mut reminted_claim = initial();
        reminted_claim.all_claim_supplies_zero = false;
        cases.push(reminted_claim);

        for case in cases {
            let error = route_terminal_progress_v1(case).unwrap_err();
            assert!(error.to_string().starts_with("REFUSED terminal sequence:"));
        }
    }

    #[test]
    fn unresolved_journal_never_silently_advances_or_replays() {
        assert!(
            authenticate_journal_route_v1(
                TerminalRouteV1::Execute(TerminalStageV1::DirectBeginRetiring),
                StageJournalPhaseV1::Planned,
                TerminalRouteV1::Execute(TerminalStageV1::DirectBeginRetiring),
            )
            .is_ok()
        );
        for phase in [
            StageJournalPhaseV1::SignedNotSubmitted,
            StageJournalPhaseV1::Submitted,
            StageJournalPhaseV1::Finalized,
        ] {
            assert!(
                authenticate_journal_route_v1(
                    TerminalRouteV1::Execute(TerminalStageV1::DirectBeginRetiring),
                    phase,
                    TerminalRouteV1::Execute(TerminalStageV1::ResolutionCloseFund),
                )
                .is_err()
            );
        }
        assert!(
            authenticate_journal_route_v1(
                TerminalRouteV1::Execute(TerminalStageV1::AggregateRetirement),
                StageJournalPhaseV1::Submitted,
                TerminalRouteV1::Complete,
            )
            .is_err()
        );
        assert!(
            authenticate_journal_route_v1(
                TerminalRouteV1::PrepayResolutionReceipt,
                StageJournalPhaseV1::Planned,
                TerminalRouteV1::Execute(TerminalStageV1::ResolutionCloseFund),
            )
            .is_err()
        );
    }

    #[test]
    fn terminal_resume_routes_both_ambiguous_signed_phases_to_poll_only() {
        for phase in [
            StageJournalPhaseV1::SignedNotSubmitted,
            StageJournalPhaseV1::Submitted,
        ] {
            for execute in [false, true] {
                assert_eq!(
                    terminal_resume_route_v1(phase, execute),
                    TerminalResumeRouteV1::PollOnly
                );
            }
        }
        assert_eq!(
            terminal_resume_route_v1(StageJournalPhaseV1::Planned, false),
            TerminalResumeRouteV1::PlannedReadOnly
        );
        assert_eq!(
            terminal_resume_route_v1(StageJournalPhaseV1::Planned, true),
            TerminalResumeRouteV1::SignAndSubmitOnce
        );
    }

    #[test]
    fn submitted_is_fsynced_before_send_and_a_withheld_response_never_moves_it_back() {
        let path = unique_test_path("submitted-before-send");
        let (mut journal, expected_signature) = signed_synthetic_system_transfer_journal();
        let original_schema = journal.schema.clone();
        let original_cluster = journal.cluster.clone();
        write_terminal_journal_v1(&path, &mut journal, true).expect("persist signed journal");

        let mut calls = 0_u8;
        let error = submit_terminal_packet_once_v1(&path, &mut journal, |packet| {
            calls = calls.saturating_add(1);
            let visible = read_terminal_journal_v1(&path).expect("submitted before transport");
            assert_eq!(visible.phase, StageJournalPhaseV1::Submitted);
            assert_eq!(
                visible.expected_signature.as_deref(),
                Some(expected_signature.to_string().as_str())
            );
            assert_eq!(visible.signed_packet_base64.as_deref(), Some(packet));
            assert_eq!(visible.schema, original_schema);
            assert_eq!(visible.cluster, original_cluster);
            Err(Error::new("synthetic withheld send response"))
        })
        .expect_err("withheld response remains ambiguous");
        assert!(
            error
                .to_string()
                .contains("synthetic withheld send response")
        );
        assert_eq!(calls, 1);

        let mut restarted = read_terminal_journal_v1(&path).expect("durable submitted restart");
        assert_eq!(restarted.phase, StageJournalPhaseV1::Submitted);
        assert_eq!(restarted.schema, TERMINAL_JOURNAL_SCHEMA_V1);
        assert_eq!(restarted.cluster, "devnet");
        let mut replayed = false;
        assert!(
            submit_terminal_packet_once_v1(&path, &mut restarted, |_| {
                replayed = true;
                Ok(expected_signature)
            })
            .is_err()
        );
        assert!(!replayed, "a Submitted restart reached the send closure");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn mismatched_rpc_signature_refuses_and_retains_submitted_journal() {
        let path = unique_test_path("mismatched-send-signature");
        let (mut journal, expected_signature) = signed_synthetic_system_transfer_journal();
        write_terminal_journal_v1(&path, &mut journal, true).expect("persist signed journal");
        let wrong_signature = Keypair::new().sign_message(b"another terminal packet");
        assert_ne!(wrong_signature, expected_signature);

        let error = submit_terminal_packet_once_v1(&path, &mut journal, |_| Ok(wrong_signature))
            .expect_err("RPC signature substitution must refuse");
        assert!(
            error
                .to_string()
                .contains("different from the locally persisted packet")
        );
        let persisted = read_terminal_journal_v1(&path).expect("retained submitted journal");
        assert_eq!(persisted.phase, StageJournalPhaseV1::Submitted);
        assert_eq!(
            persisted.expected_signature.as_deref(),
            Some(expected_signature.to_string().as_str())
        );
        assert_eq!(persisted.schema, TERMINAL_JOURNAL_SCHEMA_V1);
        assert_eq!(persisted.cluster, "devnet");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn stage_order_is_stable_and_exhaustive() {
        for (index, stage) in TerminalStageV1::ORDERED.into_iter().enumerate() {
            assert_eq!(usize::from(stage.ordinal()), index);
        }
    }

    #[test]
    fn terminal_alt_resumes_only_at_exact_pages_then_freezes() {
        let payer = key(1);
        let addresses = (10_u8..55).map(key).collect::<Vec<_>>();
        let rent = Rent::default();
        let plan = plan_terminal_lookup_table_v1(payer, 99, &addresses, default_scaled_rent_rate())
            .expect("plan");
        assert_eq!(plan.addresses.len(), 45);
        assert_eq!(plan.extensions.len(), 3);
        assert!(plan.maximum_preflight_wire_bytes <= PACKET_DATA_BYTES);
        assert_eq!(
            route_terminal_lookup_table_v1(&plan, None, default_scaled_rent_rate())
                .expect("vacant"),
            TerminalLookupTableRouteV1::Create(plan.create.clone())
        );

        for (prefix, start, extension_index) in [(0, 0, 0), (20, 0, 1), (40, 20, 2)] {
            let table = observed_table(
                &plan,
                &rent,
                plan.addresses[..prefix].to_vec(),
                Some(payer),
                start,
                0,
            );
            assert_eq!(
                route_terminal_lookup_table_v1(&plan, Some(&table), default_scaled_rent_rate())
                    .expect("prefix"),
                TerminalLookupTableRouteV1::Extend {
                    prefix_len: prefix,
                    instruction: plan.extensions[extension_index].clone(),
                }
            );
        }

        let mutable = observed_table(&plan, &rent, plan.addresses.clone(), Some(payer), 40, 0);
        assert_eq!(
            route_terminal_lookup_table_v1(&plan, Some(&mutable), default_scaled_rent_rate())
                .expect("full mutable"),
            TerminalLookupTableRouteV1::Freeze(plan.freeze.clone())
        );
        let frozen = observed_table(&plan, &rent, plan.addresses.clone(), None, 40, 0);
        assert_eq!(
            route_terminal_lookup_table_v1(&plan, Some(&frozen), default_scaled_rent_rate())
                .expect("frozen"),
            TerminalLookupTableRouteV1::Complete
        );
        authenticate_supplied_terminal_lookup_table_v1(
            &plan.addresses,
            &frozen,
            default_scaled_rent_rate(),
        )
        .expect("supplied exact frozen table");
    }

    #[test]
    fn terminal_alt_refuses_divergence_partial_freeze_surplus_and_wrong_boundary() {
        let payer = key(1);
        let addresses = (10_u8..55).map(key).collect::<Vec<_>>();
        let rent = Rent::default();
        let plan = plan_terminal_lookup_table_v1(payer, 99, &addresses, default_scaled_rent_rate())
            .expect("plan");

        let mut divergent = plan.addresses[..20].to_vec();
        divergent[19] = key(99);
        let cases = [
            observed_table(&plan, &rent, divergent, Some(payer), 0, 0),
            observed_table(&plan, &rent, plan.addresses[..19].to_vec(), None, 0, 0),
            observed_table(
                &plan,
                &rent,
                plan.addresses[..20].to_vec(),
                Some(payer),
                0,
                1,
            ),
            observed_table(
                &plan,
                &rent,
                plan.addresses[..20].to_vec(),
                Some(payer),
                1,
                0,
            ),
            observed_table(
                &plan,
                &rent,
                plan.addresses[..19].to_vec(),
                Some(payer),
                0,
                0,
            ),
        ];
        for table in cases {
            assert!(
                route_terminal_lookup_table_v1(&plan, Some(&table), default_scaled_rent_rate())
                    .is_err()
            );
        }

        let mutable = observed_table(&plan, &rent, plan.addresses.clone(), Some(payer), 40, 0);
        assert!(
            authenticate_supplied_terminal_lookup_table_v1(
                &plan.addresses,
                &mutable,
                default_scaled_rent_rate()
            )
            .is_err()
        );
    }

    #[test]
    fn terminal_alt_union_uses_only_semantic_owner_lookup_stable_classes() {
        let payer = key(1);
        let mut closures = TerminalStageV1::ORDERED
            .into_iter()
            .enumerate()
            .map(|(index, stage)| meta_closure(stage, u8::try_from(index + 10).unwrap()))
            .collect::<Vec<_>>();
        closures[4].accounts.extend([
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new_readonly(key(80), false),
            AccountMeta::new_readonly(key(81), false),
        ]);
        closures[4].classes.extend([
            TerminalAddressClassV1::InlineSigner,
            TerminalAddressClassV1::InlineProgram,
            TerminalAddressClassV1::InlineRequestBound,
        ]);
        let union = terminal_lookup_union_from_closures_v1(payer, &closures).expect("union");
        assert!(!union.contains(&payer));
        assert!(!union.contains(&key(80)));
        assert!(!union.contains(&key(81)));
        assert!(union.contains(&key(90)));
    }

    fn meta_closure(stage: TerminalStageV1, byte: u8) -> TerminalMetaClosureV1 {
        TerminalMetaClosureV1 {
            stage,
            program_id: key(byte.saturating_add(100)),
            program_class: TerminalAddressClassV1::InlineProgram,
            accounts: vec![
                AccountMeta::new(key(byte), false),
                AccountMeta::new_readonly(key(90), false),
            ],
            classes: vec![
                TerminalAddressClassV1::LookupStable,
                TerminalAddressClassV1::LookupStable,
            ],
        }
    }

    #[test]
    fn terminal_alt_union_requires_all_six_typed_closures_in_order() {
        let payer = key(1);
        let closures = TerminalStageV1::ORDERED
            .into_iter()
            .enumerate()
            .map(|(index, stage)| meta_closure(stage, u8::try_from(index + 10).unwrap()))
            .collect::<Vec<_>>();
        let union = terminal_lookup_union_from_closures_v1(payer, &closures).expect("six closures");
        assert_eq!(union.len(), 7, "six distinct roles plus one shared role");
        assert!(union.contains(&key(90)));

        assert!(terminal_lookup_union_from_closures_v1(payer, &closures[..5]).is_err());
        let mut reordered = closures.clone();
        reordered.swap(1, 2);
        assert!(terminal_lookup_union_from_closures_v1(payer, &reordered).is_err());
        let mut vacant = closures;
        vacant[0].accounts[0].pubkey = Pubkey::default();
        assert!(terminal_lookup_union_from_closures_v1(payer, &vacant).is_err());
    }

    #[test]
    fn fresh_stage_instruction_must_equal_its_coordinate_closure() {
        let closure =
            core_begin_retiring_meta_closure_v1(key(10), key(11), key(12), key(13), key(14));
        let exact = Instruction {
            program_id: closure.program_id,
            accounts: closure.accounts.clone(),
            data: vec![7; 96],
        };
        closure
            .authenticate_instruction(&exact)
            .expect("exact semantic frame");
        let mut substituted = exact.clone();
        substituted.accounts[2].pubkey = key(99);
        assert!(closure.authenticate_instruction(&substituted).is_err());
        let mut privilege = exact.clone();
        privilege.accounts[2].is_writable = true;
        assert!(closure.authenticate_instruction(&privilege).is_err());
        let mut shifted = exact;
        shifted.accounts.swap(1, 2);
        assert!(closure.authenticate_instruction(&shifted).is_err());

        let mut reclassified = closure.clone();
        reclassified.classes[0] = TerminalAddressClassV1::InlineRequestBound;
        assert!(closure.authenticate_fresh_closure(&reclassified).is_err());
    }

    #[test]
    fn replay_handoff_request_replan_changes_inline_pda_but_not_frozen_alt() {
        let payer = key(1);
        let mut before = TerminalStageV1::ORDERED
            .into_iter()
            .enumerate()
            .map(|(index, stage)| meta_closure(stage, u8::try_from(index + 10).unwrap()))
            .collect::<Vec<_>>();
        before[4].accounts.extend([
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new_readonly(key(80), false),
            AccountMeta::new_readonly(key(81), false),
        ]);
        before[4].classes.extend([
            TerminalAddressClassV1::InlineSigner,
            TerminalAddressClassV1::InlineProgram,
            TerminalAddressClassV1::InlineRequestBound,
        ]);
        let before_union =
            terminal_lookup_union_from_closures_v1(payer, &before).expect("initial stable union");

        let mut replanned = before.clone();
        replanned[4].accounts[4].pubkey = key(82);
        let replan_union = terminal_lookup_union_from_closures_v1(payer, &replanned)
            .expect("replanned stable union");
        assert_eq!(before_union, replan_union);
        assert!(before[4].authenticate_fresh_closure(&replanned[4]).is_err());

        let mut misclassified = replanned.clone();
        misclassified[4].classes[4] = TerminalAddressClassV1::LookupStable;
        let wrong_union = terminal_lookup_union_from_closures_v1(payer, &misclassified)
            .expect("misclassification is visible in projected union");
        assert_ne!(before_union, wrong_union);

        let mut conflicting = before;
        conflicting[0]
            .accounts
            .push(AccountMeta::new_readonly(payer, false));
        conflicting[0]
            .classes
            .push(TerminalAddressClassV1::LookupStable);
        assert!(terminal_lookup_union_from_closures_v1(payer, &conflicting).is_err());
    }

    #[test]
    fn terminal_v0_places_stable_keys_in_frozen_alt_and_every_other_class_inline() {
        let payer = key(1);
        let rent = Rent::default();
        let mut closures = TerminalStageV1::ORDERED
            .into_iter()
            .enumerate()
            .map(|(index, stage)| meta_closure(stage, u8::try_from(index + 10).unwrap()))
            .collect::<Vec<_>>();
        closures[4].accounts.extend([
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new_readonly(key(80), false),
            AccountMeta::new_readonly(key(81), false),
        ]);
        closures[4].classes.extend([
            TerminalAddressClassV1::InlineSigner,
            TerminalAddressClassV1::InlineProgram,
            TerminalAddressClassV1::InlineRequestBound,
        ]);
        let union = terminal_lookup_union_from_closures_v1(payer, &closures).expect("stable union");
        let plan = plan_terminal_lookup_table_v1(payer, 99, &union, default_scaled_rent_rate())
            .expect("ALT plan");
        let table = observed_table(&plan, &rent, union, None, 0, 0);
        let instruction = Instruction {
            program_id: closures[4].program_id,
            accounts: closures[4].accounts.clone(),
            data: vec![7; 208],
        };
        let compiled = compile_v0_message_with_optional_tables(
            payer,
            &[instruction],
            Hash::new_from_array([9; 32]),
            table.observation,
            std::slice::from_ref(&table),
        )
        .expect("v0 compile");
        authenticate_terminal_v0_placement_v1(
            payer,
            &closures[4],
            table.key,
            &plan.addresses,
            &compiled,
        )
        .expect("exact placement");

        let mut altered = closures[4].clone();
        altered.classes[4] = TerminalAddressClassV1::LookupStable;
        assert!(
            authenticate_terminal_v0_placement_v1(
                payer,
                &altered,
                table.key,
                &plan.addresses,
                &compiled,
            )
            .is_err()
        );
    }

    #[test]
    fn resolution_close_closure_is_the_exact_direct_v7_frame() {
        let coordinates = ResolutionCloseMetaCoordinatesV2 {
            market: key(10),
            activation_cache: key(11),
            registry_program: key(12),
            core_program: key(13),
            core_programdata: key(14),
            resolution_program: key(15),
            resolution_programdata: key(16),
            source_material: key(17),
            source_material_staging: key(18),
            capability_manifest: key(19),
            capability_manifest_staging: key(20),
            source_state: key(21),
            funding_ledger: key(22),
            certificate: key(23),
            closure_receipt: key(24),
            beneficiary: key(25),
            clock_sysvar: key(26),
            rent_sysvar: key(27),
            system_program: key(28),
            recovery_policy: Some((key(29), key(30))),
        };
        let closure = resolution_close_meta_closure_v2(&coordinates).expect("closure");
        assert_eq!(closure.accounts.len(), 21);
        assert_eq!(closure.accounts[0].pubkey, coordinates.market);
        assert_eq!(closure.accounts[14].pubkey, coordinates.closure_receipt);
        assert_eq!(closure.accounts[19].pubkey, key(29));
        assert_eq!(closure.accounts[20].pubkey, key(30));
        assert_eq!(
            closure
                .accounts
                .iter()
                .enumerate()
                .filter_map(|(index, account)| account.is_writable.then_some(index))
                .collect::<Vec<_>>(),
            vec![11, 12, 14, 15]
        );

        let mut another = coordinates;
        another.market = key(31);
        let another = resolution_close_meta_closure_v2(&another).expect("other market");
        assert_ne!(closure.accounts[0].pubkey, another.accounts[0].pubkey);
    }

    #[test]
    fn self_consistent_journal_cannot_authorize_another_stage_owner() {
        let (journal, mutation, closure, prestate) = synthetic_system_transfer_journal();
        authenticate_terminal_journal_v1(&journal).expect("self-consistent durable envelope");
        authenticate_planned_protocol_owner_v1(
            &journal,
            DurableTerminalMutationV1::Protocol {
                stage: TerminalStageV1::CoreBeginRetiring,
            },
            &mutation,
            &closure,
            &prestate,
        )
        .expect("exact rerun owner output");

        let mut substituted = mutation;
        substituted.instruction =
            solana_system_interface::instruction::transfer(&key(1), &key(2), 9);
        assert!(
            authenticate_planned_protocol_owner_v1(
                &journal,
                DurableTerminalMutationV1::Protocol {
                    stage: TerminalStageV1::CoreBeginRetiring,
                },
                &substituted,
                &closure,
                &prestate,
            )
            .is_err()
        );
    }

    #[test]
    fn durable_message_requires_canonical_payer_header_and_static_prefix() {
        let (mut journal, _, _, _) = synthetic_system_transfer_journal();
        let bytes = BASE64
            .decode(&journal.intent.message_base64)
            .expect("message");
        let mut message: VersionedMessage = bincode::deserialize(&bytes).expect("v0");
        let VersionedMessage::V0(value) = &mut message else {
            panic!("v0")
        };
        value.header.num_readonly_signed_accounts = 1;
        let bytes = message.serialize();
        journal.intent.message_base64 = BASE64.encode(&bytes);
        journal.intent.message_sha256 = sha256_hex(&bytes);
        journal.intent_sha256 = sha256_hex(&serde_json::to_vec(&journal.intent).expect("intent"));
        refresh_terminal_journal_digest_v1(&mut journal).expect("state digest");
        assert!(authenticate_terminal_journal_v1(&journal).is_err());
    }

    #[test]
    fn planned_owner_rerun_accepts_a_later_identical_snapshot_only() {
        let (journal, mut mutation, closure, mut prestate) = synthetic_system_transfer_journal();
        mutation.observation.slot += 1;
        for account in &mut prestate {
            account.observation = mutation.observation;
        }
        authenticate_planned_protocol_owner_v1(
            &journal,
            DurableTerminalMutationV1::Protocol {
                stage: TerminalStageV1::CoreBeginRetiring,
            },
            &mutation,
            &closure,
            &prestate,
        )
        .expect("later exact finalized snapshot");

        prestate[0].lamports += 1;
        assert!(
            authenticate_planned_protocol_owner_v1(
                &journal,
                DurableTerminalMutationV1::Protocol {
                    stage: TerminalStageV1::CoreBeginRetiring,
                },
                &mutation,
                &closure,
                &prestate,
            )
            .is_err()
        );
    }

    #[test]
    fn recomputed_session_checksum_cannot_substitute_semantic_alt_union() {
        let exact = vec![key(10), key(11), key(12)];
        let mut session = test_session(&exact);
        authenticate_terminal_session_v1(&session).expect("canonical envelope");
        authenticate_terminal_session_union_v1(&session, key(4), &exact).expect("semantic union");

        let substituted = vec![key(10), key(11), key(13)];
        session.lookup_addresses = substituted.iter().map(ToString::to_string).collect();
        session.lookup_addresses_sha256 = pubkey_vector_sha256(&substituted);
        refresh_terminal_session_digest_v1(&mut session).expect("edited checksum");
        authenticate_terminal_session_v1(&session)
            .expect("checksum detects corruption, not semantic authority");
        assert!(authenticate_terminal_session_union_v1(&session, key(4), &exact).is_err());

        session.source_receipt = key(99).to_string();
        refresh_terminal_session_digest_v1(&mut session).expect("edited receipt checksum");
        assert!(authenticate_terminal_session_union_v1(&session, key(4), &exact).is_err());
    }

    #[test]
    fn terminal_session_publication_is_no_clobber_and_decode_is_hostile() {
        let path = unique_test_path("session");
        let session = test_session(&[key(10), key(11)]);
        assert!(read_terminal_session_v1(Path::new("relative-session.json")).is_err());
        write_new_terminal_session_v1(&path, &session).expect("publish session");
        assert_eq!(
            read_terminal_session_v1(&path).expect("read session"),
            session
        );
        assert!(write_new_terminal_session_v1(&path, &session).is_err());
        assert_eq!(read_terminal_session_v1(&path).expect("preserved"), session);
        let _ = fs::remove_file(&path);

        let exact = serde_json::to_string(&session).expect("session JSON");
        let duplicate = exact.replacen("\"schema\":", "\"schema\":\"substituted\",\"schema\":", 1);
        fs::write(&path, duplicate).expect("duplicate session");
        assert!(read_terminal_session_v1(&path).is_err());
        let unknown = exact.replacen("{", "{\"unownedTerminalTruth\":true,", 1);
        fs::write(&path, unknown).expect("unknown session");
        assert!(read_terminal_session_v1(&path).is_err());
        fs::write(&path, format!("{exact}\ntrue")).expect("trailing session value");
        assert!(read_terminal_session_v1(&path).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn terminal_cluster_provenance_is_disjoint_without_changing_devnet_shape() {
        let devnet = test_session(&[key(10), key(11)]);
        let devnet_json = serde_json::to_value(&devnet).expect("devnet session JSON");
        assert_eq!(
            devnet_json.get("schema").and_then(Value::as_str),
            Some(TERMINAL_SESSION_SCHEMA_V1)
        );
        assert_eq!(
            devnet_json.get("devnetGenesisHash").and_then(Value::as_str),
            Some(DEVNET_GENESIS_HASH)
        );
        assert!(devnet_json.get("ownedLoopbackGenesisHash").is_none());
        assert_eq!(
            terminal_session_cluster_v1(&devnet).expect("devnet provenance"),
            ExpectedClusterV1::Devnet
        );

        let mut local = devnet.clone();
        local.schema = OWNED_LOOPBACK_TERMINAL_SESSION_SCHEMA_V1.into();
        local.devnet_genesis_hash = None;
        local.owned_loopback_genesis_hash = Some(key(88).to_string());
        refresh_terminal_session_digest_v1(&mut local).expect("local session digest");
        authenticate_terminal_session_v1(&local).expect("local session provenance");
        let local_json = serde_json::to_value(&local).expect("local session JSON");
        assert!(local_json.get("devnetGenesisHash").is_none());
        assert_eq!(
            local_json
                .get("ownedLoopbackGenesisHash")
                .and_then(Value::as_str),
            local.owned_loopback_genesis_hash.as_deref()
        );
        assert_eq!(
            terminal_session_cluster_v1(&local).expect("loopback provenance"),
            ExpectedClusterV1::OwnedLoopback
        );

        local.schema = TERMINAL_SESSION_SCHEMA_V1.into();
        refresh_terminal_session_digest_v1(&mut local).expect("hostile digest");
        assert!(authenticate_terminal_session_v1(&local).is_err());

        let (mut journal, _, _, _) = synthetic_system_transfer_journal();
        journal.schema = OWNED_LOOPBACK_TERMINAL_JOURNAL_SCHEMA_V1.into();
        refresh_terminal_journal_digest_v1(&mut journal).expect("hostile journal digest");
        assert!(authenticate_terminal_journal_v1(&journal).is_err());
        journal.cluster = "owned-loopback".into();
        refresh_terminal_journal_digest_v1(&mut journal).expect("local journal digest");
        authenticate_terminal_journal_v1(&journal).expect("local journal provenance");
        assert_eq!(
            terminal_journal_cluster_v1(&journal).expect("loopback journal"),
            ExpectedClusterV1::OwnedLoopback
        );
    }

    #[test]
    fn terminal_completion_is_exact_ordered_and_hostile_to_substitution() {
        let completion = test_terminal_completion();
        authenticate_terminal_completion_v1(&completion).expect("exact terminal completion");
        let json = serde_json::to_value(&completion).expect("terminal completion JSON");
        assert_eq!(
            json.pointer("/journals/0/finalizedSlot")
                .and_then(Value::as_str),
            Some("100")
        );
        assert_eq!(
            json.pointer("/journals/0/protocolLamportDeltas")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(
            json.pointer("/journals/5/mutation/kind")
                .and_then(Value::as_str),
            Some("aggregate-retirement")
        );

        let mut hostile = completion.clone();
        hostile.journals.swap(0, 1);
        assert!(authenticate_terminal_completion_v1(&hostile).is_err());

        let mut hostile = completion.clone();
        hostile.journals[0].fee_payer = key(9).to_string();
        assert!(authenticate_terminal_completion_v1(&hostile).is_err());

        let mut hostile = completion.clone();
        hostile.journals[0].finalized_slot = "0100".into();
        assert!(authenticate_terminal_completion_v1(&hostile).is_err());

        let mut hostile = completion.clone();
        hostile.journals[0].path = "journals/../substituted.json".into();
        assert!(authenticate_terminal_completion_v1(&hostile).is_err());

        let mut hostile = completion.clone();
        hostile.journals[1].signature = hostile.journals[0].signature.clone();
        assert!(authenticate_terminal_completion_v1(&hostile).is_err());

        let mut hostile = completion.clone();
        hostile.journals[0].protocol_lamport_deltas = vec![TerminalCompletionLamportDeltaV1 {
            account_address: key(7).to_string(),
            delta_lamports: "1".into(),
        }];
        assert!(authenticate_terminal_completion_v1(&hostile).is_err());

        let mut hostile = completion.clone();
        hostile.transaction_fees_lamports = "29".into();
        assert!(authenticate_terminal_completion_v1(&hostile).is_err());

        let mut hostile = completion;
        hostile.schema = TERMINAL_COMPLETION_SCHEMA_V1.into();
        hostile.cluster = "devnet".into();
        assert!(authenticate_terminal_completion_v1(&hostile).is_err());
    }

    #[test]
    fn terminal_cli_is_read_only_by_default_and_refuses_ambiguous_shape() {
        let arguments = terminal_cli_arguments();
        let parsed =
            parse_terminal_sequence_arguments_v1(arguments.clone(), ExpectedClusterV1::Devnet)
                .expect("read plan");
        assert!(!parsed.execute);
        assert!(
            ExpectedClusterV1::Devnet
                .authenticate(&parsed.origin)
                .is_err()
        );

        let local = parse_terminal_sequence_arguments_v1(
            arguments.clone(),
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect("owned-loopback read plan");
        ExpectedClusterV1::OwnedLoopback
            .authenticate(&local.origin)
            .expect("owned-loopback command admits only its local origin");

        let mut external = arguments.clone();
        external[1] = "https://api.devnet.solana.com".into();
        external.extend([
            DEVNET_ACKNOWLEDGMENT_FLAG.into(),
            DEVNET_GENESIS_HASH.into(),
        ]);
        let external =
            parse_terminal_sequence_arguments_v1(external, ExpectedClusterV1::OwnedLoopback)
                .expect("parse typed external origin");
        assert!(
            ExpectedClusterV1::OwnedLoopback
                .authenticate(&external.origin)
                .is_err()
        );

        let mut execute = arguments.clone();
        execute.push("--execute".into());
        assert!(
            parse_terminal_sequence_arguments_v1(execute, ExpectedClusterV1::Devnet)
                .expect("execute")
                .execute
        );
        let mut duplicate = arguments.clone();
        duplicate.extend(["--execute".into(), "--execute".into()]);
        assert!(
            parse_terminal_sequence_arguments_v1(duplicate, ExpectedClusterV1::Devnet).is_err()
        );
        let mut unknown = arguments.clone();
        unknown.extend(["--unknown".into(), "value".into()]);
        assert!(parse_terminal_sequence_arguments_v1(unknown, ExpectedClusterV1::Devnet).is_err());
        let mut relative = arguments;
        let plan_index = relative.iter().position(|value| value == "--plan").unwrap() + 1;
        relative[plan_index] = "relative.json".into();
        assert!(parse_terminal_sequence_arguments_v1(relative, ExpectedClusterV1::Devnet).is_err());
    }

    #[test]
    fn terminal_protocol_and_prepay_journals_require_one_exact_prefix() {
        let directory = unique_test_path("prefix-dir");
        fs::create_dir(&directory).expect("journal directory");
        let second = directory.join(stage_journal_name_v1(TerminalStageV1::DirectBeginRetiring));
        fs::write(&second, b"later").expect("later stage");
        assert!(authenticate_terminal_journal_prefix_v1(&directory, false).is_err());
        fs::remove_file(&second).expect("remove later stage");

        // The complete ordered prefix in FRONT of the prepay's own seat, so the
        // only hole this case leaves is the prepay journal. Before the reorder
        // this list stopped at `DirectBeginRetiring` and the missing stage was
        // the Direct close, which now refuses for its own named reason -- and
        // would have made this case pass while measuring nothing.
        for stage in [
            TerminalStageV1::CoreBeginRetiring,
            TerminalStageV1::DirectBeginRetiring,
            TerminalStageV1::DirectCloseCapability,
        ] {
            fs::write(directory.join(stage_journal_name_v1(stage)), b"prefix")
                .expect("prefix stage");
        }
        fs::write(
            directory.join(stage_journal_name_v1(TerminalStageV1::ResolutionCloseFund)),
            b"close without prepay",
        )
        .expect("later close");
        let error = authenticate_terminal_journal_prefix_v1(&directory, true)
            .expect_err("a CloseFund journal over a missing prepay")
            .to_string();
        assert!(
            error.contains("later action after a missing durable prefix"),
            "the hole here is the prepay, not the Direct close: {error}"
        );
        fs::remove_dir_all(directory).expect("remove journal directory");
    }

    #[test]
    fn journal_publication_is_no_clobber_and_updates_refuse_stale_writers() {
        let path = unique_test_path("writer");
        let lock = path.with_file_name(format!(
            ".{}.terminal-sequence.lock",
            path.file_name().unwrap().to_str().unwrap()
        ));
        let (mut journal, _, _, _) = synthetic_system_transfer_journal();
        write_terminal_journal_v1(&path, &mut journal, true).expect("publish");
        let original = fs::read(&path).expect("original");
        let mut duplicate = journal.clone();
        assert!(write_terminal_journal_v1(&path, &mut duplicate, true).is_err());
        assert_eq!(fs::read(&path).expect("preserved"), original);
        assert!(!lock.exists());

        let mut first = read_terminal_journal_v1(&path).expect("first writer");
        let mut stale = first.clone();
        first.authorized_mutation = true;
        write_terminal_journal_v1(&path, &mut first, false).expect("first update");
        stale.rpc_url.push_str("/stale");
        assert!(write_terminal_journal_v1(&path, &mut stale, false).is_err());
        assert_eq!(
            read_terminal_journal_v1(&path)
                .expect("winner")
                .state_sha256,
            first.state_sha256
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn writer_failure_after_lock_acquisition_leaves_fail_closed_lock() {
        let path = unique_test_path("fail-closed");
        let name = path.file_name().unwrap().to_str().unwrap();
        let lock = path.with_file_name(format!(".{name}.terminal-sequence.lock"));
        let temporary = path.with_file_name(format!(
            ".{name}.terminal-sequence-{}.tmp",
            std::process::id()
        ));
        fs::write(&temporary, b"occupied").expect("occupy exact temporary");
        let (mut journal, _, _, _) = synthetic_system_transfer_journal();
        assert!(write_terminal_journal_v1(&path, &mut journal, true).is_err());
        assert!(lock.exists(), "ambiguous interrupted writer remains locked");
        assert!(!path.exists());
        let _ = fs::remove_file(temporary);
        let _ = fs::remove_file(lock);
    }
}
