//! Post-compaction fractional redemption on a real private validator.
//!
//! The opening exterior proves that a live curved product can be wrapped,
//! transferred to an independent sleeper, and partially unwrapped. The real-ELF
//! campaign proves that the sleeper can later be compacted into a claim check.
//! This exterior starts at that durable boundary and consumes the public
//! redemption wire over RPC: four hostile transactions, a partial burn/pay, a
//! refused premature close, a settling burn/pay, and a permissionless close.
//! The staged state is canonical output of compaction, not a second economic
//! model: 40 shards at denominator 10 secure 16 collateral atoms at payout 4.

use std::{
    fs,
    path::Path,
    thread::sleep,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use dclutch_fractional_exterior::bridge::{
    AccountImageV1, canonical_bytes as bridge_bytes, digest as bridge_digest, read_compaction,
};
use serde_json::{Value, json};
use solana_client::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey as ProgramPubkey;
use solana_sdk::{
    account::Account,
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer, keypair_from_seed},
    transaction::Transaction,
};

use dclutch_claims_svm::{
    claim_check_request_v1::CloseClaimCheckEscrowRequestV1,
    claim_check_v1::{
        CLAIM_CHECK_ESCROW_BYTES_V1, ClaimCheckEscrowSeedsV1, ClaimCheckEscrowV1,
        ClaimCheckVaultSeedsV1,
    },
    fractional_claim_check_v1::{
        FRACTIONAL_CLAIM_CHECK_BYTES_V1, FractionalClaimCheckRedemptionRoleV1,
        FractionalClaimCheckSeedsV1, FractionalClaimCheckV1, FractionalRedeemClaimCheckRequestV1,
    },
};

use crate::{
    Error, Result, journal, stage,
    validator::{
        Validator, account_file, await_health, program_account_bytes, programdata_address,
        programdata_bytes, refusal_code, rent_exempt, rpc, write_account,
    },
};

const RPC_PORT: u16 = 20_962;
const PREFIX: &str = "claim-check-";
const CANONICAL: &str = "claim-check-canonical.json";
const OBSERVED: &str = "claim-check-observed.jsonl";
const MANIFEST: &str = "claim-check-manifest.json";
const SCHEMA: &str = "dclutch/fractional-claim-check-exterior/canonical/v2";
const ROUNDING_BOUNDARY: &str = "whole_claims=floor(requested_shard_atoms/denominator); consumed_shards=whole_claims*denominator";
const HOLDER_SEED: [u8; 32] = [0x5c; 32];
const CRANKER_SEED: [u8; 32] = [0x4d; 32];
const HOLDER_STARTING_LAMPORTS: u64 = 1_000_000_000;
const DENOMINATOR: u64 = 10;
const PAYOUT_PER_CLAIM: u64 = 4;
const OPENING_SHARDS: u64 = 40;
const OPENING_COLLATERAL: u64 = 16;
const PARTIAL_SHARDS: u64 = 20;
const PARTIAL_COLLATERAL: u64 = 8;

#[derive(Clone)]
struct StagedAccount {
    key: Pubkey,
    owner: Pubkey,
    data: Vec<u8>,
    lamports: u64,
}

struct Stage {
    aggregate: [u8; 32],
    record: Pubkey,
    escrow: Pubkey,
    vault: Pubkey,
    collateral_mint: Pubkey,
    shard_mint: Pubkey,
    holder: Pubkey,
    cranker: Pubkey,
    holder_collateral: Pubkey,
    cranker_collateral: Pubkey,
    holder_shards: Pubkey,
    accounts: Vec<StagedAccount>,
    compaction_bridge_sha256: String,
}

#[derive(Clone, Copy)]
enum Signers {
    Holder,
    Payer,
}

struct Action {
    name: &'static str,
    instruction: Instruction,
    signers: Signers,
    accepted: bool,
    refusal: Option<u32>,
    phase: Phase,
}

#[derive(Clone, Copy)]
enum Phase {
    Opening,
    Partial,
    Settled,
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SnapshotAccount {
    lamports: u64,
    owner: Pubkey,
    executable: bool,
    data: Vec<u8>,
}

#[derive(Clone, Debug)]
struct Entry {
    name: String,
    data_digest: String,
    frame_digest: String,
    accepted: bool,
    refusal: Option<u32>,
    poststate: Value,
}

struct ProgramDigests {
    claims: String,
    token_2022: String,
}

fn fixed(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    output[offset..offset + input.len()].copy_from_slice(input);
}

fn compacted_mint_bytes(controller: Pubkey, escrow: Pubkey, supply: u64) -> Vec<u8> {
    const TLV_START: usize = 166;
    let mut bytes = vec![0_u8; TLV_START];
    put(&mut bytes, 0, &1_u32.to_le_bytes());
    put(&mut bytes, 4, controller.as_ref());
    put(&mut bytes, 36, &supply.to_le_bytes());
    bytes[45] = 1;
    bytes[165] = 1;
    for (extension, authority) in [(3_u16, controller), (28_u16, escrow)] {
        bytes.extend_from_slice(&extension.to_le_bytes());
        bytes.extend_from_slice(&32_u16.to_le_bytes());
        bytes.extend_from_slice(authority.as_ref());
    }
    bytes
}

fn collateral_mint_bytes(supply: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; 82];
    put(&mut bytes, 36, &supply.to_le_bytes());
    bytes[44] = 6;
    bytes[45] = 1;
    bytes
}

fn token_account_bytes(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; 165];
    put(&mut bytes, 0, mint.as_ref());
    put(&mut bytes, 32, owner.as_ref());
    put(&mut bytes, 64, &amount.to_le_bytes());
    bytes[108] = 1;
    bytes
}

fn synthetic_stage() -> Result<Stage> {
    let claims = rpc(stage::CLAIMS);
    let token_program = rpc(stage::token_program());
    let holder = keypair_from_seed(&HOLDER_SEED)
        .map_err(|error| Error::new(error.to_string()))?
        .pubkey();
    let cranker = keypair_from_seed(&CRANKER_SEED)
        .map_err(|error| Error::new(error.to_string()))?
        .pubkey();
    let aggregate = [0x11; 32];
    let market = [0x12; 32];
    let release_set = [0x13; 32];
    let collateral_mint = fixed(0x14);
    let shard_mint = fixed(0x15);
    let controller = fixed(0x16);
    let holder_collateral = fixed(0x17);
    let holder_shards = fixed(0x18);
    let cranker_collateral = fixed(0x19);
    let position_atoms_digest = [0x1a; 32];
    let escrow_seeds = ClaimCheckEscrowSeedsV1::new(aggregate)
        .map_err(|_| Error::new("canonical aggregate did not derive an escrow"))?;
    let (escrow_program, escrow_bump) =
        ProgramPubkey::find_program_address(&escrow_seeds.as_slices(), &stage::CLAIMS);
    let vault_seeds = ClaimCheckVaultSeedsV1::new(aggregate)
        .map_err(|_| Error::new("canonical aggregate did not derive a vault"))?;
    let vault_program =
        ProgramPubkey::find_program_address(&vault_seeds.as_slices(), &stage::CLAIMS).0;
    let record_seeds = FractionalClaimCheckSeedsV1::new(aggregate, shard_mint.to_bytes())
        .map_err(|_| Error::new("canonical coordinates did not derive a record"))?;
    let (record_program, record_bump) =
        ProgramPubkey::find_program_address(&record_seeds.as_slices(), &stage::CLAIMS);
    let escrow = rpc(escrow_program);
    let vault = rpc(vault_program);
    let record = rpc(record_program);

    let escrow_bytes = ClaimCheckEscrowV1 {
        aggregate,
        market,
        release_set,
        vault: vault.to_bytes(),
        collateral_mint: collateral_mint.to_bytes(),
        opener: cranker.to_bytes(),
        opened_slot: 1,
        opener_outlay: rent_exempt(CLAIM_CHECK_ESCROW_BYTES_V1),
        outstanding_claim_checks: 1,
        generation: 37,
        bump: escrow_bump,
    }
    .new()
    .map_err(|_| Error::new("canonical escrow was internally invalid"))?
    .to_bytes()
    .map_err(|_| Error::new("canonical escrow did not encode"))?
    .to_vec();
    let record_bytes = FractionalClaimCheckV1 {
        aggregate,
        shard_mint: shard_mint.to_bytes(),
        market,
        release_set,
        vault: vault.to_bytes(),
        collateral_mint: collateral_mint.to_bytes(),
        position_atoms_digest,
        escrowed_atoms: OPENING_COLLATERAL,
        denominator: DENOMINATOR,
        payout_per_claim: PAYOUT_PER_CLAIM,
        compacted_shard_supply: OPENING_SHARDS,
        compacted_slot: 99,
        generation: 37,
        representation_coordinate: 1,
        bump: record_bump,
    }
    .new()
    .map_err(|_| Error::new("canonical fractional claim check was internally invalid"))?
    .to_bytes()
    .map_err(|_| Error::new("canonical fractional claim check did not encode"))?
    .to_vec();

    let accounts = vec![
        StagedAccount {
            key: record,
            owner: claims,
            lamports: rent_exempt(FRACTIONAL_CLAIM_CHECK_BYTES_V1),
            data: record_bytes,
        },
        StagedAccount {
            key: escrow,
            owner: claims,
            lamports: rent_exempt(CLAIM_CHECK_ESCROW_BYTES_V1),
            data: escrow_bytes,
        },
        StagedAccount {
            key: vault,
            owner: token_program,
            lamports: rent_exempt(165),
            data: token_account_bytes(collateral_mint, escrow, OPENING_COLLATERAL),
        },
        StagedAccount {
            key: collateral_mint,
            owner: token_program,
            lamports: rent_exempt(82),
            data: collateral_mint_bytes(OPENING_COLLATERAL),
        },
        StagedAccount {
            key: shard_mint,
            owner: token_program,
            lamports: rent_exempt(238),
            data: compacted_mint_bytes(controller, escrow, OPENING_SHARDS),
        },
        StagedAccount {
            key: holder_collateral,
            owner: token_program,
            lamports: rent_exempt(165),
            data: token_account_bytes(collateral_mint, holder, 0),
        },
        StagedAccount {
            key: cranker_collateral,
            owner: token_program,
            lamports: rent_exempt(165),
            data: token_account_bytes(collateral_mint, cranker, 0),
        },
        StagedAccount {
            key: holder_shards,
            owner: token_program,
            lamports: rent_exempt(165),
            data: token_account_bytes(shard_mint, holder, OPENING_SHARDS),
        },
        StagedAccount {
            key: holder,
            owner: Pubkey::default(),
            lamports: HOLDER_STARTING_LAMPORTS,
            data: Vec::new(),
        },
    ];

    Ok(Stage {
        aggregate,
        record,
        escrow,
        vault,
        collateral_mint,
        shard_mint,
        holder,
        cranker,
        holder_collateral,
        cranker_collateral,
        holder_shards,
        accounts,
        compaction_bridge_sha256: bridge_digest(b"standalone canonical compaction fixture v2"),
    })
}

fn staged_image(image: &AccountImageV1) -> Result<StagedAccount> {
    Ok(StagedAccount {
        key: Pubkey::new_from_array(image.address),
        owner: Pubkey::new_from_array(image.owner),
        data: STANDARD
            .decode(&image.data_base64)
            .map_err(|error| Error::new(format!("account image is not base64: {error}")))?,
        lamports: image.lamports,
    })
}

fn propagated_stage(path: &Path) -> Result<Stage> {
    if !path.is_absolute() {
        return Err(Error::new("compaction bridge path must be absolute").into());
    }
    let bridge = read_compaction(path).map_err(Error::new)?;
    let holder = keypair_from_seed(&HOLDER_SEED)
        .map_err(|error| Error::new(error.to_string()))?
        .pubkey();
    let cranker = keypair_from_seed(&CRANKER_SEED)
        .map_err(|error| Error::new(error.to_string()))?
        .pubkey();
    if holder.to_bytes() != bridge.preterminal.holder || cranker.to_bytes() != bridge.closer {
        return Err(Error::new("bridge substituted a transaction signer").into());
    }
    let aggregate = bridge.preterminal.aggregate;
    let shard_mint = Pubkey::new_from_array(bridge.preterminal.shard_mint);
    let record = Pubkey::new_from_array(bridge.record.address);
    let escrow = Pubkey::new_from_array(bridge.escrow.address);
    let vault = Pubkey::new_from_array(bridge.vault.address);
    let expected_record = ProgramPubkey::find_program_address(
        &FractionalClaimCheckSeedsV1::new(aggregate, shard_mint.to_bytes())
            .map_err(|_| Error::new("bridge record coordinates are invalid"))?
            .as_slices(),
        &stage::CLAIMS,
    )
    .0;
    let expected_escrow = ProgramPubkey::find_program_address(
        &ClaimCheckEscrowSeedsV1::new(aggregate)
            .map_err(|_| Error::new("bridge aggregate cannot derive escrow"))?
            .as_slices(),
        &stage::CLAIMS,
    )
    .0;
    let expected_vault = ProgramPubkey::find_program_address(
        &ClaimCheckVaultSeedsV1::new(aggregate)
            .map_err(|_| Error::new("bridge aggregate cannot derive vault"))?
            .as_slices(),
        &stage::CLAIMS,
    )
    .0;
    if record.to_bytes() != expected_record.to_bytes()
        || escrow.to_bytes() != expected_escrow.to_bytes()
        || vault.to_bytes() != expected_vault.to_bytes()
    {
        return Err(Error::new("bridge substituted a Claims PDA").into());
    }

    let accounts = [
        &bridge.record,
        &bridge.escrow,
        &bridge.vault,
        &bridge.collateral_mint,
        &bridge.shard_mint,
        &bridge.holder_collateral,
        &bridge.closer_collateral,
        &bridge.holder_shards,
    ]
    .into_iter()
    .map(staged_image)
    .collect::<Result<Vec<_>>>()?;
    let claims = rpc(stage::CLAIMS);
    let token = rpc(stage::token_program());
    for (index, account) in accounts.iter().enumerate() {
        let expected_owner = if index < 2 { claims } else { token };
        if account.owner != expected_owner {
            return Err(Error::new("bridge account owner substitution").into());
        }
    }
    let record_bytes = &accounts[0].data;
    let escrow_bytes = &accounts[1].data;
    let record_state = FractionalClaimCheckV1::decode(record_bytes)
        .map_err(|_| Error::new("bridge claim-check record does not decode"))?;
    let escrow_state = ClaimCheckEscrowV1::decode(escrow_bytes)
        .map_err(|_| Error::new("bridge escrow does not decode"))?;
    if record_state.aggregate != aggregate
        || record_state.shard_mint != shard_mint.to_bytes()
        || record_state.vault != vault.to_bytes()
        || record_state.denominator != DENOMINATOR
        || record_state.payout_per_claim != PAYOUT_PER_CLAIM
        || record_state.compacted_shard_supply != OPENING_SHARDS
        || record_state.escrowed_atoms != OPENING_COLLATERAL
        || escrow_state.aggregate != aggregate
        || escrow_state.vault != vault.to_bytes()
        || escrow_state.outstanding_claim_checks != 1
    {
        return Err(Error::new("bridge persisted state does not match its lifecycle facts").into());
    }
    if balance(&accounts[2].data, 64)? != OPENING_COLLATERAL
        || balance(&accounts[4].data, 36)? != OPENING_SHARDS
        || balance(&accounts[7].data, 64)? != OPENING_SHARDS
        || balance(&accounts[5].data, 64)? != 0
        || balance(&accounts[6].data, 64)? != 0
    {
        return Err(Error::new("bridge Token-2022 conservation mismatch").into());
    }
    let compaction_bridge_sha256 =
        bridge_digest(&bridge_bytes(&bridge).map_err(|error| Error::new(error.to_string()))?);
    let mut accounts = accounts;
    accounts.push(StagedAccount {
        key: holder,
        owner: Pubkey::default(),
        lamports: HOLDER_STARTING_LAMPORTS,
        data: Vec::new(),
    });
    Ok(Stage {
        aggregate,
        record,
        escrow,
        vault,
        collateral_mint: Pubkey::new_from_array(bridge.collateral_mint.address),
        shard_mint,
        holder,
        cranker,
        holder_collateral: Pubkey::new_from_array(bridge.holder_collateral.address),
        cranker_collateral: Pubkey::new_from_array(bridge.closer_collateral.address),
        holder_shards: Pubkey::new_from_array(bridge.holder_shards.address),
        accounts,
        compaction_bridge_sha256,
    })
}

fn stage(bridge: Option<&Path>) -> Result<Stage> {
    bridge.map_or_else(synthetic_stage, propagated_stage)
}

fn redemption_instruction(
    staged: &Stage,
    requested_shards: u64,
    holder: Pubkey,
    holder_collateral: Pubkey,
) -> Result<Instruction> {
    use FractionalClaimCheckRedemptionRoleV1 as Role;
    let accounts = Role::frame()
        .iter()
        .map(|role| {
            let key = match role {
                Role::Holder => holder,
                Role::FractionalClaimCheckRecord => staged.record,
                Role::Escrow => staged.escrow,
                Role::Vault => staged.vault,
                Role::HolderCollateralTokens => holder_collateral,
                Role::CollateralMint => staged.collateral_mint,
                Role::ShardMint => staged.shard_mint,
                Role::HolderShardTokens => staged.holder_shards,
                Role::TokenProgram => rpc(stage::token_program()),
                Role::FractionalCapabilityRoot => unreachable!("retired root is not in the frame"),
            };
            let (signer, writable) = role.privileges();
            AccountMeta {
                pubkey: key,
                is_signer: signer,
                is_writable: writable,
            }
        })
        .collect();
    let data = FractionalRedeemClaimCheckRequestV1 {
        aggregate: staged.aggregate,
        shard_mint: staged.shard_mint.to_bytes(),
        requested_shard_atoms: requested_shards,
    }
    .new()
    .map_err(|_| Error::new("fractional redemption request was invalid"))?
    .to_bytes()
    .map_err(|_| Error::new("fractional redemption request did not encode"))?
    .to_vec();
    Ok(Instruction {
        program_id: rpc(stage::CLAIMS),
        accounts,
        data,
    })
}

fn close_instruction(staged: &Stage) -> Result<Instruction> {
    let data = CloseClaimCheckEscrowRequestV1 {
        aggregate: staged.aggregate,
    }
    .new()
    .map_err(|_| Error::new("claim-check close request was invalid"))?
    .to_bytes()
    .map_err(|_| Error::new("claim-check close request did not encode"))?
    .to_vec();
    Ok(Instruction {
        program_id: rpc(stage::CLAIMS),
        accounts: vec![
            AccountMeta::new(staged.cranker, true),
            AccountMeta::new(staged.escrow, false),
            AccountMeta::new(staged.vault, false),
            AccountMeta::new(staged.cranker_collateral, false),
            AccountMeta::new_readonly(staged.collateral_mint, false),
            AccountMeta::new_readonly(rpc(stage::token_program()), false),
        ],
        data,
    })
}

fn actions(staged: &Stage) -> Result<Vec<Action>> {
    Ok(vec![
        Action {
            name: "dust-refusal",
            instruction: redemption_instruction(
                staged,
                DENOMINATOR - 1,
                staged.holder,
                staged.holder_collateral,
            )?,
            signers: Signers::Holder,
            accepted: false,
            refusal: Some(0x5665),
            phase: Phase::Opening,
        },
        Action {
            name: "overdraw-refusal",
            instruction: redemption_instruction(
                staged,
                OPENING_SHARDS + 1,
                staged.holder,
                staged.holder_collateral,
            )?,
            signers: Signers::Holder,
            accepted: false,
            refusal: Some(0x5663),
            phase: Phase::Opening,
        },
        Action {
            name: "substituted-payout-refusal",
            instruction: redemption_instruction(
                staged,
                DENOMINATOR,
                staged.holder,
                staged.cranker_collateral,
            )?,
            signers: Signers::Holder,
            accepted: false,
            refusal: Some(0x5661),
            phase: Phase::Opening,
        },
        Action {
            name: "substituted-holder-refusal",
            instruction: redemption_instruction(
                staged,
                DENOMINATOR,
                staged.cranker,
                staged.holder_collateral,
            )?,
            signers: Signers::Payer,
            accepted: false,
            refusal: Some(0x5661),
            phase: Phase::Opening,
        },
        Action {
            name: "partial-redemption",
            instruction: redemption_instruction(
                staged,
                PARTIAL_SHARDS,
                staged.holder,
                staged.holder_collateral,
            )?,
            signers: Signers::Holder,
            accepted: true,
            refusal: None,
            phase: Phase::Partial,
        },
        Action {
            name: "premature-close-refusal",
            instruction: close_instruction(staged)?,
            signers: Signers::Payer,
            accepted: false,
            refusal: Some(0x5625),
            phase: Phase::Partial,
        },
        Action {
            name: "settling-redemption",
            instruction: redemption_instruction(
                staged,
                OPENING_SHARDS - PARTIAL_SHARDS,
                staged.holder,
                staged.holder_collateral,
            )?,
            signers: Signers::Holder,
            accepted: true,
            refusal: None,
            phase: Phase::Settled,
        },
        Action {
            name: "permissionless-close",
            instruction: close_instruction(staged)?,
            signers: Signers::Payer,
            accepted: true,
            refusal: None,
            phase: Phase::Closed,
        },
    ])
}

fn with_lamports(mut value: Value, lamports: u64) -> Value {
    value["account"]["lamports"] = json!(lamports);
    value
}

fn prepare_with_bridge(elf_dir: &Path, out: &Path, bridge: Option<&Path>) -> Result<usize> {
    let accounts_dir = out.join(format!("{PREFIX}accounts"));
    if accounts_dir.exists() {
        fs::remove_dir_all(&accounts_dir)?;
    }
    fs::create_dir_all(&accounts_dir)?;
    for name in [CANONICAL, OBSERVED] {
        let path = out.join(name);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }

    let staged = stage(bridge)?;
    let mut written = 0_usize;
    for account in &staged.accounts {
        let value = with_lamports(
            account_file(&account.key, &account.owner, &account.data, false),
            account.lamports,
        );
        write_account(&accounts_dir, &value)?;
        written += 1;
    }

    let claims = crate::read_elf(elf_dir, "dclutch_claims_sbf.so")?;
    let token = crate::read_elf(elf_dir, "spl_token_2022.so")?;
    for (program, elf) in [
        (rpc(stage::CLAIMS), claims.as_slice()),
        (rpc(stage::token_program()), token.as_slice()),
    ] {
        let programdata = programdata_address(&program);
        write_account(
            &accounts_dir,
            &account_file(
                &program,
                &rpc(stage::loader()),
                &program_account_bytes(&programdata),
                true,
            ),
        )?;
        write_account(
            &accounts_dir,
            &account_file(
                &programdata,
                &rpc(stage::loader()),
                &programdata_bytes(elf),
                false,
            ),
        )?;
        written += 2;
    }

    let manifest = json!({
        "schema": "dclutch/fractional-claim-check-exterior/manifest/v1",
        "accounts": written,
        "claims_elf_sha256": journal::digest(&claims),
        "token_2022_elf_sha256": journal::digest(&token),
        "holder": staged.holder.to_string(),
        "permissionless_closer": staged.cranker.to_string(),
        "aggregate": Pubkey::new_from_array(staged.aggregate).to_string(),
        "record": staged.record.to_string(),
        "escrow": staged.escrow.to_string(),
        "vault": staged.vault.to_string(),
        "shard_mint": staged.shard_mint.to_string(),
        "compaction_bridge_sha256": staged.compaction_bridge_sha256,
        "actions": actions(&staged)?.iter().map(|action| action.name).collect::<Vec<_>>(),
    });
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    fs::write(out.join(MANIFEST), bytes)?;
    Ok(written)
}

/// Stage the standalone canonical post-compaction control and both real ELFs.
pub fn prepare(elf_dir: &Path, out: &Path) -> Result<usize> {
    prepare_with_bridge(elf_dir, out, None)
}

fn account_at(client: &RpcClient, key: Pubkey) -> Result<Option<Account>> {
    Ok(client
        .get_account_with_commitment(&key, CommitmentConfig::finalized())?
        .value)
}

fn balance(data: &[u8], offset: usize) -> Result<u64> {
    let bytes: [u8; 8] = data
        .get(offset..offset + 8)
        .ok_or_else(|| Error::new("Token account was shorter than its base balance field"))?
        .try_into()
        .map_err(|_| Error::new("Token balance field did not have exact width"))?;
    Ok(u64::from_le_bytes(bytes))
}

fn poststate(client: &RpcClient, staged: &Stage) -> Result<Value> {
    let record = account_at(client, staged.record)?;
    let escrow = account_at(client, staged.escrow)?;
    let vault = account_at(client, staged.vault)?;
    let shard_mint = account_at(client, staged.shard_mint)?
        .ok_or_else(|| Error::new("shard Mint disappeared"))?;
    let holder_shards = account_at(client, staged.holder_shards)?
        .ok_or_else(|| Error::new("holder shard account disappeared"))?;
    let holder_collateral = account_at(client, staged.holder_collateral)?
        .ok_or_else(|| Error::new("holder collateral account disappeared"))?;
    let cranker_collateral = account_at(client, staged.cranker_collateral)?
        .ok_or_else(|| Error::new("closer collateral account disappeared"))?;
    let holder = account_at(client, staged.holder)?
        .ok_or_else(|| Error::new("holder system account disappeared"))?;
    let record_escrowed = record
        .as_ref()
        .map(|account| FractionalClaimCheckV1::decode(&account.data))
        .transpose()
        .map_err(|_| Error::new("live fractional record did not decode"))?
        .map(|record| record.escrowed_atoms);
    let outstanding = escrow
        .as_ref()
        .map(|account| ClaimCheckEscrowV1::decode(&account.data))
        .transpose()
        .map_err(|_| Error::new("live claim-check escrow did not decode"))?
        .map(|escrow| escrow.outstanding_claim_checks);
    let vault_amount = vault
        .as_ref()
        .map(|account| balance(&account.data, 64))
        .transpose()?;
    Ok(json!({
        "shard_mint_supply": balance(&shard_mint.data, 36)?,
        "holder_shard_atoms": balance(&holder_shards.data, 64)?,
        "vault_collateral_atoms": vault_amount,
        "holder_collateral_atoms": balance(&holder_collateral.data, 64)?,
        "closer_collateral_atoms": balance(&cranker_collateral.data, 64)?,
        "record_escrowed_atoms": record_escrowed,
        "record_lamports": record.as_ref().map_or(0, |account| account.lamports),
        "holder_lamports": holder.lamports,
        "escrow_outstanding_claim_checks": outstanding,
        "escrow_lamports": escrow.as_ref().map_or(0, |account| account.lamports),
        "vault_lamports": vault.as_ref().map_or(0, |account| account.lamports),
    }))
}

fn expected_poststate(phase: Phase) -> Value {
    let record_rent = rent_exempt(FRACTIONAL_CLAIM_CHECK_BYTES_V1);
    let escrow_rent = rent_exempt(CLAIM_CHECK_ESCROW_BYTES_V1);
    let vault_rent = rent_exempt(165);
    match phase {
        Phase::Opening => json!({
            "shard_mint_supply": OPENING_SHARDS,
            "holder_shard_atoms": OPENING_SHARDS,
            "vault_collateral_atoms": OPENING_COLLATERAL,
            "holder_collateral_atoms": 0,
            "closer_collateral_atoms": 0,
            "record_escrowed_atoms": OPENING_COLLATERAL,
            "record_lamports": record_rent,
            "holder_lamports": HOLDER_STARTING_LAMPORTS,
            "escrow_outstanding_claim_checks": 1,
            "escrow_lamports": escrow_rent,
            "vault_lamports": vault_rent,
        }),
        Phase::Partial => json!({
            "shard_mint_supply": OPENING_SHARDS - PARTIAL_SHARDS,
            "holder_shard_atoms": OPENING_SHARDS - PARTIAL_SHARDS,
            "vault_collateral_atoms": OPENING_COLLATERAL - PARTIAL_COLLATERAL,
            "holder_collateral_atoms": PARTIAL_COLLATERAL,
            "closer_collateral_atoms": 0,
            "record_escrowed_atoms": OPENING_COLLATERAL - PARTIAL_COLLATERAL,
            "record_lamports": record_rent,
            "holder_lamports": HOLDER_STARTING_LAMPORTS,
            "escrow_outstanding_claim_checks": 1,
            "escrow_lamports": escrow_rent,
            "vault_lamports": vault_rent,
        }),
        Phase::Settled => json!({
            "shard_mint_supply": 0,
            "holder_shard_atoms": 0,
            "vault_collateral_atoms": 0,
            "holder_collateral_atoms": OPENING_COLLATERAL,
            "closer_collateral_atoms": 0,
            "record_escrowed_atoms": Value::Null,
            "record_lamports": 0,
            "holder_lamports": HOLDER_STARTING_LAMPORTS + record_rent,
            "escrow_outstanding_claim_checks": 0,
            "escrow_lamports": escrow_rent,
            "vault_lamports": vault_rent,
        }),
        Phase::Closed => json!({
            "shard_mint_supply": 0,
            "holder_shard_atoms": 0,
            "vault_collateral_atoms": Value::Null,
            "holder_collateral_atoms": OPENING_COLLATERAL,
            "closer_collateral_atoms": 0,
            "record_escrowed_atoms": Value::Null,
            "record_lamports": 0,
            "holder_lamports": HOLDER_STARTING_LAMPORTS + record_rent,
            "escrow_outstanding_claim_checks": Value::Null,
            "escrow_lamports": 0,
            "vault_lamports": 0,
        }),
    }
}

fn protocol_snapshot(client: &RpcClient, staged: &Stage) -> Result<Vec<Option<SnapshotAccount>>> {
    let keys = [
        staged.record,
        staged.escrow,
        staged.vault,
        staged.collateral_mint,
        staged.shard_mint,
        staged.holder_collateral,
        staged.cranker_collateral,
        staged.holder_shards,
    ];
    Ok(client
        .get_multiple_accounts(&keys)?
        .into_iter()
        .map(|account| {
            account.map(|account| SnapshotAccount {
                lamports: account.lamports,
                owner: account.owner,
                executable: account.executable,
                data: account.data,
            })
        })
        .collect())
}

fn frame_bytes(instruction: &Instruction) -> Vec<u8> {
    let mut output = Vec::with_capacity(instruction.accounts.len() * 34);
    for account in &instruction.accounts {
        output.extend_from_slice(account.pubkey.as_ref());
        output.push(u8::from(account.is_signer));
        output.push(u8::from(account.is_writable));
    }
    output
}

fn append_observed(out: &Path, value: &Value) -> Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join(OBSERVED))?;
    file.write_all(serde_json::to_string(value)?.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn submit(
    client: &RpcClient,
    payer: &Keypair,
    holder: &Keypair,
    action: &Action,
) -> Result<(bool, Option<u32>, String, usize, u64)> {
    let budget = ComputeBudgetInstruction::set_compute_unit_limit(500_000);
    let blockhash = client.get_latest_blockhash()?;
    let signers: Vec<&dyn Signer> = match action.signers {
        Signers::Holder => vec![payer, holder],
        Signers::Payer => vec![payer],
    };
    let transaction = Transaction::new_signed_with_payer(
        &[budget, action.instruction.clone()],
        Some(&payer.pubkey()),
        &signers,
        blockhash,
    );
    let wire = bincode::serialize(&transaction)?.len();
    let fee = client.get_fee_for_message(&transaction.message)?;
    match client.send_and_confirm_transaction(&transaction) {
        Ok(signature) => Ok((true, None, signature.to_string(), wire, fee)),
        Err(error) => {
            let detail = error.to_string();
            Ok((false, refusal_code(&detail), detail, wire, fee))
        }
    }
}

fn manifest_digests(out: &Path) -> Result<ProgramDigests> {
    let value: Value = serde_json::from_slice(&fs::read(out.join(MANIFEST))?)?;
    if value.get("schema").and_then(Value::as_str)
        != Some("dclutch/fractional-claim-check-exterior/manifest/v1")
    {
        return Err(Error::new("claim-check manifest has the wrong schema").into());
    }
    let read = |field: &str| -> Result<String> {
        let value = value
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new(format!("claim-check manifest has no {field}")))?;
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::new(format!("claim-check manifest has malformed {field}")).into());
        }
        Ok(value.to_string())
    };
    Ok(ProgramDigests {
        claims: read("claims_elf_sha256")?,
        token_2022: read("token_2022_elf_sha256")?,
    })
}

fn write_canonical(
    out: &Path,
    entries: &[Entry],
    programs: &ProgramDigests,
    staged: &Stage,
) -> Result<String> {
    let value = json!({
        "schema": SCHEMA,
        "claims_elf_sha256": programs.claims,
        "token_2022_elf_sha256": programs.token_2022,
        "compaction_bridge_sha256": staged.compaction_bridge_sha256,
        "rounding_boundary": ROUNDING_BOUNDARY,
        "entries": entries.iter().map(|entry| json!({
            "action": entry.name,
            "instruction_data_sha256": entry.data_digest,
            "account_frame_sha256": entry.frame_digest,
            "accepted": entry.accepted,
            "refusal": entry.refusal,
            "poststate": entry.poststate,
        })).collect::<Vec<_>>(),
    });
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    fs::write(out.join(CANONICAL), &bytes)?;
    Ok(journal::digest(&bytes))
}

fn run_with_bridge(elf_dir: &Path, out: &Path, keep: bool, bridge: Option<&Path>) -> Result<()> {
    let written = prepare_with_bridge(elf_dir, out, bridge)?;
    println!("staged {written} post-compaction genesis accounts");
    let staged = stage(bridge)?;
    let holder = keypair_from_seed(&HOLDER_SEED).map_err(|error| Error::new(error.to_string()))?;
    let payer = keypair_from_seed(&CRANKER_SEED).map_err(|error| Error::new(error.to_string()))?;
    if holder.pubkey() != staged.holder || payer.pubkey() != staged.cranker {
        return Err(Error::new("deterministic exterior identities drifted").into());
    }
    let validator = Validator::start_on(out, RPC_PORT, PREFIX)?;
    let client = RpcClient::new_with_commitment(
        format!("http://127.0.0.1:{RPC_PORT}"),
        CommitmentConfig::finalized(),
    );
    let outcome = (|| -> Result<Vec<Entry>> {
        await_health(&client)?;
        let signature = client.request_airdrop(&payer.pubkey(), 5_000_000_000)?;
        let deadline = Instant::now() + Duration::from_secs(60);
        while !client.confirm_transaction(&signature)? {
            if Instant::now() > deadline {
                return Err(Error::new("claim-check payer airdrop never confirmed").into());
            }
            sleep(Duration::from_millis(250));
        }
        let mut entries = Vec::new();
        for action in actions(&staged)? {
            let before = protocol_snapshot(&client, &staged)?;
            let closer_before = client.get_balance(&staged.cranker)?;
            let vault_rent_before =
                account_at(&client, staged.vault)?.map_or(0, |account| account.lamports);
            let escrow_rent_before =
                account_at(&client, staged.escrow)?.map_or(0, |account| account.lamports);
            let (accepted, refusal, signature_or_detail, wire, fee) =
                submit(&client, &payer, &holder, &action)?;
            if accepted != action.accepted || refusal != action.refusal {
                return Err(Error::new(format!(
                    "{} outcome drifted: expected accepted={} refusal={:?}, got accepted={accepted} refusal={refusal:?}: {signature_or_detail}",
                    action.name, action.accepted, action.refusal,
                ))
                .into());
            }
            if !accepted {
                let after = protocol_snapshot(&client, &staged)?;
                if after != before {
                    return Err(Error::new(format!(
                        "{} changed a protocol or Token account despite refusal",
                        action.name
                    ))
                    .into());
                }
            }
            let state = poststate(&client, &staged)?;
            let expected = expected_poststate(action.phase);
            if state != expected {
                return Err(Error::new(format!(
                    "{} reached an unexpected poststate: expected {} got {}",
                    action.name,
                    serde_json::to_string(&expected)?,
                    serde_json::to_string(&state)?,
                ))
                .into());
            }
            let closer_after = client.get_balance(&staged.cranker)?;
            if action.name == "permissionless-close" {
                let protocol_credit = vault_rent_before
                    .checked_add(escrow_rent_before)
                    .ok_or_else(|| Error::new("closer rent credit overflow"))?;
                let expected = closer_before
                    .checked_sub(fee)
                    .and_then(|balance| balance.checked_add(protocol_credit))
                    .ok_or_else(|| Error::new("closer balance conservation overflow"))?;
                if closer_after != expected {
                    return Err(Error::new("closer did not receive both live rent balances").into());
                }
            }
            append_observed(
                out,
                &json!({
                    "action": action.name,
                    "accepted": accepted,
                    "signature_or_detail": signature_or_detail,
                    "wire_bytes": wire,
                    "transaction_fee_lamports": fee,
                    "closer_balance_before": closer_before,
                    "closer_balance_after": closer_after,
                }),
            )?;
            println!(
                "{:>30}  accepted={accepted} refusal={refusal:?}  {}",
                action.name,
                serde_json::to_string(&state)?
            );
            entries.push(Entry {
                name: action.name.to_string(),
                data_digest: journal::digest(&action.instruction.data),
                frame_digest: journal::digest(&frame_bytes(&action.instruction)),
                accepted,
                refusal,
                poststate: state,
            });
        }
        Ok(entries)
    })();

    if keep {
        println!("claim-check validator left running on port {RPC_PORT}");
    } else {
        validator.stop();
    }
    let entries = outcome?;
    let programs = manifest_digests(out)?;
    let digest = write_canonical(out, &entries, &programs, &staged)?;
    println!("claim-check canonical journal sha256 {digest}");
    Ok(())
}

/// Run the standalone canonical hostile and successful claim-check control.
pub fn run(elf_dir: &Path, out: &Path, keep: bool) -> Result<()> {
    run_with_bridge(elf_dir, out, keep, None)
}

/// Run the hostile and successful claim-check life from exact compaction output.
pub fn run_propagated(elf_dir: &Path, out: &Path, keep: bool, bridge: &Path) -> Result<()> {
    run_with_bridge(elf_dir, out, keep, Some(bridge))
}

fn digest_is_valid(value: &Value) -> bool {
    value.as_str().is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn verify_with_bridge(out: &Path, bridge: Option<&Path>) -> Result<(usize, String)> {
    let bytes = fs::read(out.join(CANONICAL))?;
    let value: Value = serde_json::from_slice(&bytes)?;
    if value.get("schema").and_then(Value::as_str) != Some(SCHEMA) {
        return Err(Error::new("claim-check journal has the wrong schema").into());
    }
    if value.as_object().map(|object| object.len()) != Some(6)
        || value.get("rounding_boundary").and_then(Value::as_str) != Some(ROUNDING_BOUNDARY)
    {
        return Err(Error::new("claim-check journal header is not exact").into());
    }
    let programs = manifest_digests(out)?;
    if value.get("claims_elf_sha256").and_then(Value::as_str) != Some(programs.claims.as_str())
        || value.get("token_2022_elf_sha256").and_then(Value::as_str)
            != Some(programs.token_2022.as_str())
    {
        return Err(Error::new("claim-check journal is not bound to its staged ELFs").into());
    }
    let staged = stage(bridge)?;
    if value
        .get("compaction_bridge_sha256")
        .and_then(Value::as_str)
        != Some(staged.compaction_bridge_sha256.as_str())
    {
        return Err(Error::new("claim-check journal is not bound to its compaction bridge").into());
    }
    let expected_actions = actions(&staged)?;
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new("claim-check journal has no entries array"))?;
    if entries.len() != expected_actions.len() {
        return Err(Error::new(format!(
            "claim-check journal has {} entries; {} are required",
            entries.len(),
            expected_actions.len()
        ))
        .into());
    }
    for (entry, expected) in entries.iter().zip(&expected_actions) {
        let expected_data_digest = journal::digest(&expected.instruction.data);
        let expected_frame_digest = journal::digest(&frame_bytes(&expected.instruction));
        let expected_refusal = expected.refusal.map_or(Value::Null, |value| json!(value));
        if entry.as_object().map(|object| object.len()) != Some(6)
            || entry.get("action").and_then(Value::as_str) != Some(expected.name)
            || entry.get("accepted").and_then(Value::as_bool) != Some(expected.accepted)
            || entry.get("refusal") != Some(&expected_refusal)
            || entry.get("poststate") != Some(&expected_poststate(expected.phase))
            || entry.get("instruction_data_sha256").and_then(Value::as_str)
                != Some(expected_data_digest.as_str())
            || entry.get("account_frame_sha256").and_then(Value::as_str)
                != Some(expected_frame_digest.as_str())
        {
            return Err(Error::new(format!(
                "claim-check journal entry {} is not exact",
                expected.name
            ))
            .into());
        }
        if !digest_is_valid(&entry["instruction_data_sha256"])
            || !digest_is_valid(&entry["account_frame_sha256"])
        {
            return Err(Error::new(format!(
                "claim-check journal entry {} has a malformed digest",
                expected.name
            ))
            .into());
        }
    }
    let mut canonical = serde_json::to_vec_pretty(&value)?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(Error::new("claim-check journal bytes are not canonical JSON").into());
    }
    Ok((entries.len(), journal::digest(&bytes)))
}

/// Re-read the standalone claim-check journal and verify its exact contract.
pub fn verify(out: &Path) -> Result<(usize, String)> {
    verify_with_bridge(out, None)
}

/// Verify a claim-check journal against the exact compaction bridge it consumed.
pub fn verify_propagated(out: &Path, bridge: &Path) -> Result<(usize, String)> {
    verify_with_bridge(out, Some(bridge))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_claim_check_is_exact_and_internally_joined() {
        let staged = stage(None).expect("canonical stage");
        let record = staged
            .accounts
            .iter()
            .find(|account| account.key == staged.record)
            .expect("record");
        let escrow = staged
            .accounts
            .iter()
            .find(|account| account.key == staged.escrow)
            .expect("escrow");
        let record = FractionalClaimCheckV1::decode(&record.data).expect("record decodes");
        let escrow = ClaimCheckEscrowV1::decode(&escrow.data).expect("escrow decodes");
        assert_eq!(record.escrowed_atoms, OPENING_COLLATERAL);
        assert!(record.opening_escrow_is_consistent(OPENING_COLLATERAL));
        assert_eq!(record.vault, escrow.vault);
        assert_eq!(record.collateral_mint, escrow.collateral_mint);
        assert_eq!(escrow.outstanding_claim_checks, 1);
    }

    #[test]
    fn campaign_has_four_hostile_walls_and_a_complete_life() {
        let staged = stage(None).expect("canonical stage");
        let actions = actions(&staged).expect("actions");
        assert_eq!(actions.len(), 8);
        assert_eq!(
            actions
                .iter()
                .filter(|action| !action.accepted)
                .map(|action| action.refusal.expect("refusal"))
                .collect::<Vec<_>>(),
            vec![0x5665, 0x5663, 0x5661, 0x5661, 0x5625]
        );
        assert!(matches!(
            actions.last().map(|action| action.phase),
            Some(Phase::Closed)
        ));
    }
}
