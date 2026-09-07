//! The third act of every terminal: Core admits the certificate the Source
//! minted and the Market's phase byte moves.
//!
//! # Why a family-neutral verb
//!
//! `execute_provider_v3`'s own module comment states the contract: a provider
//! execution "invokes the Registry-selected Resolution program, checks its
//! immediate receipt and terminal poststate. A LATER STANDALONE CORE
//! `AdmitTerminal` consumes that durable certificate and commits the Market
//! transition." The sponsored transport drives that act as
//! `devnet-sponsored-push-v1 --action admit-terminal`, the flagship command as
//! `--through accept`, and the journey tier as its own stage -- each over the
//! sponsored family's input document or the tier's own address derivation. A
//! deadline walk (`commit-deadline-failure`) is none of those families: it
//! mints a `ResolutionFailure` certificate on a market that may have bought no
//! sponsored window at all, and the ladder tier that drives it links neither
//! the sponsored input producer nor the journey's ledger. So the act gets the
//! verb it always implied, over the one builder every caller already shares:
//! `build_resolution_admit_terminal_v3`.
//!
//! # What it does not choose
//!
//! The certificate KIND. The terminal Source already decided it -- `Resolved`
//! is a success seat, `FailureCommitted` a failure seat -- and reading it
//! anywhere else would let a caller ask Core to accept a success certificate
//! for a failed walk. The kind is read off the Source's phase and the seat is
//! derived from it; a Source on any other phase is refused by name.
//!
//! # The frame rides a routing table
//!
//! The AdmitTerminal frame is 1,508 bytes against the 1,232-byte legacy
//! ceiling (JOURNEY-8), so an execute publishes one frozen routing table
//! through the producer's own publisher (`market::publish_routing_table`) and
//! sends a v0 message over it. The table's rent is reported in the evidence
//! under its own field so a census can classify it rather than discover it.

use std::path::{Path, PathBuf};

use serde_json::json;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use solana_sdk_ids::{system_program, sysvar};

use dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;
use dclutch_market::{CoreState, Phase};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_resolution_core_v3_operator::{
    Observation, ObservedAccount, ResolutionAdmitTerminalSnapshotV3,
    build_resolution_admit_terminal_v3, validate_resolution_admit_terminal_report_v3,
};
use dclutch_source::resolution::{
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, ResolutionCertificateKindV2,
};
use dclutch_source::{
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
    SourceResolutionPhaseV1, SourceResolutionStateV2,
};

use crate::campaign::{
    parse_campaign_terminal_evidence_with_expected_cluster_v1, read_keypair_file,
};
use crate::cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1};
use crate::model::{SuccessorPlan, TransactionEvidence};
use crate::plan::pubkey;
use crate::rpc::{Rpc, WritePolicyV1};
use crate::terminal_lifecycle::routed_record;
use crate::wallet_terminal::RecordPairV1;
use crate::{Error, Result};

/// The owned-loopback command name.
pub(crate) const COMMAND_V1: &str = "local-private-validator-admit-terminal-v1";
/// The public arm.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-admit-terminal-v1";

const fn command(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => COMMAND_DEVNET_V1,
        ExpectedClusterV1::OwnedLoopback => COMMAND_V1,
    }
}

/// The label the admission carries, naming the kind it admits so a census
/// binding for the failure arm is not the honest arm's.
fn label_for(kind: ResolutionCertificateKindV2) -> &'static str {
    match kind {
        ResolutionCertificateKindV2::ResolutionFailure => {
            "Core AdmitTerminal: the failure selector"
        }
        _ => "Core AdmitTerminal: the honest selector",
    }
}

pub(crate) fn devnet_usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-admit-terminal-v1 --rpc-url URL --i-mean-devnet DEVNET_GENESIS --plan ABSOLUTE_JSON --evidence ABSOLUTE_JSON --market PUBKEY --terminal-sequence U64 --fee-payer PUBKEY --output ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON]\n\
     \nThe public arm of the same family-neutral terminal admission. It consumes an executed devnet campaign report and refuses every non-devnet origin."
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-admit-terminal-v1 --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON --evidence ABSOLUTE_JSON --market PUBKEY --terminal-sequence U64 --fee-payer PUBKEY --output ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON]\n\
     \nCore admits the terminal certificate the Source already minted -- a ResolutionSuccess for a Resolved Source, a ResolutionFailure for a FailureCommitted one -- and the Market's phase byte moves Open -> Terminal. The kind is read off the Source's own phase and never chosen; the frame is chain-derived by build_resolution_admit_terminal_v3, the one builder every terminal admission in this tree calls. No account signs but the fee payer. Execute publishes one frozen routing table (the frame outgrows a legacy packet), sends one v0 transaction, and reads the Market back to prove the phase, the receipt and the winner."
}

#[derive(Debug)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    evidence: PathBuf,
    market: Pubkey,
    terminal_sequence: u64,
    fee_payer: Pubkey,
    fee_payer_keypair: Option<PathBuf>,
    output: PathBuf,
    execute: bool,
}

struct PlanV1 {
    instruction: solana_sdk::instruction::Instruction,
    market: Pubkey,
    source_state: Pubkey,
    certificate: Pubkey,
    kind: ResolutionCertificateKindV2,
    selector: u32,
    outcome_count: u32,
    terminal_sequence: u64,
    phase_before: Phase,
}

/// What one admission was and, when it executed, what it landed.
pub(crate) struct AdmitTerminalOutcomeV1 {
    pub(crate) certificate: Pubkey,
    pub(crate) kind: ResolutionCertificateKindV2,
    /// The Product-authenticated selector the operator READ from the Source's
    /// own decision -- never a number this driver chose.
    pub(crate) selector: u32,
    pub(crate) outcome_count: u32,
    /// The routing-table transactions the execute published, then the
    /// admission itself, last. Empty on a preflight.
    pub(crate) transactions: Vec<TransactionEvidence>,
    /// Lamports the routing table holds, which no ledger can watch in advance.
    pub(crate) table_rent_lamports: u64,
}

pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run_v1(arguments, ExpectedClusterV1::OwnedLoopback).map(|_| ())
}

pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run_v1(arguments, ExpectedClusterV1::Devnet).map(|_| ())
}

/// One admission, as a value rather than as a process exit code.
pub(crate) fn run_v1(
    arguments: Vec<String>,
    expected: ExpectedClusterV1,
) -> Result<AdmitTerminalOutcomeV1> {
    let arguments = parse(arguments, expected)?;
    expected.authenticate(&arguments.origin)?;
    let mut rpc = Rpc::connect_cluster(
        &arguments.origin,
        if arguments.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;
    let plan = plan(&mut rpc, &arguments, expected)?;
    report(&plan);

    if !arguments.execute {
        write_evidence(&arguments.output, &plan, expected, &[], 0)?;
        println!("preflight only; no key was opened and nothing was sent");
        return Ok(AdmitTerminalOutcomeV1 {
            certificate: plan.certificate,
            kind: plan.kind,
            selector: plan.selector,
            outcome_count: plan.outcome_count,
            transactions: Vec::new(),
            table_rent_lamports: 0,
        });
    }

    let path = arguments
        .fee_payer_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --fee-payer-keypair"))?;
    let payer = Keypair::new_from_array(read_keypair_file(path, "admit-terminal fee payer")?);
    if payer.pubkey() != arguments.fee_payer {
        return Err(Error::new(format!(
            "the plan was built for fee payer {} and --fee-payer-keypair holds {}",
            arguments.fee_payer,
            payer.pubkey()
        )));
    }

    let mut transactions = Vec::new();
    let (routing, tables) = crate::market::publish_routing_table(
        &mut rpc,
        &payer,
        "Core AdmitTerminal",
        std::slice::from_ref(&plan.instruction),
        &mut transactions,
    )?;
    let table_rent_lamports = tables.iter().map(|table| table.lamports).sum();
    let evidence = rpc.send_v0_with_signers(
        label_for(plan.kind),
        std::slice::from_ref(&plan.instruction),
        &payer,
        &[],
        routing,
        &tables,
    )?;
    if let Some(error) = evidence.error.as_ref() {
        return Err(Error::new(format!(
            "the terminal admission refused on chain: {error}"
        )));
    }
    println!("signature            {}", evidence.signature);
    println!("slot                 {}", evidence.slot);
    println!(
        "compute units        {}",
        evidence
            .compute_units_consumed
            .map_or_else(|| "unreported".to_string(), |units| units.to_string())
    );
    transactions.push(evidence);

    // THE PHASE BYTE, THE RECEIPT AND THE SELECTOR, read back off the chain.
    let after = CoreState::decode(&rpc.required_account(plan.market, "terminal Market")?.data)
        .map_err(|error| Error::new(format!("terminal Market: {error:?}")))?;
    if after.phase != Phase::Terminal {
        return Err(Error::new(format!(
            "AdmitTerminal left the Market at {:?}, not Terminal",
            after.phase
        )));
    }
    let receipt = after
        .terminal_receipt
        .ok_or_else(|| Error::new("a Terminal Market carries no terminal receipt"))?;
    if receipt.to_bytes() != plan.certificate.to_bytes() {
        return Err(Error::new(format!(
            "the Market's terminal receipt names {} and this admission consumed the certificate \
             at {}",
            Pubkey::new_from_array(receipt.to_bytes()),
            plan.certificate
        )));
    }
    if after.terminal_winner != plan.selector {
        return Err(Error::new(format!(
            "the Market records terminal winner {} and the operator read the Product-authenticated \
             selector {}",
            after.terminal_winner, plan.selector
        )));
    }
    println!(
        "market after         phase {:?}, terminal winner {} of {}, receipt {} (read back from chain)",
        after.phase, after.terminal_winner, plan.outcome_count, plan.certificate
    );
    write_evidence(
        &arguments.output,
        &plan,
        expected,
        &transactions,
        table_rent_lamports,
    )?;
    Ok(AdmitTerminalOutcomeV1 {
        certificate: plan.certificate,
        kind: plan.kind,
        selector: plan.selector,
        outcome_count: plan.outcome_count,
        transactions,
        table_rent_lamports,
    })
}

/// The certificate kind a terminal Source names, and nothing else.
fn terminal_certificate_kind(
    phase: SourceResolutionPhaseV1,
) -> Result<ResolutionCertificateKindV2> {
    match phase {
        SourceResolutionPhaseV1::Resolved => Ok(ResolutionCertificateKindV2::ResolutionSuccess),
        SourceResolutionPhaseV1::FailureCommitted => {
            Ok(ResolutionCertificateKindV2::ResolutionFailure)
        }
        other => Err(Error::new(format!(
            "the Source stands at {other:?}: Core admits a terminal only from Resolved or \
             FailureCommitted, and a market standing anywhere else has no certificate to admit"
        ))),
    }
}

fn vacant(observation: Observation, key: Pubkey) -> ObservedAccount {
    ObservedAccount {
        observation,
        key,
        owner: system_program::ID,
        lamports: 0,
        executable: false,
        data: Vec::new(),
    }
}

fn plan(rpc: &mut Rpc, arguments: &ArgumentsV1, expected: ExpectedClusterV1) -> Result<PlanV1> {
    let plan_bytes = std::fs::read(&arguments.plan)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let evidence_bytes = std::fs::read(&arguments.evidence)?;
    let evidence =
        parse_campaign_terminal_evidence_with_expected_cluster_v1(&evidence_bytes, expected)?;

    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let core_programdata = pubkey(&plan.core.programdata_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let resolution_programdata = pubkey(&plan.resolution.programdata_id)?;
    let activation = pubkey(&plan.activation)?;
    let market = arguments.market;

    let material: RecordPairV1 = routed_record(
        &evidence,
        "source_material_record",
        registry,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    )?;
    let manifest = routed_record(
        &evidence,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let product = routed_record(
        &evidence,
        "product_record",
        registry,
        PRODUCT_RECORD_SCHEMA_ID_V2,
    )?;
    let result_domain = routed_record(
        &evidence,
        "result_domain_record",
        registry,
        RESULT_DOMAIN_SCHEMA_ID_V2,
    )?;
    let portfolio = routed_record(
        &evidence,
        "portfolio_record",
        registry,
        PORTFOLIO_SCHEMA_ID_V2,
    )?;
    let funding_ledger = pubkey(
        &evidence
            .accounts
            .get("resolution_funding_ledger")
            .ok_or_else(|| Error::new("the campaign report names no resolution_funding_ledger"))?
            .address,
    )?;

    let market_account = rpc.required_account(market, "Core Market")?;
    if market_account.owner != core {
        return Err(Error::new(format!(
            "the account at {market} is owned by {}, not the plan's Core program {core}",
            market_account.owner
        )));
    }
    let before = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    let generation = before.identity.generation;
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let source = SourceResolutionStateV2::decode(
        &rpc.required_account(source_state, "Source resolution state")?
            .data,
    )
    .map_err(|error| Error::new(format!("Source resolution state: {error:?}")))?;
    let kind = terminal_certificate_kind(source.phase())?;
    let certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &[kind.kind_seed()],
            &arguments.terminal_sequence.to_le_bytes(),
        ],
        &resolution,
    )
    .0;

    let (observation, present) = rpc.finalized_observed_accounts(
        &[
            market,
            activation,
            registry,
            core,
            core_programdata,
            resolution,
            resolution_programdata,
            material.raw,
            manifest.raw,
            source_state,
            funding_ledger,
            certificate,
            sysvar::rent::ID,
            product.raw,
            result_domain.raw,
            portfolio.raw,
        ],
        0,
    )?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    // The five staging cursors are vacant by construction: every record this
    // Market selects was published in a transaction and never staged again,
    // and a cursor that is NOT vacant is exactly what
    // `authenticate_finalized_record` refuses inside the builder.
    let snapshot = ResolutionAdmitTerminalSnapshotV3 {
        market: at(0)?,
        activation_cache: at(1)?,
        registry_program: at(2)?,
        core_program: at(3)?,
        core_programdata: at(4)?,
        resolution_program: at(5)?,
        resolution_programdata: at(6)?,
        source_material: at(7)?,
        source_material_staging: vacant(observation, material.staging),
        capability_manifest: at(8)?,
        capability_manifest_staging: vacant(observation, manifest.staging),
        source_state: at(9)?,
        funding_ledger: at(10)?,
        certificate: at(11)?,
        rent_sysvar: at(12)?,
        product_raw: at(13)?,
        product_staging: vacant(observation, product.staging),
        result_domain_raw: at(14)?,
        result_domain_staging: vacant(observation, result_domain.staging),
        portfolio_raw: at(15)?,
        portfolio_staging: vacant(observation, portfolio.staging),
    };
    let report = build_resolution_admit_terminal_v3(&snapshot)
        .map_err(|error| Error::new(format!("chain-derived AdmitTerminal: {error:?}")))?;
    validate_resolution_admit_terminal_report_v3(&report)
        .map_err(|error| Error::new(format!("AdmitTerminal report: {error:?}")))?;
    if report.instruction.program_id != core {
        return Err(Error::new(format!(
            "AdmitTerminal is addressed to {} and this Market's Core program is {core}",
            report.instruction.program_id
        )));
    }
    if report.terminal_sequence != arguments.terminal_sequence {
        return Err(Error::new(format!(
            "the operator admits terminal sequence {} and this command was asked for {}",
            report.terminal_sequence, arguments.terminal_sequence
        )));
    }
    Ok(PlanV1 {
        instruction: report.instruction,
        market,
        source_state,
        certificate,
        kind,
        selector: report.selector,
        outcome_count: report.outcome_count,
        terminal_sequence: report.terminal_sequence,
        phase_before: before.phase,
    })
}

fn report(plan: &PlanV1) {
    println!("market               {}", plan.market);
    println!("phase before         {:?}", plan.phase_before);
    println!("source state         {}", plan.source_state);
    println!(
        "certificate          {} ({:?})",
        plan.certificate, plan.kind
    );
    println!(
        "selector             {} of {} (the Source's own decision, Product-authenticated)",
        plan.selector, plan.outcome_count
    );
    println!("terminal sequence    {}", plan.terminal_sequence);
    println!("frame accounts       {}", plan.instruction.accounts.len());
}

fn usage_for(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => devnet_usage(),
        ExpectedClusterV1::OwnedLoopback => usage(),
    }
}

fn write_evidence(
    path: &Path,
    plan: &PlanV1,
    expected: ExpectedClusterV1,
    transactions: &[TransactionEvidence],
    table_rent_lamports: u64,
) -> Result<()> {
    let document = json!({
        "schema": "dclutch-admit-terminal-evidence-v1",
        "cluster": expected.evidence_label(),
        "market": plan.market.to_string(),
        "phaseBefore": format!("{:?}", plan.phase_before),
        "sourceState": plan.source_state.to_string(),
        "certificate": plan.certificate.to_string(),
        "certificateKind": format!("{:?}", plan.kind),
        "selector": plan.selector,
        "outcomeCount": plan.outcome_count,
        "terminalSequence": plan.terminal_sequence,
        "frameAccounts": plan.instruction.accounts.len(),
        "routingTableRentLamports": table_rent_lamports,
        "transactions": transactions.iter().map(|evidence| json!({
            "label": evidence.label,
            "signature": evidence.signature,
            "slot": evidence.slot,
            "computeUnitsConsumed": evidence.compute_units_consumed,
            "feeLamports": evidence.fee_lamports,
        })).collect::<Vec<_>>(),
    });
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&document)?),
    )?;
    Ok(())
}

fn parse(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut evidence = None;
    let mut market = None;
    let mut terminal_sequence = None;
    let mut fee_payer = None;
    let mut fee_payer_keypair = None;
    let mut output = None;
    let mut execute = false;
    let mut cursor = arguments.into_iter();
    while let Some(flag) = cursor.next() {
        if flag == "--execute" {
            if execute {
                return Err(Error::new("--execute was given twice"));
            }
            execute = true;
            continue;
        }
        let value = cursor.next().ok_or_else(|| {
            Error::new(format!(
                "{flag} needs a value; usage: {}",
                usage_for(expected)
            ))
        })?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG if expected == ExpectedClusterV1::Devnet => {
                &mut acknowledgment
            }
            "--plan" => &mut plan,
            "--evidence" => &mut evidence,
            "--market" => &mut market,
            "--terminal-sequence" => &mut terminal_sequence,
            "--fee-payer" => &mut fee_payer,
            "--fee-payer-keypair" => &mut fee_payer_keypair,
            "--output" => &mut output,
            other => {
                return Err(Error::new(format!(
                    "unknown {} argument: {other}",
                    command(expected)
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{flag} was given twice")));
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| {
            Error::new(format!(
                "{name} is required; usage: {}",
                usage_for(expected)
            ))
        })
    };
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let terminal_sequence = required(terminal_sequence, "--terminal-sequence")?
        .parse::<u64>()
        .map_err(|_| Error::new("--terminal-sequence must be a decimal u64"))?;
    if terminal_sequence == 0 {
        return Err(Error::new(
            "--terminal-sequence must be positive; the wire refuses zero",
        ));
    }
    Ok(ArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: PathBuf::from(required(plan, "--plan")?),
        evidence: PathBuf::from(required(evidence, "--evidence")?),
        market: required(market, "--market")?
            .parse()
            .map_err(|error| Error::new(format!("--market: {error}")))?,
        terminal_sequence,
        fee_payer: required(fee_payer, "--fee-payer")?
            .parse()
            .map_err(|error| Error::new(format!("--fee-payer: {error}")))?,
        fee_payer_keypair: fee_payer_keypair.map(PathBuf::from),
        output: PathBuf::from(required(output, "--output")?),
        execute,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kind is the Source's, and every non-terminal phase is refused by
    /// name rather than admitted at a guessed seat.
    #[test]
    fn the_certificate_kind_is_read_off_the_source_and_never_chosen() {
        assert_eq!(
            terminal_certificate_kind(SourceResolutionPhaseV1::Resolved).expect("success"),
            ResolutionCertificateKindV2::ResolutionSuccess
        );
        assert_eq!(
            terminal_certificate_kind(SourceResolutionPhaseV1::FailureCommitted).expect("failure"),
            ResolutionCertificateKindV2::ResolutionFailure
        );
        for phase in [
            SourceResolutionPhaseV1::Primary,
            SourceResolutionPhaseV1::Recovery,
            SourceResolutionPhaseV1::Exhausted,
            SourceResolutionPhaseV1::Retired,
        ] {
            let refusal = terminal_certificate_kind(phase).expect_err("not a terminal");
            assert!(
                refusal.to_string().contains("no certificate to admit"),
                "{phase:?}: {refusal}"
            );
        }
    }

    /// The two kinds are two labels, so the failure arm's admission binds to
    /// its own census row.
    #[test]
    fn the_label_names_the_kind_it_admits() {
        assert_ne!(
            label_for(ResolutionCertificateKindV2::ResolutionFailure),
            label_for(ResolutionCertificateKindV2::ResolutionSuccess)
        );
    }

    #[test]
    fn a_zero_terminal_sequence_refuses_at_the_parser() {
        let refusal = parse(
            vec![
                "--rpc-url".into(),
                "http://127.0.0.1:21400".into(),
                "--plan".into(),
                "/abs/plan.json".into(),
                "--evidence".into(),
                "/abs/evidence.json".into(),
                "--market".into(),
                Pubkey::new_from_array([0x21; 32]).to_string(),
                "--terminal-sequence".into(),
                "0".into(),
                "--fee-payer".into(),
                Pubkey::new_from_array([0x22; 32]).to_string(),
                "--output".into(),
                "/abs/out.json".into(),
            ],
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect_err("a zero terminal sequence must refuse");
        assert!(
            refusal.to_string().contains("wire refuses zero"),
            "got {refusal}"
        );
    }
}
