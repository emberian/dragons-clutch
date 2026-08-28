//! Read-only production of the wallet terminal-payout input.
//!
//! Persisted campaign evidence supplies routing hints, never state authority.
//! This producer uses those hints to name one bounded `getMultipleAccounts`
//! observation, reauthenticates the complete payout graph through the existing
//! wallet planner, and emits exactly the input that planner already owns.

use std::{collections::BTreeMap, io::Write, path::PathBuf};

use dclutch_claims_svm::liability_basis_state_v2::LiabilityBasisPositionViewV2;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use solana_program::{hash::hashv, pubkey::Pubkey};

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG},
    model::SuccessorPlan,
    plan::{hex, hex32, pubkey},
    rpc::{Rpc, WritePolicyV1},
    wallet_terminal::{
        FinalizedSnapshotV1, INPUT_FORMAT, LookupTableRequirementV1, PlanInputV1,
        ProgramSelectorsV1, RecordSelectorsV1, SelectedInputV1, build_report,
    },
};

const PARENT_CONTEXT_DOMAIN_V1: &[u8] = b"dclutch/wallet-terminal-parent-context/v1";

const TERMINAL_COMPOSITION_LABELS_V1: [&str; 5] = [
    "terminal_execution_descriptor_record",
    "terminal_composition_descriptor_record",
    "terminal_composition_graph_record",
    "terminal_composition_translation_record",
    "terminal_composition_exposure_record",
];

#[derive(Debug, Deserialize)]
struct PayoutEvidenceV1 {
    plan_sha256: String,
    #[serde(rename = "foundingCustodyContext")]
    founding_custody_context: String,
    accounts: BTreeMap<String, PayoutAccountEvidenceV1>,
}

#[derive(Debug, Deserialize)]
struct PayoutAccountEvidenceV1 {
    address: String,
    owner: String,
    data_sha256: String,
}

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
    let arguments = parse_arguments(arguments)?;
    let plan_source = std::fs::read(&arguments.plan)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let evidence: PayoutEvidenceV1 = serde_json::from_slice(&std::fs::read(&arguments.evidence)?)?;
    authenticate_plan_source(&plan_source, &evidence.plan_sha256)?;
    require_terminal_composition_evidence(&evidence)?;

    let mut input = routed_input(&plan, &evidence, &arguments)?;
    let routed = SelectedInputV1::parse(&input, LookupTableRequirementV1::Absent)?;
    authenticate_routing_hints(&routed, &evidence)?;

    let addresses = routed.addresses();
    let mut rpc = Rpc::connect_cluster(&arguments.origin, WritePolicyV1::ReadsOnly)?;
    let floor = rpc.finalized_slot()?;
    let (slot, values) = rpc.finalized_accounts(&addresses, floor)?;
    let snapshot = FinalizedSnapshotV1::from_rpc(slot, rpc.block_time(slot)?, &addresses, values)?;

    let position_account = snapshot.required(routed.position, "Claims Position")?;
    let position = LiabilityBasisPositionViewV2::decode(&position_account.data)
        .map_err(|error| Error::new(format!("Claims Position: {error:?}")))?;
    let full_balance = position
        .balance(&position_account.data, arguments.claim_index)
        .map_err(|error| Error::new(format!("Claims Position balance: {error:?}")))?;
    let quantity = arguments.quantity.unwrap_or(full_balance);
    if quantity == 0 || quantity > full_balance {
        return Err(Error::new(format!(
            "payout quantity must be within 1..={full_balance} atoms at claim index {}",
            arguments.claim_index
        )));
    }
    input.quantity = quantity.to_string();
    input.parent_context = hex(&stable_parent_context_v1(
        &routed,
        &snapshot,
        quantity,
        arguments.claim_index,
    )?);

    let selected = SelectedInputV1::parse(&input, LookupTableRequirementV1::Absent)?;
    if selected.addresses() != addresses {
        return Err(Error::new(
            "wallet payout selectors changed after authenticated quantity/context construction",
        ));
    }
    let _authenticated = build_report(&selected, &snapshot)?;
    eprintln!(
        "wallet-terminal-payout-input: authenticated one finalized snapshot at slot {}",
        snapshot.observation.slot
    );
    stdout_json(&input)
}

fn routed_input(
    plan: &SuccessorPlan,
    evidence: &PayoutEvidenceV1,
    arguments: &ArgumentsV1,
) -> Result<PlanInputV1> {
    let record_digest = |label: &str| -> Result<String> {
        Ok(required_account(evidence, label)?.data_sha256.clone())
    };
    let mint = required_account(evidence, "collateral_mint")?;
    Ok(PlanInputV1 {
        format: INPUT_FORMAT.into(),
        market: arguments.market.to_string(),
        owner: arguments.owner.to_string(),
        recipient_owner: arguments.owner.to_string(),
        recipient: arguments.recipient.to_string(),
        collateral_mint: mint.address.clone(),
        token_program: mint.owner.clone(),
        // Quantity and parent context do not select accounts. They are filled
        // from the authenticated snapshot before this input is emitted.
        quantity: "1".into(),
        claim_index: arguments.claim_index,
        transfer_index: 0,
        parent_context: hex(&[1; 32]),
        custody_context: evidence.founding_custody_context.clone(),
        release_set: plan.release_set_id.clone(),
        lookup_table: None,
        programs: ProgramSelectorsV1 {
            registry: plan.registry.program_id.clone(),
            core: plan.core.program_id.clone(),
            claims: plan.claims.program_id.clone(),
            custody: plan.custody.program_id.clone(),
        },
        records: RecordSelectorsV1 {
            realm: record_digest("realm_record")?,
            product: record_digest("product_record")?,
            result_domain: record_digest("result_domain_record")?,
            portfolio: record_digest("portfolio_record")?,
            product_basis: record_digest("linked_liability_basis_record")?,
            execution_descriptor: record_digest(TERMINAL_COMPOSITION_LABELS_V1[0])?,
            composition_descriptor: record_digest(TERMINAL_COMPOSITION_LABELS_V1[1])?,
            composition_graph: record_digest(TERMINAL_COMPOSITION_LABELS_V1[2])?,
            composition_translation: record_digest(TERMINAL_COMPOSITION_LABELS_V1[3])?,
            composition_exposure: record_digest(TERMINAL_COMPOSITION_LABELS_V1[4])?,
            terminal_record: record_digest("terminal_record")?,
        },
    })
}

fn authenticate_routing_hints(
    selected: &SelectedInputV1,
    evidence: &PayoutEvidenceV1,
) -> Result<()> {
    let expected = [
        ("realm_record", selected.realm.raw),
        ("product_record", selected.product.raw),
        ("result_domain_record", selected.result_domain.raw),
        ("portfolio_record", selected.portfolio.raw),
        ("linked_liability_basis_record", selected.product_basis.raw),
        (
            TERMINAL_COMPOSITION_LABELS_V1[0],
            selected.execution_descriptor.raw,
        ),
        (
            TERMINAL_COMPOSITION_LABELS_V1[1],
            selected.composition_descriptor.raw,
        ),
        (
            TERMINAL_COMPOSITION_LABELS_V1[2],
            selected.composition_graph.raw,
        ),
        (
            TERMINAL_COMPOSITION_LABELS_V1[3],
            selected.composition_translation.raw,
        ),
        (
            TERMINAL_COMPOSITION_LABELS_V1[4],
            selected.composition_exposure.raw,
        ),
        ("terminal_record", selected.terminal_coordinate.raw),
    ];
    for (label, derived) in expected {
        let persisted = pubkey(&required_account(evidence, label)?.address)?;
        if persisted != derived {
            return Err(Error::new(format!(
                "persisted {label} address {persisted} is not the canonical raw-record PDA {derived}"
            )));
        }
    }
    Ok(())
}

fn stable_parent_context_v1(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    quantity: u64,
    claim_index: u32,
) -> Result<[u8; 32]> {
    let market = snapshot.required(selected.market, "Core Market")?;
    let aggregate = snapshot.required(selected.aggregate, "Claims aggregate")?;
    let position = snapshot.required(selected.position, "Claims Position")?;
    let replay = snapshot.required(selected.custody_replay, "Claims Custody replay")?;
    let hoard = snapshot.required(selected.hoard, "Hoard token account")?;
    let recipient = snapshot.required(selected.recipient, "recipient token account")?;
    let market_digest = Sha256::digest(&market.data);
    let aggregate_digest = Sha256::digest(&aggregate.data);
    let position_digest = Sha256::digest(&position.data);
    let replay_digest = Sha256::digest(&replay.data);
    let hoard_digest = Sha256::digest(&hoard.data);
    let recipient_digest = Sha256::digest(&recipient.data);
    let quantity_bytes = quantity.to_le_bytes();
    let claim_index_bytes = claim_index.to_le_bytes();
    let transfer_index_bytes = 0_u16.to_le_bytes();
    let context = hashv(&[
        PARENT_CONTEXT_DOMAIN_V1,
        selected.market.as_ref(),
        selected.owner.as_ref(),
        selected.position.as_ref(),
        selected.recipient.as_ref(),
        &quantity_bytes,
        &claim_index_bytes,
        &transfer_index_bytes,
        &selected.release_set,
        &selected.terminal_record_digest,
        &market_digest,
        &aggregate_digest,
        &position_digest,
        &replay_digest,
        &hoard_digest,
        &recipient_digest,
    ])
    .to_bytes();
    if context == [0; 32] {
        return Err(Error::new("derived wallet payout parent context was zero"));
    }
    Ok(context)
}

fn authenticate_plan_source(source: &[u8], expected: &str) -> Result<()> {
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

fn require_terminal_composition_evidence(evidence: &PayoutEvidenceV1) -> Result<()> {
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

fn required_account<'a>(
    evidence: &'a PayoutEvidenceV1,
    label: &str,
) -> Result<&'a PayoutAccountEvidenceV1> {
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
     campaign and terminal-publication evidence only to route one finalized account snapshot, \
     reauthenticates the complete payout graph, derives a crash-stable parent context from the \
     immutable request and authenticated prestate (never the observation slot), and emits the \
     exact dclutch-wallet-terminal-payout-plan-input-v1 accepted by the existing ALT planner. \
     Missing canonical native-composition publication evidence is a hard lifecycle blocker. \
     Mainnet-beta is refused unconditionally."
}

#[cfg(test)]
mod tests {
    use dclutch_operator::{Finality, Observation, ObservedAccount};
    use solana_sdk_ids::system_program;

    use super::*;

    fn observed(key: Pubkey, tag: u8, slot: u64) -> ObservedAccount {
        ObservedAccount {
            observation: Observation {
                slot,
                unix_timestamp: 1_700_000_000,
                finality: Finality::Finalized,
            },
            key,
            owner: system_program::ID,
            lamports: 1,
            executable: false,
            data: vec![tag; 32],
        }
    }

    fn context_fixture(slot: u64) -> (SelectedInputV1, FinalizedSnapshotV1) {
        let mut value = super::super::wallet_terminal::tests::input();
        value.lookup_table = None;
        let selected = SelectedInputV1::parse(&value, LookupTableRequirementV1::Absent)
            .expect("selected input");
        let keys = [
            selected.market,
            selected.aggregate,
            selected.position,
            selected.custody_replay,
            selected.hoard,
            selected.recipient,
        ];
        let accounts = keys
            .into_iter()
            .enumerate()
            .map(|(index, key)| (key, observed(key, u8::try_from(index + 1).unwrap(), slot)))
            .collect();
        (
            selected,
            FinalizedSnapshotV1 {
                observation: Observation {
                    slot,
                    unix_timestamp: 1_700_000_000,
                    finality: Finality::Finalized,
                },
                accounts,
            },
        )
    }

    #[test]
    fn retry_context_ignores_observation_slot_but_binds_request_and_prestate() {
        let (selected_a, snapshot_a) = context_fixture(100);
        let (mut selected_b, mut snapshot_b) = context_fixture(200);
        let first = stable_parent_context_v1(&selected_a, &snapshot_a, 7, 1).unwrap();
        let retry = stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap();
        assert_eq!(first, retry, "finalized slot is not caller entropy");

        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 6, 1).unwrap()
        );
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 0).unwrap()
        );
        selected_b.owner = Pubkey::new_unique();
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
        selected_b.owner = selected_a.owner;
        selected_b.terminal_record_digest[0] ^= 1;
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
        selected_b.terminal_record_digest = selected_a.terminal_record_digest;
        snapshot_b
            .accounts
            .get_mut(&selected_b.custody_replay)
            .expect("replay")
            .data[0] ^= 1;
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
        snapshot_b
            .accounts
            .get_mut(&selected_b.custody_replay)
            .expect("replay")
            .data[0] ^= 1;
        selected_b.recipient = Pubkey::new_unique();
        snapshot_b
            .accounts
            .insert(selected_b.recipient, observed(selected_b.recipient, 6, 200));
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
    }

    #[test]
    fn context_refuses_a_missing_authenticated_prestate() {
        let (selected, mut snapshot) = context_fixture(100);
        snapshot.accounts.remove(&selected.custody_replay);
        let error = stable_parent_context_v1(&selected, &snapshot, 7, 1)
            .expect_err("missing replay must refuse");
        assert!(error.to_string().contains("snapshot omitted"));
    }

    #[test]
    fn missing_native_composition_is_an_explicit_lifecycle_blocker() {
        let evidence = PayoutEvidenceV1 {
            plan_sha256: hex(&[1; 32]),
            founding_custody_context: hex(&[2; 32]),
            accounts: BTreeMap::new(),
        };
        let error = require_terminal_composition_evidence(&evidence)
            .expect_err("missing composition must refuse");
        assert!(error.to_string().contains("canonical native-composition"));
        for label in TERMINAL_COMPOSITION_LABELS_V1 {
            assert!(error.to_string().contains(label));
        }
    }

    #[test]
    fn evidence_uses_the_persisted_campaign_field_names() {
        let decoded: PayoutEvidenceV1 = serde_json::from_value(serde_json::json!({
            "plan_sha256": hex(&[1; 32]),
            "foundingCustodyContext": hex(&[2; 32]),
            "accounts": {
                "terminal_execution_descriptor_record": {
                    "address": Pubkey::new_unique().to_string(),
                    "owner": Pubkey::new_unique().to_string(),
                    "data_sha256": hex(&[3; 32]),
                    "ignoredExistingEvidenceField": true
                }
            },
            "completed": []
        }))
        .expect("campaign evidence projection");
        assert_eq!(decoded.plan_sha256, hex(&[1; 32]));
        assert_eq!(decoded.founding_custody_context, hex(&[2; 32]));
    }

    #[test]
    fn canonical_decimal_parsers_refuse_aliases() {
        assert_eq!(canonical_u32("0", "index").unwrap(), 0);
        assert!(canonical_u32("00", "index").is_err());
        assert_eq!(canonical_u64("1", "quantity").unwrap(), 1);
        assert!(canonical_u64("0", "quantity").is_err());
        assert!(canonical_u64("01", "quantity").is_err());
    }
}
