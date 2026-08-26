//! Physical refinement of exact delegated-allowance Custody transfers.

use super::*;

use dclutch_custody_contract::{
    DELEGATED_CUSTODY_RECEIPT_BYTES_V2, DelegatedAllowanceObservationV2, DelegatedCustodyReceiptV2,
    DelegatedCustodyRequestV2,
};

const DELEGATED_POSTSTATE_DOMAIN_V2: &[u8] = b"dclutch:custody-delegated-poststate:v2";

/// Decode and authenticate the distinct successor outside the V1 dispatcher frame.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = DelegatedCustodyRequestV2::decode(instruction_data)
        .map_err(|_| CustodySbfError::Instruction)?;
    let custody = request.custody;
    require_account_count(accounts, custody.operation)?;
    let request_digest = hash(instruction_data).to_bytes();
    let market = authenticate_common_frame(program_id, accounts, custody, request_digest)?;
    let realm = authenticate_realm(program_id, accounts, custody, market)?;
    execute_transfer(program_id, accounts, request, request_digest, realm)
}

/// Execute one positive external debit and refuse any unexpected residual authority.
#[inline(never)]
pub(super) fn execute_transfer(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: DelegatedCustodyRequestV2,
    request_digest: [u8; 32],
    realm: RealmFacts,
) -> ProgramResult {
    let outcome = execute_token_effect(program_id, accounts, &request, realm)?;
    commit_delegated(accounts, request, request_digest, outcome)
}

#[derive(Clone, Copy)]
struct TransferOutcome {
    source: [u8; 32],
    destination: [u8; 32],
    before: TransferBalances,
    after: TransferBalances,
    allowance_before: AllowanceFacts,
    allowance_after: AllowanceFacts,
}

#[inline(never)]
fn execute_token_effect(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &DelegatedCustodyRequestV2,
    realm: RealmFacts,
) -> Result<TransferOutcome, ProgramError> {
    let custody = request.custody;
    let mint = account(accounts, 9)?;
    let source = account(accounts, 10)?;
    let destination = account(accounts, 11)?;
    let authority = account(accounts, 12)?;
    let token_program = account(accounts, 13)?;
    validate_token_program_and_mint(mint, token_program, custody, realm)?;
    validate_custody_authority(program_id, authority, custody)?;
    if request.delegate_before != authority.key.to_bytes()
        || source.key.to_bytes() != custody.source
        || destination.key.to_bytes() != custody.destination
        || source.owner != token_program.key
        || destination.owner != token_program.key
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    if custody.destination_compartment != CompartmentV1::External {
        validate_vault_key(program_id, destination, custody, false)?;
    }
    let transfer_accounts = TransferAccounts {
        source,
        destination,
        mint,
        authority,
        token_program,
    };
    let before = authenticate_transfer_accounts(transfer_accounts, custody, realm.profile, true)?;
    let before_allowance = read_delegate(source, token_program, realm.profile)?;
    if before_allowance.delegate != request.delegate_before
        || before_allowance.amount != request.allowance_before
    {
        return Err(CustodySbfError::TokenState.into());
    }
    invoke_exact_transfer(transfer_accounts, custody, before.decimals, program_id)?;
    let after = authenticate_transfer_accounts(transfer_accounts, custody, realm.profile, false)?;
    let after_allowance = read_delegate(source, token_program, realm.profile)?;
    if before.source.checked_sub(custody.amount) != Some(after.source)
        || before.destination.checked_add(custody.amount) != Some(after.destination)
        || after_allowance.delegate != request.delegate_after
        || after_allowance.amount != request.allowance_after
    {
        return Err(CustodySbfError::Postcondition.into());
    }

    Ok(TransferOutcome {
        source: source.key.to_bytes(),
        destination: destination.key.to_bytes(),
        before,
        after,
        allowance_before: before_allowance,
        allowance_after: after_allowance,
    })
}

#[inline(never)]
fn commit_delegated(
    accounts: &[AccountInfo<'_>],
    request: DelegatedCustodyRequestV2,
    request_digest: [u8; 32],
    outcome: TransferOutcome,
) -> ProgramResult {
    let custody = request.custody;
    let poststate = delegated_poststate_commitment(request_digest, outcome);
    let replay = read_replay(account(accounts, REPLAY)?)?;
    let next = replay
        .advance(custody, request_digest, poststate)
        .map_err(|_| CustodySbfError::Replay)?;
    let replay_bytes = next.to_bytes().map_err(|_| CustodySbfError::Replay)?;
    let evidence = ReceiptEvidenceV1 {
        source_before: outcome.before.source,
        source_after: outcome.after.source,
        destination_before: outcome.before.destination,
        destination_after: outcome.after.destination,
        poststate_commitment: poststate,
        replay_state_digest: hash(&replay_bytes).to_bytes(),
    };
    let receipt = DelegatedCustodyReceiptV2::new(
        request,
        request_digest,
        evidence,
        DelegatedAllowanceObservationV2 {
            delegate_before: outcome.allowance_before.delegate,
            allowance_before: outcome.allowance_before.amount,
            delegate_after: outcome.allowance_after.delegate,
            allowance_after: outcome.allowance_after.amount,
        },
    )
    .map_err(|_| CustodySbfError::Postcondition)?;
    let receipt_bytes = receipt
        .encode()
        .map_err(|_| CustodySbfError::Postcondition)?;
    if receipt_bytes.len() != DELEGATED_CUSTODY_RECEIPT_BYTES_V2 {
        return Err(CustodySbfError::Postcondition.into());
    }
    let replay_account = account(accounts, REPLAY)?;
    let mut data = replay_account
        .try_borrow_mut_data()
        .map_err(|_| CustodySbfError::Commit)?;
    if data.len() != replay_bytes.len() {
        return Err(CustodySbfError::Commit.into());
    }
    data.copy_from_slice(&replay_bytes);
    drop(data);
    set_return_data(&receipt_bytes);
    Ok(())
}

#[derive(Clone, Copy)]
struct AllowanceFacts {
    delegate: [u8; 32],
    amount: u64,
}

fn read_delegate(
    source: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    profile: ExactTransferProfileV1,
) -> Result<AllowanceFacts, ProgramError> {
    if source.owner != token_program.key {
        return Err(CustodySbfError::TokenState.into());
    }
    let data = source
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let token = profile
        .check_transfer_account(token_program.key.to_bytes(), &data)
        .map_err(|_| CustodySbfError::TokenState)?;
    Ok(AllowanceFacts {
        delegate: match token.delegate {
            COption::None => [0; 32],
            COption::Some(delegate) => delegate,
        },
        amount: token.delegated_amount,
    })
}

fn delegated_poststate_commitment(request_digest: [u8; 32], outcome: TransferOutcome) -> [u8; 32] {
    hashv(&[
        DELEGATED_POSTSTATE_DOMAIN_V2,
        &request_digest,
        &outcome.source,
        &outcome.destination,
        &outcome.before.source.to_le_bytes(),
        &outcome.after.source.to_le_bytes(),
        &outcome.before.destination.to_le_bytes(),
        &outcome.after.destination.to_le_bytes(),
        &outcome.allowance_before.delegate,
        &outcome.allowance_before.amount.to_le_bytes(),
        &outcome.allowance_after.delegate,
        &outcome.allowance_after.amount.to_le_bytes(),
    ])
    .to_bytes()
}
