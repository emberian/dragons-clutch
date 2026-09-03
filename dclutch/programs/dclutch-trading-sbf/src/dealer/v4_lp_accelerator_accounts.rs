//! Physical selector-7/8 authentication for the admitted Dealer accelerator.
//!
//! Common Hot authenticates release selection, every finalized interpreter
//! artifact, the request projection, the lifecycle preplan, and the complete
//! input register bank. This module independently rejoins the Dealer root,
//! obligation, LP PDA and exact optimistic prestates, then evaluates the small
//! LP transition into a staged bank. The caller's candidate buffer changes only
//! after every semantic and physical check succeeds.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_dealer_codec::{
    config_v4::{DEALER_CONFIG_SCHEMA_PREIMAGE_V4, DealerConfigV4},
    root_admission_v1::DEALER_ROOT_OPEN_ADMISSIBLE_STATES_V1,
    root_tail::{ROOT_TAIL_BYTES, RootTail},
};
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

use crate::hot_v3::AuthenticatedAcceleratorInvocationV4;

use super::{
    v3_lp_artifacts::{
        DEALER_LP_IDENTITY_COUNT_V3, DEALER_LP_OBLIGATION_ACCOUNT_V3, DEALER_LP_SCALAR_COUNT_V3,
        DEALER_LP_STATE_ACCOUNT_V3, LP_CANONICAL_BUMP_SCALAR_V3, LP_CHILD_ROOT_IDENTITY_V3,
        LP_CREATED_SCALAR_V3, LP_CURRENT_SLOT_SCALAR_V3, LP_EXPECTED_OBLIGATION_REVISION_SCALAR_V3,
        LP_EXPECTED_REVISION_SCALAR_V3, LP_EXPIRY_SCALAR_V3, LP_GENERATION_SCALAR_V3,
        LP_INITIAL_REVISION_SCALAR_V3, LP_LIFECYCLE_BENEFICIARY_IDENTITY_V3,
        LP_LIFECYCLE_OWNER_IDENTITY_V3, LP_LIFECYCLE_RENT_PRINCIPAL_SCALAR_V3,
        LP_LIFECYCLE_STATE_IDENTITY_V3, LP_MAGIC_SCALAR_V3, LP_MARKET_IDENTITY_V3,
        LP_OBLIGATION_DIGEST_IDENTITY_V3, LP_OBLIGATION_IDENTITY_V3,
        LP_OBSERVED_CHILD_ROOT_IDENTITY_V3, LP_OBSERVED_LAMPORTS_SCALAR_V3,
        LP_OBSERVED_MARKET_IDENTITY_V3, LP_OBSERVED_OBLIGATION_IDENTITY_V3,
        LP_OBSERVED_OBLIGATION_REVISION_SCALAR_V3, LP_OBSERVED_OWNER_IDENTITY_V3,
        LP_OBSERVED_POSITION_IDENTITY_V3, LP_OBSERVED_RELEASE_IDENTITY_V3,
        LP_OBSERVED_RENT_PRINCIPAL_SCALAR_V3, LP_OBSERVED_REVISION_SCALAR_V3,
        LP_OBSERVED_SHARES_SCALAR_V3, LP_OWNER_IDENTITY_V3, LP_POSITION_IDENTITY_V3,
        LP_PRESTATE_DIGEST_IDENTITY_V3, LP_RELEASE_IDENTITY_V3,
        LP_REQUEST_RENT_PRINCIPAL_SCALAR_V3, LP_VERSION_SCALAR_V3, LP_ZERO_SCALAR_V3,
        dealer_lp_account_count_v3,
    },
    v3_multi_lp::{
        DEALER_LP_POSITION_MAGIC_V3, DEALER_LP_POSITION_PDA_DOMAIN_V3,
        DEALER_LP_POSITION_VERSION_V3, DealerLpPositionV3,
    },
    v3_obligation::{DEALER_OBLIGATION_PDA_DOMAIN_V3, DealerObligationProjectionV3},
    v3_operator::{DealerMultiLpRequestV3, MultiLpRequestActionV3},
};

/// Stable refusal at selector 7/8's physical accelerator boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerLpAcceleratorErrorV4 {
    /// Request, selector, register, span, or account geometry differed.
    Invocation,
    /// Immutable release, Market, root, config, or Product joins differed.
    SemanticJoin,
    /// Trading-owned obligation state or exact optimistic digest differed.
    Obligation,
    /// LP PDA, vacancy/live state, Rent, owner, or refund joins differed.
    Position,
    /// The canonical Open/Close transition refused the authenticated bank.
    Transition,
    /// Checked bank or account-width arithmetic overflowed.
    Arithmetic,
}

impl DealerLpAcceleratorErrorV4 {
    /// This refusal's own word, for the one log line a reader greps.
    ///
    /// The accelerator boundary decides ACCEPTED or REFUSED and has no wire
    /// field for which conjunct refused, so `.is_ok()` was throwing this away
    /// at the one line that held it. Exhaustive on purpose: a new variant does
    /// not compile until its author writes the word a validator log will carry.
    #[must_use]
    pub const fn refusal_name(self) -> &'static str {
        match self {
            Self::Invocation => "lp:Invocation",
            Self::SemanticJoin => "lp:SemanticJoin",
            Self::Obligation => "lp:Obligation",
            Self::Position => "lp:Position",
            Self::Transition => "lp:Transition",
            Self::Arithmetic => "lp:Arithmetic",
        }
    }
}

/// Authenticate selector 7/8 and evaluate its exact candidate bank.
///
/// `candidate_bank` is commit-last and remains byte-for-byte unchanged on every
/// refusal.
pub fn evaluate_authenticated_dealer_lp_v4(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    candidate_bank: &mut [u8],
) -> Result<(), DealerLpAcceleratorErrorV4> {
    let request = DealerMultiLpRequestV3::decode(invocation.family_request())
        .map_err(|_| DealerLpAcceleratorErrorV4::Invocation)?;
    let runtime = invocation.runtime_accounts();
    if invocation.selected_action() != u32::from(request.action.selector())
        || invocation.request().scalar_count() != u32::from(DEALER_LP_SCALAR_COUNT_V3)
        || invocation.request().identity_count() != u32::from(DEALER_LP_IDENTITY_COUNT_V3)
        || runtime.len() != usize::from(dealer_lp_account_count_v3(request.action))
        || invocation.input_bank().len() != candidate_bank.len()
        || runtime
            .iter()
            .any(|account| account.is_signer || account.is_writable)
    {
        return Err(DealerLpAcceleratorErrorV4::Invocation);
    }

    authenticate_context(invocation, request, runtime)?;
    authenticate_obligation(invocation, request, runtime)?;
    authenticate_position(invocation, request, runtime)?;
    evaluate_transition_commit_last(invocation, request, candidate_bank)
}

fn authenticate_context(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerMultiLpRequestV3,
    runtime: &[AccountInfo<'_>],
) -> Result<(), DealerLpAcceleratorErrorV4> {
    let context = invocation.context();
    let descriptor = invocation.descriptor();
    if request.release_set != context.release_set.to_bytes()
        || request.market != context.market.to_bytes()
        || request.child_root != context.root.to_bytes()
        || request.generation != invocation.envelope().generation()
        || descriptor.config_schema().to_bytes()
            != hash(DEALER_CONFIG_SCHEMA_PREIMAGE_V4).to_bytes()
    {
        return Err(DealerLpAcceleratorErrorV4::SemanticJoin);
    }
    let root = account(runtime, 0)?;
    let config = account(runtime, 1)?;
    if root.key.to_bytes() != request.child_root
        || root.owner.to_bytes() != context.trading_program.to_bytes()
        || root.executable
        || config.executable
    {
        return Err(DealerLpAcceleratorErrorV4::SemanticJoin);
    }
    let root_data = root
        .try_borrow_data()
        .map_err(|_| DealerLpAcceleratorErrorV4::SemanticJoin)?;
    let tail_start = CAPABILITY_ROOT_HEADER_BYTES_V1;
    let tail_end = tail_start
        .checked_add(ROOT_TAIL_BYTES)
        .ok_or(DealerLpAcceleratorErrorV4::Arithmetic)?;
    if root_data.len() != tail_end {
        return Err(DealerLpAcceleratorErrorV4::SemanticJoin);
    }
    let tail = RootTail::decode(
        root_data
            .get(tail_start..tail_end)
            .ok_or(DealerLpAcceleratorErrorV4::SemanticJoin)?,
    )
    .map_err(|_| DealerLpAcceleratorErrorV4::SemanticJoin)?;
    if request.action == MultiLpRequestActionV3::Open
        && !DEALER_ROOT_OPEN_ADMISSIBLE_STATES_V1.admits(tail.phase)
    {
        return Err(DealerLpAcceleratorErrorV4::SemanticJoin);
    }
    let config_data = config
        .try_borrow_data()
        .map_err(|_| DealerLpAcceleratorErrorV4::SemanticJoin)?;
    if hash(&config_data).to_bytes() != context.config.to_bytes()
        || DealerConfigV4::decode(&config_data).is_err()
    {
        return Err(DealerLpAcceleratorErrorV4::SemanticJoin);
    }
    Ok(())
}

fn authenticate_obligation(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerMultiLpRequestV3,
    runtime: &[AccountInfo<'_>],
) -> Result<(), DealerLpAcceleratorErrorV4> {
    let account = account(runtime, usize::from(DEALER_LP_OBLIGATION_ACCOUNT_V3))?;
    let trading = Pubkey::new_from_array(invocation.context().trading_program.to_bytes());
    let expected = Pubkey::find_program_address(
        &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &request.child_root],
        &trading,
    )
    .0;
    if account.key != &expected
        || account.key.to_bytes() != request.obligation
        || account.owner != &trading
        || account.executable
    {
        return Err(DealerLpAcceleratorErrorV4::Obligation);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerLpAcceleratorErrorV4::Obligation)?;
    if hash(&data).to_bytes() != request.obligation_digest {
        return Err(DealerLpAcceleratorErrorV4::Obligation);
    }
    let obligation = DealerObligationProjectionV3::decode(&data)
        .map_err(|_| DealerLpAcceleratorErrorV4::Obligation)?;
    let semantic = obligation.descriptor(0);
    let product = invocation.product_runtime();
    if obligation.revision() != request.obligation_revision
        || obligation.child_root() != request.child_root
        || obligation.width() != product.runtime.outcome_count
        || semantic.market_id != request.market
        || semantic.product_id != product.runtime.product_id.to_bytes()
        || semantic.liability_basis_id != product.semantic_basis_id.to_bytes()
    {
        return Err(DealerLpAcceleratorErrorV4::Obligation);
    }
    Ok(())
}

fn authenticate_position(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerMultiLpRequestV3,
    runtime: &[AccountInfo<'_>],
) -> Result<(), DealerLpAcceleratorErrorV4> {
    let position = account(runtime, usize::from(DEALER_LP_STATE_ACCOUNT_V3))?;
    let trading = Pubkey::new_from_array(invocation.context().trading_program.to_bytes());
    let expected = Pubkey::find_program_address(
        &[
            DEALER_LP_POSITION_PDA_DOMAIN_V3,
            &request.child_root,
            &request.lp_owner,
        ],
        &trading,
    )
    .0;
    let system_coordinate = match request.action {
        MultiLpRequestActionV3::Open => 9,
        MultiLpRequestActionV3::Close => 8,
    };
    let system = account(runtime, system_coordinate)?;
    if position.key != &expected
        || position.key.to_bytes() != request.lp_position
        || system.key != &system_program::ID
        || !system.executable
    {
        return Err(DealerLpAcceleratorErrorV4::Position);
    }
    let data = position
        .try_borrow_data()
        .map_err(|_| DealerLpAcceleratorErrorV4::Position)?;
    match request.action {
        MultiLpRequestActionV3::Open => {
            if position.owner != &system_program::ID || !data.is_empty() {
                return Err(DealerLpAcceleratorErrorV4::Position);
            }
        }
        MultiLpRequestActionV3::Close => {
            if position.owner != &trading || hash(&data).to_bytes() != request.lp_digest {
                return Err(DealerLpAcceleratorErrorV4::Position);
            }
            let current = DealerLpPositionV3::decode(&data)
                .map_err(|_| DealerLpAcceleratorErrorV4::Position)?;
            if current.revision != request.lp_revision
                || current.release_set != request.release_set
                || current.market != request.market
                || current.child_root != request.child_root
                || current.lp_owner != request.lp_owner
                || current.obligation_account != request.obligation
                || current.equity_shares != 0
                || current.generation != request.generation
                || current.rent_principal != request.rent_principal
                || position.lamports() < current.rent_principal
            {
                return Err(DealerLpAcceleratorErrorV4::Position);
            }
        }
    }
    Ok(())
}

fn evaluate_transition_commit_last(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    request: DealerMultiLpRequestV3,
    candidate_bank: &mut [u8],
) -> Result<(), DealerLpAcceleratorErrorV4> {
    let mut scalars = invocation.scalars().to_vec();
    let identities = invocation.identities().to_vec();
    let trading = Pubkey::new_from_array(invocation.context().trading_program.to_bytes());
    let (_, canonical_bump) = Pubkey::find_program_address(
        &[
            DEALER_LP_POSITION_PDA_DOMAIN_V3,
            &request.child_root,
            &request.lp_owner,
        ],
        &trading,
    );
    if scalar(&scalars, LP_GENERATION_SCALAR_V3)? != request.generation
        || scalar(&scalars, LP_EXPIRY_SCALAR_V3)? != request.expires_at
        || scalar(&scalars, LP_REQUEST_RENT_PRINCIPAL_SCALAR_V3)? != request.rent_principal
        || scalar(&scalars, LP_EXPECTED_OBLIGATION_REVISION_SCALAR_V3)?
            != request.obligation_revision
        || scalar(&scalars, LP_EXPECTED_REVISION_SCALAR_V3)? != request.lp_revision
        || scalar(&scalars, LP_OBSERVED_OBLIGATION_REVISION_SCALAR_V3)?
            != request.obligation_revision
        || scalar(&scalars, LP_CURRENT_SLOT_SCALAR_V3)? > request.expires_at
        || identity(&identities, LP_RELEASE_IDENTITY_V3)? != request.release_set
        || identity(&identities, LP_MARKET_IDENTITY_V3)? != request.market
        || identity(&identities, LP_CHILD_ROOT_IDENTITY_V3)? != request.child_root
        || identity(&identities, LP_POSITION_IDENTITY_V3)? != request.lp_position
        || identity(&identities, LP_OWNER_IDENTITY_V3)? != request.lp_owner
        || identity(&identities, LP_OBLIGATION_IDENTITY_V3)? != request.obligation
        || identity(&identities, LP_OBLIGATION_DIGEST_IDENTITY_V3)? != request.obligation_digest
        || identity(&identities, LP_PRESTATE_DIGEST_IDENTITY_V3)? != request.lp_digest
        || identity(&identities, LP_OBSERVED_OBLIGATION_IDENTITY_V3)? != request.obligation
        || identity(&identities, LP_OBSERVED_POSITION_IDENTITY_V3)? != request.lp_position
    {
        return Err(DealerLpAcceleratorErrorV4::Transition);
    }
    set_scalar(
        &mut scalars,
        LP_MAGIC_SCALAR_V3,
        u64::from_le_bytes(DEALER_LP_POSITION_MAGIC_V3),
    )?;
    set_scalar(
        &mut scalars,
        LP_VERSION_SCALAR_V3,
        u64::from(DEALER_LP_POSITION_VERSION_V3),
    )?;
    set_scalar(&mut scalars, LP_INITIAL_REVISION_SCALAR_V3, 1)?;
    set_scalar(&mut scalars, LP_ZERO_SCALAR_V3, 0)?;
    match request.action {
        MultiLpRequestActionV3::Open => {
            if scalar(&scalars, LP_OBSERVED_LAMPORTS_SCALAR_V3)? != request.rent_principal
                || scalar(&scalars, LP_OBSERVED_REVISION_SCALAR_V3)? != 0
                || scalar(&scalars, LP_CREATED_SCALAR_V3)? != 1
                || scalar(&scalars, LP_LIFECYCLE_RENT_PRINCIPAL_SCALAR_V3)?
                    != request.rent_principal
                || identity(&identities, LP_LIFECYCLE_BENEFICIARY_IDENTITY_V3)? != request.lp_owner
                || identity(&identities, LP_LIFECYCLE_STATE_IDENTITY_V3)? != request.lp_position
                || identity(&identities, LP_LIFECYCLE_OWNER_IDENTITY_V3)?
                    != invocation.context().trading_program.to_bytes()
                || scalar(&scalars, LP_CANONICAL_BUMP_SCALAR_V3)? != u64::from(canonical_bump)
            {
                return Err(DealerLpAcceleratorErrorV4::Transition);
            }
        }
        MultiLpRequestActionV3::Close => {
            if scalar(&scalars, LP_OBSERVED_REVISION_SCALAR_V3)? != request.lp_revision
                || scalar(&scalars, LP_OBSERVED_SHARES_SCALAR_V3)? != 0
                || scalar(&scalars, LP_OBSERVED_RENT_PRINCIPAL_SCALAR_V3)? != request.rent_principal
                || identity(&identities, LP_OBSERVED_RELEASE_IDENTITY_V3)? != request.release_set
                || identity(&identities, LP_OBSERVED_MARKET_IDENTITY_V3)? != request.market
                || identity(&identities, LP_OBSERVED_CHILD_ROOT_IDENTITY_V3)? != request.child_root
                || identity(&identities, LP_OBSERVED_OWNER_IDENTITY_V3)? != request.lp_owner
                || identity(&identities, LP_PRESTATE_DIGEST_IDENTITY_V3)? == [0; 32]
            {
                return Err(DealerLpAcceleratorErrorV4::Transition);
            }
        }
    }
    let staged = encode_bank(&scalars, &identities, candidate_bank.len())?;
    candidate_bank.copy_from_slice(&staged);
    Ok(())
}

fn encode_bank(
    scalars: &[u64],
    identities: &[[u8; 32]],
    expected_bytes: usize,
) -> Result<Vec<u8>, DealerLpAcceleratorErrorV4> {
    let scalar_bytes = scalars
        .len()
        .checked_mul(8)
        .ok_or(DealerLpAcceleratorErrorV4::Arithmetic)?;
    let identity_bytes = identities
        .len()
        .checked_mul(32)
        .ok_or(DealerLpAcceleratorErrorV4::Arithmetic)?;
    if scalar_bytes
        .checked_add(identity_bytes)
        .ok_or(DealerLpAcceleratorErrorV4::Arithmetic)?
        != expected_bytes
    {
        return Err(DealerLpAcceleratorErrorV4::Invocation);
    }
    let mut output = vec![0_u8; expected_bytes];
    for (index, value) in scalars.iter().copied().enumerate() {
        let start = index
            .checked_mul(8)
            .ok_or(DealerLpAcceleratorErrorV4::Arithmetic)?;
        output
            .get_mut(start..start + 8)
            .ok_or(DealerLpAcceleratorErrorV4::Arithmetic)?
            .copy_from_slice(&value.to_le_bytes());
    }
    for (index, value) in identities.iter().copied().enumerate() {
        let start = scalar_bytes
            .checked_add(
                index
                    .checked_mul(32)
                    .ok_or(DealerLpAcceleratorErrorV4::Arithmetic)?,
            )
            .ok_or(DealerLpAcceleratorErrorV4::Arithmetic)?;
        output
            .get_mut(start..start + 32)
            .ok_or(DealerLpAcceleratorErrorV4::Arithmetic)?
            .copy_from_slice(&value);
    }
    Ok(output)
}

fn scalar(scalars: &[u64], index: u16) -> Result<u64, DealerLpAcceleratorErrorV4> {
    scalars
        .get(usize::from(index))
        .copied()
        .ok_or(DealerLpAcceleratorErrorV4::Invocation)
}

fn set_scalar(
    scalars: &mut [u64],
    index: u16,
    value: u64,
) -> Result<(), DealerLpAcceleratorErrorV4> {
    *scalars
        .get_mut(usize::from(index))
        .ok_or(DealerLpAcceleratorErrorV4::Invocation)? = value;
    Ok(())
}

fn identity(identities: &[[u8; 32]], index: u16) -> Result<[u8; 32], DealerLpAcceleratorErrorV4> {
    identities
        .get(usize::from(index))
        .copied()
        .ok_or(DealerLpAcceleratorErrorV4::Invocation)
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, DealerLpAcceleratorErrorV4> {
    accounts
        .get(index)
        .ok_or(DealerLpAcceleratorErrorV4::Invocation)
}
