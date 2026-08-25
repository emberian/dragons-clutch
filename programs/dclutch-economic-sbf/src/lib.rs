#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Thin Solana account and exact-token-custody boundary for the shared economic
//! microkernel.
//!
//! Account memory, release-set hashing, SPL state parsing, and CPI live here,
//! outside both the economic kernel and its fixed-layout persistence contract.

extern crate std;

use dclutch_economic_adapter_contract::{
    CustodyRequestV1, FoundingV1, OperationV1, PROJECTION_BYTES_V1, ReleaseContextV1,
    execute_projection, found_projection,
};
use dclutch_economic_kernel::Party;
use dclutch_release_set_contract::{EXECUTION_RELEASE_SET_BYTES_V1, ExecutionReleaseSetV1};
use dclutch_token_svm::{
    ExactTransferInput, ExactTransferProfileV1, LEGACY_TOKEN_PROGRAM_ID, transfer_checked,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// PDA seed for the exact legacy-token Hoard authority.
pub const HOARD_AUTHORITY_SEED_V1: &[u8] = b"dclutch-economic-hoard-v1";
/// Exact account count for founding or a claim-only transition.
pub const LOGICAL_ACCOUNT_COUNT_V1: usize = 3;
/// Exact account count for a transition with one collateral movement.
pub const CUSTODY_ACCOUNT_COUNT_V1: usize = 9;

/// Stable refusal returned by the physical successor program.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterError {
    /// Account count did not match the derived physical plan.
    AccountFrame = 0,
    /// Signer, writable, executable, or alias privileges were not canonical.
    AccountPrivilege = 1,
    /// Projection ownership or exact width was wrong.
    ProjectionAccount = 2,
    /// Account data borrowing refused.
    Borrow = 3,
    /// Instruction bytes refused canonical decoding.
    Instruction = 4,
    /// Release-set bytes, hash, owner, or role authorization refused.
    Release = 5,
    /// The persistence boundary or economic kernel refused.
    Economic = 6,
    /// Token program identity, exact Mint, or exact Account state refused.
    TokenState = 7,
    /// Physical account keys did not match the immutable projection binding.
    PhysicalBinding = 8,
    /// The derived custody direction was outside the one-Hoard profile.
    CustodyPlan = 9,
    /// Checked token arithmetic refused.
    Arithmetic = 10,
    /// The exact Token CPI refused.
    TokenCpi = 11,
    /// Complete post-CPI Mint and Account facts differed from the derivation.
    Postcondition = 12,
}

impl From<AdapterError> for ProgramError {
    fn from(error: AdapterError) -> Self {
        Self::Custom(error as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Found or execute one canonical economic projection.
///
/// Common accounts are exactly admission signer, writable projection, and
/// read-only release-set account. A nonempty custody plan additionally requires
/// holder authority, collateral Mint, holder token Account, Hoard token
/// Account, Hoard-authority PDA, and executable legacy Token program.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let (authority, projection, release_account) = common_accounts(program_id, accounts)?;
    if instruction_data.len() == dclutch_economic_adapter_contract::FOUNDING_BYTES_V1 {
        if accounts.len() != LOGICAL_ACCOUNT_COUNT_V1 {
            return Err(AdapterError::AccountFrame.into());
        }
        let founding =
            FoundingV1::decode(instruction_data).map_err(|_| AdapterError::Instruction)?;
        let context = release_context(
            program_id,
            authority.owner,
            release_account,
            founding.release_set_id(),
        )?;
        let mut data = projection
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::Borrow)?;
        found_projection(&mut data, founding, &context).map_err(|_| AdapterError::Economic)?;
        return Ok(());
    }

    let operation = OperationV1::decode(instruction_data).map_err(|_| AdapterError::Instruction)?;
    let context = release_context(
        program_id,
        authority.owner,
        release_account,
        release_set_digest(release_account)?,
    )?;
    let mut data = projection
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::Borrow)?;
    execute_projection(&mut data, operation, &context, |request| {
        execute_custody(program_id, projection.key, accounts, request).map_err(|_| ())
    })
    .map_err(|_| AdapterError::Economic)?;
    Ok(())
}

fn common_accounts<'a, 'info>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
) -> Result<
    (
        &'a AccountInfo<'info>,
        &'a AccountInfo<'info>,
        &'a AccountInfo<'info>,
    ),
    ProgramError,
> {
    if accounts.len() < LOGICAL_ACCOUNT_COUNT_V1 {
        return Err(AdapterError::AccountFrame.into());
    }
    let authority = accounts.first().ok_or(AdapterError::AccountFrame)?;
    let projection = accounts.get(1).ok_or(AdapterError::AccountFrame)?;
    let release_account = accounts.get(2).ok_or(AdapterError::AccountFrame)?;
    if !authority.is_signer
        || authority.is_writable
        || authority.executable
        || projection.is_signer
        || !projection.is_writable
        || projection.executable
        || release_account.is_signer
        || release_account.is_writable
        || release_account.executable
        || !all_distinct([authority.key, projection.key, release_account.key])
    {
        return Err(AdapterError::AccountPrivilege.into());
    }
    if projection.owner != program_id || projection.data_len() != PROJECTION_BYTES_V1 {
        return Err(AdapterError::ProjectionAccount.into());
    }
    Ok((authority, projection, release_account))
}

fn release_set_digest(release_account: &AccountInfo<'_>) -> Result<[u8; 32], ProgramError> {
    let data = release_account
        .try_borrow_data()
        .map_err(|_| AdapterError::Borrow)?;
    if data.len() != EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(AdapterError::Release.into());
    }
    Ok(hash(&data).to_bytes())
}

fn release_context(
    program_id: &Pubkey,
    admission_program: &Pubkey,
    release_account: &AccountInfo<'_>,
    expected_digest: [u8; 32],
) -> Result<ReleaseContextV1, ProgramError> {
    let data = release_account
        .try_borrow_data()
        .map_err(|_| AdapterError::Borrow)?;
    if data.len() != EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(AdapterError::Release.into());
    }
    let digest = hash(&data).to_bytes();
    if digest != expected_digest {
        return Err(AdapterError::Release.into());
    }
    let release_set = ExecutionReleaseSetV1::decode(&data).map_err(|_| AdapterError::Release)?;
    Ok(ReleaseContextV1 {
        release_set_id: digest,
        release_set,
        current_program: program_id.to_bytes(),
        release_set_owner_program: release_account.owner.to_bytes(),
        admission_program: admission_program.to_bytes(),
    })
}

fn execute_custody(
    program_id: &Pubkey,
    projection_key: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &CustodyRequestV1,
) -> Result<(), AdapterError> {
    let plan = request.plan();
    if plan.is_empty() {
        return if accounts.len() == LOGICAL_ACCOUNT_COUNT_V1 {
            Ok(())
        } else {
            Err(AdapterError::AccountFrame)
        };
    }
    if plan.len() != 1 || accounts.len() != CUSTODY_ACCOUNT_COUNT_V1 {
        return Err(AdapterError::AccountFrame);
    }
    let semantic_authority = accounts.first().ok_or(AdapterError::AccountFrame)?;
    let projection_account = accounts.get(1).ok_or(AdapterError::AccountFrame)?;
    let release_account = accounts.get(2).ok_or(AdapterError::AccountFrame)?;
    let holder_authority = accounts.get(3).ok_or(AdapterError::AccountFrame)?;
    let mint = accounts.get(4).ok_or(AdapterError::AccountFrame)?;
    let holder_token = accounts.get(5).ok_or(AdapterError::AccountFrame)?;
    let hoard_token = accounts.get(6).ok_or(AdapterError::AccountFrame)?;
    let hoard_authority = accounts.get(7).ok_or(AdapterError::AccountFrame)?;
    let token_program = accounts.get(8).ok_or(AdapterError::AccountFrame)?;
    if holder_authority.is_writable
        || holder_authority.executable
        || mint.is_signer
        || mint.is_writable
        || mint.executable
        || holder_token.is_signer
        || !holder_token.is_writable
        || holder_token.executable
        || hoard_token.is_signer
        || !hoard_token.is_writable
        || hoard_token.executable
        || hoard_authority.is_signer
        || hoard_authority.is_writable
        || hoard_authority.executable
        || token_program.is_signer
        || token_program.is_writable
        || !token_program.executable
        || !all_distinct([
            semantic_authority.key,
            projection_account.key,
            release_account.key,
            holder_authority.key,
            mint.key,
            holder_token.key,
            hoard_token.key,
            hoard_authority.key,
            token_program.key,
        ])
    {
        return Err(AdapterError::AccountPrivilege);
    }

    let transfer = plan.transfer(0).ok_or(AdapterError::CustodyPlan)?;
    let (holder_party, inbound) = match (transfer.source, transfer.destination) {
        (Party::Seller, Party::Venue) => (Party::Seller, true),
        (Party::Buyer, Party::Venue) => (Party::Buyer, true),
        (Party::Venue, Party::Seller) => (Party::Seller, false),
        (Party::Venue, Party::Buyer) => (Party::Buyer, false),
        _ => return Err(AdapterError::CustodyPlan),
    };
    if holder_authority.is_signer != inbound {
        return Err(AdapterError::AccountPrivilege);
    }
    let expected_holder = request.party_identity(holder_party);
    let (expected_hoard_authority, bump) = Pubkey::find_program_address(
        &[HOARD_AUTHORITY_SEED_V1, projection_key.as_ref()],
        program_id,
    );
    let token_id = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);
    if holder_authority.key.to_bytes() != expected_holder
        || mint.key.to_bytes() != request.collateral_mint()
        || hoard_token.key.to_bytes() != request.hoard_account()
        || hoard_authority.key != &expected_hoard_authority
        || token_program.key != &token_id
        || mint.owner != &token_id
        || holder_token.owner != &token_id
        || hoard_token.owner != &token_id
    {
        return Err(AdapterError::PhysicalBinding);
    }

    let profile = ExactTransferProfileV1::LegacyExactTransferV1;
    let (mint_before, holder_before, hoard_before) = {
        let mint_data = mint.try_borrow_data().map_err(|_| AdapterError::Borrow)?;
        let holder_data = holder_token
            .try_borrow_data()
            .map_err(|_| AdapterError::Borrow)?;
        let hoard_data = hoard_token
            .try_borrow_data()
            .map_err(|_| AdapterError::Borrow)?;
        let mint_state = profile
            .check_mint(LEGACY_TOKEN_PROGRAM_ID, &mint_data)
            .map_err(|_| AdapterError::TokenState)?;
        let holder_state = profile
            .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &holder_data)
            .map_err(|_| AdapterError::TokenState)?;
        let hoard_state = profile
            .check_custody_account(
                LEGACY_TOKEN_PROGRAM_ID,
                &hoard_data,
                mint.key.to_bytes(),
                hoard_authority.key.to_bytes(),
            )
            .map_err(|_| AdapterError::TokenState)?;
        if holder_state.mint != mint.key.to_bytes() || holder_state.owner != expected_holder {
            return Err(AdapterError::PhysicalBinding);
        }
        (mint_state, holder_state, hoard_state)
    };

    let (source, destination, transfer_authority, source_before, destination_before) = if inbound {
        (
            holder_token,
            hoard_token,
            holder_authority,
            holder_before,
            hoard_before,
        )
    } else {
        (
            hoard_token,
            holder_token,
            hoard_authority,
            hoard_before,
            holder_before,
        )
    };
    profile
        .check_transfer(ExactTransferInput {
            program_id: LEGACY_TOKEN_PROGRAM_ID,
            mint_address: mint.key.to_bytes(),
            mint_data: &mint.try_borrow_data().map_err(|_| AdapterError::Borrow)?,
            source_data: &source.try_borrow_data().map_err(|_| AdapterError::Borrow)?,
            destination_data: &destination
                .try_borrow_data()
                .map_err(|_| AdapterError::Borrow)?,
            authority: transfer_authority.key.to_bytes(),
            amount: transfer.amount,
            decimals: mint_before.decimals,
        })
        .map_err(|_| AdapterError::TokenState)?;

    invoke_transfer(
        projection_key,
        bump,
        inbound,
        source,
        mint,
        destination,
        transfer_authority,
        token_program,
        transfer.amount,
        mint_before.decimals,
    )?;

    let (mint_after, source_after, destination_after) = {
        let mint_data = mint
            .try_borrow_data()
            .map_err(|_| AdapterError::Postcondition)?;
        let source_data = source
            .try_borrow_data()
            .map_err(|_| AdapterError::Postcondition)?;
        let destination_data = destination
            .try_borrow_data()
            .map_err(|_| AdapterError::Postcondition)?;
        (
            profile
                .check_mint(LEGACY_TOKEN_PROGRAM_ID, &mint_data)
                .map_err(|_| AdapterError::Postcondition)?,
            profile
                .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &source_data)
                .map_err(|_| AdapterError::Postcondition)?,
            profile
                .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &destination_data)
                .map_err(|_| AdapterError::Postcondition)?,
        )
    };
    let mut expected_source = source_before;
    expected_source.amount = expected_source
        .amount
        .checked_sub(transfer.amount)
        .ok_or(AdapterError::Arithmetic)?;
    let mut expected_destination = destination_before;
    expected_destination.amount = expected_destination
        .amount
        .checked_add(transfer.amount)
        .ok_or(AdapterError::Arithmetic)?;
    if mint_after != mint_before
        || source_after != expected_source
        || destination_after != expected_destination
    {
        return Err(AdapterError::Postcondition);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn invoke_transfer<'info>(
    projection_key: &Pubkey,
    bump: u8,
    inbound: bool,
    source: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount: u64,
    decimals: u8,
) -> Result<(), AdapterError> {
    let specification = transfer_checked(
        LEGACY_TOKEN_PROGRAM_ID,
        source.key.to_bytes(),
        mint.key.to_bytes(),
        destination.key.to_bytes(),
        authority.key.to_bytes(),
        amount,
        decimals,
    )
    .map_err(|_| AdapterError::TokenState)?;
    let instruction = Instruction {
        program_id: *token_program.key,
        accounts: std::vec![
            AccountMeta::new(*source.key, false),
            AccountMeta::new_readonly(*mint.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
        data: std::vec::Vec::from(*specification.data()),
    };
    let infos = [
        source.clone(),
        mint.clone(),
        destination.clone(),
        authority.clone(),
        token_program.clone(),
    ];
    if inbound {
        invoke(&instruction, &infos).map_err(|_| AdapterError::TokenCpi)
    } else {
        let bump_seed = [bump];
        let signer_seeds: &[&[u8]] =
            &[HOARD_AUTHORITY_SEED_V1, projection_key.as_ref(), &bump_seed];
        invoke_signed(&instruction, &infos, &[signer_seeds]).map_err(|_| AdapterError::TokenCpi)
    }
}

fn all_distinct<const N: usize>(keys: [&Pubkey; N]) -> bool {
    for (left_index, left) in keys.iter().enumerate() {
        for right in keys.iter().skip(left_index.saturating_add(1)) {
            if left == right {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::{boxed::Box, vec, vec::Vec};

    use dclutch_economic_adapter_contract::{FoundingV1, OperationV1, ProjectionV1};
    use dclutch_economic_kernel::{Holder, Representation};
    use dclutch_release_set_contract::{
        ArtifactReleaseIdV1, ExecutionRoleBindingV1, ProgramIdentityV1,
    };
    use dclutch_token_svm::{ACCOUNT_BYTES, MINT_BYTES};

    use super::*;

    fn role(program: u8, release: u8) -> ExecutionRoleBindingV1 {
        ExecutionRoleBindingV1::new(
            ProgramIdentityV1::new([program; 32]).expect("program"),
            ArtifactReleaseIdV1::new([release; 32]).expect("release"),
        )
    }

    fn release_bytes() -> [u8; EXECUTION_RELEASE_SET_BYTES_V1] {
        let shared = role(2, 12);
        ExecutionReleaseSetV1::new(role(1, 11), shared, role(3, 13), role(4, 14), shared)
            .expect("release set")
            .to_bytes()
    }

    fn account(
        key: Pubkey,
        signer: bool,
        writable: bool,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(1)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn founding(release_set_id: [u8; 32]) -> FoundingV1 {
        FoundingV1::new(
            [8; 32],
            release_set_id,
            [5; 32],
            [6; 32],
            [7; 32],
            [10; 32],
            3,
        )
        .expect("founding")
    }

    fn common_frame() -> (Pubkey, [AccountInfo<'static>; 3], [u8; 32]) {
        let program_id = Pubkey::new_from_array([2; 32]);
        let release = release_bytes();
        let digest = hash(&release).to_bytes();
        let frame = [
            account(
                Pubkey::new_unique(),
                true,
                false,
                Vec::new(),
                Pubkey::new_from_array([1; 32]),
                false,
            ),
            account(
                Pubkey::new_unique(),
                false,
                true,
                vec![0; PROJECTION_BYTES_V1],
                program_id,
                false,
            ),
            account(
                Pubkey::new_unique(),
                false,
                false,
                release.to_vec(),
                Pubkey::new_from_array([1; 32]),
                false,
            ),
        ];
        (program_id, frame, digest)
    }

    fn mint_data() -> Vec<u8> {
        let mut bytes = vec![0_u8; MINT_BYTES];
        if let Some(initialized) = bytes.get_mut(45) {
            *initialized = 1;
        }
        bytes
    }

    fn token_data(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
        let mut bytes = vec![0_u8; ACCOUNT_BYTES];
        bytes
            .get_mut(..32)
            .expect("mint")
            .copy_from_slice(mint.as_ref());
        bytes
            .get_mut(32..64)
            .expect("owner")
            .copy_from_slice(owner.as_ref());
        bytes
            .get_mut(64..72)
            .expect("amount")
            .copy_from_slice(&amount.to_le_bytes());
        if let Some(state) = bytes.get_mut(108) {
            *state = 1;
        }
        bytes
    }

    #[test]
    fn founding_hashes_release_set_and_commits_exact_projection() {
        let (program_id, accounts, digest) = common_frame();
        process_instruction(&program_id, &accounts, &founding(digest).to_bytes()).expect("found");
        let data = accounts
            .get(1)
            .expect("projection")
            .try_borrow_data()
            .expect("data");
        let projection = ProjectionV1::decode(&data).expect("projection");
        assert_eq!(projection.release_set_id(), digest);
        assert_eq!(projection.revision(), 0);
    }

    #[test]
    fn hostile_founder_and_kernel_refusal_leave_projection_unchanged() {
        let (program_id, accounts, digest) = common_frame();
        let before = accounts
            .get(1)
            .expect("projection")
            .try_borrow_data()
            .expect("data")
            .to_vec();
        let wrong = founding([99; 32]).to_bytes();
        assert_eq!(
            process_instruction(&program_id, &accounts, &wrong),
            Err(AdapterError::Release.into())
        );
        assert_eq!(
            accounts
                .get(1)
                .expect("projection")
                .try_borrow_data()
                .expect("data")
                .as_ref(),
            before.as_slice()
        );

        process_instruction(&program_id, &accounts, &founding(digest).to_bytes()).expect("found");
        let before = accounts
            .get(1)
            .expect("projection")
            .try_borrow_data()
            .expect("data")
            .to_vec();
        let trading = account(
            Pubkey::new_unique(),
            true,
            false,
            Vec::new(),
            Pubkey::new_from_array([3; 32]),
            false,
        );
        let operation = OperationV1::materialize(0, 1, 0).to_bytes();
        assert_eq!(
            process_instruction(
                &program_id,
                &[
                    trading,
                    accounts.get(1).expect("projection").clone(),
                    accounts.get(2).expect("release").clone(),
                ],
                &operation
            ),
            Err(AdapterError::Economic.into())
        );
        assert_eq!(
            accounts
                .get(1)
                .expect("projection")
                .try_borrow_data()
                .expect("data")
                .as_ref(),
            before.as_slice()
        );
    }

    #[test]
    fn late_token_postcondition_refusal_does_not_commit_projection() {
        let (program_id, common, digest) = common_frame();
        process_instruction(&program_id, &common, &founding(digest).to_bytes()).expect("found");
        let projection = common.get(1).expect("projection").clone();
        let release = common.get(2).expect("release").clone();
        let projection_key = *projection.key;
        let (hoard_authority, _) = Pubkey::find_program_address(
            &[HOARD_AUTHORITY_SEED_V1, projection_key.as_ref()],
            &program_id,
        );
        let mint = Pubkey::new_from_array([7; 32]);
        let holder = Pubkey::new_from_array([5; 32]);
        let hoard = Pubkey::new_from_array([10; 32]);
        let token_id = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);
        let trading = account(
            Pubkey::new_unique(),
            true,
            false,
            Vec::new(),
            Pubkey::new_from_array([3; 32]),
            false,
        );
        let frame = [
            trading,
            projection.clone(),
            release,
            account(holder, true, false, Vec::new(), Pubkey::default(), false),
            account(mint, false, false, mint_data(), token_id, false),
            account(
                Pubkey::new_unique(),
                false,
                true,
                token_data(mint, holder, 100),
                token_id,
                false,
            ),
            account(
                hoard,
                false,
                true,
                token_data(mint, hoard_authority, 0),
                token_id,
                false,
            ),
            account(hoard_authority, false, false, Vec::new(), program_id, false),
            account(token_id, false, false, Vec::new(), Pubkey::default(), true),
        ];
        let before = projection.try_borrow_data().expect("projection").to_vec();
        let split = OperationV1::split(Holder::Source, Representation::Native, 7, 0).to_bytes();
        assert_eq!(
            process_instruction(&program_id, &frame, &split),
            Err(AdapterError::Economic.into())
        );
        assert_eq!(
            projection.try_borrow_data().expect("projection").as_ref(),
            before.as_slice()
        );
    }
}
