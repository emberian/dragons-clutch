//! First-use creation of a Market's CLAIMS-role Custody replay.
//!
//! # Why this route exists
//!
//! [`CustodyReplayV1::advance`] binds `request.caller_role == self.caller_role`,
//! and decision 0008's addendum makes the role a seed component of the replay
//! PDA. Each executing role therefore owns its own replay compartment under one
//! Market namespace — and each role's compartment has to be created by someone.
//!
//! Custody's `InitializeReplay` is the generic transition that creates one, and
//! it is not reachable by proxy: its frame requires a `CallerAuthority` PDA
//! derived under `request.caller_program` and SIGNED, and Custody separately
//! authenticates that `caller_program` is the Registry-activated program for
//! `request.caller_role`. Only the Claims program can produce a Claims-role
//! caller authority, so only the Claims program can create the Claims-role
//! replay. Not the founding, not Core's legacy Open, not a wallet.
//!
//! Every other family already creates its own the same way and as its own
//! transition: Direct's escrow plan opens with an `InitializeReplay`
//! (`trading-sbf/direct/buy_escrow.rs`), Series' does
//! (`trading-sbf/series/custody_v3.rs`), and Core's legacy Open dispatches one
//! as a distinct outer route (`core-sbf/open_market.rs`). NO route in the tree
//! creates a replay as a side effect of a transfer, and this one does not
//! either — folding creation into a payout would put a variable-width frame and
//! a rent payer inside the fixed frame of an economic transition that must not
//! depend on either.
//!
//! # Prepaid, selected, canonically addressed
//!
//! Decision 0001 allows physical lazy creation only for an "already selected,
//! canonically addressed, fully prepaid child". All three hold here and none of
//! them is a caller's promise:
//!
//! - **Selected**: the role is Claims and the namespace is the aggregate's
//!   persisted `custody_context` (decision 0008 §1). Neither is a request field
//!   this route will read from the caller.
//! - **Canonically addressed**: the replay PDA is
//!   [`CustodyReplaySeedsV1`] under the Custody program, and both Claims and
//!   Custody derive it independently from the same request.
//! - **Fully prepaid**: `rent_lamports` must equal the Rent sysvar's exact
//!   minimum for the replay width, the payer signs for it, and the payer is
//!   written into the replay as its immutable `rent_refund`, so `CloseReplay`
//!   returns the rent to whoever advanced it.
//!
//! # The request is not caller-chosen
//!
//! The instruction data is one canonical [`CustodyRequestV1`] — the Custody ABI
//! itself, dispatched by its own magic, exactly as `core-sbf/open_market.rs`
//! carries a Custody request through Core. This route does not TRUST one byte of
//! it: it recomputes the whole request from the aggregate, the Rent sysvar and
//! the payer account, and refuses anything that is not byte-identical. The only
//! thing a caller decides is which account pays the rent and receives the
//! refund. Creation is therefore permissionless without being permissive: two
//! different callers submitting this route against one Market submit the same
//! 672 bytes except for the payer, and the second one finds the account already
//! exists.
//!
//! # Return data
//!
//! Custody's receipt is left in place rather than re-emitted under this
//! program's name. Its `producer` field names Custody, which is the truth —
//! Claims performed no economic transition here, it authorized one physical
//! creation. This route is a top-level transaction and is not composed as a
//! child of the Trading walk, which is what would need a Claims-produced
//! receipt.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::{
    ClaimsAggregateSeedsV1, liability_basis_state_v2::LiabilityBasisMarketViewV2,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_RECEIPT_BYTES_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyReceiptV1, CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestV1, OperationV1,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::ClaimsSbfError;

/// Domain separating this route's synthetic parent digest.
///
/// `CustodyRequestV1::validate` requires a nonzero `parent_request_digest`, and
/// this route has no parent action to name: it is submitted on its own, before
/// any redemption exists. The digest is therefore a function of exactly the
/// facts that determine the request, so it is reproducible by anyone holding the
/// aggregate and adds no caller freedom.
pub const CLAIMS_CUSTODY_REPLAY_PARENT_DOMAIN_V1: &[u8] =
    b"dclutch:claims-custody-replay-parent:v1";

/// Exact account count for this route.
pub const CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1: usize = 14;

/// Custody frame coordinate: release-pinned Claims caller authority.
pub const CUSTODY_CALLER_AUTHORITY: usize = 0;
/// Custody frame coordinate: canonical Core Market state.
pub const CORE_MARKET: usize = 1;
/// Custody frame coordinate: Registry activation cache.
pub const ACTIVATION_CACHE: usize = 2;
/// Custody frame coordinate: Registry program.
pub const REGISTRY_PROGRAM: usize = 3;
/// Custody frame coordinate: the calling program, which is this one.
pub const CLAIMS_PROGRAM: usize = 4;
/// Custody frame coordinate: this program's ProgramData.
pub const CLAIMS_PROGRAMDATA: usize = 5;
/// Custody frame coordinate: finalized Realm record.
pub const REALM: usize = 6;
/// Custody frame coordinate: vacant Realm staging cursor.
pub const REALM_STAGING: usize = 7;
/// Custody frame coordinate: the vacant Claims-role replay to create.
pub const CUSTODY_REPLAY: usize = 8;
/// Custody frame coordinate: rent payer.
pub const PAYER: usize = 9;
/// Custody frame coordinate: System program.
pub const SYSTEM_PROGRAM: usize = 10;
/// Custody frame coordinate: Rent sysvar.
pub const RENT_SYSVAR: usize = 11;
/// The Custody program invoked by this route.
pub const CUSTODY_PROGRAM: usize = 12;
/// The Claims aggregate that owns the Market's Custody namespace.
pub const AGGREGATE: usize = 13;

/// The exact width of the Custody frame this route forwards, unchanged.
const CUSTODY_FRAME_ACCOUNT_COUNT: usize =
    dclutch_custody_contract::INITIALIZE_REPLAY_ACCOUNT_COUNT_V1 as usize;

const _: () = assert!(
    CUSTODY_FRAME_ACCOUNT_COUNT == CUSTODY_PROGRAM,
    "the first accounts of this route are the Custody InitializeReplay frame verbatim"
);

/// Recompute the sole Custody request this route will accept.
///
/// One author for the program and for every builder: a campaign, an operator or
/// a browser that constructs this instruction calls THIS function rather than
/// restating the twenty-two fields, so a builder cannot disagree with the guard
/// and there is no second place for the namespace to be re-guessed.
pub fn expected_request_v1(
    aggregate: LiabilityBasisMarketViewV2,
    claims_program: [u8; 32],
    payer: [u8; 32],
    rent_lamports: u64,
) -> Result<CustodyRequestV1, ProgramError> {
    let parent_request_digest = hashv(&[
        CLAIMS_CUSTODY_REPLAY_PARENT_DOMAIN_V1,
        &aggregate.logical_market,
        &aggregate.release_set,
        &aggregate.custody_context,
        &payer,
        &rent_lamports.to_le_bytes(),
    ])
    .to_bytes();
    let request = CustodyRequestV1 {
        operation: OperationV1::InitializeReplay,
        caller_role: CallerRoleV1::Claims,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set: aggregate.release_set,
        market: aggregate.logical_market,
        realm: aggregate.realm_id,
        // Decision 0008 §1: the aggregate is the sole persisted owner of this
        // Market's Custody namespace, and no route may re-guess it.
        context: aggregate.custody_context,
        caller_program: claims_program,
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: [0; 32],
            parent_request_digest,
            order_nonce: 0,
            generation: aggregate.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer,
        // Whoever prepays owns the refund. The field is immutable once written,
        // so this is the only moment it is decided.
        rent_refund: payer,
        expected_revision: 0,
        resulting_revision: 1,
        amount: 0,
        rent_lamports,
    };
    request.validate().map_err(|_| ClaimsSbfError::Identity)?;
    Ok(request)
}

/// Create this Market's Claims-role Custody replay from prepaid rent.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1 {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let submitted =
        CustodyRequestV1::decode(instruction_data).map_err(|_| ClaimsSbfError::Instruction)?;
    let aggregate = authenticate_aggregate(program_id, accounts, submitted)?;
    let payer = account(accounts, PAYER)?;
    let rent_account = account(accounts, RENT_SYSVAR)?;
    if rent_account.key != &sysvar::rent::ID {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let rent = Rent::from_account_info(rent_account).map_err(|_| ClaimsSbfError::Accounts)?;
    let request = expected_request_v1(
        aggregate,
        program_id.to_bytes(),
        payer.key.to_bytes(),
        rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1),
    )?;
    // Every field of the request is a function of the aggregate, the Rent
    // sysvar and the payer account. A submitted request that differs anywhere
    // is refused whole rather than corrected.
    if submitted != request {
        return Err(ClaimsSbfError::Identity.into());
    }
    let request_bytes = request.to_bytes().map_err(|_| ClaimsSbfError::Identity)?;
    let request_digest = hash(&request_bytes).to_bytes();
    authenticate_frame(program_id, accounts, request, request_digest)?;
    invoke_custody(program_id, accounts, &request_bytes, request_digest)?;
    authenticate_created_replay(accounts, request, request_digest)
}

fn authenticate_aggregate(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    submitted: CustodyRequestV1,
) -> Result<LiabilityBasisMarketViewV2, ProgramError> {
    let aggregate = account(accounts, AGGREGATE)?;
    // The Market coordinate is the only field of the submitted request read
    // before authentication, and it is read solely to ADDRESS the aggregate.
    // The aggregate then determines every field, this one included.
    let seeds =
        ClaimsAggregateSeedsV1::new(submitted.market).map_err(|_| ClaimsSbfError::Identity)?;
    if aggregate.owner != program_id
        || aggregate.is_signer
        || aggregate.is_writable
        || aggregate.executable
        || aggregate.key != &Pubkey::find_program_address(&seeds.as_slices(), program_id).0
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let bytes = aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let view = LiabilityBasisMarketViewV2::decode(&bytes).map_err(|_| ClaimsSbfError::Identity)?;
    if view.logical_market != submitted.market {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(view)
}

fn authenticate_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    let caller_authority = account(accounts, CUSTODY_CALLER_AUTHORITY)?;
    let claims_program = account(accounts, CLAIMS_PROGRAM)?;
    let replay = account(accounts, CUSTODY_REPLAY)?;
    let payer = account(accounts, PAYER)?;
    let system = account(accounts, SYSTEM_PROGRAM)?;
    let custody_program = account(accounts, CUSTODY_PROGRAM)?;
    if claims_program.key != program_id
        || !claims_program.executable
        || !custody_program.executable
        || custody_program.key == program_id
        || system.key != &system_program::ID
        || !payer.is_signer
        || !payer.is_writable
        || !replay.is_writable
        || replay.is_signer
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| ClaimsSbfError::Identity)?,
        request.market,
        ExecutionRoleV1::Claims,
        request.context,
        request_digest,
    )
    .map_err(|_| ClaimsSbfError::Identity)?;
    // The replay address carries the ROLE, so this is the Claims compartment of
    // the namespace and not the Trading one a founding realizes. Custody derives
    // the same seeds from the same request; two independent authors have to
    // agree before an account is created.
    let replay_seeds = CustodyReplaySeedsV1::from_request(request);
    if caller_authority.key
        != &Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).0
        || replay.key
            != &Pubkey::find_program_address(&replay_seeds.as_slices(), custody_program.key).0
        || replay.owner != &system_program::ID
        || replay.data_len() != 0
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

fn invoke_custody(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request_bytes: &[u8],
    request_digest: [u8; 32],
) -> ProgramResult {
    let custody_program = account(accounts, CUSTODY_PROGRAM)?;
    let frame = accounts
        .get(..CUSTODY_FRAME_ACCOUNT_COUNT)
        .ok_or(ClaimsSbfError::Accounts)?;
    let mut metas = Vec::with_capacity(CUSTODY_FRAME_ACCOUNT_COUNT);
    for (index, info) in frame.iter().enumerate() {
        // The Custody frame's privileges, in Custody's own order: coordinate 0
        // is the readonly caller-authority signer this program signs for,
        // coordinate 9 is the writable payer signer, and everything else is
        // passed with the privileges it already declares.
        let signer = index == CUSTODY_CALLER_AUTHORITY || info.is_signer;
        metas.push(if info.is_writable {
            AccountMeta::new(*info.key, signer)
        } else {
            AccountMeta::new_readonly(*info.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };
    let mut infos = Vec::with_capacity(CUSTODY_FRAME_ACCOUNT_COUNT + 1);
    infos.extend(frame.iter().cloned());
    infos.push(custody_program.clone());
    let request =
        CustodyRequestV1::decode(request_bytes).map_err(|_| ClaimsSbfError::Instruction)?;
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| ClaimsSbfError::Identity)?,
        request.market,
        ExecutionRoleV1::Claims,
        request.context,
        request_digest,
    )
    .map_err(|_| ClaimsSbfError::Identity)?;
    let bump = Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = caller_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| ClaimsSbfError::Receipt)?;
    Ok(())
}

fn authenticate_created_replay(
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    let custody_program = account(accounts, CUSTODY_PROGRAM)?;
    let replay = account(accounts, CUSTODY_REPLAY)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(ClaimsSbfError::Receipt)?;
    if producer != *custody_program.key || receipt_bytes.len() != CUSTODY_RECEIPT_BYTES_V1 {
        return Err(ClaimsSbfError::Receipt.into());
    }
    let receipt = CustodyReceiptV1::decode(&receipt_bytes).map_err(|_| ClaimsSbfError::Receipt)?;
    let bytes = replay
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if replay.owner != custody_program.key || bytes.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(ClaimsSbfError::Identity.into());
    }
    let replay_digest = hashv(&[&bytes]).to_bytes();
    let state = CustodyReplayV1::decode(&bytes).map_err(|_| ClaimsSbfError::Identity)?;
    drop(bytes);
    receipt
        .verify_for(request, request_digest, replay_digest)
        .map_err(|_| ClaimsSbfError::Receipt)?;
    // What a payout route will later demand of this account, demanded here so a
    // creation that produced something a redemption cannot use fails NOW rather
    // than at the redemption.
    if state.caller_role != CallerRoleV1::Claims
        || state.release_set != request.release_set
        || state.market != request.market
        || state.realm != request.realm
        || state.context != request.context
        || state.caller_program != request.caller_program
        || state.rent_refund != request.rent_refund
        || state.generation != request.semantic.generation
        || state.next_revision != 1
        || state.open_vault_count != 0
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts.get(index).ok_or(ClaimsSbfError::Accounts.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aggregate() -> LiabilityBasisMarketViewV2 {
        LiabilityBasisMarketViewV2 {
            claim_count: 2,
            revision: 0,
            logical_market: [1; 32],
            release_set: [2; 32],
            registry_program: [3; 32],
            product_instance_id: [4; 32],
            basis_id: [5; 32],
            realm_id: [6; 32],
            custody_context: [7; 32],
            generation: 9,
        }
    }

    #[test]
    fn the_request_is_a_function_of_the_aggregate_the_payer_and_the_rent() {
        let first = expected_request_v1(aggregate(), [8; 32], [9; 32], 1_000).expect("request");
        let again = expected_request_v1(aggregate(), [8; 32], [9; 32], 1_000).expect("request");
        assert_eq!(first, again);
        assert_eq!(first.caller_role, CallerRoleV1::Claims);
        assert_eq!(first.context, aggregate().custody_context);
        assert_eq!(first.rent_refund, [9; 32]);
        assert_eq!(first.expected_revision, 0);
        assert_eq!(first.resulting_revision, 1);

        let other_payer =
            expected_request_v1(aggregate(), [8; 32], [10; 32], 1_000).expect("request");
        assert_ne!(first, other_payer);
        assert_ne!(
            first.semantic.parent_request_digest, other_payer.semantic.parent_request_digest,
            "the synthetic parent digest binds the payer it prepaid for"
        );
    }

    #[test]
    fn the_namespace_is_never_the_market_address() {
        let request = expected_request_v1(aggregate(), [8; 32], [9; 32], 1_000).expect("request");
        assert_ne!(
            request.context, request.market,
            "decision 0008: the Market address is not a Custody namespace"
        );
    }

    #[test]
    fn the_claims_replay_is_not_the_trading_replay() {
        let request = expected_request_v1(aggregate(), [8; 32], [9; 32], 1_000).expect("request");
        let custody = Pubkey::new_from_array([0x3a; 32]);
        let claims = Pubkey::find_program_address(
            &CustodyReplaySeedsV1::from_request(request).as_slices(),
            &custody,
        )
        .0;
        for role in [
            CallerRoleV1::Core,
            CallerRoleV1::Trading,
            CallerRoleV1::Resolution,
        ] {
            let other = Pubkey::find_program_address(
                &CustodyReplaySeedsV1::new(
                    request.market,
                    request.release_set,
                    role,
                    request.context,
                )
                .as_slices(),
                &custody,
            )
            .0;
            assert_ne!(
                claims, other,
                "one namespace, one replay compartment per executing role"
            );
        }
    }
}
