//! Chain-derived unsigned Bearer lifecycle construction.
//!
//! This child module owns Bearer-specific hostile decoding, PDA derivation,
//! exact account frames, and value/rent projections. Shared observation and
//! refusal types remain owned by the parent vertical module.

use dclutch_bearer_contract::{
    frame::{
        AccountMetaV1 as BearerAccountMetaV1, validate_account_frame as validate_bearer_frame,
    },
    instruction::{ActionV1 as BearerActionTagV1, InstructionV1 as BearerInstructionV1},
    state::{
        BEARER_CAPABILITY_PDA_DOMAIN, BEARER_MINT_BYTES, BEARER_MINT_PDA_DOMAIN,
        BEARER_TOKEN_ACCOUNT_BYTES, BearerCapabilityV1, BearerConfigV1, MintObservationV1,
        TokenAccountObservationV1, TokenAccountStateV1,
    },
    transition::{
        CollateralDirectionV1, RealmBindingV1, activate as activate_bearer, dematerialize,
        materialize, merge_from_position, redeem_native, retire as retire_bearer,
        split_to_position,
    },
};
use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId as CapabilityContentId,
    FUNDING_STATE_BYTES, FundingCustodyObservationV1, FundingStateV1,
};
use dclutch_collateral_contract::{
    COLLATERAL_CUSTODY_PDA_DOMAIN, COLLATERAL_VAULT_PDA_DOMAIN, CollateralCustodyV1,
};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, POSITION_PDA_DOMAIN, PositionV1, REALM_PDA_DOMAIN,
    RealmV1,
};
use dclutch_token_svm::{
    AccountState as TokenAccountState, COption, ExactTransferInput, Mint as TokenMint,
    PRODUCTION_ADAPTER_RELEASES, TokenAccount,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader, bpf_loader_upgradeable, system_program};
use spl_token_2022_interface::extension::ExtensionType;

use crate::{Observation, ObservedAccount, authenticate_rent_credit, foundation};

use super::{
    VerticalError, authenticate_system_actor, authenticate_system_program, decode_owned,
    observation,
};

/// One irreducible holder choice for a native bearer-capability value route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BearerNativeValueActionV1 {
    /// Deposit collateral and credit the same quantity to every native outcome.
    DepositCompleteSet {
        /// Raw collateral and per-outcome native claim atoms.
        quantity: u64,
    },
    /// Debit the same native quantity from every outcome and withdraw collateral.
    WithdrawCompleteSet {
        /// Raw collateral and per-outcome native claim atoms.
        quantity: u64,
    },
    /// Debit one selected resolved outcome and withdraw its exact collateral payout.
    RedeemOutcome {
        /// Zero-based resolved outcome to redeem.
        outcome: u8,
        /// Raw native claim atoms to redeem.
        quantity: u64,
    },
}

impl BearerNativeValueActionV1 {
    const fn quantity(self) -> u64 {
        match self {
            Self::DepositCompleteSet { quantity }
            | Self::WithdrawCompleteSet { quantity }
            | Self::RedeemOutcome { quantity, .. } => quantity,
        }
    }
}

/// Finalized accounts required for one native Position/collateral route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BearerNativeValueState {
    /// Canonical writable Market.
    pub market: ObservedAccount,
    /// Canonical writable native Position derived from Market and holder.
    pub position: ObservedAccount,
    /// Immutable Realm selected by the Market identity.
    pub realm: ObservedAccount,
    /// Program-owned collateral-custody root.
    pub collateral_custody: ObservedAccount,
    /// Token-program-owned canonical Market collateral Vault.
    pub collateral_vault: ObservedAccount,
    /// Holder-selected collateral source or destination account.
    pub holder_collateral: ObservedAccount,
    /// Holder whose Position is mutated and whose deposit requires a signature.
    pub holder: ObservedAccount,
    /// Realm-selected executable collateral token program.
    pub collateral_token_program: ObservedAccount,
    /// Realm-selected immutable collateral Mint.
    pub collateral_mint: ObservedAccount,
}

/// Exact value movement exposed by one native bearer-capability instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerNativeValueMovementV1 {
    /// Raw collateral atoms transferred.
    pub collateral_atoms: u64,
    /// Exact collateral source account.
    pub collateral_source: Pubkey,
    /// Exact collateral destination account.
    pub collateral_destination: Pubkey,
    /// Native Position atoms debited per named outcome.
    pub position_debit_atoms: u64,
    /// Native Position atoms credited per named outcome.
    pub position_credit_atoms: u64,
    /// Selected outcome for redemption, or `None` when the quantity applies to every outcome.
    pub selected_outcome: Option<u8>,
}

/// Chain-derived native bearer-capability instruction and exact effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BearerNativeValueReport {
    /// Exact unsigned nine-account bearer instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every account fact.
    pub observation: Observation,
    /// The only outer signer required by this holder route.
    pub required_signer: Pubkey,
    /// Exact preflight value movement derived by the canonical transition.
    pub movement: BearerNativeValueMovementV1,
}

/// One irreducible holder choice for moving a claim between native and bearer form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BearerRepresentationActionV1 {
    /// Debit a native Position outcome and mint bearer claim atoms.
    Wrap {
        /// Zero-based categorical outcome.
        outcome: u8,
        /// Raw claim atoms to move.
        quantity: u64,
    },
    /// Burn bearer claim atoms and credit the native Position outcome.
    Unwrap {
        /// Zero-based categorical outcome.
        outcome: u8,
        /// Raw claim atoms to move.
        quantity: u64,
    },
}

/// Finalized accounts required to wrap or unwrap one outcome claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BearerRepresentationState {
    /// Canonical Market selecting generation and categorical width.
    pub market: ObservedAccount,
    /// Canonical bearer capability controller state.
    pub bearer_state: ObservedAccount,
    /// Holder's canonical native Position.
    pub position: ObservedAccount,
    /// Canonical outcome Mint derived from Market, generation, and outcome.
    pub claim_mint: ObservedAccount,
    /// Holder-selected Token-2022 claim account.
    pub claim_account: ObservedAccount,
    /// Holder whose Position and claim account are bound by state.
    pub holder: ObservedAccount,
    /// Official executable Token-2022 program.
    pub token_2022_program: ObservedAccount,
}

/// Exact preflight movement for one native/bearer representation change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerRepresentationMovementV1 {
    /// Selected outcome.
    pub outcome: u8,
    /// Native Position atoms debited.
    pub native_debit_atoms: u64,
    /// Native Position atoms credited.
    pub native_credit_atoms: u64,
    /// Bearer claim atoms burned.
    pub bearer_debit_atoms: u64,
    /// Bearer claim atoms minted.
    pub bearer_credit_atoms: u64,
    /// Authenticated Mint supply before the planned CPI.
    pub mint_supply_before: u64,
    /// Required Mint supply after the planned CPI.
    pub mint_supply_after: u64,
}

/// Chain-derived wrap/unwrap instruction and exact claim effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BearerRepresentationReport {
    /// Exact unsigned seven-account bearer instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every account fact.
    pub observation: Observation,
    /// The sole outer holder signer.
    pub required_signer: Pubkey,
    /// Exact native and bearer claim deltas.
    pub movement: BearerRepresentationMovementV1,
}

/// Finalized accounts required to retire one zero-supply bearer capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BearerRetireState {
    /// Retiring Market whose direct-child count is decremented.
    pub market: ObservedAccount,
    /// Program-owned bearer capability state to close.
    pub bearer_state: ObservedAccount,
    /// Manifest content selected by the Market identity.
    pub capability_manifest: ObservedAccount,
    /// Bearer config selected by the state's manifest entry.
    pub bearer_config: ObservedAccount,
    /// Permanent RentCredit selected by the config's immutable beneficiary.
    pub rent_credit: ObservedAccount,
    /// Official executable Token-2022 program.
    pub token_2022_program: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Canonical outcome Mints in exact outcome order.
    pub claim_mints: Vec<ObservedAccount>,
}

/// Chain-derived bearer capability retirement and rent-credit attribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BearerRetireReport {
    /// Exact unsigned dynamic `8 + N`-account retire instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every close fact.
    pub observation: Observation,
    /// Exact lamports routed to the immutable RentCredit from state and Mints.
    pub rent_credit_lamports: u64,
    /// Market direct-child count before the exact decrement.
    pub market_child_count_before: u64,
    /// Market direct-child count after the exact decrement.
    pub market_child_count_after: u64,
}

/// Finalized activation inputs plus exact vacant destinations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BearerActivateState {
    /// Founding or Open Market admitting the optional direct child.
    pub market: ObservedAccount,
    /// Vacant canonical bearer-state PDA.
    pub bearer_state: ObservedAccount,
    /// Manifest content selected by the Market identity.
    pub capability_manifest: ObservedAccount,
    /// Bearer config selected by the activating manifest entry.
    pub bearer_config: ObservedAccount,
    /// Mutable segregated capability funding state.
    pub funding_state: ObservedAccount,
    /// Permanent RentCredit selected by the config.
    pub rent_credit: ObservedAccount,
    /// System payer reimbursed atomically from capability funding.
    pub creation_payer: ObservedAccount,
    /// Official executable Token-2022 program.
    pub token_2022_program: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Vacant canonical outcome Mint PDAs in exact outcome order.
    pub claim_mints: Vec<ObservedAccount>,
}

/// Exact capability-funding debit and reimbursed creation spend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerActivationDebitV1 {
    /// Bearer-state Rent principal released from segregated funding.
    pub state_rent_lamports: u64,
    /// Aggregate outcome-Mint Rent principal released from segregated funding.
    pub mint_rent_lamports: u64,
    /// Exact payer reimbursement, equal to state plus Mint Rent.
    pub payer_reimbursement_lamports: u64,
}

/// Chain-derived optional bearer capability activation instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BearerActivateReport {
    /// Exact unsigned dynamic `10 + N`-account activation instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every activation fact.
    pub observation: Observation,
    /// Sole outer System payer signer.
    pub required_signer: Pubkey,
    /// Exact segregated funding debit and reimbursement.
    pub debit: BearerActivationDebitV1,
}

/// Build one exact native Position/collateral bearer-capability route.
///
/// Quantity and, for redemption, outcome are the only holder choices. Every
/// account identity, generation, Realm, token program, Mint, PDA, and transfer
/// direction is recovered from one finalized account observation.
pub fn build_bearer_native_value_v1(
    program_id: Pubkey,
    state: &BearerNativeValueState,
    action: BearerNativeValueActionV1,
) -> Result<BearerNativeValueReport, VerticalError> {
    let count =
        decode_market_outcome_count(&state.market.data).map_err(|_| VerticalError::InvalidState)?;
    match count {
        2 => bearer_native_value::<2>(program_id, state, action),
        3 => bearer_native_value::<3>(program_id, state, action),
        4 => bearer_native_value::<4>(program_id, state, action),
        5 => bearer_native_value::<5>(program_id, state, action),
        6 => bearer_native_value::<6>(program_id, state, action),
        7 => bearer_native_value::<7>(program_id, state, action),
        8 => bearer_native_value::<8>(program_id, state, action),
        9 => bearer_native_value::<9>(program_id, state, action),
        10 => bearer_native_value::<10>(program_id, state, action),
        11 => bearer_native_value::<11>(program_id, state, action),
        12 => bearer_native_value::<12>(program_id, state, action),
        13 => bearer_native_value::<13>(program_id, state, action),
        14 => bearer_native_value::<14>(program_id, state, action),
        15 => bearer_native_value::<15>(program_id, state, action),
        16 => bearer_native_value::<16>(program_id, state, action),
        _ => Err(VerticalError::AbiUnavailable),
    }
}

/// Build one exact native-to-bearer wrap or bearer-to-native unwrap route.
pub fn build_bearer_representation_v1(
    program_id: Pubkey,
    state: &BearerRepresentationState,
    action: BearerRepresentationActionV1,
) -> Result<BearerRepresentationReport, VerticalError> {
    let count =
        decode_market_outcome_count(&state.market.data).map_err(|_| VerticalError::InvalidState)?;
    match count {
        2 => bearer_representation::<2>(program_id, state, action),
        3 => bearer_representation::<3>(program_id, state, action),
        4 => bearer_representation::<4>(program_id, state, action),
        5 => bearer_representation::<5>(program_id, state, action),
        6 => bearer_representation::<6>(program_id, state, action),
        7 => bearer_representation::<7>(program_id, state, action),
        8 => bearer_representation::<8>(program_id, state, action),
        9 => bearer_representation::<9>(program_id, state, action),
        10 => bearer_representation::<10>(program_id, state, action),
        11 => bearer_representation::<11>(program_id, state, action),
        12 => bearer_representation::<12>(program_id, state, action),
        13 => bearer_representation::<13>(program_id, state, action),
        14 => bearer_representation::<14>(program_id, state, action),
        15 => bearer_representation::<15>(program_id, state, action),
        16 => bearer_representation::<16>(program_id, state, action),
        _ => Err(VerticalError::AbiUnavailable),
    }
}

/// Build the permissionless zero-supply bearer capability retirement route.
pub fn build_bearer_retire_v1(
    program_id: Pubkey,
    state: &BearerRetireState,
) -> Result<BearerRetireReport, VerticalError> {
    let count =
        decode_market_outcome_count(&state.market.data).map_err(|_| VerticalError::InvalidState)?;
    match count {
        2 => bearer_retire::<2>(program_id, state),
        3 => bearer_retire::<3>(program_id, state),
        4 => bearer_retire::<4>(program_id, state),
        5 => bearer_retire::<5>(program_id, state),
        6 => bearer_retire::<6>(program_id, state),
        7 => bearer_retire::<7>(program_id, state),
        8 => bearer_retire::<8>(program_id, state),
        9 => bearer_retire::<9>(program_id, state),
        10 => bearer_retire::<10>(program_id, state),
        11 => bearer_retire::<11>(program_id, state),
        12 => bearer_retire::<12>(program_id, state),
        13 => bearer_retire::<13>(program_id, state),
        14 => bearer_retire::<14>(program_id, state),
        15 => bearer_retire::<15>(program_id, state),
        16 => bearer_retire::<16>(program_id, state),
        _ => Err(VerticalError::AbiUnavailable),
    }
}

/// Build one manifest-selected, exactly funded bearer capability activation.
pub fn build_bearer_activate_v1(
    program_id: Pubkey,
    state: &BearerActivateState,
) -> Result<BearerActivateReport, VerticalError> {
    let count =
        decode_market_outcome_count(&state.market.data).map_err(|_| VerticalError::InvalidState)?;
    match count {
        2 => bearer_activate::<2>(program_id, state),
        3 => bearer_activate::<3>(program_id, state),
        4 => bearer_activate::<4>(program_id, state),
        5 => bearer_activate::<5>(program_id, state),
        6 => bearer_activate::<6>(program_id, state),
        7 => bearer_activate::<7>(program_id, state),
        8 => bearer_activate::<8>(program_id, state),
        9 => bearer_activate::<9>(program_id, state),
        10 => bearer_activate::<10>(program_id, state),
        11 => bearer_activate::<11>(program_id, state),
        12 => bearer_activate::<12>(program_id, state),
        13 => bearer_activate::<13>(program_id, state),
        14 => bearer_activate::<14>(program_id, state),
        15 => bearer_activate::<15>(program_id, state),
        16 => bearer_activate::<16>(program_id, state),
        _ => Err(VerticalError::AbiUnavailable),
    }
}

fn bearer_native_value<const N: usize>(
    program_id: Pubkey,
    state: &BearerNativeValueState,
    action: BearerNativeValueActionV1,
) -> Result<BearerNativeValueReport, VerticalError> {
    let observation = observation(&[
        &state.market,
        &state.position,
        &state.realm,
        &state.collateral_custody,
        &state.collateral_vault,
        &state.holder_collateral,
        &state.holder,
        &state.collateral_token_program,
        &state.collateral_mint,
    ])?;
    let mut market = canonical_market::<N>(program_id, &state.market)?;
    let generation = market.root().identity().generation();
    let realm = authenticate_bearer_realm(
        program_id,
        &state.realm,
        &state.collateral_token_program,
        &state.collateral_mint,
        market.root().identity().realm_id().to_bytes(),
    )?;
    authenticate_bearer_custody(
        program_id,
        &state.collateral_custody,
        state.market.key,
        generation,
    )?;
    let vault = authenticate_bearer_vault(
        program_id,
        &state.collateral_vault,
        state.market.key,
        &state.collateral_mint,
        realm,
    )?;
    let mut position = decode_owned(&state.position, program_id, PositionV1::<N>::decode)?;
    let mut canonical_position =
        vec![0; PositionV1::<N>::encoded_len().map_err(|_| VerticalError::InvalidState)?];
    position
        .encode(&mut canonical_position)
        .map_err(|_| VerticalError::InvalidState)?;
    let (expected_position, _) = Pubkey::find_program_address(
        &[
            POSITION_PDA_DOMAIN,
            state.market.key.as_ref(),
            state.holder.key.as_ref(),
        ],
        &program_id,
    );
    if canonical_position != state.position.data
        || state.position.key != expected_position
        || position.market() != state.market.key.as_ref()
        || position.owner() != state.holder.key.as_ref()
        || position.generation() != generation
    {
        return Err(VerticalError::PdaMismatch);
    }
    if state.holder.key == Pubkey::default()
        || state.holder.executable
        || state.holder.key == state.holder_collateral.key
    {
        return Err(VerticalError::InvalidAuthority);
    }

    let quantity = action.quantity();
    let realm_binding = RealmBindingV1 {
        content_id: dclutch_capability_contract::ContentId::new(
            hash(&realm.realm.to_bytes()).to_bytes(),
        )
        .map_err(|_| VerticalError::ContentMismatch)?,
        realm: realm.realm,
    };
    let plan = match action {
        BearerNativeValueActionV1::DepositCompleteSet { .. } => split_to_position(
            state.market.key.to_bytes(),
            &mut market,
            &mut position,
            state.holder.key.to_bytes(),
            realm_binding,
            quantity,
        ),
        BearerNativeValueActionV1::WithdrawCompleteSet { .. } => merge_from_position(
            state.market.key.to_bytes(),
            &mut market,
            &mut position,
            state.holder.key.to_bytes(),
            realm_binding,
            quantity,
        ),
        BearerNativeValueActionV1::RedeemOutcome { outcome, .. } => redeem_native(
            state.market.key.to_bytes(),
            &mut market,
            &mut position,
            state.holder.key.to_bytes(),
            realm_binding,
            usize::from(outcome),
            quantity,
        )
        .map(|redemption| redemption.payout),
    }
    .map_err(|_| VerticalError::InvalidPhase)?;

    let (source, destination, authority) = match plan.direction() {
        CollateralDirectionV1::DepositToHoard => (
            &state.holder_collateral,
            &state.collateral_vault,
            state.holder.key,
        ),
        CollateralDirectionV1::WithdrawFromHoard => (
            &state.collateral_vault,
            &state.holder_collateral,
            state.market.key,
        ),
    };
    let source_data = source.data.as_slice();
    let destination_data = destination.data.as_slice();
    realm
        .release
        .profile()
        .check_transfer(ExactTransferInput {
            program_id: state.collateral_token_program.key.to_bytes(),
            mint_address: state.collateral_mint.key.to_bytes(),
            mint_data: &state.collateral_mint.data,
            source_data,
            destination_data,
            authority: authority.to_bytes(),
            amount: plan.amount(),
            decimals: realm.mint.decimals,
        })
        .map_err(|_| VerticalError::InvalidState)?;
    if vault.amount
        < if matches!(plan.direction(), CollateralDirectionV1::WithdrawFromHoard) {
            plan.amount()
        } else {
            0
        }
    {
        return Err(VerticalError::InvalidState);
    }

    let instruction_kind = match action {
        BearerNativeValueActionV1::DepositCompleteSet { .. } => BearerActionTagV1::SplitNative,
        BearerNativeValueActionV1::WithdrawCompleteSet { .. } => BearerActionTagV1::MergeNative,
        BearerNativeValueActionV1::RedeemOutcome { .. } => BearerActionTagV1::RedeemNative,
    };
    let outcome_count = u8::try_from(N).map_err(|_| VerticalError::AbiUnavailable)?;
    let wire = match action {
        BearerNativeValueActionV1::RedeemOutcome { outcome, .. } => BearerInstructionV1::Outcome {
            action: instruction_kind,
            outcome_count,
            generation,
            quantity,
            outcome,
        },
        _ => BearerInstructionV1::Set {
            action: instruction_kind,
            outcome_count,
            generation,
            quantity,
        },
    };
    let mut data = vec![0; wire.encoded_len()];
    wire.encode(&mut data)
        .map_err(|_| VerticalError::InvalidState)?;
    let accounts = [
        &state.market,
        &state.position,
        &state.realm,
        &state.collateral_custody,
        &state.collateral_vault,
        &state.holder_collateral,
        &state.holder,
        &state.collateral_token_program,
        &state.collateral_mint,
    ];
    let frame = accounts.map(|account| BearerAccountMetaV1 {
        key: account.key.to_bytes(),
        is_signer: account.key == state.holder.key,
        is_writable: matches!(
            account.key,
            key if key == state.market.key
                || key == state.position.key
                || key == state.collateral_vault.key
                || key == state.holder_collateral.key
        ),
        is_executable: account.executable,
    });
    validate_bearer_frame::<N>(instruction_kind, &frame)
        .map_err(|_| VerticalError::InvalidState)?;
    let selected_outcome = match action {
        BearerNativeValueActionV1::RedeemOutcome { outcome, .. } => Some(outcome),
        _ => None,
    };
    let (position_debit_atoms, position_credit_atoms) = match action {
        BearerNativeValueActionV1::DepositCompleteSet { .. } => (0, quantity),
        BearerNativeValueActionV1::WithdrawCompleteSet { .. }
        | BearerNativeValueActionV1::RedeemOutcome { .. } => (quantity, 0),
    };
    Ok(BearerNativeValueReport {
        instruction: Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(state.market.key, false),
                AccountMeta::new(state.position.key, false),
                AccountMeta::new_readonly(state.realm.key, false),
                AccountMeta::new_readonly(state.collateral_custody.key, false),
                AccountMeta::new(state.collateral_vault.key, false),
                AccountMeta::new(state.holder_collateral.key, false),
                AccountMeta::new_readonly(state.holder.key, true),
                AccountMeta::new_readonly(state.collateral_token_program.key, false),
                AccountMeta::new_readonly(state.collateral_mint.key, false),
            ],
            data,
        },
        observation,
        required_signer: state.holder.key,
        movement: BearerNativeValueMovementV1 {
            collateral_atoms: plan.amount(),
            collateral_source: source.key,
            collateral_destination: destination.key,
            position_debit_atoms,
            position_credit_atoms,
            selected_outcome,
        },
    })
}

fn bearer_representation<const N: usize>(
    program_id: Pubkey,
    state: &BearerRepresentationState,
    action: BearerRepresentationActionV1,
) -> Result<BearerRepresentationReport, VerticalError> {
    let observation = observation(&[
        &state.market,
        &state.bearer_state,
        &state.position,
        &state.claim_mint,
        &state.claim_account,
        &state.holder,
        &state.token_2022_program,
    ])?;
    let market = canonical_market::<N>(program_id, &state.market)?;
    let generation = market.root().identity().generation();
    let mut bearer = canonical_bearer_state::<N>(
        program_id,
        &state.bearer_state,
        state.market.key,
        generation,
    )?;
    let mut position = canonical_bearer_position::<N>(
        program_id,
        &state.position,
        state.market.key,
        state.holder.key,
        generation,
    )?;
    authenticate_token_2022_program(&state.token_2022_program)?;
    if state.holder.key == Pubkey::default() || state.holder.executable {
        return Err(VerticalError::InvalidAuthority);
    }
    let (outcome, quantity, tag) = match action {
        BearerRepresentationActionV1::Wrap { outcome, quantity } => {
            (outcome, quantity, BearerActionTagV1::Materialize)
        }
        BearerRepresentationActionV1::Unwrap { outcome, quantity } => {
            (outcome, quantity, BearerActionTagV1::Dematerialize)
        }
    };
    let expected_mint = canonical_bearer_mint(
        program_id,
        state.market.key,
        generation,
        usize::from(outcome),
        N,
    )?;
    if state.claim_mint.key != expected_mint {
        return Err(VerticalError::PdaMismatch);
    }
    let mint = parse_bearer_mint(&state.claim_mint, &state.token_2022_program)?;
    let claim = parse_bearer_claim_account(&state.claim_account, &state.token_2022_program)?;
    let plan = match action {
        BearerRepresentationActionV1::Wrap { .. } => materialize(
            state.market.key.to_bytes(),
            &market,
            &mut bearer,
            &mut position,
            state.holder.key.to_bytes(),
            usize::from(outcome),
            quantity,
            state.bearer_state.key.to_bytes(),
            expected_mint.to_bytes(),
            mint,
            claim,
        ),
        BearerRepresentationActionV1::Unwrap { .. } => dematerialize(
            state.market.key.to_bytes(),
            &market,
            &mut bearer,
            &mut position,
            state.holder.key.to_bytes(),
            usize::from(outcome),
            quantity,
            state.bearer_state.key.to_bytes(),
            expected_mint.to_bytes(),
            mint,
            claim,
        ),
    }
    .map_err(|_| VerticalError::InvalidPhase)?;
    let outcome_count = u8::try_from(N).map_err(|_| VerticalError::AbiUnavailable)?;
    let wire = BearerInstructionV1::Outcome {
        action: tag,
        outcome_count,
        generation,
        quantity,
        outcome,
    };
    let mut data = vec![0; wire.encoded_len()];
    wire.encode(&mut data)
        .map_err(|_| VerticalError::InvalidState)?;
    let accounts = [
        &state.market,
        &state.bearer_state,
        &state.position,
        &state.claim_mint,
        &state.claim_account,
        &state.holder,
        &state.token_2022_program,
    ];
    let frame = accounts.map(|account| BearerAccountMetaV1 {
        key: account.key.to_bytes(),
        is_signer: account.key == state.holder.key,
        is_writable: matches!(
            account.key,
            key if key == state.bearer_state.key
                || key == state.position.key
                || key == state.claim_mint.key
                || key == state.claim_account.key
        ),
        is_executable: account.executable,
    });
    validate_bearer_frame::<N>(tag, &frame).map_err(|_| VerticalError::InvalidState)?;
    let (native_debit_atoms, native_credit_atoms, bearer_debit_atoms, bearer_credit_atoms) =
        match action {
            BearerRepresentationActionV1::Wrap { .. } => (quantity, 0, 0, quantity),
            BearerRepresentationActionV1::Unwrap { .. } => (0, quantity, quantity, 0),
        };
    Ok(BearerRepresentationReport {
        instruction: Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new_readonly(state.market.key, false),
                AccountMeta::new(state.bearer_state.key, false),
                AccountMeta::new(state.position.key, false),
                AccountMeta::new(state.claim_mint.key, false),
                AccountMeta::new(state.claim_account.key, false),
                AccountMeta::new_readonly(state.holder.key, true),
                AccountMeta::new_readonly(state.token_2022_program.key, false),
            ],
            data,
        },
        observation,
        required_signer: state.holder.key,
        movement: BearerRepresentationMovementV1 {
            outcome,
            native_debit_atoms,
            native_credit_atoms,
            bearer_debit_atoms,
            bearer_credit_atoms,
            mint_supply_before: plan.mint_supply_before,
            mint_supply_after: plan.mint_supply_after,
        },
    })
}

fn bearer_retire<const N: usize>(
    program_id: Pubkey,
    state: &BearerRetireState,
) -> Result<BearerRetireReport, VerticalError> {
    if state.claim_mints.len() != N {
        return Err(VerticalError::InvalidState);
    }
    let mut observed = vec![
        &state.market,
        &state.bearer_state,
        &state.capability_manifest,
        &state.bearer_config,
        &state.rent_credit,
        &state.token_2022_program,
        &state.system_program,
        &state.rent_sysvar,
    ];
    observed.extend(state.claim_mints.iter());
    let observation = observation(&observed)?;
    authenticate_token_2022_program(&state.token_2022_program)?;
    authenticate_system_program(&state.system_program)?;
    foundation::decode_rent(&state.rent_sysvar).map_err(|_| VerticalError::InvalidState)?;
    let mut market = canonical_market::<N>(program_id, &state.market)?;
    let generation = market.root().identity().generation();
    let bearer = canonical_bearer_state::<N>(
        program_id,
        &state.bearer_state,
        state.market.key,
        generation,
    )?;
    if state.capability_manifest.owner != program_id
        || state.capability_manifest.executable
        || state.bearer_config.owner != program_id
        || state.bearer_config.executable
    {
        return Err(VerticalError::InvalidOwner);
    }
    let manifest = CapabilityManifestV1::decode(&state.capability_manifest.data)
        .map_err(|_| VerticalError::InvalidState)?;
    let manifest_id = CapabilityContentId::new(hash(&state.capability_manifest.data).to_bytes())
        .map_err(|_| VerticalError::ContentMismatch)?;
    if market.root().identity().capability_manifest_id().to_bytes() != manifest_id.to_bytes() {
        return Err(VerticalError::ContentMismatch);
    }
    let config = BearerConfigV1::decode(&state.bearer_config.data)
        .map_err(|_| VerticalError::InvalidState)?;
    if config.to_bytes().as_slice() != state.bearer_config.data.as_slice() {
        return Err(VerticalError::InvalidState);
    }
    let config_id = CapabilityContentId::new(hash(&state.bearer_config.data).to_bytes())
        .map_err(|_| VerticalError::ContentMismatch)?;
    let beneficiary = Pubkey::new_from_array(config.rent_refund());
    authenticate_rent_credit(program_id, &state.rent_credit, beneficiary)
        .map_err(|_| VerticalError::ContentMismatch)?;

    let mut mint_keys = [[0; 32]; N];
    let mut mint_observations = [empty_bearer_mint(); N];
    let mut mint_lamports = 0u64;
    for (index, account) in state.claim_mints.iter().enumerate() {
        let expected = canonical_bearer_mint(program_id, state.market.key, generation, index, N)?;
        if account.key != expected {
            return Err(VerticalError::PdaMismatch);
        }
        *mint_keys
            .get_mut(index)
            .ok_or(VerticalError::InvalidState)? = expected.to_bytes();
        *mint_observations
            .get_mut(index)
            .ok_or(VerticalError::InvalidState)? =
            parse_bearer_mint(account, &state.token_2022_program)?;
        mint_lamports = mint_lamports
            .checked_add(account.lamports)
            .ok_or(VerticalError::InvalidState)?;
    }
    let expected_prior_child_count = market.root().outstanding_children();
    let plan = retire_bearer(
        state.market.key.to_bytes(),
        &mut market,
        bearer,
        manifest_id,
        manifest,
        config_id,
        config,
        expected_prior_child_count,
        state.bearer_state.key.to_bytes(),
        mint_keys,
        mint_observations,
    )
    .map_err(|_| VerticalError::InvalidPhase)?;
    if plan.rent_refund != beneficiary.to_bytes() {
        return Err(VerticalError::ContentMismatch);
    }
    let rent_credit_lamports = state
        .bearer_state
        .lamports
        .checked_add(mint_lamports)
        .ok_or(VerticalError::InvalidState)?;
    let outcome_count = u8::try_from(N).map_err(|_| VerticalError::AbiUnavailable)?;
    let wire = BearerInstructionV1::Retire {
        outcome_count,
        generation,
        expected_prior_child_count,
    };
    let mut data = vec![0; wire.encoded_len()];
    wire.encode(&mut data)
        .map_err(|_| VerticalError::InvalidState)?;
    let mut instruction_accounts = vec![
        AccountMeta::new(state.market.key, false),
        AccountMeta::new(state.bearer_state.key, false),
        AccountMeta::new_readonly(state.capability_manifest.key, false),
        AccountMeta::new_readonly(state.bearer_config.key, false),
        AccountMeta::new(state.rent_credit.key, false),
        AccountMeta::new_readonly(state.token_2022_program.key, false),
        AccountMeta::new_readonly(state.system_program.key, false),
        AccountMeta::new_readonly(state.rent_sysvar.key, false),
    ];
    instruction_accounts.extend(
        state
            .claim_mints
            .iter()
            .map(|account| AccountMeta::new(account.key, false)),
    );
    let mut frame = vec![
        bearer_meta(&state.market, false, true),
        bearer_meta(&state.bearer_state, false, true),
        bearer_meta(&state.capability_manifest, false, false),
        bearer_meta(&state.bearer_config, false, false),
        bearer_meta(&state.rent_credit, false, true),
        bearer_meta(&state.token_2022_program, false, false),
        bearer_meta(&state.system_program, false, false),
        bearer_meta(&state.rent_sysvar, false, false),
    ];
    frame.extend(
        state
            .claim_mints
            .iter()
            .map(|account| bearer_meta(account, false, true)),
    );
    validate_bearer_frame::<N>(BearerActionTagV1::Retire, &frame)
        .map_err(|_| VerticalError::InvalidState)?;
    Ok(BearerRetireReport {
        instruction: Instruction {
            program_id,
            accounts: instruction_accounts,
            data,
        },
        observation,
        rent_credit_lamports,
        market_child_count_before: plan.market_child_count_before,
        market_child_count_after: plan.market_child_count_after,
    })
}

fn bearer_activate<const N: usize>(
    program_id: Pubkey,
    state: &BearerActivateState,
) -> Result<BearerActivateReport, VerticalError> {
    if state.claim_mints.len() != N {
        return Err(VerticalError::InvalidState);
    }
    let mut observed = vec![
        &state.market,
        &state.bearer_state,
        &state.capability_manifest,
        &state.bearer_config,
        &state.funding_state,
        &state.rent_credit,
        &state.creation_payer,
        &state.token_2022_program,
        &state.system_program,
        &state.rent_sysvar,
    ];
    observed.extend(state.claim_mints.iter());
    let observation = observation(&observed)?;
    authenticate_token_2022_program(&state.token_2022_program)?;
    authenticate_system_program(&state.system_program)?;
    authenticate_system_actor(&state.creation_payer)?;
    let rent =
        foundation::decode_rent(&state.rent_sysvar).map_err(|_| VerticalError::InvalidState)?;
    let mut market = canonical_market::<N>(program_id, &state.market)?;
    let generation = market.root().identity().generation();
    if state.capability_manifest.owner != program_id
        || state.capability_manifest.executable
        || state.bearer_config.owner != program_id
        || state.bearer_config.executable
    {
        return Err(VerticalError::InvalidOwner);
    }
    let manifest = CapabilityManifestV1::decode(&state.capability_manifest.data)
        .map_err(|_| VerticalError::InvalidState)?;
    let manifest_id = CapabilityContentId::new(hash(&state.capability_manifest.data).to_bytes())
        .map_err(|_| VerticalError::ContentMismatch)?;
    if market.root().identity().capability_manifest_id().to_bytes() != manifest_id.to_bytes() {
        return Err(VerticalError::ContentMismatch);
    }
    let config = BearerConfigV1::decode(&state.bearer_config.data)
        .map_err(|_| VerticalError::InvalidState)?;
    if config.to_bytes().as_slice() != state.bearer_config.data.as_slice() {
        return Err(VerticalError::InvalidState);
    }
    let config_id = CapabilityContentId::new(hash(&state.bearer_config.data).to_bytes())
        .map_err(|_| VerticalError::ContentMismatch)?;
    authenticate_rent_credit(
        program_id,
        &state.rent_credit,
        Pubkey::new_from_array(config.rent_refund()),
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    let mut funding = decode_owned(&state.funding_state, program_id, FundingStateV1::decode)?;
    if funding.to_bytes().as_slice() != state.funding_state.data.as_slice() {
        return Err(VerticalError::InvalidState);
    }
    let funding_derivation = CapabilityFundingDerivationV1::new(
        state.market.key.to_bytes(),
        generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    let (expected_funding, _) =
        Pubkey::find_program_address(&funding_derivation.seed_components(), &program_id);
    if state.funding_state.key != expected_funding {
        return Err(VerticalError::PdaMismatch);
    }
    let funding_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let funding_custody =
        FundingCustodyObservationV1::native_only(state.funding_state.lamports, funding_rent)
            .map_err(|_| VerticalError::InvalidState)?;
    let state_rent = rent.minimum_balance(
        BearerCapabilityV1::<N>::encoded_len().map_err(|_| VerticalError::InvalidState)?,
    );
    let one_mint_rent = rent.minimum_balance(BEARER_MINT_BYTES);
    let mint_rent = one_mint_rent
        .checked_mul(u64::try_from(N).map_err(|_| VerticalError::InvalidState)?)
        .ok_or(VerticalError::InvalidState)?;
    let total_rent = state_rent
        .checked_add(mint_rent)
        .ok_or(VerticalError::InvalidState)?;
    if state.creation_payer.lamports < total_rent {
        return Err(VerticalError::InvalidAuthority);
    }
    let (expected_state, _) = Pubkey::find_program_address(
        &[
            BEARER_CAPABILITY_PDA_DOMAIN,
            state.market.key.as_ref(),
            &generation.to_le_bytes(),
        ],
        &program_id,
    );
    authenticate_vacant_observation(&state.bearer_state, expected_state)?;
    let mut mint_keys = [[0; 32]; N];
    for (index, account) in state.claim_mints.iter().enumerate() {
        let expected = canonical_bearer_mint(program_id, state.market.key, generation, index, N)?;
        authenticate_vacant_observation(account, expected)?;
        *mint_keys
            .get_mut(index)
            .ok_or(VerticalError::InvalidState)? = expected.to_bytes();
    }
    let expected_prior_child_count = market.root().outstanding_children();
    activate_bearer(
        state.market.key.to_bytes(),
        &mut market,
        manifest_id,
        manifest,
        config_id,
        config,
        &mut funding,
        funding_custody,
        observation.slot,
        state_rent,
        mint_rent,
        expected_prior_child_count,
        state.bearer_state.key.to_bytes(),
        mint_keys,
    )
    .map_err(|_| VerticalError::InvalidPhase)?;
    let outcome_count = u8::try_from(N).map_err(|_| VerticalError::AbiUnavailable)?;
    let wire = BearerInstructionV1::Activate {
        outcome_count,
        generation,
        expected_prior_child_count,
    };
    let mut data = vec![0; wire.encoded_len()];
    wire.encode(&mut data)
        .map_err(|_| VerticalError::InvalidState)?;
    let mut instruction_accounts = vec![
        AccountMeta::new(state.market.key, false),
        AccountMeta::new(state.bearer_state.key, false),
        AccountMeta::new_readonly(state.capability_manifest.key, false),
        AccountMeta::new_readonly(state.bearer_config.key, false),
        AccountMeta::new(state.funding_state.key, false),
        AccountMeta::new_readonly(state.rent_credit.key, false),
        AccountMeta::new(state.creation_payer.key, true),
        AccountMeta::new_readonly(state.token_2022_program.key, false),
        AccountMeta::new_readonly(state.system_program.key, false),
        AccountMeta::new_readonly(state.rent_sysvar.key, false),
    ];
    instruction_accounts.extend(
        state
            .claim_mints
            .iter()
            .map(|account| AccountMeta::new(account.key, false)),
    );
    let mut frame = vec![
        bearer_meta(&state.market, false, true),
        bearer_meta(&state.bearer_state, false, true),
        bearer_meta(&state.capability_manifest, false, false),
        bearer_meta(&state.bearer_config, false, false),
        bearer_meta(&state.funding_state, false, true),
        bearer_meta(&state.rent_credit, false, false),
        bearer_meta(&state.creation_payer, true, true),
        bearer_meta(&state.token_2022_program, false, false),
        bearer_meta(&state.system_program, false, false),
        bearer_meta(&state.rent_sysvar, false, false),
    ];
    frame.extend(
        state
            .claim_mints
            .iter()
            .map(|account| bearer_meta(account, false, true)),
    );
    validate_bearer_frame::<N>(BearerActionTagV1::Activate, &frame)
        .map_err(|_| VerticalError::InvalidState)?;
    Ok(BearerActivateReport {
        instruction: Instruction {
            program_id,
            accounts: instruction_accounts,
            data,
        },
        observation,
        required_signer: state.creation_payer.key,
        debit: BearerActivationDebitV1 {
            state_rent_lamports: state_rent,
            mint_rent_lamports: mint_rent,
            payer_reimbursement_lamports: total_rent,
        },
    })
}

fn authenticate_vacant_observation(
    account: &ObservedAccount,
    expected: Pubkey,
) -> Result<(), VerticalError> {
    if account.key != expected
        || account.owner != system_program::ID
        || account.lamports != 0
        || account.executable
        || !account.data.is_empty()
    {
        return Err(VerticalError::PdaMismatch);
    }
    Ok(())
}

fn bearer_meta(
    account: &ObservedAccount,
    is_signer: bool,
    is_writable: bool,
) -> BearerAccountMetaV1 {
    BearerAccountMetaV1 {
        key: account.key.to_bytes(),
        is_signer,
        is_writable,
        is_executable: account.executable,
    }
}

fn empty_bearer_mint() -> MintObservationV1 {
    MintObservationV1 {
        key: [0; 32],
        program_owner: [0; 32],
        data_len: 0,
        supply: 0,
        decimals: 0,
        initialized: false,
        mint_authority: None,
        freeze_authority: None,
        close_authority: None,
        permissioned_burn_authority: None,
        extension_count: 0,
    }
}

fn canonical_bearer_state<const N: usize>(
    program_id: Pubkey,
    account: &ObservedAccount,
    market: Pubkey,
    generation: u64,
) -> Result<BearerCapabilityV1<N>, VerticalError> {
    let state = decode_owned(account, program_id, BearerCapabilityV1::<N>::decode)?;
    let mut canonical =
        vec![0; BearerCapabilityV1::<N>::encoded_len().map_err(|_| VerticalError::InvalidState)?];
    state
        .encode(&mut canonical)
        .map_err(|_| VerticalError::InvalidState)?;
    let (expected, _) = Pubkey::find_program_address(
        &[
            BEARER_CAPABILITY_PDA_DOMAIN,
            market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &program_id,
    );
    if canonical != account.data
        || account.key != expected
        || state.market() != market.to_bytes()
        || state.generation() != generation
    {
        return Err(VerticalError::PdaMismatch);
    }
    Ok(state)
}

fn canonical_bearer_position<const N: usize>(
    program_id: Pubkey,
    account: &ObservedAccount,
    market: Pubkey,
    holder: Pubkey,
    generation: u64,
) -> Result<PositionV1<N>, VerticalError> {
    let position = decode_owned(account, program_id, PositionV1::<N>::decode)?;
    let mut canonical =
        vec![0; PositionV1::<N>::encoded_len().map_err(|_| VerticalError::InvalidState)?];
    position
        .encode(&mut canonical)
        .map_err(|_| VerticalError::InvalidState)?;
    let (expected, _) = Pubkey::find_program_address(
        &[POSITION_PDA_DOMAIN, market.as_ref(), holder.as_ref()],
        &program_id,
    );
    if canonical != account.data
        || account.key != expected
        || position.market() != market.as_ref()
        || position.owner() != holder.as_ref()
        || position.generation() != generation
    {
        return Err(VerticalError::PdaMismatch);
    }
    Ok(position)
}

fn authenticate_token_2022_program(account: &ObservedAccount) -> Result<(), VerticalError> {
    if account.key.to_bytes() != dclutch_token_svm::TOKEN_2022_PROGRAM_ID
        || !account.executable
        || !matches!(account.owner, key if key == bpf_loader::ID || key == bpf_loader_upgradeable::ID)
    {
        return Err(VerticalError::InvalidOwner);
    }
    Ok(())
}

fn canonical_bearer_mint(
    program_id: Pubkey,
    market: Pubkey,
    generation: u64,
    outcome: usize,
    outcome_count: usize,
) -> Result<Pubkey, VerticalError> {
    if outcome >= outcome_count {
        return Err(VerticalError::InvalidState);
    }
    let outcome = u8::try_from(outcome).map_err(|_| VerticalError::InvalidState)?;
    Ok(Pubkey::find_program_address(
        &[
            BEARER_MINT_PDA_DOMAIN,
            market.as_ref(),
            &generation.to_le_bytes(),
            &[outcome],
        ],
        &program_id,
    )
    .0)
}

fn parse_bearer_mint(
    account: &ObservedAccount,
    token_program: &ObservedAccount,
) -> Result<MintObservationV1, VerticalError> {
    if account.owner != token_program.key
        || account.executable
        || account.data.len() != BEARER_MINT_BYTES
    {
        return Err(VerticalError::InvalidOwner);
    }
    let base = TokenMint::parse(
        account
            .data
            .get(..dclutch_token_svm::MINT_BYTES)
            .ok_or(VerticalError::InvalidState)?,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    if account.data.get(82..165) != Some(&[0; 83]) || account.data.get(165) != Some(&1) {
        return Err(VerticalError::InvalidState);
    }
    let mut close_authority = None;
    let mut permissioned_burn_authority = None;
    let mut offset = 166usize;
    let mut extension_count = 0u16;
    while offset < account.data.len() {
        let kind = read_u16(&account.data, offset)?;
        let length = usize::from(read_u16(&account.data, offset + 2)?);
        let end = offset
            .checked_add(4)
            .and_then(|start| start.checked_add(length))
            .ok_or(VerticalError::InvalidState)?;
        let value: [u8; 32] = account
            .data
            .get(offset + 4..end)
            .ok_or(VerticalError::InvalidState)?
            .try_into()
            .map_err(|_| VerticalError::InvalidState)?;
        if length != 32 {
            return Err(VerticalError::InvalidState);
        }
        if kind == ExtensionType::MintCloseAuthority as u16 && close_authority.is_none() {
            close_authority = Some(value);
        } else if kind == ExtensionType::PermissionedBurn as u16
            && permissioned_burn_authority.is_none()
        {
            permissioned_burn_authority = Some(value);
        } else {
            return Err(VerticalError::InvalidState);
        }
        extension_count = extension_count
            .checked_add(1)
            .ok_or(VerticalError::InvalidState)?;
        offset = end;
    }
    Ok(MintObservationV1 {
        key: account.key.to_bytes(),
        program_owner: account.owner.to_bytes(),
        data_len: account.data.len(),
        supply: base.supply,
        decimals: base.decimals,
        initialized: base.is_initialized,
        mint_authority: coption_address(base.mint_authority),
        freeze_authority: coption_address(base.freeze_authority),
        close_authority,
        permissioned_burn_authority,
        extension_count,
    })
}

fn parse_bearer_claim_account(
    account: &ObservedAccount,
    token_program: &ObservedAccount,
) -> Result<TokenAccountObservationV1, VerticalError> {
    if account.owner != token_program.key
        || account.executable
        || account.data.len() != BEARER_TOKEN_ACCOUNT_BYTES
    {
        return Err(VerticalError::InvalidOwner);
    }
    let token = TokenAccount::parse(&account.data).map_err(|_| VerticalError::InvalidState)?;
    let state = match token.state {
        TokenAccountState::Uninitialized => TokenAccountStateV1::Uninitialized,
        TokenAccountState::Initialized => TokenAccountStateV1::Initialized,
        TokenAccountState::Frozen => TokenAccountStateV1::Frozen,
    };
    Ok(TokenAccountObservationV1 {
        key: account.key.to_bytes(),
        program_owner: account.owner.to_bytes(),
        data_len: account.data.len(),
        mint: token.mint,
        authority: token.owner,
        amount: token.amount,
        state,
        has_native_reserve: !matches!(token.native_reserve, COption::None),
        extension_count: 0,
    })
}

fn coption_address(value: COption<[u8; 32]>) -> Option<[u8; 32]> {
    match value {
        COption::None => None,
        COption::Some(address) => Some(address),
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, VerticalError> {
    let value: [u8; 2] = bytes
        .get(offset..offset + 2)
        .ok_or(VerticalError::InvalidState)?
        .try_into()
        .map_err(|_| VerticalError::InvalidState)?;
    Ok(u16::from_le_bytes(value))
}

#[derive(Clone, Copy)]
struct BearerRealmFacts {
    realm: RealmV1,
    release: dclutch_token_svm::CollateralAdapterReleaseV1,
    mint: dclutch_token_svm::Mint,
}

fn authenticate_bearer_realm(
    program_id: Pubkey,
    realm_account: &ObservedAccount,
    token_program: &ObservedAccount,
    mint_account: &ObservedAccount,
    selected_realm_id: [u8; 32],
) -> Result<BearerRealmFacts, VerticalError> {
    if realm_account.owner != program_id || realm_account.executable {
        return Err(VerticalError::InvalidOwner);
    }
    if !token_program.executable
        || !matches!(token_program.owner, key if key == bpf_loader::ID || key == bpf_loader_upgradeable::ID)
        || mint_account.owner != token_program.key
        || mint_account.executable
    {
        return Err(VerticalError::InvalidOwner);
    }
    let realm = RealmV1::decode(&realm_account.data).map_err(|_| VerticalError::InvalidState)?;
    if realm.to_bytes().as_slice() != realm_account.data.as_slice() {
        return Err(VerticalError::InvalidState);
    }
    let realm_id = hash(&realm_account.data).to_bytes();
    let (expected_realm, _) =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_id], &program_id);
    if realm_account.key != expected_realm
        || realm_id != selected_realm_id
        || realm.token_program() != token_program.key.as_ref()
        || realm.collateral_mint() != mint_account.key.as_ref()
    {
        return Err(VerticalError::ContentMismatch);
    }
    let release = PRODUCTION_ADAPTER_RELEASES
        .into_iter()
        .find(|release| {
            hash(&release.to_bytes()).to_bytes() == *realm.collateral_adapter_release_id()
        })
        .ok_or(VerticalError::ContentMismatch)?;
    if release.token_program() != token_program.key.to_bytes() {
        return Err(VerticalError::ContentMismatch);
    }
    let mint = release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_account.data)
        .map_err(|_| VerticalError::InvalidState)?;
    if matches!(
        realm.mint_authority_policy(),
        MintAuthorityPolicy::RequireAbsent
    ) && !matches!(mint.mint_authority, COption::None)
        || matches!(
            realm.freeze_authority_policy(),
            FreezeAuthorityPolicy::RequireAbsent
        ) && !matches!(mint.freeze_authority, COption::None)
    {
        return Err(VerticalError::InvalidState);
    }
    Ok(BearerRealmFacts {
        realm,
        release,
        mint,
    })
}

fn authenticate_bearer_custody(
    program_id: Pubkey,
    account: &ObservedAccount,
    market: Pubkey,
    generation: u64,
) -> Result<(), VerticalError> {
    let custody = decode_owned(account, program_id, CollateralCustodyV1::decode)?;
    let (expected, _) = Pubkey::find_program_address(
        &[COLLATERAL_CUSTODY_PDA_DOMAIN, market.as_ref()],
        &program_id,
    );
    if account.key != expected
        || custody.to_bytes().as_slice() != account.data.as_slice()
        || custody.market() != market.to_bytes()
        || custody.generation() != generation
    {
        return Err(VerticalError::PdaMismatch);
    }
    Ok(())
}

fn authenticate_bearer_vault(
    program_id: Pubkey,
    account: &ObservedAccount,
    market: Pubkey,
    mint: &ObservedAccount,
    realm: BearerRealmFacts,
) -> Result<dclutch_token_svm::TokenAccount, VerticalError> {
    let (expected, _) =
        Pubkey::find_program_address(&[COLLATERAL_VAULT_PDA_DOMAIN, market.as_ref()], &program_id);
    if account.key != expected
        || account.owner != realm.release.token_program().into()
        || account.executable
    {
        return Err(VerticalError::PdaMismatch);
    }
    realm
        .release
        .profile()
        .check_custody_account(
            realm.release.token_program(),
            &account.data,
            mint.key.to_bytes(),
            market.to_bytes(),
        )
        .map_err(|_| VerticalError::InvalidState)
}

fn canonical_market<const N: usize>(
    program_id: Pubkey,
    account: &ObservedAccount,
) -> Result<CategoricalMarketV1<N>, VerticalError> {
    let market = decode_owned(account, program_id, CategoricalMarketV1::<N>::decode)?;
    let mut canonical =
        vec![0; CategoricalMarketV1::<N>::encoded_len().map_err(|_| VerticalError::InvalidState)?];
    market
        .encode(&mut canonical)
        .map_err(|_| VerticalError::InvalidState)?;
    let identity = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[crate::MARKET_SEED, &identity], &program_id);
    if canonical != account.data || account.key != expected {
        return Err(VerticalError::PdaMismatch);
    }
    Ok(market)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Finality;
    use dclutch_bearer_contract::state::{
        BEARER_CAPABILITY_KIND_ID, BEARER_CHILD_DERIVATION_ID, BEARER_CHILD_SCHEMA_ID,
        BEARER_SEMANTIC_RELEASE_ID,
    };
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
    use dclutch_market_contract::market::CategoricalSettlementSummaryV1;
    use dclutch_product_contract::terminal::ResolutionKind;
    use dclutch_realm_contract::RealmV1Input;
    use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
    use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::SysvarSerialize};
    use solana_sdk_ids::{native_loader, sysvar};

    fn observed(
        observation: Observation,
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
        data: Vec<u8>,
    ) -> ObservedAccount {
        ObservedAccount {
            observation,
            key,
            owner,
            lamports,
            executable,
            data,
        }
    }

    fn token_mint_bytes(supply: u64, decimals: u8) -> Vec<u8> {
        let mut bytes = vec![0; dclutch_token_svm::MINT_BYTES];
        bytes
            .get_mut(36..44)
            .expect("Mint supply field")
            .copy_from_slice(&supply.to_le_bytes());
        *bytes.get_mut(44).expect("Mint decimals field") = decimals;
        *bytes.get_mut(45).expect("Mint initialized field") = 1;
        bytes
    }

    fn token_account_bytes(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
        let mut bytes = vec![0; dclutch_token_svm::ACCOUNT_BYTES];
        bytes
            .get_mut(..32)
            .expect("token Mint field")
            .copy_from_slice(mint.as_ref());
        bytes
            .get_mut(32..64)
            .expect("token owner field")
            .copy_from_slice(owner.as_ref());
        bytes
            .get_mut(64..72)
            .expect("token amount field")
            .copy_from_slice(&amount.to_le_bytes());
        *bytes.get_mut(108).expect("token state field") = 1;
        bytes
    }

    fn native_bearer_fixture(
        hoard: u64,
        supply: [u64; 2],
        balances: [u64; 2],
        vault_amount: u64,
        holder_amount: u64,
    ) -> (Pubkey, BearerNativeValueState) {
        let program_id = Pubkey::new_from_array([90; 32]);
        let observation = Observation {
            slot: 41,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        };
        let holder = Pubkey::new_from_array([91; 32]);
        let mint = Pubkey::new_from_array([92; 32]);
        let token_program = Pubkey::new_from_array(dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID);
        let release = dclutch_token_svm::CollateralAdapterReleaseV1::legacy_exact_transfer();
        let realm = RealmV1::new(RealmV1Input {
            token_program: token_program.to_bytes(),
            collateral_mint: mint.to_bytes(),
            collateral_adapter_release_id: hash(&release.to_bytes()).to_bytes(),
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("canonical fixture Realm");
        let realm_id = ContentId::new(hash(&realm.to_bytes()).to_bytes())
            .expect("nonzero Realm identity");
        let realm_key =
            Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_id.to_bytes()], &program_id).0;
        let market = market_account(MarketAccountInput {
            program_id,
            observation,
            realm_id,
            manifest_id: ContentId::new([93; 32]).expect("fixture manifest identity"),
            phase: Phase::Open,
            child_count: 0,
            hoard,
            supply,
        });
        let position = PositionV1::<2>::new(
            market.key.to_bytes(),
            holder.to_bytes(),
            7,
            balances,
        )
        .expect("canonical native Position");
        let mut position_data =
            vec![0; PositionV1::<2>::encoded_len().expect("supported Position width")];
        position
            .encode(&mut position_data)
            .expect("encode native Position");
        let position_key = Pubkey::find_program_address(
            &[POSITION_PDA_DOMAIN, market.key.as_ref(), holder.as_ref()],
            &program_id,
        )
        .0;
        let custody_key = Pubkey::find_program_address(
            &[COLLATERAL_CUSTODY_PDA_DOMAIN, market.key.as_ref()],
            &program_id,
        )
        .0;
        let custody = CollateralCustodyV1::new(market.key.to_bytes(), 7, [94; 32])
            .expect("canonical collateral custody");
        let vault_key = Pubkey::find_program_address(
            &[COLLATERAL_VAULT_PDA_DOMAIN, market.key.as_ref()],
            &program_id,
        )
        .0;
        let holder_collateral = Pubkey::new_from_array([95; 32]);
        let market_key = market.key;
        (
            program_id,
            BearerNativeValueState {
                market,
                position: observed(
                    observation,
                    position_key,
                    program_id,
                    1,
                    false,
                    position_data,
                ),
                realm: observed(
                    observation,
                    realm_key,
                    program_id,
                    1,
                    false,
                    realm.to_bytes().to_vec(),
                ),
                collateral_custody: observed(
                    observation,
                    custody_key,
                    program_id,
                    1,
                    false,
                    custody.to_bytes().to_vec(),
                ),
                collateral_vault: observed(
                    observation,
                    vault_key,
                    token_program,
                    1,
                    false,
                    token_account_bytes(mint, market_key, vault_amount),
                ),
                holder_collateral: observed(
                    observation,
                    holder_collateral,
                    token_program,
                    1,
                    false,
                    token_account_bytes(mint, holder, holder_amount),
                ),
                holder: observed(
                    observation,
                    holder,
                    system_program::ID,
                    1,
                    false,
                    Vec::new(),
                ),
                collateral_token_program: observed(
                    observation,
                    token_program,
                    bpf_loader_upgradeable::ID,
                    1,
                    true,
                    Vec::new(),
                ),
                collateral_mint: observed(
                    observation,
                    mint,
                    token_program,
                    1,
                    false,
                    token_mint_bytes(1_000, 6),
                ),
            },
        )
    }

    fn resolve_native_fixture(state: &mut BearerNativeValueState) {
        let mut market = CategoricalMarketV1::<2>::decode(&state.market.data)
            .expect("fixture Market decodes");
        let summary = CategoricalSettlementSummaryV1::resolved::<2>(
            dclutch_product_contract::ContentId::new([96; 32])
                .expect("fixture evidence identity"),
            ResolutionKind::Occurrence,
            0,
            1,
        )
        .expect("canonical settlement summary");
        market
            .resolve_with_summary(7, summary)
            .expect("resolve fixture Market");
        state.market.data = encode_market(market);
    }

    fn bearer_mint_bytes(controller: Pubkey, supply: u64) -> Vec<u8> {
        let mut bytes = vec![0; BEARER_MINT_BYTES];
        bytes
            .get_mut(..4)
            .expect("Mint authority option")
            .copy_from_slice(&1u32.to_le_bytes());
        bytes
            .get_mut(4..36)
            .expect("Mint authority")
            .copy_from_slice(controller.as_ref());
        bytes
            .get_mut(36..44)
            .expect("Mint supply")
            .copy_from_slice(&supply.to_le_bytes());
        *bytes.get_mut(45).expect("Mint initialized") = 1;
        *bytes.get_mut(165).expect("Token-2022 account kind") = 1;
        let extensions = [
            ExtensionType::MintCloseAuthority as u16,
            ExtensionType::PermissionedBurn as u16,
        ];
        let mut offset = 166usize;
        for extension in extensions {
            bytes
                .get_mut(offset..offset + 2)
                .expect("extension kind")
                .copy_from_slice(&extension.to_le_bytes());
            bytes
                .get_mut(offset + 2..offset + 4)
                .expect("extension length")
                .copy_from_slice(&32u16.to_le_bytes());
            bytes
                .get_mut(offset + 4..offset + 36)
                .expect("extension authority")
                .copy_from_slice(controller.as_ref());
            offset += 36;
        }
        bytes
    }

    fn rent_account(observation: Observation) -> ObservedAccount {
        let rent = Rent::default();
        let mut data = vec![0u8; Rent::size_of()];
        let mut lamports = 1;
        let mut info = AccountInfo::new(
            &sysvar::rent::ID,
            false,
            false,
            &mut lamports,
            &mut data,
            &sysvar::ID,
            false,
        );
        rent.to_account_info(&mut info).expect("serialize Rent");
        drop(info);
        observed(observation, sysvar::rent::ID, sysvar::ID, 1, false, data)
    }

    struct MarketAccountInput<const N: usize> {
        program_id: Pubkey,
        observation: Observation,
        realm_id: ContentId,
        manifest_id: ContentId,
        phase: Phase,
        child_count: u64,
        hoard: u64,
        supply: [u64; N],
    }

    fn market_account<const N: usize>(input: MarketAccountInput<N>) -> ObservedAccount {
        let content = |byte| ContentId::new([byte; 32]).expect("nonzero fixture content ID");
        let identity = MarketIdentity::new(
            input.realm_id,
            content(22),
            content(23),
            content(24),
            input.manifest_id,
            7,
        );
        let identity_digest = hash(&identity.to_bytes()).to_bytes();
        let key =
            Pubkey::find_program_address(&[crate::MARKET_SEED, &identity_digest], &input.program_id)
                .0;
        let mut root = MarketRoot::founding(identity, [25; 32]).expect("founding Market");
        for expected_prior_count in 0..input.child_count {
            root.register_child(7, expected_prior_count)
                .expect("register fixture child");
        }
        if input.phase != Phase::Founding {
            root.transition_phase(7, input.phase)
                .expect("admitted fixture phase");
        }
        let market = CategoricalMarketV1::<N>::new(
            root,
            input.hoard,
            input.supply,
            CategoricalSettlementSummaryV1::empty(),
        )
        .expect("canonical fixture Market");
        let mut data =
            vec![0; CategoricalMarketV1::<N>::encoded_len().expect("supported fixture width")];
        market.encode(&mut data).expect("encode fixture Market");
        observed(
            input.observation,
            key,
            input.program_id,
            10_000_000,
            false,
            data,
        )
    }

    struct BearerCapabilityFixture {
        state: BearerActivateState,
        manifest_id: CapabilityContentId,
        config_id: CapabilityContentId,
        config: BearerConfigV1,
        state_rent: u64,
        mint_rent: u64,
    }

    fn bearer_capability_fixture() -> (Pubkey, BearerCapabilityFixture) {
        let program_id = Pubkey::new_from_array([90; 32]);
        let observation = Observation {
            slot: 41,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        };
        let rent = Rent::default();
        let state_rent = rent.minimum_balance(
            BearerCapabilityV1::<2>::encoded_len().expect("supported bearer width"),
        );
        let mint_rent = rent
            .minimum_balance(BEARER_MINT_BYTES)
            .checked_mul(2)
            .expect("fixture Mint rent");
        let refund = Pubkey::new_from_array([70; 32]);
        let config =
            BearerConfigV1::new(dclutch_token_svm::TOKEN_2022_PROGRAM_ID, refund.to_bytes())
                .expect("canonical bearer config");
        let config_data = config.to_bytes().to_vec();
        let config_id = CapabilityContentId::new(hash(&config_data).to_bytes())
            .expect("nonzero config identity");
        let amounts = FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(state_rent).expect("state Rent quote"),
            CompartmentFundingV1::native_lamports(mint_rent).expect("Mint Rent quote"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("canonical funding amounts");
        let quote = FundingQuoteV1::new(amounts, None).expect("native-only funding quote");
        let id = |bytes: [u8; 32]| {
            CapabilityContentId::new(bytes).expect("nonzero fixture capability content")
        };
        let entry = CapabilityEntryV1::new(
            id(BEARER_CAPABILITY_KIND_ID),
            id(BEARER_SEMANTIC_RELEASE_ID),
            config_id,
            id([71; 32]),
            id(BEARER_CHILD_SCHEMA_ID),
            id(BEARER_CHILD_DERIVATION_ID),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("canonical bearer entry");
        let mut manifest_data = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest_data)
            .expect("canonical bearer manifest");
        let manifest_id = CapabilityContentId::new(hash(&manifest_data).to_bytes())
            .expect("nonzero manifest identity");
        let realm_id = ContentId::new([72; 32]).expect("fixture Realm identity");
        let market = market_account(MarketAccountInput {
            program_id,
            observation,
            realm_id,
            manifest_id: ContentId::new(manifest_id.to_bytes())
                .expect("Market manifest identity"),
            phase: Phase::Founding,
            child_count: 0,
            hoard: 0,
            supply: [0, 0],
        });
        let manifest = CapabilityManifestV1::decode(&manifest_data).expect("fixture manifest");
        let funding_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
        let custody = FundingCustodyObservationV1::native_only(
            funding_rent + state_rent + mint_rent,
            funding_rent,
        )
        .expect("exact fixture custody");
        let funding = FundingStateV1::new(manifest_id, manifest, 0, custody)
            .expect("canonical funding state");
        let derivation = CapabilityFundingDerivationV1::new(
            market.key.to_bytes(),
            7,
            manifest_id,
            manifest,
            funding,
        )
        .expect("funding derivation");
        let funding_key =
            Pubkey::find_program_address(&derivation.seed_components(), &program_id).0;
        let bearer_state_key = Pubkey::find_program_address(
            &[
                BEARER_CAPABILITY_PDA_DOMAIN,
                market.key.as_ref(),
                &7u64.to_le_bytes(),
            ],
            &program_id,
        )
        .0;
        let claim_mints = (0..2)
            .map(|outcome| {
                let key = canonical_bearer_mint(program_id, market.key, 7, outcome, 2)
                    .expect("canonical fixture Mint");
                observed(observation, key, system_program::ID, 0, false, Vec::new())
            })
            .collect();
        let (rent_credit_key, rent_credit_bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, refund.as_ref()],
            &program_id,
        );
        let rent_credit = RentCreditV1::new(
            RefundAuthority::new(refund.to_bytes()).expect("refund authority"),
            rent_credit_bump,
        );
        let token_program = Pubkey::new_from_array(dclutch_token_svm::TOKEN_2022_PROGRAM_ID);
        (
            program_id,
            BearerCapabilityFixture {
                state: BearerActivateState {
                    market,
                    bearer_state: observed(
                        observation,
                        bearer_state_key,
                        system_program::ID,
                        0,
                        false,
                        Vec::new(),
                    ),
                    capability_manifest: observed(
                        observation,
                        Pubkey::new_from_array([73; 32]),
                        program_id,
                        1,
                        false,
                        manifest_data,
                    ),
                    bearer_config: observed(
                        observation,
                        Pubkey::new_from_array([74; 32]),
                        program_id,
                        1,
                        false,
                        config_data,
                    ),
                    funding_state: observed(
                        observation,
                        funding_key,
                        program_id,
                        funding_rent + state_rent + mint_rent,
                        false,
                        funding.to_bytes().to_vec(),
                    ),
                    rent_credit: observed(
                        observation,
                        rent_credit_key,
                        program_id,
                        1,
                        false,
                        rent_credit.to_bytes().to_vec(),
                    ),
                    creation_payer: observed(
                        observation,
                        Pubkey::new_from_array([75; 32]),
                        system_program::ID,
                        u64::MAX,
                        false,
                        Vec::new(),
                    ),
                    token_2022_program: observed(
                        observation,
                        token_program,
                        bpf_loader_upgradeable::ID,
                        1,
                        true,
                        Vec::new(),
                    ),
                    system_program: observed(
                        observation,
                        system_program::ID,
                        native_loader::ID,
                        1,
                        true,
                        Vec::new(),
                    ),
                    rent_sysvar: rent_account(observation),
                    claim_mints,
                },
                manifest_id,
                config_id,
                config,
                state_rent,
                mint_rent,
            },
        )
    }

    fn activate_fixture_parts(
        fixture: &BearerCapabilityFixture,
    ) -> (CategoricalMarketV1<2>, BearerCapabilityV1<2>) {
        let mut market = CategoricalMarketV1::<2>::decode(&fixture.state.market.data)
            .expect("fixture Market decodes");
        let manifest = CapabilityManifestV1::decode(&fixture.state.capability_manifest.data)
            .expect("fixture manifest decodes");
        let mut funding = FundingStateV1::decode(&fixture.state.funding_state.data)
            .expect("fixture funding decodes");
        let custody = FundingCustodyObservationV1::native_only(
            fixture.state.funding_state.lamports,
            Rent::default().minimum_balance(FUNDING_STATE_BYTES),
        )
        .expect("fixture funding custody");
        let mint_keys = [
            fixture
                .state
                .claim_mints
                .first()
                .expect("outcome zero Mint")
                .key
                .to_bytes(),
            fixture
                .state
                .claim_mints
                .get(1)
                .expect("outcome one Mint")
                .key
                .to_bytes(),
        ];
        let (bearer, _) = activate_bearer(
            fixture.state.market.key.to_bytes(),
            &mut market,
            fixture.manifest_id,
            manifest,
            fixture.config_id,
            fixture.config,
            &mut funding,
            custody,
            fixture.state.market.observation.slot,
            fixture.state_rent,
            fixture.mint_rent,
            0,
            fixture.state.bearer_state.key.to_bytes(),
            mint_keys,
        )
        .expect("fixture canonical activation");
        (market, bearer)
    }

    fn encode_market<const N: usize>(market: CategoricalMarketV1<N>) -> Vec<u8> {
        let mut data =
            vec![0; CategoricalMarketV1::<N>::encoded_len().expect("supported Market width")];
        market.encode(&mut data).expect("encode fixture Market");
        data
    }

    fn encode_bearer<const N: usize>(bearer: BearerCapabilityV1<N>) -> Vec<u8> {
        let mut data =
            vec![0; BearerCapabilityV1::<N>::encoded_len().expect("supported bearer width")];
        bearer.encode(&mut data).expect("encode fixture bearer");
        data
    }

    fn bearer_representation_fixture() -> (Pubkey, BearerRepresentationState) {
        let (program_id, fixture) = bearer_capability_fixture();
        let (mut market, bearer) = activate_fixture_parts(&fixture);
        market
            .transition_phase(7, Phase::Open)
            .expect("open fixture Market");
        market
            .split_complete_set(5)
            .expect("fund fixture complete set");
        let holder = Pubkey::new_from_array([81; 32]);
        let position_key = Pubkey::find_program_address(
            &[
                POSITION_PDA_DOMAIN,
                fixture.state.market.key.as_ref(),
                holder.as_ref(),
            ],
            &program_id,
        )
        .0;
        let position = PositionV1::<2>::new(
            fixture.state.market.key.to_bytes(),
            holder.to_bytes(),
            7,
            [5, 5],
        )
        .expect("canonical holder Position");
        let mut position_data =
            vec![0; PositionV1::<2>::encoded_len().expect("supported Position width")];
        position
            .encode(&mut position_data)
            .expect("encode fixture Position");
        let mint = fixture
            .state
            .claim_mints
            .first()
            .expect("outcome zero Mint")
            .key;
        let claim_account = Pubkey::new_from_array([82; 32]);
        let observation = fixture.state.market.observation;
        (
            program_id,
            BearerRepresentationState {
                market: observed(
                    observation,
                    fixture.state.market.key,
                    program_id,
                    fixture.state.market.lamports,
                    false,
                    encode_market(market),
                ),
                bearer_state: observed(
                    observation,
                    fixture.state.bearer_state.key,
                    program_id,
                    fixture.state_rent,
                    false,
                    encode_bearer(bearer),
                ),
                position: observed(
                    observation,
                    position_key,
                    program_id,
                    1,
                    false,
                    position_data,
                ),
                claim_mint: observed(
                    observation,
                    mint,
                    fixture.state.token_2022_program.key,
                    fixture.mint_rent / 2,
                    false,
                    bearer_mint_bytes(fixture.state.bearer_state.key, 0),
                ),
                claim_account: observed(
                    observation,
                    claim_account,
                    fixture.state.token_2022_program.key,
                    1,
                    false,
                    token_account_bytes(mint, holder, 0),
                ),
                holder: observed(
                    observation,
                    holder,
                    system_program::ID,
                    1,
                    false,
                    Vec::new(),
                ),
                token_2022_program: fixture.state.token_2022_program,
            },
        )
    }

    fn bearer_unwrap_fixture() -> (Pubkey, BearerRepresentationState) {
        let (program_id, mut state) = bearer_representation_fixture();
        let market = CategoricalMarketV1::<2>::decode(&state.market.data)
            .expect("fixture Market decodes");
        let mut bearer = BearerCapabilityV1::<2>::decode(&state.bearer_state.data)
            .expect("fixture bearer decodes");
        let mut position =
            PositionV1::<2>::decode(&state.position.data).expect("fixture Position decodes");
        let mint = parse_bearer_mint(&state.claim_mint, &state.token_2022_program)
            .expect("fixture Mint parses");
        let claim = parse_bearer_claim_account(&state.claim_account, &state.token_2022_program)
            .expect("fixture claim account parses");
        materialize(
            state.market.key.to_bytes(),
            &market,
            &mut bearer,
            &mut position,
            state.holder.key.to_bytes(),
            0,
            3,
            state.bearer_state.key.to_bytes(),
            state.claim_mint.key.to_bytes(),
            mint,
            claim,
        )
        .expect("materialize canonical unwrap fixture");
        state.bearer_state.data = encode_bearer(bearer);
        let mut position_data =
            vec![0; PositionV1::<2>::encoded_len().expect("supported Position width")];
        position
            .encode(&mut position_data)
            .expect("encode materialized Position");
        state.position.data = position_data;
        state
            .claim_mint
            .data
            .get_mut(36..44)
            .expect("Mint supply")
            .copy_from_slice(&3u64.to_le_bytes());
        state
            .claim_account
            .data
            .get_mut(64..72)
            .expect("claim amount")
            .copy_from_slice(&3u64.to_le_bytes());
        (program_id, state)
    }

    fn bearer_retire_fixture() -> (Pubkey, BearerRetireState, u64) {
        let (program_id, fixture) = bearer_capability_fixture();
        let (mut market, bearer) = activate_fixture_parts(&fixture);
        market
            .transition_phase(7, Phase::Retiring)
            .expect("retire fixture Market");
        let observation = fixture.state.market.observation;
        let claim_mints: Vec<_> = fixture
            .state
            .claim_mints
            .iter()
            .map(|mint| {
                observed(
                    observation,
                    mint.key,
                    fixture.state.token_2022_program.key,
                    fixture.mint_rent / 2,
                    false,
                    bearer_mint_bytes(fixture.state.bearer_state.key, 0),
                )
            })
            .collect();
        let expected_credit = fixture.state_rent + fixture.mint_rent;
        (
            program_id,
            BearerRetireState {
                market: observed(
                    observation,
                    fixture.state.market.key,
                    program_id,
                    fixture.state.market.lamports,
                    false,
                    encode_market(market),
                ),
                bearer_state: observed(
                    observation,
                    fixture.state.bearer_state.key,
                    program_id,
                    fixture.state_rent,
                    false,
                    encode_bearer(bearer),
                ),
                capability_manifest: fixture.state.capability_manifest,
                bearer_config: fixture.state.bearer_config,
                rent_credit: fixture.state.rent_credit,
                token_2022_program: fixture.state.token_2022_program,
                system_program: fixture.state.system_program,
                rent_sysvar: fixture.state.rent_sysvar,
                claim_mints,
            },
            expected_credit,
        )
    }

    #[test]
    fn native_bearer_value_routes_are_exact_and_refuse_hostile_state() {
        let (program_id, deposit) = native_bearer_fixture(0, [0, 0], [0, 0], 0, 10);
        let report = build_bearer_native_value_v1(
            program_id,
            &deposit,
            BearerNativeValueActionV1::DepositCompleteSet { quantity: 3 },
        )
        .expect("canonical deposit is callable");
        assert_eq!(report.required_signer, deposit.holder.key);
        assert_eq!(report.instruction.accounts.len(), 9);
        assert_eq!(report.movement.collateral_atoms, 3);
        assert_eq!(report.movement.collateral_source, deposit.holder_collateral.key);
        assert_eq!(report.movement.collateral_destination, deposit.collateral_vault.key);
        assert_eq!(report.movement.position_credit_atoms, 3);

        let (program_id, withdraw) = native_bearer_fixture(5, [5, 5], [5, 5], 5, 0);
        let report = build_bearer_native_value_v1(
            program_id,
            &withdraw,
            BearerNativeValueActionV1::WithdrawCompleteSet { quantity: 3 },
        )
        .expect("canonical withdrawal is callable");
        assert_eq!(report.movement.collateral_source, withdraw.collateral_vault.key);
        assert_eq!(report.movement.collateral_destination, withdraw.holder_collateral.key);
        assert_eq!(report.movement.position_debit_atoms, 3);

        let (program_id, mut redeem) = native_bearer_fixture(5, [5, 5], [5, 5], 5, 0);
        resolve_native_fixture(&mut redeem);
        let report = build_bearer_native_value_v1(
            program_id,
            &redeem,
            BearerNativeValueActionV1::RedeemOutcome {
                outcome: 0,
                quantity: 3,
            },
        )
        .expect("canonical redemption is callable");
        assert_eq!(report.movement.selected_outcome, Some(0));
        assert_eq!(report.movement.collateral_atoms, 3);

        let mut mixed = deposit.clone();
        mixed.position.observation.slot += 1;
        assert_eq!(
            build_bearer_native_value_v1(
                program_id,
                &mixed,
                BearerNativeValueActionV1::DepositCompleteSet { quantity: 1 },
            ),
            Err(VerticalError::ObservationMismatch)
        );

        let mut wrong_position = deposit.clone();
        wrong_position.position.key = Pubkey::new_unique();
        assert_eq!(
            build_bearer_native_value_v1(
                program_id,
                &wrong_position,
                BearerNativeValueActionV1::DepositCompleteSet { quantity: 1 },
            ),
            Err(VerticalError::PdaMismatch)
        );

        let mut wrong_collateral_owner = deposit.clone();
        wrong_collateral_owner
            .holder_collateral
            .data
            .get_mut(32..64)
            .expect("collateral owner")
            .copy_from_slice(Pubkey::new_unique().as_ref());
        assert_eq!(
            build_bearer_native_value_v1(
                program_id,
                &wrong_collateral_owner,
                BearerNativeValueActionV1::DepositCompleteSet { quantity: 1 },
            ),
            Err(VerticalError::InvalidState)
        );

        assert_eq!(
            build_bearer_native_value_v1(
                program_id,
                &deposit,
                BearerNativeValueActionV1::DepositCompleteSet { quantity: 0 },
            ),
            Err(VerticalError::InvalidPhase)
        );
    }

    #[test]
    fn bearer_activation_is_exact_and_refuses_hostile_funding_state() {
        let (program_id, fixture) = bearer_capability_fixture();
        let mut frame = vec![
            bearer_meta(&fixture.state.market, false, true),
            bearer_meta(&fixture.state.bearer_state, false, true),
            bearer_meta(&fixture.state.capability_manifest, false, false),
            bearer_meta(&fixture.state.bearer_config, false, false),
            bearer_meta(&fixture.state.funding_state, false, true),
            bearer_meta(&fixture.state.rent_credit, false, false),
            bearer_meta(&fixture.state.creation_payer, true, true),
            bearer_meta(&fixture.state.token_2022_program, false, false),
            bearer_meta(&fixture.state.system_program, false, false),
            bearer_meta(&fixture.state.rent_sysvar, false, false),
        ];
        frame.extend(
            fixture
                .state
                .claim_mints
                .iter()
                .map(|account| bearer_meta(account, false, true)),
        );
        assert_eq!(
            validate_bearer_frame::<2>(BearerActionTagV1::Activate, &frame),
            Ok(())
        );
        let report = build_bearer_activate_v1(program_id, &fixture.state)
            .expect("canonical finalized activation is callable");
        assert_eq!(report.observation, fixture.state.market.observation);
        assert_eq!(report.required_signer, fixture.state.creation_payer.key);
        assert_eq!(report.debit.state_rent_lamports, fixture.state_rent);
        assert_eq!(report.debit.mint_rent_lamports, fixture.mint_rent);
        assert_eq!(
            report.debit.payer_reimbursement_lamports,
            fixture.state_rent + fixture.mint_rent
        );

        let mut mixed = fixture.state.clone();
        mixed
            .claim_mints
            .get_mut(1)
            .expect("second Mint")
            .observation
            .slot += 1;
        assert_eq!(
            build_bearer_activate_v1(program_id, &mixed),
            Err(VerticalError::ObservationMismatch)
        );

        let mut occupied = fixture.state.clone();
        occupied.bearer_state.lamports = 1;
        assert_eq!(
            build_bearer_activate_v1(program_id, &occupied),
            Err(VerticalError::PdaMismatch)
        );

        let mut wrong_funding = fixture.state.clone();
        wrong_funding.funding_state.key = Pubkey::new_unique();
        assert_eq!(
            build_bearer_activate_v1(program_id, &wrong_funding),
            Err(VerticalError::PdaMismatch)
        );

        let mut wrong_payer = fixture.state.clone();
        wrong_payer.creation_payer.owner = program_id;
        assert_eq!(
            build_bearer_activate_v1(program_id, &wrong_payer),
            Err(VerticalError::InvalidAuthority)
        );
    }

    #[test]
    fn bearer_wrap_is_exact_and_hostile_to_parallel_claim_truth() {
        let (program_id, state) = bearer_representation_fixture();
        let report = build_bearer_representation_v1(
            program_id,
            &state,
            BearerRepresentationActionV1::Wrap {
                outcome: 0,
                quantity: 3,
            },
        )
        .expect("canonical wrap is callable");
        assert_eq!(report.required_signer, state.holder.key);
        assert_eq!(report.instruction.accounts.len(), 7);
        assert_eq!(report.movement.outcome, 0);
        assert_eq!(report.movement.native_debit_atoms, 3);
        assert_eq!(report.movement.bearer_credit_atoms, 3);
        assert_eq!(report.movement.mint_supply_before, 0);
        assert_eq!(report.movement.mint_supply_after, 3);

        let (program_id, unwrap) = bearer_unwrap_fixture();
        let report = build_bearer_representation_v1(
            program_id,
            &unwrap,
            BearerRepresentationActionV1::Unwrap {
                outcome: 0,
                quantity: 2,
            },
        )
        .expect("canonical unwrap is callable");
        assert_eq!(report.movement.native_credit_atoms, 2);
        assert_eq!(report.movement.bearer_debit_atoms, 2);
        assert_eq!(report.movement.mint_supply_before, 3);
        assert_eq!(report.movement.mint_supply_after, 1);

        let mut wrong_claim_owner = state.clone();
        wrong_claim_owner
            .claim_account
            .data
            .get_mut(32..64)
            .expect("claim authority")
            .copy_from_slice(Pubkey::new_unique().as_ref());
        assert_eq!(
            build_bearer_representation_v1(
                program_id,
                &wrong_claim_owner,
                BearerRepresentationActionV1::Wrap {
                    outcome: 0,
                    quantity: 3,
                },
            ),
            Err(VerticalError::InvalidPhase)
        );

        let mut hostile_extension = state.clone();
        *hostile_extension
            .claim_mint
            .data
            .get_mut(166)
            .expect("first extension tag") = 0xff;
        assert_eq!(
            build_bearer_representation_v1(
                program_id,
                &hostile_extension,
                BearerRepresentationActionV1::Wrap {
                    outcome: 0,
                    quantity: 3,
                },
            ),
            Err(VerticalError::InvalidState)
        );

        assert_eq!(
            build_bearer_representation_v1(
                program_id,
                &state,
                BearerRepresentationActionV1::Wrap {
                    outcome: 2,
                    quantity: 1,
                },
            ),
            Err(VerticalError::InvalidState)
        );
    }

    #[test]
    fn bearer_retirement_is_zero_supply_and_rent_attributed() {
        let (program_id, state, expected_credit) = bearer_retire_fixture();
        let mut frame = vec![
            bearer_meta(&state.market, false, true),
            bearer_meta(&state.bearer_state, false, true),
            bearer_meta(&state.capability_manifest, false, false),
            bearer_meta(&state.bearer_config, false, false),
            bearer_meta(&state.rent_credit, false, true),
            bearer_meta(&state.token_2022_program, false, false),
            bearer_meta(&state.system_program, false, false),
            bearer_meta(&state.rent_sysvar, false, false),
        ];
        frame.extend(
            state
                .claim_mints
                .iter()
                .map(|account| bearer_meta(account, false, true)),
        );
        assert_eq!(
            validate_bearer_frame::<2>(BearerActionTagV1::Retire, &frame),
            Ok(())
        );
        let report = build_bearer_retire_v1(program_id, &state)
            .expect("canonical zero-supply retirement is callable");
        assert_eq!(report.instruction.accounts.len(), 10);
        assert_eq!(report.rent_credit_lamports, expected_credit);
        assert_eq!(report.market_child_count_before, 1);
        assert_eq!(report.market_child_count_after, 0);

        let mut nonzero_supply = state.clone();
        nonzero_supply
            .claim_mints
            .first_mut()
            .expect("first Mint")
            .data
            .get_mut(36..44)
            .expect("Mint supply")
            .copy_from_slice(&1u64.to_le_bytes());
        assert_eq!(
            build_bearer_retire_v1(program_id, &nonzero_supply),
            Err(VerticalError::InvalidPhase)
        );

        let mut wrong_mint = state.clone();
        wrong_mint.claim_mints.get_mut(1).expect("second Mint").key = Pubkey::new_unique();
        assert_eq!(
            build_bearer_retire_v1(program_id, &wrong_mint),
            Err(VerticalError::PdaMismatch)
        );
    }
}
