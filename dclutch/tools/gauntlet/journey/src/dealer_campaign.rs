//! A nonzero scoring Dealer campaign over the shared checked validator substrate.
//! Every protocol act calls a shipped operator. No protocol account is seeded.

use std::{collections::BTreeMap, path::Path};

use dclutch_claims::liability_basis_state_v2::{
    LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_custody::token_svm::TokenAccount;
use serde_json::{Value, json};
use solana_sdk::{pubkey::Pubkey, signature::Signature, signer::Signer};

use crate::{Error, Result, journey::JourneyRequestV1, model::TransactionEvidence, rpc::Rpc};

struct Progress {
    stage: String,
    stages: Vec<Value>,
    transactions: Vec<TransactionEvidence>,
    accounts: BTreeMap<String, crate::model::AccountEvidence>,
    rpc_url: String,
}

pub(crate) fn execute(request: JourneyRequestV1) -> Result<()> {
    std::fs::create_dir_all(&request.work)?;
    let mut progress = Progress {
        stage: "checked substrate".into(),
        stages: Vec::new(),
        transactions: Vec::new(),
        accounts: BTreeMap::new(),
        rpc_url: String::new(),
    };
    let result = campaign(&request, &mut progress);
    // A failure preserves the successful prefix and the exact stage that stopped.
    let transcript = json!({
        "schema": "dclutch-scoring-dealer-campaign-v1",
        "evidenceLevel": "local-validator",
        "releaseSourceRevision": request.expected_source_revision,
        "checkedReleaseGateSha256": request.expected_gate_sha256,
        "completed": result.is_ok(),
        "wall": result.as_ref().err().map(|error| json!({"stage": progress.stage, "sentence": error.to_string()})),
        "stages": progress.stages,
    });
    write_json(&request.transcript, &transcript)?;
    write_json(
        &request.work.join("evidence.json"),
        &json!({
            "schema": "dclutch-local-successor-run-evidence-v2",
            "rpc_url": progress.rpc_url,
            "transactions": progress.transactions,
            "accounts": progress.accounts,
        }),
    )?;
    result
}

fn campaign(request: &JourneyRequestV1, progress: &mut Progress) -> Result<()> {
    let substrate_dir = request.work.join("substrate");
    std::fs::create_dir_all(&substrate_dir)?;
    let checked = crate::substrate::bring_up(&crate::substrate::SubstrateRequestV1 {
        work: &substrate_dir,
        checked_release_gate: &request.checked_release_gate,
        expected_gate_sha256: &request.expected_gate_sha256,
        expected_source_revision: &request.expected_source_revision,
        expected_source_tree_sha256: &request.expected_source_tree_sha256,
        seed: &request.seed,
        rpc_port: request.rpc_port,
    })?;
    progress.rpc_url = checked.rpc_url.clone();
    progress
        .stages
        .push(json!({"stage": progress.stage, "outcome": "executed"}));
    progress.stage = "compile and found through Open".into();
    let registry = crate::plan::pubkey(&checked.plan.registry.program_id)?;
    let fee_recipient = solana_sdk::signature::Keypair::new();
    let direct = crate::direct_market::DirectMarketCompilerOwnedV1::load_local(
        &checked.plan_path,
        &checked.rpc_url,
        registry,
        Some(50),
        Some(fee_recipient.pubkey()),
    )?;
    let shape = crate::market::LocalMarketShapeV1 {
        recovery: Some(crate::local_mutable::parse_recovery_rungs_v1("2500:120")?),
        terminal_max_age_seconds: Some(crate::failure::FAILURE_WALK_MAX_AGE_SECONDS_V1),
        ..crate::market::LocalMarketShapeV1::default()
    };
    let market_input =
        crate::market::demo_market_input_shaped(registry, direct.compiler(), &shape)?;
    let market_path = request.work.join("market.json");
    write_json(&market_path, &serde_json::to_value(&market_input)?)?;
    let founding_path = request.work.join("founding-evidence.json");
    let mut rpc = Rpc::connect(&checked.rpc_url)?;
    let founding =
        crate::substrate::found_market(&checked, &mut rpc, &market_path, &founding_path)?;
    progress.transactions = founding.transactions;
    progress.accounts = founding.market.accounts;
    let addresses = crate::stages::MarketAddressesV1::from_evidence(&progress.accounts)?;
    progress.stages.push(json!({"stage": progress.stage, "outcome": "executed", "market": addresses.founding_market.to_string()}));

    // Sponsor and taker are separate ordinary wallets. The sponsor uses its
    // remaining founding collateral; admission transfers a bounded share to the taker.
    let sponsor = crate::substrate::campaign_payer_keypair(&checked)?;
    let sponsor_key = checked
        .report
        .campaign_founding_keypairs
        .get("campaign-payer")
        .ok_or_else(|| Error::new("prepare omitted campaign payer key"))?;
    let taker_key = checked
        .report
        .keypairs
        .get("participant")
        .ok_or_else(|| Error::new("prepare omitted participant key"))?;
    let taker = crate::substrate::load_keypair(Path::new(taker_key))?;
    progress.transactions.push(rpc.airdrop(
        "Dealer campaign: fund disposable taker",
        taker.pubkey(),
        2_000_000_000,
    )?);
    let routing = crate::market::recover_frozen_routing_table_v1(
        &mut rpc,
        &progress.transactions,
        addresses.founding_market,
        sponsor.pubkey(),
    )?
    .ok_or_else(|| Error::new("founding omitted its frozen routing table"))?;
    let admission = request.work.join("admission.json");
    progress.stage = "admit and fund taker".into();
    let context = crate::spine::SpineContextV1 {
        rpc_url: &checked.rpc_url,
        plan: &checked.plan_path,
        campaign_report: &founding_path,
        market_input: &market_path,
        market: addresses.founding_market,
        work: &request.work,
        keypairs: &checked.report.keypairs,
        founding_keypairs: &checked.report.campaign_founding_keypairs,
    };
    let mut spine = crate::spine::SpineV1::new();
    crate::spine::admit_strangers(
        &mut rpc,
        &context,
        &mut spine,
        &[crate::spine::StrangerV1 {
            label: "dealer taker".into(),
            owner: taker.pubkey(),
            keypair: taker_key.into(),
            report: admission.clone(),
            collateral: Some(crate::spine::StrangerCollateralV1 {
                source_owner: sponsor.pubkey(),
                source_owner_keypair: sponsor_key.into(),
                source_account: addresses.founder_wallet,
                quantity_atoms: 1_000_000,
            }),
        }],
        sponsor.pubkey(),
        Path::new(sponsor_key),
        &[routing],
    )?;
    progress.transactions.extend(spine.transactions);
    if !spine.refusals.is_empty() {
        return Err(Error::new(spine.refusals.join("; ")));
    }
    let admitted: Value = serde_json::from_slice(&std::fs::read(&admission)?)?;
    let participant =
        crate::user_position_admission::parse_finalized_direct_participant_evidence_v1(
            &std::fs::read(&admission)?,
            &mut rpc,
        )?;
    let taker_token = participant.collateral_account;
    progress
        .stages
        .push(json!({"stage": progress.stage, "outcome": "executed", "report": admitted}));

    let dealer_id = crate::plan::hex(
        &solana_program::hash::hash(b"dclutch/scoring-dealer/local-validator-campaign/v1")
            .to_bytes(),
    );
    let common = vec![
        "--rpc-url".into(),
        checked.rpc_url.clone(),
        "--market".into(),
        addresses.founding_market.to_string(),
        "--campaign-report".into(),
        founding_path.display().to_string(),
        "--dealer-id".into(),
        dealer_id,
    ];
    let sponsor_args = vec![
        "--sponsor".into(),
        sponsor.pubkey().to_string(),
        "--sponsor-token".into(),
        addresses.founder_wallet.to_string(),
        "--sponsor-keypair".into(),
        sponsor_key.clone(),
    ];
    let mut found_args = sponsor_args.clone();
    found_args.extend(strings(&[
        "--liquidity",
        "1000",
        "--scale",
        "1000000",
        "--tolerance",
        "1000",
        "--deposit",
        "3000000",
    ]));
    let found = drive(
        progress,
        &mut rpc,
        &request.work,
        "dealer-found",
        &common,
        found_args,
        crate::scoring_dealer::run_found_owned_loopback_v1,
    )?;
    let vault = pointer_key(&found, "/facts/vault")?;
    let dealer_position = pointer_key(&found, "/dealerPosition")?;
    if token_balance(&mut rpc, vault)? != 3_000_000 {
        return Err(Error::new(
            "Dealer founding did not deposit exactly 3000000 atoms",
        ));
    }
    drive(
        progress,
        &mut rpc,
        &request.work,
        "dealer-quote",
        &common,
        vec![
            "--fee-payer".into(),
            sponsor.pubkey().to_string(),
            "--fee-payer-keypair".into(),
            sponsor_key.clone(),
        ],
        crate::scoring_dealer::run_quote_owned_loopback_v1,
    )?;
    let before = snapshot(
        &mut rpc,
        &addresses,
        vault,
        dealer_position,
        taker_token,
        participant.position,
    )?;
    let fill = drive(
        progress,
        &mut rpc,
        &request.work,
        "dealer-fill",
        &common,
        vec![
            "--taker".into(),
            taker.pubkey().to_string(),
            "--taker-token".into(),
            taker_token.to_string(),
            "--taker-keypair".into(),
            taker_key.clone(),
            "--buy-outcome".into(),
            "0".into(),
            "--buy-claims".into(),
            "9".into(),
        ],
        crate::scoring_dealer::run_fill_owned_loopback_v1,
    )?;
    let after = snapshot(
        &mut rpc,
        &addresses,
        vault,
        dealer_position,
        taker_token,
        participant.position,
    )?;
    require_fill(&before, &after, &fill)?;
    progress.stages.push(json!({"stage": "nonzero fill poststates", "outcome": "executed", "before": before, "after": after}));
    let before_withdraw = token_balance(&mut rpc, addresses.founder_wallet)?;
    let before_vault = token_balance(&mut rpc, vault)?;
    let mut withdraw_args = sponsor_args.clone();
    withdraw_args.extend(strings(&["--amount", "3"]));
    drive(
        progress,
        &mut rpc,
        &request.work,
        "dealer-withdraw",
        &common,
        withdraw_args,
        crate::scoring_dealer::run_withdraw_owned_loopback_v1,
    )?;
    if token_balance(&mut rpc, addresses.founder_wallet)?
        != before_withdraw
            .checked_add(3)
            .ok_or_else(|| Error::new("withdraw balance overflow"))?
        || token_balance(&mut rpc, vault)?.checked_add(3) != Some(before_vault)
    {
        return Err(Error::new(
            "Dealer withdrawal did not transfer exactly three atoms",
        ));
    }
    progress.stage = "resolve the unanswered market".into();
    let (failure_report, _, failure_detail, _) = crate::failure::walk_to_failure(
        &crate::failure::FailureWalkContextV1 {
            rpc_url: &checked.rpc_url,
            plan: &checked.plan_path,
            campaign_report: &founding_path,
            market: addresses.founding_market,
            work: &request.work,
            worker: sponsor.pubkey(),
            worker_keypair: Path::new(sponsor_key),
        },
        &mut progress.transactions,
    )?;
    progress.stages.push(json!({"stage":progress.stage,"outcome":"executed","report":failure_report,"detail":failure_detail}));
    progress.stage = "admit the failure terminal".into();
    let terminal = crate::admit_terminal::run_v1(
        vec![
            "--rpc-url".into(),
            checked.rpc_url.clone(),
            "--plan".into(),
            checked.plan_path.display().to_string(),
            "--evidence".into(),
            founding_path.display().to_string(),
            "--market".into(),
            addresses.founding_market.to_string(),
            "--terminal-sequence".into(),
            "1".into(),
            "--fee-payer".into(),
            sponsor.pubkey().to_string(),
            "--fee-payer-keypair".into(),
            sponsor_key.clone(),
            "--output".into(),
            request
                .work
                .join("admit-terminal.json")
                .display()
                .to_string(),
            "--execute".into(),
        ],
        crate::cluster::ExpectedClusterV1::OwnedLoopback,
    )?;
    progress.transactions.extend(terminal.transactions);
    progress.stages.push(json!({"stage":progress.stage,"outcome":"executed","certificate":terminal.certificate.to_string()}));

    let pre_redeem = snapshot(
        &mut rpc,
        &addresses,
        vault,
        dealer_position,
        taker_token,
        participant.position,
    )?;
    let inventory: Vec<u64> = serde_json::from_value(pre_redeem["inventory"].clone())?;
    let mut payout = 0u64;
    for (index, quantity) in inventory.iter().copied().enumerate() {
        if quantity == 0 {
            continue;
        }
        let report = drive(
            progress,
            &mut rpc,
            &request.work,
            &format!("dealer-redeem-{index}"),
            &common,
            vec![
                "--fee-payer".into(),
                sponsor.pubkey().to_string(),
                "--fee-payer-keypair".into(),
                sponsor_key.clone(),
                "--claim-index".into(),
                index.to_string(),
            ],
            crate::scoring_dealer::run_redeem_owned_loopback_v1,
        )?;
        payout = payout
            .checked_add(
                report["facts"]["payoutAtoms"]
                    .as_u64()
                    .ok_or_else(|| Error::new("redemption omitted payout atoms"))?,
            )
            .ok_or_else(|| Error::new("redemption sum overflow"))?;
    }
    let post_redeem = snapshot(
        &mut rpc,
        &addresses,
        vault,
        dealer_position,
        taker_token,
        participant.position,
    )?;
    if payout == 0
        || post_redeem["inventory"]
            .as_array()
            .ok_or_else(|| Error::new("missing inventory"))?
            .iter()
            .any(|value| value.as_u64() != Some(0))
        || post_redeem["vault"]
            .as_u64()
            .and_then(|v| v.checked_sub(pre_redeem["vault"].as_u64()?))
            != Some(payout)
        || pre_redeem["hoard"]
            .as_u64()
            .and_then(|v| v.checked_sub(post_redeem["hoard"].as_u64()?))
            != Some(payout)
    {
        return Err(Error::new(
            "terminal Dealer redemption did not exhaust inventory and return exact positive hoard capital",
        ));
    }
    progress.stages.push(json!({"stage":"terminal redemption poststates","outcome":"executed","before":pre_redeem,"after":post_redeem,"payoutAtoms":payout}));
    let remaining = token_balance(&mut rpc, vault)?;
    let sponsor_before_exit = token_balance(&mut rpc, addresses.founder_wallet)?;
    let mut exit_args = sponsor_args;
    exit_args.extend(["--amount".into(), remaining.to_string()]);
    drive(
        progress,
        &mut rpc,
        &request.work,
        "dealer-capital-exit",
        &common,
        exit_args,
        crate::scoring_dealer::run_withdraw_owned_loopback_v1,
    )?;
    if token_balance(&mut rpc, vault)? != 0
        || token_balance(&mut rpc, addresses.founder_wallet)?.checked_sub(sponsor_before_exit)
            != Some(remaining)
    {
        return Err(Error::new(
            "terminal capital exit did not empty the vault into the sponsor wallet exactly",
        ));
    }
    progress.stages.push(json!({"stage":"complete sponsor collateral exit","outcome":"executed","withdrawnAtoms":remaining,"vaultAtoms":0}));
    drive(
        progress,
        &mut rpc,
        &request.work,
        "dealer-close",
        &common,
        vec![
            "--fee-payer".into(),
            taker.pubkey().to_string(),
            "--fee-payer-keypair".into(),
            taker_key.clone(),
        ],
        crate::scoring_dealer::run_close_campaign_v1,
    )?;

    Ok(())
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

fn drive(
    progress: &mut Progress,
    rpc: &mut Rpc,
    work: &Path,
    stage: &str,
    common: &[String],
    extra: Vec<String>,
    command: fn(Vec<String>) -> Result<()>,
) -> Result<Value> {
    progress.stage = stage.into();
    let output = work.join(format!("{stage}.json"));
    let mut arguments = common.to_vec();
    arguments.extend(extra);
    arguments.extend([
        "--evidence".into(),
        output.display().to_string(),
        "--execute".into(),
    ]);
    command(arguments)?;
    let document: Value = serde_json::from_slice(&std::fs::read(output)?)?;
    let signature = document
        .pointer("/landed/signature")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("{stage} returned without a landed signature")))?
        .parse::<Signature>()
        .map_err(|error| Error::new(format!("{stage} signature: {error}")))?;
    let landed = rpc
        .finalized_signed_packet(stage, signature, false)?
        .ok_or_else(|| Error::new(format!("{stage} finalized packet is absent")))?;
    if landed.evidence.error.is_some() {
        return Err(Error::new(format!("{stage} finalized transaction refused")));
    }
    progress.transactions.push(landed.evidence);
    progress
        .stages
        .push(json!({"stage": stage, "outcome": "executed", "report": document}));
    Ok(document)
}

fn pointer_key(document: &Value, pointer: &str) -> Result<Pubkey> {
    crate::plan::pubkey(
        document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new(format!("driver evidence has no address at {pointer}")))?,
    )
}

fn token_balance(rpc: &mut Rpc, key: Pubkey) -> Result<u64> {
    let account = rpc.required_account(key, "campaign token account")?;
    TokenAccount::parse(&account.data)
        .map(|value| value.amount)
        .map_err(|error| Error::new(format!("token account: {error:?}")))
}

fn snapshot(
    rpc: &mut Rpc,
    addresses: &crate::stages::MarketAddressesV1,
    vault: Pubkey,
    dealer_position: Pubkey,
    taker_token: Pubkey,
    taker_position: Pubkey,
) -> Result<Value> {
    let aggregate = rpc.required_account(addresses.aggregate, "aggregate")?;
    let aggregate_view = LiabilityBasisMarketViewV2::decode(&aggregate.data)
        .map_err(|error| Error::new(format!("aggregate: {error:?}")))?;
    let position = rpc.required_account(dealer_position, "Dealer Position")?;
    let position_view = LiabilityBasisPositionViewV2::decode(&position.data)
        .map_err(|error| Error::new(format!("Dealer Position: {error:?}")))?;
    let supply = (0..aggregate_view.claim_count)
        .map(|i| aggregate_view.supply(&aggregate.data, i))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| Error::new(format!("supply: {error:?}")))?;
    let inventory = (0..position_view.claim_count)
        .map(|i| position_view.balance(&position.data, i))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| Error::new(format!("inventory: {error:?}")))?;
    let taker = rpc.required_account(taker_position, "taker Position")?;
    let taker_view = LiabilityBasisPositionViewV2::decode(&taker.data)
        .map_err(|error| Error::new(format!("taker Position: {error:?}")))?;
    let taker_claims = (0..taker_view.claim_count)
        .map(|i| taker_view.balance(&taker.data, i))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| Error::new(format!("taker balances: {error:?}")))?;
    Ok(
        json!({"hoard": token_balance(rpc, addresses.hoard)?, "vault": token_balance(rpc, vault)?,
        "takerToken": token_balance(rpc, taker_token)?, "supply": supply, "inventory": inventory, "takerClaims": taker_claims}),
    )
}

fn require_fill(before: &Value, after: &Value, report: &Value) -> Result<()> {
    let n = |doc: &Value, pointer: &str| {
        doc.pointer(pointer)
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::new(format!("missing integer {pointer}")))
    };
    let minted = n(report, "/facts/mintAtoms")?;
    if minted == 0 || n(after, "/hoard")?.checked_sub(n(before, "/hoard")?) != Some(minted) {
        return Err(Error::new(
            "nonzero Dealer fill did not back its minted claims with exact par",
        ));
    }
    let total = |doc: &Value| -> Result<u128> {
        Ok(u128::from(n(doc, "/hoard")?)
            + u128::from(n(doc, "/vault")?)
            + u128::from(n(doc, "/takerToken")?))
    };
    if total(before)? != total(after)? {
        return Err(Error::new("Dealer fill did not conserve collateral atoms"));
    }
    if before.get("inventory") == after.get("inventory")
        || before.get("supply") == after.get("supply")
    {
        return Err(Error::new(
            "nonzero Dealer fill left its Claims child unchanged",
        ));
    }
    let array = |doc: &Value, key: &str| -> Result<Vec<u64>> {
        serde_json::from_value(
            doc.get(key)
                .cloned()
                .ok_or_else(|| Error::new(format!("missing {key}")))?,
        )
        .map_err(|error| Error::new(format!("{key}: {error}")))
    };
    let old = array(before, "takerClaims")?;
    let new = array(after, "takerClaims")?;
    let outcome = usize::try_from(n(report, "/facts/buyOutcome")?)
        .map_err(|_| Error::new("outcome exceeds host width"))?;
    let bought = n(report, "/facts/buyClaimUnits")?;
    if old.len() != new.len()
        || outcome >= old.len()
        || bought == 0
        || old
            .iter()
            .zip(&new)
            .enumerate()
            .any(|(i, (a, b))| a.checked_add(if i == outcome { bought } else { 0 }) != Some(*b))
    {
        return Err(Error::new(
            "Dealer taker did not receive exactly the requested outcome claims",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepted() -> (Value, Value, Value) {
        (
            json!({"hoard":300,"vault":3000,"takerToken":100,"supply":[100,100,100,0],"inventory":[0,0,0,0],"takerClaims":[0,0,0,0]}),
            json!({"hoard":327,"vault":2982,"takerToken":91,"supply":[109,109,109,0],"inventory":[0,9,9,0],"takerClaims":[9,0,0,0]}),
            json!({"facts":{"mintAtoms":27,"buyOutcome":0,"buyClaimUnits":9}}),
        )
    }

    #[test]
    fn accepted_nonzero_poststates_pass() {
        let (before, after, report) = accepted();
        require_fill(&before, &after, &report).unwrap();
    }

    #[test]
    fn a_null_fill_cannot_satisfy_the_campaign() {
        let (before, _, mut report) = accepted();
        report["facts"]["mintAtoms"] = json!(0);
        assert_eq!(
            require_fill(&before, &before, &report)
                .unwrap_err()
                .to_string(),
            "nonzero Dealer fill did not back its minted claims with exact par"
        );
    }

    #[test]
    fn lost_atoms_and_wrong_outcome_are_distinct_failures() {
        let (before, mut after, report) = accepted();
        after["vault"] = json!(2981);
        assert_eq!(
            require_fill(&before, &after, &report)
                .unwrap_err()
                .to_string(),
            "Dealer fill did not conserve collateral atoms"
        );
        after["vault"] = json!(2982);
        after["takerClaims"] = json!([0, 9, 0, 0]);
        assert_eq!(
            require_fill(&before, &after, &report)
                .unwrap_err()
                .to_string(),
            "Dealer taker did not receive exactly the requested outcome claims"
        );
    }
}

fn write_json(path: &Path, document: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(document)?),
    )?;
    Ok(())
}
