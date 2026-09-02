//! Read-only production of wallet payout planning inputs and shared terminal
//! authentication helpers.
//!
//! Persisted campaign evidence supplies routing hints, never state authority.
//! These producers use those hints to name bounded finalized observations,
//! reauthenticate each complete protocol graph through its existing semantic
//! owner. The canonical six-stage terminal transaction lifecycle lives only in
//! `terminal_sequence`; this module neither reads keys nor signs or submits a
//! transaction.

use std::{io::Write, path::PathBuf};

use dclutch_market_core_codec::CoreState;
use dclutch_operator::ObservedAccount;
use dclutch_wallet_terminal_input_operator::{
    ProtocolCoordinatesV1, RoutedRecordV1, TerminalPayoutRequestV1, TerminalRecordRoutingV1,
    TerminalRoutingTableV1, complete_terminal_payout_input_v1, decode_routed_market_v1,
    route_terminal_payout_frame_v1, terminal_payout_round_one_addresses_v1,
};
use sha2::{Digest, Sha256};
use solana_program::pubkey::Pubkey;

// The three pure phases moved to `dclutch-wallet-terminal-input-operator`, and
// every item this binary's other modules were reaching is re-exported at its
// old path. `terminal_sequence` and `aggregate_retirement_exterior` call these
// unchanged; the move changed where the code lives, not what may call it.
pub(crate) use dclutch_wallet_terminal_input_operator::authenticate_zero_claims_v1 as authenticate_zero_claims;

use crate::wallet_terminal::snapshot_from_rpc;
use crate::{
    Error, Result,
    campaign::{
        CampaignAccountEvidenceV1, CampaignTerminalEvidenceV1, parse_campaign_terminal_evidence_v1,
        parse_campaign_terminal_evidence_with_expected_cluster_v1,
    },
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1},
    model::SuccessorPlan,
    plan::{hex, hex32, pubkey},
    rpc::{Rpc, WritePolicyV1},
    wallet_terminal::{FinalizedSnapshotV1, PlanInputV1, RecordPairV1, record_pair},
};

pub(crate) const OWNED_LOOPBACK_WALLET_TERMINAL_INPUT_COMMAND_V1: &str =
    "local-private-validator-wallet-terminal-payout-input-v1";
const TERMINAL_COMPOSITION_LABELS_V1: [&str; 4] = [
    "terminal_composition_descriptor_record",
    "terminal_composition_graph_record",
    "terminal_composition_translation_record",
    "terminal_composition_exposure_record",
];

pub(crate) const DIRECT_BEGIN_RETIRING_LABELS_V1: [&str; 3] = [
    "direct_begin_retiring_account_profile_record",
    "direct_begin_retiring_effect_record",
    "direct_begin_retiring_descriptor_record",
];

pub(crate) const DIRECT_NATIVE_CLOSE_LABELS_V1: [&str; 3] = [
    "direct_native_close_account_profile_record",
    "direct_native_close_effect_record",
    "direct_native_close_descriptor_record",
];

/// The maker-replay close artifact records, published at founding since the
/// five-entry Direct ProgramSet landed (cohort-9, wall 22).
///
/// Deliberately NOT chained into `require_direct_retirement_evidence`, for the
/// activation trio's reason: markets founded on earlier sets carry sealed
/// campaign evidence that legitimately lacks these labels, and their terminal
/// paths must stay drivable. The on-chain close route demands a ProgramSet
/// entry those markets never selected, which is refusal enough.
#[allow(dead_code)]
pub(crate) const DIRECT_CLOSE_MAKER_LABELS_V1: [&str; 3] = [
    "direct_close_maker_account_profile_record",
    "direct_close_maker_effect_record",
    "direct_close_maker_descriptor_record",
];

/// The capability-activation artifact records, published at founding since the
/// four-entry Direct ProgramSet landed.
///
/// Deliberately NOT chained into `require_direct_retirement_evidence`: the two
/// markets founded before the activation entry existed carry sealed campaign
/// evidence that legitimately lacks these labels, and their terminal paths
/// must stay drivable. Nothing is lost by admitting them - the on-chain close
/// route demands the activated root itself, which no pre-activation market can
/// ever present.
#[allow(dead_code)]
pub(crate) const DIRECT_ACTIVATION_LABELS_V1: [&str; 3] = [
    "direct_activation_account_profile_record",
    "direct_activation_effect_record",
    "direct_activation_descriptor_record",
];

struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    evidence: PathBuf,
    market: Pubkey,
    owner: Pubkey,
    recipient: Pubkey,
    claim_index: u32,
    quantity: Option<u64>,
}

pub(crate) fn run_wallet_terminal_input(arguments: Vec<String>) -> Result<()> {
    stdout_json(&produce_wallet_terminal_input_v1(
        arguments,
        ExpectedClusterV1::Devnet,
    )?)
}

pub(crate) fn produce_wallet_terminal_input_owned_loopback_v1(
    arguments: Vec<String>,
) -> Result<PlanInputV1> {
    produce_wallet_terminal_input_v1(arguments, ExpectedClusterV1::OwnedLoopback)
}

pub(crate) fn run_wallet_terminal_input_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    stdout_json(&produce_wallet_terminal_input_owned_loopback_v1(arguments)?)
}

fn produce_wallet_terminal_input_v1(
    arguments: Vec<String>,
    expected_cluster: ExpectedClusterV1,
) -> Result<PlanInputV1> {
    let arguments = parse_arguments(arguments)?;
    expected_cluster.authenticate(&arguments.origin)?;
    let plan_source = std::fs::read(&arguments.plan)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let evidence_source = std::fs::read(&arguments.evidence)?;
    let evidence = if expected_cluster == ExpectedClusterV1::Devnet {
        parse_campaign_terminal_evidence_v1(&evidence_source)?
    } else {
        parse_campaign_terminal_evidence_with_expected_cluster_v1(
            &evidence_source,
            expected_cluster,
        )?
    };
    authenticate_plan_source(&plan_source, &evidence.plan_sha256)?;
    require_terminal_composition_evidence(&evidence)?;

    // THE SHELL'S WHOLE PROTOCOL CONTRIBUTION, enumerated rather than assumed:
    // six coordinates out of the plan, and one address book out of the sealed
    // campaign report. Everything past this line is pure and lives in
    // `dclutch-wallet-terminal-input-operator`, which is why a browser can run
    // it. The two file reads, the RPC and the cluster-origin policy stay here.
    let coordinates = protocol_coordinates_v1(&plan)?;
    let routing = terminal_routing_table_v1(&evidence)?;
    let request = TerminalPayoutRequestV1 {
        market: arguments.market,
        owner: arguments.owner,
        recipient: arguments.recipient,
        claim_index: arguments.claim_index,
        quantity: arguments.quantity,
    };

    // ROUND ONE, and there are only two.
    //
    // Decision 0008 §1: the Claims aggregate is the SOLE persisted owner of
    // this Market's Custody namespace, and no route may re-guess it. The
    // campaign report's `founding_custody_context` records the founding's own
    // action pre-image, and Custody addresses the Claims-role replay a payout
    // decodes under that pre-image's projected-hoard digest -- so taking the
    // context from evidence here addressed a replay that has never existed at
    // any market, and the refusal named the raw-form address. This reads the
    // owner of the namespace directly instead, which is the decision applied
    // rather than worked around.
    //
    // And the aggregate's address is a PDA of the Market under the plan's
    // Claims program, derivable before any read -- so it shares the Market's
    // round rather than costing one of its own. This command took three
    // rounds and takes two.
    let round_one_keys = terminal_payout_round_one_addresses_v1(&coordinates, &routing, &request)?;
    let mut rpc = Rpc::connect_cluster(&arguments.origin, WritePolicyV1::ReadsOnly)?;
    let round_one = finalized_snapshot(&mut rpc, &round_one_keys)?;
    let frame = route_terminal_payout_frame_v1(&coordinates, &routing, &request, &round_one)?;

    // ROUND TWO, at the floor round one established, over the addresses the
    // derivation itself names. Nothing here assembles that list.
    let addresses = frame.addresses();
    let floor = round_one.observation.slot;
    let (slot, values) = rpc.finalized_accounts(&addresses, floor)?;
    let round_two = snapshot_from_rpc(slot, rpc.block_time(slot)?, &addresses, values)?;

    let completed = complete_terminal_payout_input_v1(&frame, &round_two, &request)?;
    eprintln!(
        "wallet-terminal-payout-input: authenticated one finalized snapshot at slot {} with live Core {}",
        round_two.observation.slot,
        completed.receipt_meaning.label(),
    );
    Ok(completed.input)
}

/// The six protocol coordinates the derivation takes from the plan.
///
/// Enumerated rather than assumed: every `plan.*` access the producer made is
/// one of these. A browser holds five of them from its own deployment table and
/// reads the sixth out of the Market's Core state.
fn protocol_coordinates_v1(plan: &SuccessorPlan) -> Result<ProtocolCoordinatesV1> {
    Ok(ProtocolCoordinatesV1 {
        registry: pubkey(&plan.registry.program_id)?,
        core: pubkey(&plan.core.program_id)?,
        claims: pubkey(&plan.claims.program_id)?,
        custody: pubkey(&plan.custody.program_id)?,
        resolution: pubkey(&plan.resolution.program_id)?,
        release_set: hex32(&plan.release_set_id)?,
    })
}

/// The routing table the derivation takes from one sealed campaign report.
///
/// The campaign emitter and its parser deliberately live together, so this
/// PROJECTS the report rather than moving it: exactly the eleven rows the
/// derivation reads, and nothing of the report's execution, transaction or
/// account vocabulary crosses the boundary. Every address it names is
/// re-derived from its own digest and re-authenticated against finalized state
/// on the far side, which is what keeps it an address book rather than an
/// authority.
fn terminal_routing_table_v1(
    evidence: &CampaignTerminalEvidenceV1,
) -> Result<TerminalRoutingTableV1> {
    let routed = |label: &str| -> Result<RoutedRecordV1> {
        let persisted = required_account(evidence, label)?;
        Ok(RoutedRecordV1 {
            digest: hex32(&persisted.data_sha256)?,
            address: pubkey(&persisted.address)?,
        })
    };
    let mint = required_account(evidence, "collateral_mint")?;
    Ok(TerminalRoutingTableV1 {
        founding_market: pubkey(&required_account(evidence, "founding_market")?.address)?,
        collateral_mint: pubkey(&mint.address)?,
        token_program: pubkey(&mint.owner)?,
        records: TerminalRecordRoutingV1 {
            realm: routed("realm_record")?,
            product: routed("product_record")?,
            result_domain: routed("result_domain_record")?,
            portfolio: routed("portfolio_record")?,
            product_basis: routed("linked_liability_basis_record")?,
            composition_descriptor: routed(TERMINAL_COMPOSITION_LABELS_V1[0])?,
            composition_graph: routed(TERMINAL_COMPOSITION_LABELS_V1[1])?,
            composition_translation: routed(TERMINAL_COMPOSITION_LABELS_V1[2])?,
            composition_exposure: routed(TERMINAL_COMPOSITION_LABELS_V1[3])?,
        },
    })
}

/// Decode one Core Market against the plan's own routing coordinates.
///
/// The derivation moved; the four call sites in `terminal_sequence` keep the
/// shape they had, with `core` passed explicitly as they always passed it.
pub(crate) fn decode_routed_market(
    account: &ObservedAccount,
    core: Pubkey,
    plan: &SuccessorPlan,
) -> Result<CoreState> {
    let mut coordinates = protocol_coordinates_v1(plan)?;
    coordinates.core = core;
    Ok(decode_routed_market_v1(account, &coordinates)?)
}

pub(crate) fn routed_record(
    evidence: &CampaignTerminalEvidenceV1,
    label: &str,
    registry: Pubkey,
    schema: [u8; 32],
) -> Result<RecordPairV1> {
    let persisted = required_account(evidence, label)?;
    let pair = record_pair(registry, schema, hex32(&persisted.data_sha256)?);
    let persisted_address = pubkey(&persisted.address)?;
    if pair.raw != persisted_address {
        return Err(Error::new(format!(
            "persisted {label} address {persisted_address} is not canonical {}",
            pair.raw
        )));
    }
    Ok(pair)
}

pub(crate) fn finalized_snapshot(rpc: &mut Rpc, keys: &[Pubkey]) -> Result<FinalizedSnapshotV1> {
    let mut keys = keys.to_vec();
    keys.sort_unstable();
    keys.dedup();
    let floor = rpc.finalized_slot()?;
    let (slot, values) = rpc.finalized_accounts(&keys, floor)?;
    snapshot_from_rpc(slot, rpc.block_time(slot)?, &keys, values)
}

pub(crate) fn authenticate_plan_source(source: &[u8], expected: &str) -> Result<()> {
    let expected = hex32(expected)?;
    let observed: [u8; 32] = Sha256::digest(source).into();
    if observed != expected {
        return Err(Error::new(format!(
            "evidence planSha256 {} does not authenticate plan {}",
            hex(&expected),
            hex(&observed)
        )));
    }
    Ok(())
}

fn require_terminal_composition_evidence(evidence: &CampaignTerminalEvidenceV1) -> Result<()> {
    let missing = TERMINAL_COMPOSITION_LABELS_V1
        .iter()
        .copied()
        .filter(|label| !evidence.accounts.contains_key(*label))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(Error::new(format!(
            "terminal payout is blocked: canonical native-composition publication evidence is missing {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

/// Accounts the Direct exterior creates permissionlessly ON FIRST USE.
///
/// Since f581af6b made the Direct exterior callable, nothing at founding
/// creates these two: the first trade does. Their ADDRESSES are known at
/// founding and are carried in the checkpoint, but no account exists to
/// collect evidence from until the route that creates them has run.
const DIRECT_FIRST_USE_LABELS_V1: [&str; 2] =
    ["direct_capability_root", "direct_trading_funding_ledger"];

pub(crate) fn require_direct_retirement_evidence(
    evidence: &CampaignTerminalEvidenceV1,
) -> Result<()> {
    // The record labels are published AT FOUNDING and are unconditional. These
    // are artifacts, not state, and a campaign that reached terminal without
    // them is missing something founding was supposed to leave behind.
    for label in DIRECT_BEGIN_RETIRING_LABELS_V1
        .into_iter()
        .chain(DIRECT_NATIVE_CLOSE_LABELS_V1)
        .chain([
            "direct_program_set_record",
            "direct_execution_config_record",
        ])
    {
        required_account(evidence, label).map_err(|_| {
            Error::new(format!(
                "terminal sequence is blocked: campaign evidence omitted exact Direct lifecycle label {label}"
            ))
        })?;
    }
    require_distinct_funding_ledgers_v1(evidence)?;
    require_direct_first_use_evidence_v1(evidence)
}

/// The Resolution and Trading funding ledgers are two accounts, never one.
///
/// The close frame names both, at adjacent indices, and its distinctness clause
/// refuses the whole thirty-eight-account projection with an undifferentiated
/// `Frame` if they alias. That refusal names nothing, so the same aliasing is
/// worth catching here, at admission, where the evidence itself can be blamed.
///
/// The hazard is a label that reads semantic and is ordinal.
/// `founding_funding_ledger_v2_N` is indexed by the campaign's own sort of its
/// controller subsets — by each mask's lowest manifest bit — and the selected
/// trade entry sits at manifest index 0 in every campaign this tree founds, so
/// ordinal 0 is the TRADING ledger. `resolution_funding_ledger` is the semantic
/// label for the other one, and it is among the eleven immutable founding
/// records, so a refresh reproduces it byte-identically.
fn require_distinct_funding_ledgers_v1(evidence: &CampaignTerminalEvidenceV1) -> Result<()> {
    let resolution = required_account(evidence, "resolution_funding_ledger")?;
    let trading = required_account(evidence, "direct_trading_funding_ledger")?;
    if resolution.address == trading.address {
        return Err(Error::new(format!(
            "terminal sequence is blocked: campaign evidence gives the Resolution and Trading \
             funding ledgers the same address {}. They are distinct controller subsets and the \
             native-close frame names both; one address for both cannot be a two-controller \
             founding.",
            resolution.address
        )));
    }
    Ok(())
}

/// Demand the first-use accounts EXACTLY WHEN the route that creates them ran.
///
/// This used to demand both unconditionally, which under first-use creation
/// walls every lifecycle that reaches terminal before trading — the campaign
/// evidence collector cannot produce evidence for an account nothing has
/// created yet. Demanding it anyway would not be strictness, it would be
/// requiring a fact that cannot exist.
///
/// The strictness that IS real is kept, and it is the pairing. Both accounts
/// are created by the same first-use path, so the admissible states are both
/// absent (nothing traded — a terminal reached from founding) or both present
/// (trading ran, and a terminal WITH trades must still account for its root).
/// Exactly one present is neither, and is refused: it means the collector saw
/// the route run and dropped half of what it left behind, which is precisely
/// the silent-omission failure the unconditional demand was there to catch.
pub(crate) fn require_direct_first_use_evidence_v1(
    evidence: &CampaignTerminalEvidenceV1,
) -> Result<()> {
    let present: Vec<&str> = DIRECT_FIRST_USE_LABELS_V1
        .into_iter()
        .filter(|label| evidence.accounts.contains_key(*label))
        .collect();
    if present.is_empty() || present.len() == DIRECT_FIRST_USE_LABELS_V1.len() {
        return Ok(());
    }
    let missing: Vec<&str> = DIRECT_FIRST_USE_LABELS_V1
        .into_iter()
        .filter(|label| !evidence.accounts.contains_key(*label))
        .collect();
    Err(Error::new(format!(
        "terminal sequence is blocked: the Direct first-use accounts are created together by the \
         first trade, so campaign evidence must carry all of them or none. It carries {} and \
         omits {}.",
        present.join(", "),
        missing.join(", ")
    )))
}

pub(crate) fn authenticate_campaign_market_v1(
    evidence: &CampaignTerminalEvidenceV1,
    expected: Pubkey,
) -> Result<()> {
    let persisted = pubkey(&required_account(evidence, "founding_market")?.address)?;
    if persisted != expected {
        return Err(Error::new(
            "terminal Market differed from exact founding campaign evidence",
        ));
    }
    Ok(())
}

pub(crate) fn required_account<'a>(
    evidence: &'a CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<&'a CampaignAccountEvidenceV1> {
    evidence
        .accounts
        .get(label)
        .ok_or_else(|| Error::new(format!("payout evidence is missing account label {label}")))
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut evidence = None;
    let mut market = None;
    let mut owner = None;
    let mut recipient = None;
    let mut claim_index = None;
    let mut quantity = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--plan" => &mut plan,
            "--evidence" => &mut evidence,
            "--market" => &mut market,
            "--owner" => &mut owner,
            "--recipient" => &mut recipient,
            "--claim-index" => &mut claim_index,
            "--quantity" => &mut quantity,
            _ => {
                return Err(Error::new(format!(
                    "unknown wallet-terminal-payout-input argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let rpc_url = required(rpc_url, "--rpc-url")?;
    Ok(ArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: absolute(plan, "--plan")?,
        evidence: absolute(evidence, "--evidence")?,
        market: pubkey(&required(market, "--market")?)?,
        owner: pubkey(&required(owner, "--owner")?)?,
        recipient: pubkey(&required(recipient, "--recipient")?)?,
        claim_index: canonical_u32(&required(claim_index, "--claim-index")?, "--claim-index")?,
        quantity: quantity
            .map(|value| canonical_u64(&value, "--quantity"))
            .transpose()?,
    })
}

fn canonical_u32(value: &str, label: &str) -> Result<u32> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::new(format!(
            "{label} must be a canonical decimal u32"
        )));
    }
    value
        .parse()
        .map_err(|_| Error::new(format!("{label} must be a canonical decimal u32")))
}

fn canonical_u64(value: &str, label: &str) -> Result<u64> {
    if value.is_empty()
        || value == "0"
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::new(format!(
            "{label} must be a positive canonical decimal u64"
        )));
    }
    value
        .parse()
        .map_err(|_| Error::new(format!("{label} must be a positive canonical decimal u64")))
}

fn required(value: Option<String>, label: &str) -> Result<String> {
    value.ok_or_else(|| Error::new(format!("{label} is required")))
}

fn absolute(value: Option<String>, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(required(value, label)?);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

fn stdout_json(value: &impl serde::Serialize) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap wallet-terminal-payout-input --rpc-url URL \\
     [--i-mean-devnet DEVNET_GENESIS_HASH] --plan ABSOLUTE_JSON \\
     --evidence ABSOLUTE_JSON --market PUBKEY --owner PUBKEY --recipient PUBKEY \\
     --claim-index U32 [--quantity U64]\n\nThis command is read-only. It uses persisted \
     one exact completed dclutch-successor-campaign-report-v1 only to route finalized account observations, \
     reauthenticates the complete payout graph, derives a crash-stable parent context from the \
     immutable request and authenticated prestate (never the observation slot), and emits the \
     exact dclutch-wallet-terminal-payout-plan-input-v1 accepted by the existing ALT planner. \
     Core's fresh terminal_receipt is the sole terminal identity and always names the accepted \
     Resolution certificate. Categorical and graded-failure payouts authenticate that certificate; \
     graded-success payout remains refused until the Claims terminal ABI consumes it directly. \
     Missing canonical native-composition publication evidence is a hard lifecycle blocker. \
     Mainnet-beta is refused unconditionally."
}

pub(crate) fn owned_loopback_usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap \
     local-private-validator-wallet-terminal-payout-input-v1 \
     --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON \
     --evidence ABSOLUTE_JSON --market PUBKEY --owner PUBKEY --recipient PUBKEY \
     --claim-index U32 [--quantity U64]\n\nThis read-only command derives the same exact \
     wallet payout input from finalized protocol state, but accepts only a validator launched and \
     owned by the private lifecycle runner. It refuses devnet, mainnet-beta, and every non-loopback \
     origin."
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use dclutch_claims_svm::liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
        LiabilityBasisMarketInputV2, encode_liability_basis_market_into_v2,
        liability_basis_vector_width_v2,
    };
    use dclutch_market_core_codec::{Identity, MarketIdentity, Phase, Readiness, StateBumpsV1};
    use dclutch_operator::{Finality, Observation, ObservedAccount};
    use serde_json::Value;
    use solana_sdk::signature::{Keypair, Signer as _};

    use super::*;

    fn identity(value: u8) -> Identity {
        Identity::new([value; 32]).expect("identity")
    }

    fn terminal_market(market: Pubkey, outstanding_capabilities: u64) -> CoreState {
        CoreState {
            phase: Phase::Terminal,
            readiness: Readiness::Consumed,
            terminal_winner: 1,
            identity: MarketIdentity {
                market_id: Identity::new(market.to_bytes()).unwrap(),
                realm_id: identity(2),
                product_record: identity(3),
                product_id: identity(4),
                resolution_policy: identity(5),
                capability_manifest: identity(6),
                selected_release_set: identity(7),
                registry_program: identity(8),
                generation: 9,
            },
            outstanding_capabilities,
            principal_cap_sets: 1,
            rent_beneficiary: identity(10),
            terminal_receipt: Some(identity(11)),
            bumps: StateBumpsV1::UNRECORDED,
        }
    }

    fn claims_aggregate(
        market: CoreState,
        claims: Pubkey,
        custody_context: [u8; 32],
        supplies: &[u64],
    ) -> ObservedAccount {
        let key = Pubkey::find_program_address(
            &[
                LIABILITY_BASIS_MARKET_SEED_V2,
                market.identity.market_id.to_bytes().as_slice(),
            ],
            &claims,
        )
        .0;
        let mut data = vec![
            0;
            liability_basis_vector_width_v2(
                LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
                u32::try_from(supplies.len()).unwrap(),
            )
            .unwrap()
        ];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision: 1,
                logical_market: market.identity.market_id.to_bytes(),
                release_set: market.identity.selected_release_set.to_bytes(),
                registry_program: market.identity.registry_program.to_bytes(),
                product_instance_id: market.identity.product_id.to_bytes(),
                basis_id: [12; 32],
                realm_id: market.identity.realm_id.to_bytes(),
                custody_context,
                generation: market.identity.generation,
            },
            supplies,
            &mut data,
        )
        .unwrap();
        ObservedAccount {
            observation: Observation {
                slot: 1,
                unix_timestamp: 1,
                finality: Finality::Finalized,
            },
            key,
            owner: claims,
            lamports: 1,
            executable: false,
            data,
        }
    }

    /// The routing table demands each of its eleven rows BY LABEL.
    ///
    /// The projection is what crosses the boundary into the pure derivation, so
    /// a report missing a row must be blamed here, at the document, rather than
    /// producing a table with a zero coordinate that the far side then refuses
    /// with a sentence about a PDA.
    #[test]
    fn the_routing_table_names_every_row_the_derivation_reads() {
        let labels = [
            "founding_market",
            "collateral_mint",
            "realm_record",
            "product_record",
            "result_domain_record",
            "portfolio_record",
            "linked_liability_basis_record",
            TERMINAL_COMPOSITION_LABELS_V1[0],
            TERMINAL_COMPOSITION_LABELS_V1[1],
            TERMINAL_COMPOSITION_LABELS_V1[2],
            TERMINAL_COMPOSITION_LABELS_V1[3],
        ];
        let exact = evidence_with_labels(&labels);
        terminal_routing_table_v1(&exact).expect("a complete address book projects");
        for label in labels {
            let mut hostile = exact.clone();
            hostile.accounts.remove(label);
            let error = terminal_routing_table_v1(&hostile)
                .expect_err("a missing row must refuse")
                .to_string();
            assert!(
                error.contains(label) && error.contains("missing account label"),
                "{label}: {error}"
            );
        }
    }

    #[test]
    fn missing_native_composition_is_an_explicit_lifecycle_blocker() {
        let evidence = CampaignTerminalEvidenceV1 {
            plan_sha256: hex(&[1; 32]),
            market_sha256: hex(&[3; 32]),
            founding_custody_context: hex(&[2; 32]),
            direct_selected_manifest_entry_index: 0,
            accounts: BTreeMap::new(),
            checkpoint_direct_capability_root: None,
        };
        let error = require_terminal_composition_evidence(&evidence)
            .expect_err("missing composition must refuse");
        assert!(error.to_string().contains("canonical native-composition"));
        for label in TERMINAL_COMPOSITION_LABELS_V1 {
            assert!(error.to_string().contains(label));
        }
    }

    fn account_evidence_for_tests() -> CampaignAccountEvidenceV1 {
        CampaignAccountEvidenceV1 {
            address: Pubkey::new_unique().to_string(),
            owner: Pubkey::new_unique().to_string(),
            lamports: 1,
            executable: false,
            data_len: 1,
            data_sha256: hex(&[5; 32]),
            account_sha256: hex(&[6; 32]),
        }
    }

    fn evidence_with_labels(labels: &[&str]) -> CampaignTerminalEvidenceV1 {
        let mut accounts = BTreeMap::new();
        for label in labels {
            accounts.insert((*label).to_owned(), account_evidence_for_tests());
        }
        CampaignTerminalEvidenceV1 {
            plan_sha256: hex(&[1; 32]),
            market_sha256: hex(&[3; 32]),
            founding_custody_context: hex(&[2; 32]),
            direct_selected_manifest_entry_index: 0,
            accounts,
            checkpoint_direct_capability_root: None,
        }
    }

    /// A terminal reached BEFORE any trade owes no first-use evidence.
    ///
    /// Since f581af6b the Direct exterior creates these permissionlessly on
    /// first use, so nothing at founding creates them. Demanding them anyway
    /// walled every lifecycle that reached terminal without trading, which is
    /// not strictness -- it is requiring a fact that cannot exist yet.
    #[test]
    fn a_terminal_before_trading_owes_no_first_use_evidence() {
        require_direct_first_use_evidence_v1(&evidence_with_labels(&[]))
            .expect("a pre-trade terminal owes nothing");
    }

    /// A terminal WITH trades still owes the whole first-use set.
    #[test]
    fn a_terminal_with_trades_owes_the_whole_first_use_set() {
        require_direct_first_use_evidence_v1(&evidence_with_labels(&DIRECT_FIRST_USE_LABELS_V1))
            .expect("a complete first-use set is admissible");
    }

    /// Half a first-use set is a dropped observation, not a pre-trade terminal.
    ///
    /// This is the strictness worth keeping: both accounts are created by the
    /// same first trade, so exactly one present means the collector saw the
    /// route run and lost the rest.
    #[test]
    fn half_a_first_use_set_is_refused_as_a_dropped_observation() {
        for label in DIRECT_FIRST_USE_LABELS_V1 {
            let refusal = require_direct_first_use_evidence_v1(&evidence_with_labels(&[label]))
                .expect_err("half a set must refuse");
            assert!(
                refusal.to_string().contains("all of them or none"),
                "{label}: {refusal}"
            );
        }
    }

    #[test]
    fn exact_emitted_campaign_report_is_the_only_accepted_terminal_evidence_schema() {
        let market = Pubkey::new_unique();
        let address = market.to_string();
        let owner = Pubkey::new_unique().to_string();
        let exact = serde_json::to_vec(&serde_json::json!({
            "schema": "dclutch-successor-campaign-report-v1",
            "cluster": "devnet",
            "genesis_hash": crate::cluster::DEVNET_GENESIS_HASH,
            "rpc_url": "https://api.devnet.solana.com/",
            "mode": "execute",
            "plan_sha256": hex(&[1; 32]),
            "market_sha256": hex(&[5; 32]),
            "execution": {
                "completed": true,
                "recoveredFinalizedFounding": false,
                "transactions": [],
                "market": {
                    "completed": ["founding", "open"],
                    "founding_custody_context": hex(&[2; 32]),
                    "direct_selected_manifest_entry_index": 0,
                    "accounts": {
                        "founding_market": {
                            "address": address,
                            "owner": owner,
                            "lamports": 1,
                            "executable": false,
                            "data_len": 1,
                            "data_sha256": hex(&[3; 32]),
                            "account_sha256": hex(&[4; 32])
                        }
                    }
                }
            }
        }))
        .expect("exact campaign report bytes");
        let decoded =
            parse_campaign_terminal_evidence_v1(&exact).expect("campaign evidence projection");
        assert_eq!(decoded.plan_sha256, hex(&[1; 32]));
        assert_eq!(decoded.founding_custody_context, hex(&[2; 32]));
        assert_eq!(decoded.direct_selected_manifest_entry_index, 0);
        assert!(decoded.accounts.contains_key("founding_market"));
        authenticate_campaign_market_v1(&decoded, market).expect("same Market");
        assert!(
            authenticate_campaign_market_v1(&decoded, Pubkey::new_unique())
                .expect_err("cross-Market evidence must refuse")
                .to_string()
                .contains("exact founding campaign evidence")
        );

        let mut local: serde_json::Value =
            serde_json::from_slice(&exact).expect("local campaign projection");
        local["cluster"] = serde_json::json!("loopback");
        let fixture_source = Pubkey::new_unique();
        let fixture_owner = Pubkey::new_unique();
        let fixture_mint = Pubkey::new_unique();
        let fixture_signature = Keypair::new()
            .sign_message(b"terminal local fixture")
            .to_string();
        let fixture_account = |address: Pubkey| {
            serde_json::json!({
                "address": address.to_string(),
                "owner": owner.clone(),
                "lamports": 1,
                "executable": false,
                "data_len": 1,
                "data_sha256": hex(&[3; 32]),
                "account_sha256": hex(&[4; 32])
            })
        };
        local["execution"]["localParticipantFixtureLiquidity"] = serde_json::json!({
            "sourceTokenAccount": fixture_source.to_string(),
            "sourceOwner": fixture_owner.to_string(),
            "quantityAtoms": 100_000_000,
            "foundingCollateralAtoms": 1_000_000_000u64,
            "totalSupplyAtoms": 1_100_000_000u64,
            "mint": fixture_mint.to_string(),
            "mintAuthorityRemoved": true,
            "transactionSignature": fixture_signature.clone(),
            "finalizedSlot": 77,
            "computeUnitsConsumed": 88_000
        });
        local["execution"]["transactions"] = serde_json::json!([{
            "label": "create local fixture",
            "signature": fixture_signature,
            "slot": 77,
            "transaction_metadata_available": true,
            "fee_lamports": 5_000,
            "fee_only_balance_change": false,
            "compute_units_consumed": 88_000,
            "error": null,
            "logs": []
        }]);
        local["execution"]["market"]["accounts"]["local_participant_fixture_source"] =
            fixture_account(fixture_source);
        local["execution"]["market"]["accounts"]["collateral_mint"] = fixture_account(fixture_mint);
        crate::campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1(
            &serde_json::to_vec(&local).expect("local campaign bytes"),
            crate::cluster::ExpectedClusterV1::OwnedLoopback,
        )
        .expect("typed owned-loopback campaign evidence");
        assert!(
            parse_campaign_terminal_evidence_v1(
                &serde_json::to_vec(&local).expect("hostile local campaign bytes")
            )
            .is_err(),
            "public parser must continue refusing loopback evidence"
        );
        assert!(
            crate::campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1(
                &exact,
                crate::cluster::ExpectedClusterV1::OwnedLoopback,
            )
            .is_err(),
            "private parser must refuse external devnet evidence"
        );

        for hostile in [
            serde_json::json!({
                "schema": "dclutch-successor-campaign-report-v1",
                "cluster": "loopback",
                "mode": "execute",
                "plan_sha256": hex(&[1; 32]),
                "execution": {"completed": true, "recoveredFinalizedFounding": false, "transactions": [], "market": null}
            }),
            serde_json::json!({
                "schema": "dclutch-successor-campaign-report-v1",
                "cluster": "devnet",
                "mode": "preflight (reads only, enforced)",
                "plan_sha256": hex(&[1; 32]),
                "execution": {"completed": true, "recoveredFinalizedFounding": false, "transactions": [], "market": null}
            }),
        ] {
            assert!(
                parse_campaign_terminal_evidence_v1(
                    &serde_json::to_vec(&hostile).expect("hostile origin evidence bytes")
                )
                .expect_err("non-exterior evidence must refuse")
                .to_string()
                .contains("executed external devnet")
            );
        }

        for hostile_execution in [
            serde_json::json!({
                "completed": true,
                "recoveredFinalizedFounding": true,
                "transactions": [],
                "market": null
            }),
            serde_json::json!({
                "completed": true,
                "recoveredFinalizedFounding": false,
                "transactions": [],
                "market": null
            }),
            serde_json::json!({
                "completed": true,
                "recoveredFinalizedFounding": false,
                "transactions": [{
                    "label": "inexact",
                    "signature": solana_sdk::signature::Signature::default().to_string(),
                    "slot": 1,
                    "transaction_metadata_available": true,
                    "fee_lamports": null,
                    "fee_only_balance_change": null,
                    "compute_units_consumed": null,
                    "error": null,
                    "logs": [],
                    "parallelDtoTruth": true
                }],
                "market": serde_json::from_slice::<Value>(&exact).expect("exact")["execution"]["market"].clone()
            }),
        ] {
            let mut hostile: Value =
                serde_json::from_slice(&exact).expect("exact campaign report value");
            hostile["execution"] = hostile_execution;
            assert!(
                parse_campaign_terminal_evidence_v1(
                    &serde_json::to_vec(&hostile).expect("hostile execution bytes")
                )
                .is_err(),
                "crash recovery, absent Market, and third-schema transactions must refuse"
            );
        }

        let mut pasted_terminal: Value =
            serde_json::from_slice(&exact).expect("exact campaign report value");
        let pasted_row = pasted_terminal
            .pointer("/execution/market/accounts/founding_market")
            .expect("founding Market row")
            .clone();
        pasted_terminal
            .pointer_mut("/execution/market/accounts")
            .and_then(Value::as_object_mut)
            .expect("campaign account map")
            .insert("terminal_record".into(), pasted_row);
        assert!(
            parse_campaign_terminal_evidence_v1(
                &serde_json::to_vec(&pasted_terminal).expect("pasted terminal evidence bytes")
            )
            .expect_err("static terminal row must never override live Core")
            .to_string()
            .contains("live Core terminal_receipt")
        );

        for stale in [
            serde_json::json!({
                "schema": "dclutch-local-successor-run-evidence-v2",
                "plan_sha256": hex(&[1; 32]),
                "accounts": {}
            }),
            serde_json::json!({
                "plan_sha256": hex(&[1; 32]),
                "foundingCustodyContext": hex(&[2; 32]),
                "directSelectedManifestEntryIndex": 0,
                "accounts": {}
            }),
        ] {
            assert!(
                parse_campaign_terminal_evidence_v1(
                    &serde_json::to_vec(&stale).expect("stale evidence bytes")
                )
                .is_err()
            );
        }
    }

    #[test]
    fn direct_retirement_requires_exact_three_selector_evidence_labels() {
        let row = || CampaignAccountEvidenceV1 {
            address: Pubkey::new_unique().to_string(),
            owner: Pubkey::new_unique().to_string(),
            lamports: 1,
            executable: false,
            data_len: 1,
            data_sha256: hex(&[3; 32]),
            account_sha256: hex(&[4; 32]),
        };
        let mut accounts = BTreeMap::new();
        for label in DIRECT_BEGIN_RETIRING_LABELS_V1
            .into_iter()
            .chain(DIRECT_NATIVE_CLOSE_LABELS_V1)
            .chain([
                "direct_program_set_record",
                "direct_execution_config_record",
                "direct_capability_root",
                "direct_trading_funding_ledger",
                "resolution_funding_ledger",
            ])
        {
            accounts.insert(label.into(), row());
        }
        let exact = CampaignTerminalEvidenceV1 {
            plan_sha256: hex(&[1; 32]),
            market_sha256: hex(&[3; 32]),
            founding_custody_context: hex(&[2; 32]),
            direct_selected_manifest_entry_index: 0,
            accounts: accounts.clone(),
            checkpoint_direct_capability_root: None,
        };
        require_direct_retirement_evidence(&exact).expect("exact Direct retirement evidence");
        for label in DIRECT_BEGIN_RETIRING_LABELS_V1 {
            let mut hostile = exact.clone();
            hostile.accounts.remove(label);
            let error = require_direct_retirement_evidence(&hostile)
                .expect_err("missing begin-retiring label must refuse");
            assert!(error.to_string().contains(label));
        }

        // The two funding ledgers are two accounts. Reading the ordinal label
        // `founding_funding_ledger_v2_0` as the Resolution one gives them a
        // single address, which the native-close frame refuses with an
        // undifferentiated `Frame` thirty-eight accounts later. Admission
        // blames the evidence instead.
        let mut aliased = exact.clone();
        let trading = aliased
            .accounts
            .get("direct_trading_funding_ledger")
            .expect("fixture trading ledger")
            .clone();
        let shared = trading.address.clone();
        aliased
            .accounts
            .insert("resolution_funding_ledger".into(), trading);
        let error = require_direct_retirement_evidence(&aliased)
            .expect_err("one address for both funding ledgers must refuse");
        assert!(
            error.to_string().contains(&shared)
                && error
                    .to_string()
                    .contains("funding ledgers the same address"),
            "refusal must name the shared address: {error}"
        );
    }

    #[test]
    fn canonical_decimal_parsers_refuse_aliases() {
        assert_eq!(canonical_u32("0", "index").unwrap(), 0);
        assert!(canonical_u32("00", "index").is_err());
        assert_eq!(canonical_u64("1", "quantity").unwrap(), 1);
        assert!(canonical_u64("0", "quantity").is_err());
        assert!(canonical_u64("01", "quantity").is_err());
    }

    #[test]
    fn begin_retiring_requires_every_claim_supply_to_be_zero() {
        let market_key = Pubkey::new_unique();
        let market = terminal_market(market_key, 1);
        let claims = Pubkey::new_unique();
        let custody_context = [13; 32];
        let zero = claims_aggregate(market, claims, custody_context, &[0, 0, 0]);
        assert!(authenticate_zero_claims(&zero, zero.key, claims, market, custody_context).is_ok());

        let live = claims_aggregate(market, claims, custody_context, &[0, 7, 0]);
        let error = authenticate_zero_claims(&live, live.key, claims, market, custody_context)
            .expect_err("live liability must block BeginRetiring");
        assert!(error.to_string().contains("index 1 is 7"));

        let error = authenticate_zero_claims(&zero, zero.key, claims, market, [14; 32])
            .expect_err("substituted custody context must refuse");
        assert!(error.to_string().contains("custody/generation join"));
    }
}
