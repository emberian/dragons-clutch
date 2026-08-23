// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capability-disabled SBF seam for permanent Market Failure replay.
//!
//! No instruction route exists here. Initialization is available only through
//! a crate-private Product adapter over the accepted slot-7 foundation
//! receipt. These helpers then authenticate the fresh `0xa3/v2` successor and
//! persist its exact one-shot terminal plan without changing any lamport.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::{
    authenticate_failure_market_root_v2, AuthenticatedFailureMarketRootV2,
};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_market::AuthenticatedMarketFoundationPreallocationV2;
use crate::seeds;
use clutch_failure_policy_runtime::market_replay_v2::{
    admit_failure_market_replay_v2, AuthenticatedFailureMarketReplayFundingV2,
    FailureMarketReplayFundingFactsV2, FailureMarketReplayFundingReceiptV2,
    FailureMarketReplayPlanV2, FailureMarketReplayStateIdV2, FailureMarketReplayTerminalReceiptV2,
    FailureMarketReplayV2, FAILURE_MARKET_REPLAY_BYTES_V2,
};
use clutch_product_series::{ContentId as ProductContentId, MarketFoundationSlotV2};
use clutch_solana_layout::failure_market_replay_v2::{
    FailureMarketReplayAccountV2, FAILURE_MARKET_REPLAY_BODY_BYTES_V2,
};
use clutch_solana_layout::registry::FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const REPLAY_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-replay-account-authentication/v2";

const _: () = assert!(FAILURE_MARKET_REPLAY_BODY_BYTES_V2 == FAILURE_MARKET_REPLAY_BYTES_V2);

/// Exact authenticated permanent shared-Market replay account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketReplayV2 {
    account: Pubkey,
    bump: u8,
    replay: FailureMarketReplayV2,
    state_id: FailureMarketReplayStateIdV2,
    data_id: ProductContentId,
    authentication_id: ProductContentId,
    observed_lamports: u64,
    admission_root_account: Pubkey,
    funding: FailureMarketReplayFundingReceiptV2,
}

impl AuthenticatedFailureMarketReplayV2 {
    /// Exact permanent replay account.
    pub(crate) const fn account(self) -> Pubkey {
        self.account
    }

    /// Complete authenticated replay state.
    pub(crate) const fn replay(self) -> FailureMarketReplayV2 {
        self.replay
    }

    /// Complete semantic state identity.
    pub(crate) const fn state_id(self) -> FailureMarketReplayStateIdV2 {
        self.state_id
    }

    /// Owner/PDA/frame/body/balance authentication identity.
    pub(crate) const fn authentication_id(self) -> ProductContentId {
        self.authentication_id
    }

    /// Exact Product foundation capitalization receipt.
    pub(crate) const fn funding(self) -> FailureMarketReplayFundingReceiptV2 {
        self.funding
    }
}

/// Atomic postimage of one Product-authorized replay foundation step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FailureMarketReplayPostimageV2 {
    replay: AuthenticatedFailureMarketReplayV2,
    funding: FailureMarketReplayFundingReceiptV2,
}

impl FailureMarketReplayPostimageV2 {
    /// Newly persisted permanent replay account.
    pub(crate) const fn replay(self) -> AuthenticatedFailureMarketReplayV2 {
        self.replay
    }

    /// Exact Product-authorized permanent funding receipt.
    pub(crate) const fn funding(self) -> FailureMarketReplayFundingReceiptV2 {
        self.funding
    }
}

/// Module-private bridge over Product's unforgeable slot-7 preallocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductFailureMarketReplayFundingV2 {
    expected: FailureMarketReplayFundingFactsV2,
}

impl AuthenticatedFailureMarketReplayFundingV2 for ProductFailureMarketReplayFundingV2 {
    fn authenticate_failure_market_replay_funding(
        &self,
        expected: FailureMarketReplayFundingFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if self.expected != expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Allocate, assign, and write the exact Product-prepaid Pending replay.
///
/// This helper is crate-private and non-routable. Its authority must be a
/// private adapter over Product's accepted slot-7 preallocation receipt; a
/// caller-built facts value cannot pass the default-refusing pure trait.
#[allow(clippy::too_many_arguments)]
pub(crate) fn initialize_failure_market_replay_v2<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    replay_account: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    product_preallocation: AuthenticatedMarketFoundationPreallocationV2,
) -> Outcome<FailureMarketReplayPostimageV2> {
    require_system_program(system_program)?;
    require_distinct(&[
        admission_root_account.clone(),
        replay_account.clone(),
        rent_sysvar.clone(),
        system_program.clone(),
    ])?;
    let live_admission =
        authenticate_failure_market_root_v2(program_id, admission_root_account, false)?;
    require(live_admission == admission, ClutchError::MismatchedState)?;
    let admission = live_admission;
    let admission_state = admission.state();
    let policy = admission_state.binding().facts();
    require(
        product_preallocation.slot() == MarketFoundationSlotV2::FailureReplay
            && product_preallocation.market_instance_id() == policy.market_instance_id
            && product_preallocation.generation() == policy.generation
            && product_preallocation.account() == *replay_account.key
            && product_preallocation.root_account() != admission.account()
            && product_preallocation.root_account() != *replay_account.key,
        ClutchError::MismatchedState,
    )?;
    let funding_facts = FailureMarketReplayFundingFactsV2 {
        failure_policy_binding_id: admission_state.binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        prepaid_funding_receipt_id: product_preallocation.id(),
        replay_account:
            clutch_failure_policy_runtime::market_policy_v1::FailureMarketAccountIdV1::from_bytes(
                replay_account.key.to_bytes(),
            ),
        permanent_rent_funder:
            clutch_failure_policy_runtime::market_policy_v1::FailureMarketAccountIdV1::from_bytes(
                product_preallocation.rent_refund_owner().to_bytes(),
            ),
        neutral_sink:
            clutch_failure_policy_runtime::market_policy_v1::FailureMarketAccountIdV1::from_bytes(
                product_preallocation.neutral_lamport_sink().to_bytes(),
            ),
        permanent_rent_principal_lamports: product_preallocation.principal_lamports(),
        donation_floor_lamports: product_preallocation.donation_lamports(),
        observed_balance_lamports: product_preallocation.observed_balance_lamports(),
    };
    let product_foundation_authority = ProductFailureMarketReplayFundingV2 {
        expected: funding_facts,
    };
    let expected_balance = funding_facts
        .permanent_rent_principal_lamports
        .checked_add(funding_facts.donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let rent = read_rent(rent_sysvar)?;
    require(
        replay_account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && replay_account.is_writable
            && !replay_account.is_signer
            && !replay_account.executable
            && replay_account.data_len() == 0
            && replay_account.lamports() == expected_balance
            && funding_facts.replay_account.bytes() == replay_account.key.to_bytes()
            && funding_facts.failure_policy_binding_id == admission_state.binding().id()
            && funding_facts.market_instance_id == policy.market_instance_id
            && funding_facts.generation == policy.generation
            && funding_facts.observed_balance_lamports == expected_balance
            && funding_facts.permanent_rent_principal_lamports
                == rent.minimum_balance(FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2)?,
        ClutchError::MismatchedState,
    )?;
    let (expected_replay, bump) = seeds::failure_market_replay_v2_pda(
        program_id,
        &policy.market_instance_id.bytes(),
        policy.generation,
    );
    expect_pda(replay_account.key, (expected_replay, bump), None)?;
    let (replay, funding) = admit_failure_market_replay_v2(
        &product_foundation_authority,
        admission_state,
        funding_facts,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = policy.market_instance_id.bytes();
    let generation = policy.generation.to_le_bytes();
    let bump_seed = [bump];
    let signer_seeds: [&[u8]; 4] = [
        seeds::SEED_FAILURE_MARKET_REPLAY_V2,
        &market,
        &generation,
        &bump_seed,
    ];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2),
        vec![AccountMeta::new(*replay_account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[replay_account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*replay_account.key, true)],
    );
    invoke_signed(
        &assign,
        &[replay_account.clone(), system_program.clone()],
        &[&signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        replay_account.owner == program_id
            && replay_account.data_len() == FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2
            && replay_account.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )?;
    let output = encode_replay(bump, replay)?;
    {
        let mut data = replay_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            data.iter().all(|byte| *byte == 0),
            ClutchError::AlreadyInitialized,
        )?;
        let destination: &mut [u8; FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2] = data
            .as_mut()
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        destination.copy_from_slice(&output);
    }
    let authenticated = authenticate_failure_market_replay_v2(
        program_id,
        replay_account,
        admission,
        funding,
        true,
    )?;
    require(
        authenticated.replay == replay && authenticated.observed_lamports == expected_balance,
        ClutchError::MismatchedState,
    )?;
    Ok(FailureMarketReplayPostimageV2 {
        replay: authenticated,
        funding,
    })
}

/// Authenticate exact fresh `0xa3/v2` owner, PDA, frame, semantic body,
/// foundation receipt, and permanent principal floor.
pub(crate) fn authenticate_failure_market_replay_v2<'a>(
    program_id: &Pubkey,
    replay_account: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    funding: FailureMarketReplayFundingReceiptV2,
    writable: bool,
) -> Outcome<AuthenticatedFailureMarketReplayV2> {
    let admission_state = admission.state();
    let policy = admission_state.binding().facts();
    let funded = funding.facts();
    require(
        funded.failure_policy_binding_id == admission_state.binding().id()
            && funded.market_instance_id == policy.market_instance_id
            && funded.generation == policy.generation
            && funded.replay_account.bytes() == replay_account.key.to_bytes()
            && *replay_account.key != admission.account()
            && replay_account.key.to_bytes() != policy.recovery_state_id.bytes()
            && replay_account.key.to_bytes() != policy.recovery_compartment_account_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    authenticate_metadata(program_id, replay_account, writable)?;
    let minimum_balance = funded
        .permanent_rent_principal_lamports
        .checked_add(funded.donation_floor_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        replay_account.lamports() >= minimum_balance,
        ClutchError::MismatchedState,
    )?;
    let input = replay_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let framed: &[u8; FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2] = input
        .as_ref()
        .try_into()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
    let frame = FailureMarketReplayAccountV2::decode(framed)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    expect_pda(
        replay_account.key,
        seeds::failure_market_replay_v2_pda(
            program_id,
            &policy.market_instance_id.bytes(),
            policy.generation,
        ),
        Some(frame.bump()),
    )?;
    let replay = FailureMarketReplayV2::decode_for_admission(
        frame.semantic_body(),
        admission_state,
        funding,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let state_id = replay
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let data_id = framed_data_id(framed);
    let authentication_id = account_authentication_id(
        replay_account,
        data_id,
        state_id,
        admission_state
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
            .bytes(),
    );
    Ok(AuthenticatedFailureMarketReplayV2 {
        account: *replay_account.key,
        bump: frame.bump(),
        replay,
        state_id,
        data_id,
        authentication_id,
        observed_lamports: replay_account.lamports(),
        admission_root_account: admission.account(),
        funding,
    })
}

/// Persist the exact one-shot replay terminal postimage and reauthenticate it.
pub(crate) fn write_failure_market_replay_terminal_v2<'a>(
    program_id: &Pubkey,
    replay_account: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    authenticated: AuthenticatedFailureMarketReplayV2,
    plan: FailureMarketReplayPlanV2,
    receipt: FailureMarketReplayTerminalReceiptV2,
) -> Outcome<AuthenticatedFailureMarketReplayV2> {
    require(
        authenticated.account == *replay_account.key
            && authenticated.admission_root_account == admission.account()
            && replay_account.is_writable,
        ClutchError::MismatchedState,
    )?;
    authenticate_live_prestate(program_id, replay_account, authenticated)?;
    let mut expected = authenticated.replay;
    expected
        .commit_plan(plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facts = receipt.facts();
    require(
        facts.replay_before == authenticated.state_id
            && facts.replay_account == authenticated.funding.facts().replay_account
            && facts.funding_receipt_id == authenticated.funding.id()
            && facts.replay_after
                == expected
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?,
        ClutchError::MismatchedState,
    )?;
    let output = encode_replay(authenticated.bump, expected)?;
    {
        let mut data = replay_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let destination: &mut [u8; FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2] = data
            .as_mut()
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?;
        destination.copy_from_slice(&output);
    }
    require(
        replay_account.lamports() == authenticated.observed_lamports,
        ClutchError::MismatchedState,
    )?;
    let reopened = authenticate_failure_market_replay_v2(
        program_id,
        replay_account,
        admission,
        authenticated.funding,
        true,
    )?;
    require(
        reopened.replay == expected
            && reopened.state_id == facts.replay_after
            && reopened.authentication_id != authenticated.authentication_id,
        ClutchError::MismatchedState,
    )?;
    Ok(reopened)
}

fn authenticate_live_prestate(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected: AuthenticatedFailureMarketReplayV2,
) -> Outcome<()> {
    authenticate_metadata(program_id, account, true)?;
    let input = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        *account.key == expected.account
            && account.lamports() == expected.observed_lamports
            && framed_data_id(input.as_ref()) == expected.data_id,
        ClutchError::MismatchedState,
    )
}

fn authenticate_metadata(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2,
        ClutchError::WrongDataLength,
    )
}

fn encode_replay(
    bump: u8,
    replay: FailureMarketReplayV2,
) -> Outcome<[u8; FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2]> {
    let mut semantic = [0; FAILURE_MARKET_REPLAY_BYTES_V2];
    replay
        .encode_into(&mut semantic)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let frame = FailureMarketReplayAccountV2::new(bump, semantic)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let mut output = [0; FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2];
    frame
        .encode_into(&mut output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    Ok(output)
}

fn account_authentication_id(
    account: &AccountInfo<'_>,
    data_id: ProductContentId,
    state_id: FailureMarketReplayStateIdV2,
    admission_state_id: [u8; 32],
) -> ProductContentId {
    ProductContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            REPLAY_AUTHENTICATION_DOMAIN_V2,
            account.key.as_ref(),
            account.owner.as_ref(),
            &data_id.bytes(),
            &state_id.bytes(),
            &admission_state_id,
            &account.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    )
}

fn framed_data_id(data: &[u8]) -> ProductContentId {
    ProductContentId::from_bytes(solana_sha256_hasher::hashv(&[data]).to_bytes())
}
