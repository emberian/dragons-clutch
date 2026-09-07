//! The two small economic records' first real callers.
//!
//! `parameters-found` creates Custody's governed parameter record and reads it
//! back. `upkeep-found` creates the upkeep vault and then credits a nonzero
//! voluntary deposit, reading both states back. The latter deliberately does
//! not claim to produce a `Donation` credit: that class is a Trading
//! close-maker CPI's statement about its own donation remainder. A wallet can
//! only prove its own `Deposit` here.
//!
//! Neither command touches a Market, a Custody replay, or Hoard principal.
//! That separation is observable in the output: the only transferred amount is
//! the payer's lamports and the resulting vault class is `Deposit`.

use std::path::PathBuf;

use dclutch_custody::upkeep_vault_v1::{UpkeepSourceClassV1, UpkeepVaultV1};
use dclutch_market::protocol_parameters::{
    PendingChangeV1, ProtocolParametersRecordV1, ProtocolParametersV1,
};
use dclutch_operator::{
    protocol_parameters_v1::{
        apply_protocol_parameters_instruction_v1, custody_programdata_address_v1,
        found_protocol_parameters_instruction_v1, propose_protocol_parameters_instruction_v1,
        protocol_parameters_receipt_address_v1, protocol_parameters_record_address_v1,
        withdraw_protocol_parameters_instruction_v1,
    },
    upkeep_vault_v1::{
        deposit_upkeep_instruction_v1, found_upkeep_vault_instruction_v1, upkeep_vault_address_v1,
    },
};
use dclutch_registry::svm::ProgramDataMetadataV3View;
use serde_json::{Value, json};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{
    Error, Result,
    campaign::read_keypair_file,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG},
    rpc::{Rpc, RpcAccount, WritePolicyV1},
};

pub(crate) const PARAMETERS_FOUND_LOCAL_COMMAND_V1: &str = "parameters-found";
pub(crate) const PARAMETERS_FOUND_DEVNET_COMMAND_V1: &str = "devnet-parameters-found";
pub(crate) const UPKEEP_FOUND_LOCAL_COMMAND_V1: &str = "upkeep-found";
pub(crate) const UPKEEP_FOUND_DEVNET_COMMAND_V1: &str = "devnet-upkeep-found";
pub(crate) const PARAMETERS_PROPOSE_LOCAL_COMMAND_V1: &str = "parameters-propose";
pub(crate) const PARAMETERS_APPLY_LOCAL_COMMAND_V1: &str = "parameters-apply";
pub(crate) const PARAMETERS_WITHDRAW_LOCAL_COMMAND_V1: &str = "parameters-withdraw";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RouteV1 {
    Parameters,
    Propose,
    Apply,
    Withdraw,
    Upkeep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClusterV1 {
    OwnedLoopback,
    Devnet,
}

impl ClusterV1 {
    const fn command(self, route: RouteV1) -> &'static str {
        match (self, route) {
            (Self::OwnedLoopback, RouteV1::Parameters) => PARAMETERS_FOUND_LOCAL_COMMAND_V1,
            (Self::Devnet, RouteV1::Parameters) => PARAMETERS_FOUND_DEVNET_COMMAND_V1,
            (Self::OwnedLoopback, RouteV1::Propose) => PARAMETERS_PROPOSE_LOCAL_COMMAND_V1,
            (Self::OwnedLoopback, RouteV1::Apply) => PARAMETERS_APPLY_LOCAL_COMMAND_V1,
            (Self::OwnedLoopback, RouteV1::Withdraw) => PARAMETERS_WITHDRAW_LOCAL_COMMAND_V1,
            (Self::Devnet, RouteV1::Propose | RouteV1::Apply | RouteV1::Withdraw) => "unavailable",
            (Self::OwnedLoopback, RouteV1::Upkeep) => UPKEEP_FOUND_LOCAL_COMMAND_V1,
            (Self::Devnet, RouteV1::Upkeep) => UPKEEP_FOUND_DEVNET_COMMAND_V1,
        }
    }
}

#[derive(Debug)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    custody: Pubkey,
    payer: Pubkey,
    payer_keypair: Option<PathBuf>,
    amount: Option<u64>,
    closer_reward_cap_lamports: Option<u64>,
    execute: bool,
}

pub(crate) fn run(route: RouteV1, cluster: ClusterV1, raw: Vec<String>) -> Result<()> {
    print_json(run_value(route, cluster, raw)?);
    Ok(())
}

/// Execute one economics founder command and return its read-back evidence.
///
/// The command-line entry point prints this value. The journey links the same
/// executor and persists it beside the finalized packet evidence, so a local
/// validator campaign is not forced to scrape its own stdout.
pub(crate) fn run_value(route: RouteV1, cluster: ClusterV1, raw: Vec<String>) -> Result<Value> {
    let arguments = parse(route, cluster, raw)?;
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&arguments.origin, policy)?;
    match route {
        RouteV1::Parameters => parameters_found(&mut rpc, &arguments),
        RouteV1::Propose => parameters_propose(&mut rpc, &arguments),
        RouteV1::Apply => parameters_apply(&mut rpc, &arguments),
        RouteV1::Withdraw => parameters_withdraw(&mut rpc, &arguments),
        RouteV1::Upkeep => upkeep_found(&mut rpc, &arguments),
    }
}

fn parse(route: RouteV1, cluster: ClusterV1, raw: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut custody = None;
    let mut payer = None;
    let mut payer_keypair = None;
    let mut amount = None;
    let mut closer_reward_cap_lamports = None;
    let mut execute = false;
    let mut cursor = raw.into_iter();
    while let Some(flag) = cursor.next() {
        if flag == "--execute" {
            if execute {
                return Err(Error::new("--execute was supplied twice"));
            }
            execute = true;
            continue;
        }
        let value = cursor.next().ok_or_else(|| {
            Error::new(format!(
                "{flag} requires a value; usage: {}",
                usage(route, cluster)
            ))
        })?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG if cluster == ClusterV1::Devnet => &mut acknowledgment,
            "--custody" => &mut custody,
            "--payer" => &mut payer,
            "--payer-keypair" => &mut payer_keypair,
            "--amount" if route == RouteV1::Upkeep => &mut amount,
            "--closer-reward-cap-lamports"
                if matches!(route, RouteV1::Propose | RouteV1::Apply) =>
            {
                &mut closer_reward_cap_lamports
            }
            other => {
                return Err(Error::new(format!(
                    "unknown {} argument: {other}",
                    cluster.command(route)
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{flag} was supplied twice")));
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| {
            Error::new(format!(
                "{name} is required; usage: {}",
                usage(route, cluster)
            ))
        })
    };
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let payer = required(payer, "--payer")?
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("--payer: {error}")))?;
    let custody = required(custody, "--custody")?
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("--custody: {error}")))?;
    let amount = match (route, amount) {
        (RouteV1::Parameters | RouteV1::Propose | RouteV1::Apply | RouteV1::Withdraw, None) => None,
        (RouteV1::Parameters | RouteV1::Propose | RouteV1::Apply | RouteV1::Withdraw, Some(_)) => {
            unreachable!("only upkeep parser accepts --amount")
        }
        (RouteV1::Upkeep, Some(raw)) => {
            let value = raw
                .parse::<u64>()
                .map_err(|_| Error::new("--amount must be a decimal u64"))?;
            if value == 0 {
                return Err(Error::new(
                    "--amount must be nonzero; a zero Credit is not an act",
                ));
            }
            Some(value)
        }
        (RouteV1::Upkeep, None) => {
            return Err(Error::new(format!(
                "--amount is required; usage: {}",
                usage(route, cluster)
            )));
        }
    };
    let closer_reward_cap_lamports = match (route, closer_reward_cap_lamports) {
        (RouteV1::Propose | RouteV1::Apply, Some(raw)) => Some(
            raw.parse::<u64>()
                .map_err(|_| Error::new("--closer-reward-cap-lamports must be a decimal u64"))?,
        ),
        (RouteV1::Propose | RouteV1::Apply, None) => {
            return Err(Error::new(format!(
                "--closer-reward-cap-lamports is required; usage: {}",
                usage(route, cluster)
            )));
        }
        (_, None) => None,
        (_, Some(_)) => unreachable!("parser only accepts the cap for propose/apply"),
    };
    Ok(ArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        custody,
        payer,
        payer_keypair: payer_keypair.map(PathBuf::from),
        amount,
        closer_reward_cap_lamports,
        execute,
    })
}

fn usage(route: RouteV1, cluster: ClusterV1) -> String {
    let public = if cluster == ClusterV1::Devnet {
        " --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
    } else {
        ""
    };
    let amount = if route == RouteV1::Upkeep {
        " --amount LAMPORTS"
    } else {
        ""
    };
    let cap = if matches!(route, RouteV1::Propose | RouteV1::Apply) {
        " --closer-reward-cap-lamports LAMPORTS"
    } else {
        ""
    };
    format!(
        "dclutch-local-successor-bootstrap {} --rpc-url URL{public} --custody CUSTODY_PROGRAM --payer PAYER{amount}{cap} [--execute --payer-keypair ABSOLUTE_JSON]",
        cluster.command(route)
    )
}

fn signer(arguments: &ArgumentsV1, label: &str) -> Result<Keypair> {
    let path = arguments.payer_keypair.as_deref().ok_or_else(|| {
        Error::new(format!(
            "--execute requires --payer-keypair for the {label} signer"
        ))
    })?;
    let signer = Keypair::new_from_array(read_keypair_file(path, label)?);
    if signer.pubkey() != arguments.payer {
        return Err(Error::new(format!(
            "--payer is {}, but --payer-keypair holds {}; the signer is a frame coordinate, not a hint",
            arguments.payer,
            signer.pubkey()
        )));
    }
    Ok(signer)
}

fn programdata_authority(rpc: &mut Rpc, custody: Pubkey) -> Result<Option<[u8; 32]>> {
    let programdata = custody_programdata_address_v1(custody);
    let account = rpc.required_account(programdata, "Custody ProgramData")?;
    if account.owner != bpf_loader_upgradeable::ID {
        return Err(Error::new(format!(
            "Custody ProgramData {programdata} is owned by {}, not the upgradeable loader {}",
            account.owner,
            bpf_loader_upgradeable::ID
        )));
    }
    ProgramDataMetadataV3View::parse(&account.data)
        .map(|view| view.upgrade_authority())
        .map_err(|error| {
            Error::new(format!(
                "Custody ProgramData {programdata} does not decode: {error:?}"
            ))
        })
}

fn require_absent(rpc: &mut Rpc, address: Pubkey, label: &str) -> Result<()> {
    if let Some(account) = rpc.account(address)? {
        return Err(Error::new(format!(
            "{label} {address} already exists (owner {}, {} bytes); founding is one-shot and this command refuses to call it again",
            account.owner,
            account.data.len()
        )));
    }
    Ok(())
}

fn parameters_found(rpc: &mut Rpc, arguments: &ArgumentsV1) -> Result<Value> {
    let record_address = protocol_parameters_record_address_v1(arguments.custody);
    let authority = programdata_authority(rpc, arguments.custody)?;
    require_absent(rpc, record_address, "protocol-parameters record")?;
    let required_founder = authority.map_or(arguments.payer, Pubkey::new_from_array);
    if required_founder != arguments.payer {
        return Err(Error::new(format!(
            "Custody ProgramData names {} as its current upgrade authority, but --payer is {}; parameters founding must use that authority",
            required_founder, arguments.payer
        )));
    }
    let governance_authority = authority.unwrap_or([0; 32]);
    let instruction = found_protocol_parameters_instruction_v1(
        arguments.custody,
        arguments.payer,
        governance_authority,
    )
    .map_err(|error| Error::new(format!("build parameters Found: {error:?}")))?;
    if !arguments.execute {
        return Ok(json!({
            "command": "parameters-found",
            "cluster": arguments.origin.label(),
            "mode": "preflight",
            "custody": arguments.custody.to_string(),
            "record": record_address.to_string(),
            "founder": arguments.payer.to_string(),
            "governanceAuthority": Pubkey::new_from_array(governance_authority).to_string(),
            "instructionAccounts": instruction.accounts.len(),
            "poststate": "would persist the canonical genesis parameters record; no transaction was submitted"
        }));
    }
    let payer = signer(arguments, "parameters founder")?;
    let transaction = rpc.send("parameters-found", &[instruction], &payer)?;
    accepted(&transaction.error, "parameters Found")?;
    let account = owned_account(
        rpc,
        record_address,
        arguments.custody,
        "protocol-parameters record",
    )?;
    let record = ProtocolParametersRecordV1::decode(&account.data).map_err(|error| {
        Error::new(format!(
            "read-back parameters record does not decode: {error:?}"
        ))
    })?;
    let expected = ProtocolParametersV1::genesis(governance_authority);
    if record.parameters != expected || record.pending != PendingChangeV1::NONE {
        return Err(Error::new(
            "parameters Found landed but read-back is not the canonical unfrozen/frozen genesis record",
        ));
    }
    Ok(json!({
        "command": "parameters-found",
        "cluster": arguments.origin.label(),
        "custody": arguments.custody.to_string(),
        "record": record_address.to_string(),
        "signature": transaction.signature,
        "slot": transaction.slot,
        "poststate": {
            "recordOwner": account.owner.to_string(),
            "governanceAuthority": Pubkey::new_from_array(record.parameters.governance_authority).to_string(),
            "protocolTakeBasisPoints": record.parameters.protocol_take_basis_points,
            "protocolBeneficiary": Pubkey::new_from_array(record.parameters.protocol_beneficiary).to_string(),
            "pendingProposal": false,
            "hoardPrincipalMoved": 0
        }
    }))
}

fn current_parameters(
    rpc: &mut Rpc,
    custody: Pubkey,
) -> Result<(Pubkey, ProtocolParametersRecordV1)> {
    let address = protocol_parameters_record_address_v1(custody);
    let account = owned_account(rpc, address, custody, "protocol-parameters record")?;
    let record = ProtocolParametersRecordV1::decode(&account.data).map_err(|error| {
        Error::new(format!(
            "read-back parameters record does not decode: {error:?}"
        ))
    })?;
    Ok((address, record))
}

fn proposed_parameters(record: ProtocolParametersRecordV1, cap: u64) -> ProtocolParametersV1 {
    ProtocolParametersV1 {
        closer_reward_cap_lamports: cap,
        ..record.parameters
    }
}

fn parameters_propose(rpc: &mut Rpc, arguments: &ArgumentsV1) -> Result<Value> {
    let cap = arguments
        .closer_reward_cap_lamports
        .expect("propose parser required cap");
    let (record_address, record) = current_parameters(rpc, arguments.custody)?;
    let body = proposed_parameters(record, cap);
    let instruction =
        propose_protocol_parameters_instruction_v1(arguments.custody, arguments.payer, body)
            .map_err(|error| Error::new(format!("build parameters Propose: {error:?}")))?;
    if !arguments.execute {
        return Ok(
            json!({"command":"parameters-propose","cluster":arguments.origin.label(),"mode":"preflight","record":record_address.to_string(),"currentGeneration":record.parameters.generation,"closerRewardCapLamports":cap,"poststate":"would stage one exact governed body; no transaction was submitted"}),
        );
    }
    let payer = signer(arguments, "parameters governance authority")?;
    let transaction = rpc.send("parameters-propose", &[instruction], &payer)?;
    accepted(&transaction.error, "parameters Propose")?;
    let (_, after) = current_parameters(rpc, arguments.custody)?;
    if !after.pending.is_standing()
        || after.pending.digest != body.body_digest()
        || after.parameters != record.parameters
    {
        return Err(Error::new(
            "parameters Propose landed but did not preserve the active body and pin the proposed digest",
        ));
    }
    Ok(
        json!({"command":"parameters-propose","cluster":arguments.origin.label(),"record":record_address.to_string(),"signature":transaction.signature,"slot":transaction.slot,"poststate":{"activeGeneration":after.parameters.generation,"pendingProposal":true,"earliestApplySlot":after.pending.earliest_apply_slot,"proposedCloserRewardCapLamports":cap,"hoardPrincipalMoved":0}}),
    )
}

fn parameters_withdraw(rpc: &mut Rpc, arguments: &ArgumentsV1) -> Result<Value> {
    let (record_address, before) = current_parameters(rpc, arguments.custody)?;
    let instruction =
        withdraw_protocol_parameters_instruction_v1(arguments.custody, arguments.payer)
            .map_err(|error| Error::new(format!("build parameters Withdraw: {error:?}")))?;
    if !arguments.execute {
        return Ok(
            json!({"command":"parameters-withdraw","cluster":arguments.origin.label(),"mode":"preflight","record":record_address.to_string(),"poststate":"would clear only the standing proposal; no transaction was submitted"}),
        );
    }
    let payer = signer(arguments, "parameters governance authority")?;
    let transaction = rpc.send("parameters-withdraw", &[instruction], &payer)?;
    accepted(&transaction.error, "parameters Withdraw")?;
    let (_, after) = current_parameters(rpc, arguments.custody)?;
    if after.pending != PendingChangeV1::NONE || after.parameters != before.parameters {
        return Err(Error::new(
            "parameters Withdraw landed but changed the active body or left a pending proposal",
        ));
    }
    Ok(
        json!({"command":"parameters-withdraw","cluster":arguments.origin.label(),"record":record_address.to_string(),"signature":transaction.signature,"slot":transaction.slot,"poststate":{"activeGeneration":after.parameters.generation,"pendingProposal":false,"hoardPrincipalMoved":0}}),
    )
}

fn parameters_apply(rpc: &mut Rpc, arguments: &ArgumentsV1) -> Result<Value> {
    let cap = arguments
        .closer_reward_cap_lamports
        .expect("apply parser required cap");
    let (record_address, record) = current_parameters(rpc, arguments.custody)?;
    let body = proposed_parameters(record, cap);
    let next_generation = record
        .parameters
        .generation
        .checked_add(1)
        .ok_or_else(|| Error::new("parameters generation overflow"))?;
    let receipt = protocol_parameters_receipt_address_v1(arguments.custody, next_generation);
    let instruction = apply_protocol_parameters_instruction_v1(
        arguments.custody,
        arguments.payer,
        body,
        next_generation,
    )
    .map_err(|error| Error::new(format!("build parameters Apply: {error:?}")))?;
    if !arguments.execute {
        return Ok(
            json!({"command":"parameters-apply","cluster":arguments.origin.label(),"mode":"preflight","record":record_address.to_string(),"receipt":receipt.to_string(),"pendingEarliestApplySlot":record.pending.earliest_apply_slot,"poststate":"would apply only a matured matching proposal; no transaction was submitted"}),
        );
    }
    let payer = signer(arguments, "parameters apply payer")?;
    let transaction = rpc.send("parameters-apply", &[instruction], &payer)?;
    accepted(&transaction.error, "parameters Apply")?;
    let (_, after) = current_parameters(rpc, arguments.custody)?;
    if after.pending != PendingChangeV1::NONE
        || after.parameters.generation != next_generation
        || after.parameters.closer_reward_cap_lamports != cap
        || after.parameters.activation_slot != transaction.slot
    {
        return Err(Error::new(
            "parameters Apply landed but its generation, activation, body, or pending state disagrees",
        ));
    }
    let receipt_account = owned_account(
        rpc,
        receipt,
        arguments.custody,
        "protocol-parameters receipt",
    )?;
    if receipt_account.data.is_empty() {
        return Err(Error::new("parameters Apply landed without its receipt"));
    }
    Ok(
        json!({"command":"parameters-apply","cluster":arguments.origin.label(),"record":record_address.to_string(),"receipt":receipt.to_string(),"signature":transaction.signature,"slot":transaction.slot,"poststate":{"activeGeneration":after.parameters.generation,"activationSlot":after.parameters.activation_slot,"pendingProposal":false,"closerRewardCapLamports":cap,"hoardPrincipalMoved":0}}),
    )
}

fn upkeep_found(rpc: &mut Rpc, arguments: &ArgumentsV1) -> Result<Value> {
    let amount = arguments.amount.expect("upkeep parser required amount");
    let vault_address = upkeep_vault_address_v1(arguments.custody);
    require_absent(rpc, vault_address, "upkeep vault")?;
    let found = found_upkeep_vault_instruction_v1(arguments.custody, arguments.payer)
        .map_err(|error| Error::new(format!("build upkeep Found: {error:?}")))?;
    let credit = deposit_upkeep_instruction_v1(arguments.custody, arguments.payer, amount)
        .map_err(|error| Error::new(format!("build upkeep Credit: {error:?}")))?;
    if !arguments.execute {
        return Ok(json!({
            "command": "upkeep-found",
            "cluster": arguments.origin.label(),
            "mode": "preflight",
            "custody": arguments.custody.to_string(),
            "vault": vault_address.to_string(),
            "payer": arguments.payer.to_string(),
            "amount": amount,
            "creditClass": "Deposit",
            "foundInstructionAccounts": found.accounts.len(),
            "creditInstructionAccounts": credit.accounts.len(),
            "poststate": "would found then receipt one nonzero voluntary deposit; no transaction was submitted"
        }));
    }
    let payer = signer(arguments, "upkeep payer")?;
    let founding = rpc.send("upkeep-found", &[found], &payer)?;
    accepted(&founding.error, "upkeep Found")?;
    let after_found = owned_account(rpc, vault_address, arguments.custody, "upkeep vault")?;
    let founded = UpkeepVaultV1::decode(&after_found.data).map_err(|error| {
        Error::new(format!(
            "read-back founded upkeep vault does not decode: {error:?}"
        ))
    })?;
    if founded.inflow_total != 0
        || founded.credit_count != 0
        || founded
            .receipted()
            .map_err(|error| Error::new(format!("founded upkeep vault legibility: {error:?}")))?
            != 0
    {
        return Err(Error::new(
            "upkeep Found landed but its read-back record is not an empty vault",
        ));
    }
    let credited = rpc.send("upkeep-credit-deposit", &[credit], &payer)?;
    accepted(&credited.error, "upkeep Credit")?;
    let after_credit = owned_account(rpc, vault_address, arguments.custody, "upkeep vault")?;
    let vault = UpkeepVaultV1::decode(&after_credit.data).map_err(|error| {
        Error::new(format!(
            "read-back credited upkeep vault does not decode: {error:?}"
        ))
    })?;
    if vault.inflow_total != amount
        || vault.inflow_deposit != amount
        || vault.credit_count != 1
        || vault.inflow_residue != 0
        || vault.inflow_donation != 0
        || vault.inflow_seat_rent != 0
        || vault
            .receipted()
            .map_err(|error| Error::new(format!("credited upkeep vault legibility: {error:?}")))?
            != amount
        || vault
            .unreceipted(after_credit.lamports, after_found.lamports)
            .map_err(|error| Error::new(format!("credited upkeep vault remainder: {error:?}")))?
            != 0
        || after_credit.lamports
            != after_found
                .lamports
                .checked_add(amount)
                .ok_or_else(|| Error::new("upkeep balance overflow in poststate check"))?
    {
        return Err(Error::new(
            "upkeep Credit landed but Found/Credit poststate did not conserve the exact deposit",
        ));
    }
    Ok(json!({
        "command": "upkeep-found",
        "cluster": arguments.origin.label(),
        "custody": arguments.custody.to_string(),
        "vault": vault_address.to_string(),
        "foundSignature": founding.signature,
        "creditSignature": credited.signature,
        "poststate": {
            "foundRentLamports": after_found.lamports,
            "vaultLamportsAfterCredit": after_credit.lamports,
            "creditAmount": amount,
            "creditClass": UpkeepSourceClassV1::Deposit.label(),
            "inflowTotal": vault.inflow_total,
            "depositInflow": vault.inflow_deposit,
            "donationInflow": vault.inflow_donation,
            "unreceiptedLamports": 0,
            "creditCount": vault.credit_count,
            "hoardPrincipalMoved": 0,
            "donationRemainderProduced": false
        }
    }))
}

fn owned_account(rpc: &mut Rpc, address: Pubkey, owner: Pubkey, label: &str) -> Result<RpcAccount> {
    let account = rpc.required_account(address, label)?;
    if account.owner != owner {
        return Err(Error::new(format!(
            "{label} {address} is owned by {}, not Custody {owner}",
            account.owner
        )));
    }
    Ok(account)
}

fn accepted(error: &Option<serde_json::Value>, label: &str) -> Result<()> {
    if let Some(error) = error {
        return Err(Error::new(format!("{label} refused on chain: {error}")));
    }
    Ok(())
}

fn print_json(value: serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(&value).expect("JSON report serializes")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voluntary_credit_refuses_zero_before_any_rpc_origin_is_constructed() {
        let refusal = parse(
            RouteV1::Upkeep,
            ClusterV1::OwnedLoopback,
            vec![
                "--rpc-url".into(),
                "http://127.0.0.1:20890".into(),
                "--custody".into(),
                Pubkey::new_from_array([7; 32]).to_string(),
                "--payer".into(),
                Pubkey::new_from_array([8; 32]).to_string(),
                "--amount".into(),
                "0".into(),
            ],
        )
        .expect_err("zero is not a Credit act");
        assert!(refusal.to_string().contains("nonzero"));
    }

    #[test]
    fn governed_proposal_requires_one_exact_cap_before_any_rpc_origin_is_constructed() {
        let refusal = parse(
            RouteV1::Propose,
            ClusterV1::OwnedLoopback,
            vec![
                "--rpc-url".into(),
                "http://127.0.0.1:20890".into(),
                "--custody".into(),
                Pubkey::new_from_array([7; 32]).to_string(),
                "--payer".into(),
                Pubkey::new_from_array([8; 32]).to_string(),
            ],
        )
        .expect_err("a governed proposal cannot invent its proposed body");
        assert!(
            refusal
                .to_string()
                .contains("--closer-reward-cap-lamports is required")
        );
    }

    #[test]
    fn governed_apply_parses_its_exact_cap_on_owned_loopback() {
        let parsed = parse(
            RouteV1::Apply,
            ClusterV1::OwnedLoopback,
            vec![
                "--rpc-url".into(),
                "http://127.0.0.1:20890".into(),
                "--custody".into(),
                Pubkey::new_from_array([7; 32]).to_string(),
                "--payer".into(),
                Pubkey::new_from_array([8; 32]).to_string(),
                "--closer-reward-cap-lamports".into(),
                "17001".into(),
            ],
        )
        .expect("local Apply argument shape");
        assert_eq!(parsed.closer_reward_cap_lamports, Some(17_001));
    }

    #[test]
    fn public_commands_require_the_named_devnet_acknowledgment() {
        let refusal = parse(
            RouteV1::Parameters,
            ClusterV1::Devnet,
            vec![
                "--rpc-url".into(),
                "https://api.devnet.solana.com".into(),
                "--custody".into(),
                Pubkey::new_from_array([7; 32]).to_string(),
                "--payer".into(),
                Pubkey::new_from_array([8; 32]).to_string(),
            ],
        )
        .expect_err("public commands require a deliberate cluster identity");
        assert!(refusal.to_string().contains(DEVNET_ACKNOWLEDGMENT_FLAG));
    }
}
