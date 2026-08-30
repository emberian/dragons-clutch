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

use dclutch_claims_svm::liability_basis_state_v2::{
    LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_market_core_codec::CoreState;
use dclutch_operator::ObservedAccount;
use sha2::{Digest, Sha256};
use solana_program::{hash::hashv, pubkey::Pubkey};

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
    wallet_terminal::{
        FinalizedSnapshotV1, INPUT_FORMAT, LookupTableRequirementV1, PlanInputV1,
        ProgramSelectorsV1, RecordPairV1, RecordSelectorsV1, SelectedInputV1, build_report,
        record_pair,
    },
};

const PARENT_CONTEXT_DOMAIN_V1: &[u8] = b"dclutch/wallet-terminal-parent-context/v1";
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CoreTerminalReceiptMeaningV1 {
    /// Every admitted Core writer persists the exact Resolution certificate
    /// account key. Market-family interpretation belongs to the authenticated
    /// certificate body, never to this identity.
    ResolutionCertificate(Pubkey),
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

    authenticate_campaign_market_v1(&evidence, arguments.market)?;
    let mut rpc = Rpc::connect_cluster(&arguments.origin, WritePolicyV1::ReadsOnly)?;
    let receipt_snapshot = finalized_snapshot(&mut rpc, &[arguments.market])?;
    let live_market = decode_routed_market(
        receipt_snapshot.required(arguments.market, "Core Market")?,
        pubkey(&plan.core.program_id)?,
        &plan,
    )?;
    let terminal_receipt = live_market
        .terminal_receipt
        .ok_or_else(|| Error::new("Core Market has no accepted terminal receipt"))?
        .to_bytes();

    let mut input = routed_input(&plan, &evidence, &arguments, terminal_receipt)?;
    let routed = SelectedInputV1::parse(&input, LookupTableRequirementV1::Absent)?;
    authenticate_routing_hints(&routed, &evidence)?;

    let addresses = routed.addresses();
    let floor = receipt_snapshot.observation.slot;
    let (slot, values) = rpc.finalized_accounts(&addresses, floor)?;
    let snapshot = FinalizedSnapshotV1::from_rpc(slot, rpc.block_time(slot)?, &addresses, values)?;
    let receipt_meaning = authenticate_core_terminal_receipt_meaning_v1(&routed, &snapshot)?;

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
    let receipt_label = match receipt_meaning {
        CoreTerminalReceiptMeaningV1::ResolutionCertificate(_) => "Resolution certificate",
    };
    eprintln!(
        "wallet-terminal-payout-input: authenticated one finalized snapshot at slot {} with live Core {}",
        snapshot.observation.slot, receipt_label,
    );
    Ok(input)
}

fn authenticate_core_terminal_receipt_meaning_v1(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
) -> Result<CoreTerminalReceiptMeaningV1> {
    let market = CoreState::decode(&snapshot.required(selected.market, "Core Market")?.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    let receipt = market
        .terminal_receipt
        .ok_or_else(|| Error::new("Core Market has no accepted terminal receipt"))?
        .to_bytes();
    if receipt != selected.terminal_certificate.to_bytes() {
        return Err(Error::new(
            "live Core terminal receipt differs from the projected payout identity",
        ));
    }
    Ok(CoreTerminalReceiptMeaningV1::ResolutionCertificate(
        Pubkey::new_from_array(receipt),
    ))
}

pub(crate) fn decode_routed_market(
    account: &ObservedAccount,
    core: Pubkey,
    plan: &SuccessorPlan,
) -> Result<CoreState> {
    let market = CoreState::decode(&account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    if account.owner != core
        || account.executable
        || account.key.to_bytes() != market.identity.market_id.to_bytes()
        || market.identity.registry_program.to_bytes()
            != pubkey(&plan.registry.program_id)?.to_bytes()
        || market.identity.selected_release_set.to_bytes() != hex32(&plan.release_set_id)?
    {
        return Err(Error::new(
            "Core Market owner/address/Registry/release-set routing authentication refused",
        ));
    }
    Ok(market)
}

pub(crate) fn authenticate_zero_claims(
    account: &ObservedAccount,
    expected: Pubkey,
    claims: Pubkey,
    market: CoreState,
    custody_context: [u8; 32],
) -> Result<()> {
    let aggregate = LiabilityBasisMarketViewV2::decode(&account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    if account.key != expected
        || account.owner != claims
        || account.executable
        || aggregate.logical_market != market.identity.market_id.to_bytes()
        || aggregate.release_set != market.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != market.identity.registry_program.to_bytes()
        || aggregate.product_instance_id != market.identity.product_id.to_bytes()
        || aggregate.realm_id != market.identity.realm_id.to_bytes()
        || aggregate.custody_context != custody_context
        || aggregate.generation != market.identity.generation
    {
        return Err(Error::new(
            "Claims aggregate address/owner/Market/release/Product/Realm/custody/generation join refused",
        ));
    }
    for claim_index in 0..aggregate.claim_count {
        let supply = aggregate
            .supply(&account.data, claim_index)
            .map_err(|error| Error::new(format!("Claims supply {claim_index}: {error:?}")))?;
        if supply != 0 {
            return Err(Error::new(format!(
                "BeginRetiring is blocked: Claims supply at index {claim_index} is {supply}; produce and execute wallet terminal payouts first"
            )));
        }
    }
    Ok(())
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
    FinalizedSnapshotV1::from_rpc(slot, rpc.block_time(slot)?, &keys, values)
}

fn routed_input(
    plan: &SuccessorPlan,
    evidence: &CampaignTerminalEvidenceV1,
    arguments: &ArgumentsV1,
    terminal_receipt: [u8; 32],
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
        terminal_certificate: Pubkey::new_from_array(terminal_receipt).to_string(),
        lookup_table: None,
        programs: ProgramSelectorsV1 {
            registry: plan.registry.program_id.clone(),
            core: plan.core.program_id.clone(),
            claims: plan.claims.program_id.clone(),
            custody: plan.custody.program_id.clone(),
            resolution: plan.resolution.program_id.clone(),
        },
        records: RecordSelectorsV1 {
            realm: record_digest("realm_record")?,
            product: record_digest("product_record")?,
            result_domain: record_digest("result_domain_record")?,
            portfolio: record_digest("portfolio_record")?,
            product_basis: record_digest("linked_liability_basis_record")?,
            composition_descriptor: record_digest(TERMINAL_COMPOSITION_LABELS_V1[0])?,
            composition_graph: record_digest(TERMINAL_COMPOSITION_LABELS_V1[1])?,
            composition_translation: record_digest(TERMINAL_COMPOSITION_LABELS_V1[2])?,
            composition_exposure: record_digest(TERMINAL_COMPOSITION_LABELS_V1[3])?,
        },
    })
}

fn authenticate_routing_hints(
    selected: &SelectedInputV1,
    evidence: &CampaignTerminalEvidenceV1,
) -> Result<()> {
    let expected = [
        ("realm_record", selected.realm.raw),
        ("product_record", selected.product.raw),
        ("result_domain_record", selected.result_domain.raw),
        ("portfolio_record", selected.portfolio.raw),
        ("linked_liability_basis_record", selected.product_basis.raw),
        (
            TERMINAL_COMPOSITION_LABELS_V1[0],
            selected.composition_descriptor.raw,
        ),
        (
            TERMINAL_COMPOSITION_LABELS_V1[1],
            selected.composition_graph.raw,
        ),
        (
            TERMINAL_COMPOSITION_LABELS_V1[2],
            selected.composition_translation.raw,
        ),
        (
            TERMINAL_COMPOSITION_LABELS_V1[3],
            selected.composition_exposure.raw,
        ),
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
        selected.terminal_certificate.as_ref(),
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
        .chain(["direct_program_set_record", "direct_execution_config_record"])
    {
        required_account(evidence, label).map_err(|_| {
            Error::new(format!(
                "terminal sequence is blocked: campaign evidence omitted exact Direct lifecycle label {label}"
            ))
        })?;
    }
    require_direct_first_use_evidence_v1(evidence)
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
    use dclutch_market_core_codec::{Identity, MarketIdentity, Phase, Readiness};
    use dclutch_operator::{Finality, Observation, ObservedAccount};
    use serde_json::Value;
    use solana_sdk::signature::{Keypair, Signer as _};
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
        let mut substituted_certificate = selected_b.terminal_certificate.to_bytes();
        substituted_certificate[0] ^= 1;
        selected_b.terminal_certificate = Pubkey::new_from_array(substituted_certificate);
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
        selected_b.terminal_certificate = selected_a.terminal_certificate;
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
    fn live_core_receipt_has_one_certificate_account_meaning() {
        let receipt = [23; 32];
        assert_eq!(
            CoreTerminalReceiptMeaningV1::ResolutionCertificate(Pubkey::new_from_array(receipt)),
            CoreTerminalReceiptMeaningV1::ResolutionCertificate(Pubkey::new_from_array(receipt))
        );
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
