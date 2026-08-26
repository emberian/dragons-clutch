//! Exact Token-2022 effects and final lifecycle-Rent closure for Fractional.
//!
//! This adapter does not own shard supply, holder balances, remainders, or
//! retirement authority. It authenticates Token-owned state against the
//! kernel action, emits the reviewed Token-2022 instructions, and delegates
//! final RentCredit closure to the canonical lifecycle V2 contract.

use dclutch_fractional_claim_contract::FractionalActionV1;
use dclutch_market_core_codec::RetirementReceiptV1;
use dclutch_rent_contract::lifecycle_v2::{
    CloseLifecycleRentCreditV2, LifecycleAccountIdV2, LifecycleClosePlanV2,
    LifecycleRentCloseReceiptV2, LifecycleRentCreditV2,
};
use dclutch_resolution_core_v3_operator::ObservedAccount;
use dclutch_token_svm::{TOKEN_2022_PROGRAM_ID, Token2022BehaviorProfileV2, TokenAccount};
use solana_hash::Hash;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use spl_token_2022_interface::{
    extension::permissioned_burn::instruction as permissioned_burn_instruction,
    instruction as token_instruction,
};

use crate::{
    Error, FractionalActionObservationV1, FractionalActionPlanV1, FractionalIntentV1,
    FractionalPreparedChainArtifactsV1, FractionalUnsignedV0PlanV1, Result,
    build_fractional_unsigned_v0_from_chain_v1,
};

/// One chain-observed Token account or Mint with its exact owning program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalTokenAccountSnapshotV1<'a> {
    /// Exact account key.
    pub key: Pubkey,
    /// Current SVM account owner; must be the selected Token-2022 program.
    pub program_owner: Pubkey,
    /// Exact current account data.
    pub data: &'a [u8],
}

/// Selected Mint plus action-dependent source and destination accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalTokenActionSnapshotV1<'a> {
    /// Terms-selected shard Mint; absent only for terminalize.
    pub mint: Option<FractionalTokenAccountSnapshotV1<'a>>,
    /// Request-selected source holder account when the action debits shards.
    pub source: Option<FractionalTokenAccountSnapshotV1<'a>>,
    /// Request-selected destination holder account for wrap or transfer.
    pub destination: Option<FractionalTokenAccountSnapshotV1<'a>>,
}

/// One terms-ordered shard Mint observed for zero-supply retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalMintSnapshotV1<'a> {
    /// Product outcome coordinate.
    pub outcome: u32,
    /// Exact Mint account.
    pub mint: FractionalTokenAccountSnapshotV1<'a>,
}

/// Explicit execution profile for the sole denominator boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalDenominatorExecutionV1 {
    /// This action does not divide shard atoms by the denominator.
    NotApplicable,
    /// The input was an exact denominator multiple; no change path is needed.
    ExactWhole {
        /// Exact terms-owned shard atoms per native claim.
        denominator: u64,
    },
    /// Only the whole multiple is burned; this exact same-Mint balance remains.
    WholeWithSameMintChange {
        /// Exact terms-owned shard atoms per native claim.
        denominator: u64,
        /// Explicit raw same-Mint Token atoms retained by the source account.
        change_shards: u64,
    },
}

/// Exact Token-2022 effect selected by one kernel action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FractionalTokenEffectV1 {
    /// Terminalization changes no Token-owned state.
    None,
    /// Mint exact raw shard atoms to the request-selected destination.
    Mint(Instruction),
    /// Transfer exact raw same-Mint shard atoms between holder accounts.
    Transfer(Instruction),
    /// Burn exact raw shard atoms through the selected permissioned-burn profile.
    Burn(Instruction),
}

/// Authenticated Token pre/post facts and the exact instruction to execute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalTokenEffectPlanV1 {
    effect: FractionalTokenEffectV1,
    denominator_execution: FractionalDenominatorExecutionV1,
    pre_supply: u64,
    post_supply: u64,
    pre_source: u64,
    post_source: u64,
    pre_destination: u64,
    post_destination: u64,
    display_decimals: u8,
}

impl FractionalTokenEffectPlanV1 {
    /// Exact Token-2022 instruction, or no instruction for terminalization.
    pub const fn effect(&self) -> &FractionalTokenEffectV1 {
        &self.effect
    }

    /// Explicit exact-multiple or same-Mint-remainder path.
    pub const fn denominator_execution(&self) -> FractionalDenominatorExecutionV1 {
        self.denominator_execution
    }

    /// Exact raw Mint supply before execution.
    pub const fn pre_supply(&self) -> u64 {
        self.pre_supply
    }

    /// Required raw Mint supply after execution.
    pub const fn post_supply(&self) -> u64 {
        self.post_supply
    }

    /// Exact raw source balance before execution, or zero when inactive.
    pub const fn pre_source(&self) -> u64 {
        self.pre_source
    }

    /// Required raw source balance after execution, or zero when inactive.
    pub const fn post_source(&self) -> u64 {
        self.post_source
    }

    /// Exact raw destination balance before execution, or zero when inactive.
    pub const fn pre_destination(&self) -> u64 {
        self.pre_destination
    }

    /// Required raw destination balance after execution, or zero when inactive.
    pub const fn post_destination(&self) -> u64 {
        self.post_destination
    }

    /// Token display decimals copied into checked instructions; never arithmetic.
    pub const fn display_decimals(&self) -> u8 {
        self.display_decimals
    }
}

/// Ordered zero-supply Mint closures into the root-bound lifecycle RentCredit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalRetirementTokenPlanV1 {
    instructions: Vec<Instruction>,
    market: [u8; 32],
    release_set: [u8; 32],
    rent_credit: Pubkey,
    current_core_program: Pubkey,
    post_revision: u64,
}

impl FractionalRetirementTokenPlanV1 {
    /// One exact CloseAccount instruction per terms-ordered, zero-supply Mint.
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    /// Logical Market whose producer subtree is retiring.
    pub const fn market(&self) -> [u8; 32] {
        self.market
    }

    /// Immutable release set.
    pub const fn release_set(&self) -> [u8; 32] {
        self.release_set
    }

    /// Root-bound lifecycle RentCredit receiving every Mint's lamports.
    pub const fn rent_credit(&self) -> Pubkey {
        self.rent_credit
    }

    /// Fractional root revision required after its retirement transition.
    pub const fn post_revision(&self) -> u64 {
        self.post_revision
    }
}

/// Canonical lifecycle-V2 request, plan, and immediate receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalLifecycleRentClosePlanV2 {
    /// Exact request carrying the hostile-decoded Core retirement receipt.
    pub request: CloseLifecycleRentCreditV2,
    /// Canonical full-balance lifecycle credit close plan.
    pub plan: LifecycleClosePlanV2,
    /// Canonical immediate Rent close receipt.
    pub receipt: LifecycleRentCloseReceiptV2,
}

/// Action-specific Token observations paired with one wallet packet build.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalPhysicalTokenObservationV1<'a> {
    /// Selected Mint and holder accounts for wrap, transfer, unwrap, redeem,
    /// losing burn, or terminalize.
    Action(&'a FractionalTokenActionSnapshotV1<'a>),
    /// Every terms-ordered shard Mint for final zero-supply retirement.
    Retirement(&'a [FractionalMintSnapshotV1<'a>]),
}

/// Exact Token child effects paired with the unchanged Fractional request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FractionalPhysicalTokenEffectsV1 {
    /// At most one ordinary Token effect; terminalize carries [`FractionalTokenEffectV1::None`].
    Action(FractionalTokenEffectPlanV1),
    /// Runtime-width zero-supply Mint closure list.
    Retirement(FractionalRetirementTokenPlanV1),
}

/// Chain-derived unsigned wallet packet plus its exact Token child postcondition.
///
/// The wallet packet remains the existing Fractional request. The Token plan
/// is adapter evidence for the child effects that Trading must independently
/// rederive and verify; it is not serialized into a second request format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalPhysicalUnsignedV0PlanV1 {
    /// Existing canonical unsigned Fractional request message.
    pub unsigned: FractionalUnsignedV0PlanV1,
    /// Exact Token effect and post-state expected from that request.
    pub token: FractionalPhysicalTokenEffectsV1,
}

/// Build the existing unsigned Fractional request from one finalized snapshot
/// and pair it with exact authenticated Token-2022 child effects.
#[allow(clippy::too_many_arguments)]
pub fn build_fractional_physical_unsigned_v0_from_chain_v1(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    intent: FractionalIntentV1,
    observed: FractionalActionObservationV1<'_>,
    token_observed: FractionalPhysicalTokenObservationV1<'_>,
    payer: Pubkey,
    recent_blockhash: Hash,
    accounts: &[AccountMeta],
    lookup_tables: &[ObservedAccount],
) -> Result<FractionalPhysicalUnsignedV0PlanV1> {
    let unsigned = build_fractional_unsigned_v0_from_chain_v1(
        prepared,
        intent,
        observed,
        payer,
        recent_blockhash,
        accounts,
        lookup_tables,
    )?;
    let token = match token_observed {
        FractionalPhysicalTokenObservationV1::Action(snapshot)
            if intent.action != FractionalActionV1::ZeroSupplyRetire =>
        {
            FractionalPhysicalTokenEffectsV1::Action(plan_fractional_token_effect_v1(
                prepared,
                &unsigned.action,
                observed,
                *snapshot,
            )?)
        }
        FractionalPhysicalTokenObservationV1::Retirement(mints)
            if intent.action == FractionalActionV1::ZeroSupplyRetire =>
        {
            FractionalPhysicalTokenEffectsV1::Retirement(
                plan_fractional_retirement_token_effects_v1(
                    prepared,
                    &unsigned.action,
                    observed,
                    mints,
                )?,
            )
        }
        FractionalPhysicalTokenObservationV1::Action(_)
        | FractionalPhysicalTokenObservationV1::Retirement(_) => return Err(Error::Token),
    };
    Ok(FractionalPhysicalUnsignedV0PlanV1 { unsigned, token })
}

/// Authenticate Token-owned state and derive one exact Token-2022 effect.
///
/// The wrapper root is the only Mint/close/permissioned-burn controller. A
/// holder remains the transfer signer and the second permissioned-burn signer.
/// The exact-multiple path is observable, while nonzero change remains in the
/// same source account and is never reminted or shadow-accounted.
pub fn plan_fractional_token_effect_v1(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    action: &FractionalActionPlanV1,
    observed: FractionalActionObservationV1<'_>,
    token: FractionalTokenActionSnapshotV1<'_>,
) -> Result<FractionalTokenEffectPlanV1> {
    require_common(prepared, action, observed)?;
    if action.request.action() == FractionalActionV1::ZeroSupplyRetire {
        return Err(Error::Token);
    }
    if action.request.action() == FractionalActionV1::Terminalize {
        if token.mint.is_some() || token.source.is_some() || token.destination.is_some() {
            return Err(Error::Token);
        }
        return Ok(FractionalTokenEffectPlanV1 {
            effect: FractionalTokenEffectV1::None,
            denominator_execution: FractionalDenominatorExecutionV1::NotApplicable,
            pre_supply: 0,
            post_supply: 0,
            pre_source: 0,
            post_source: 0,
            pre_destination: 0,
            post_destination: 0,
            display_decimals: 0,
        });
    }

    let input = action.request.input();
    let outcome = input.outcome;
    let reserve = *observed
        .reserves
        .get(usize::try_from(outcome).map_err(|_| Error::Token)?)
        .ok_or(Error::Token)?;
    let mint = token.mint.ok_or(Error::Token)?;
    require_token_owner(prepared, mint)?;
    if mint.key.to_bytes() != action.shard_mint
        || prepared
            .terms()
            .shard_mint(outcome)
            .map_err(|_| Error::Token)?
            != action.shard_mint
    {
        return Err(Error::Token);
    }
    let controller = prepared.root_key();
    let mint_facts = Token2022BehaviorProfileV2::check_mint(
        prepared.checked_release().token_program().to_bytes(),
        mint.key.to_bytes(),
        mint.data,
        controller.to_bytes(),
        reserve.shard_supply,
    )
    .map_err(|_| Error::Token)?;
    let decimals = mint_facts.display_decimals();
    let denominator_execution = denominator_execution(prepared, action)?;

    let (effect, pre_source, pre_destination) = match action.request.action() {
        FractionalActionV1::Wrap => {
            if token.source.is_some() || input.source_token_account != [0; 32] {
                return Err(Error::Token);
            }
            let destination = checked_holder(
                prepared,
                token.destination,
                input.destination_token_account,
                action.shard_mint,
                input.owner,
                observed.destination_shards,
                true,
            )?;
            let instruction = token_instruction::mint_to_checked(
                &prepared.checked_release().token_program(),
                &mint.key,
                &destination.key,
                &controller,
                &[],
                action.consumed_shards,
                decimals,
            )
            .map_err(|_| Error::Token)?;
            (
                FractionalTokenEffectV1::Mint(instruction),
                0,
                observed.destination_shards,
            )
        }
        FractionalActionV1::Transfer => {
            let source = checked_holder(
                prepared,
                token.source,
                input.source_token_account,
                action.shard_mint,
                input.owner,
                observed.source_shards,
                true,
            )?;
            let destination = checked_holder(
                prepared,
                token.destination,
                input.destination_token_account,
                action.shard_mint,
                [0; 32],
                observed.destination_shards,
                false,
            )?;
            if source.key == destination.key {
                return Err(Error::Token);
            }
            let instruction = token_instruction::transfer_checked(
                &prepared.checked_release().token_program(),
                &source.key,
                &mint.key,
                &destination.key,
                &Pubkey::new_from_array(input.owner),
                &[],
                action.consumed_shards,
                decimals,
            )
            .map_err(|_| Error::Token)?;
            (
                FractionalTokenEffectV1::Transfer(instruction),
                observed.source_shards,
                observed.destination_shards,
            )
        }
        FractionalActionV1::WholeUnwrap
        | FractionalActionV1::WinningRedeem
        | FractionalActionV1::LosingZeroBurn => {
            if token.destination.is_some() || input.destination_token_account != [0; 32] {
                return Err(Error::Token);
            }
            let source = checked_holder(
                prepared,
                token.source,
                input.source_token_account,
                action.shard_mint,
                input.owner,
                observed.source_shards,
                true,
            )?;
            let instruction = permissioned_burn_instruction::burn_checked(
                &prepared.checked_release().token_program(),
                &source.key,
                &mint.key,
                &controller,
                &Pubkey::new_from_array(input.owner),
                &[],
                action.consumed_shards,
                decimals,
            )
            .map_err(|_| Error::Token)?;
            (
                FractionalTokenEffectV1::Burn(instruction),
                observed.source_shards,
                0,
            )
        }
        FractionalActionV1::Terminalize | FractionalActionV1::ZeroSupplyRetire => {
            return Err(Error::Token);
        }
    };

    let post_supply = match action.request.action() {
        FractionalActionV1::Wrap => reserve
            .shard_supply
            .checked_add(action.consumed_shards)
            .ok_or(Error::Token)?,
        FractionalActionV1::Transfer => reserve.shard_supply,
        FractionalActionV1::WholeUnwrap
        | FractionalActionV1::WinningRedeem
        | FractionalActionV1::LosingZeroBurn => reserve
            .shard_supply
            .checked_sub(action.consumed_shards)
            .ok_or(Error::Token)?,
        FractionalActionV1::Terminalize | FractionalActionV1::ZeroSupplyRetire => {
            return Err(Error::Token);
        }
    };
    if action
        .post_reserve
        .is_some_and(|post| post.shard_supply != post_supply)
        || (action.request.action() == FractionalActionV1::Transfer
            && action.post_reserve.is_some())
    {
        return Err(Error::Token);
    }

    Ok(FractionalTokenEffectPlanV1 {
        effect,
        denominator_execution,
        pre_supply: reserve.shard_supply,
        post_supply,
        pre_source,
        post_source: action.post_source_shards,
        pre_destination,
        post_destination: action.post_destination_shards,
        display_decimals: decimals,
    })
}

/// Authenticate every terms-ordered Mint at zero supply and emit exact Mint
/// CloseAccount instructions into the root-bound lifecycle RentCredit.
pub fn plan_fractional_retirement_token_effects_v1(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    action: &FractionalActionPlanV1,
    observed: FractionalActionObservationV1<'_>,
    mints: &[FractionalMintSnapshotV1<'_>],
) -> Result<FractionalRetirementTokenPlanV1> {
    require_common(prepared, action, observed)?;
    if action.request.action() != FractionalActionV1::ZeroSupplyRetire
        || mints.len() != observed.reserves.len()
        || mints.len() != action.retirement_native_burns.len()
    {
        return Err(Error::Token);
    }
    let controller = prepared.root_key();
    let rent_credit = Pubkey::new_from_array(prepared.root().input().rent_beneficiary);
    let mut instructions = Vec::with_capacity(mints.len());
    for (expected, mint) in mints.iter().enumerate() {
        let expected_outcome = u32::try_from(expected).map_err(|_| Error::Token)?;
        if mint.outcome != expected_outcome
            || observed
                .reserves
                .get(expected)
                .is_none_or(|reserve| reserve.shard_supply != 0)
        {
            return Err(Error::Token);
        }
        require_token_owner(prepared, mint.mint)?;
        let expected_mint = prepared
            .terms()
            .shard_mint(expected_outcome)
            .map_err(|_| Error::Token)?;
        if mint.mint.key.to_bytes() != expected_mint {
            return Err(Error::Token);
        }
        Token2022BehaviorProfileV2::check_mint(
            prepared.checked_release().token_program().to_bytes(),
            expected_mint,
            mint.mint.data,
            controller.to_bytes(),
            0,
        )
        .map_err(|_| Error::Token)?;
        instructions.push(
            token_instruction::close_account(
                &prepared.checked_release().token_program(),
                &mint.mint.key,
                &rent_credit,
                &controller,
                &[],
            )
            .map_err(|_| Error::Token)?,
        );
    }
    Ok(FractionalRetirementTokenPlanV1 {
        instructions,
        market: prepared.request_context().market,
        release_set: prepared.request_context().release_set,
        rent_credit,
        current_core_program: prepared.checked_release().core_program(),
        post_revision: action.post_revision,
    })
}

/// Consume a canonical Core producer-subtree retirement receipt and derive the
/// sole lifecycle-RentV2 full-balance closure. The Core receipt remains the
/// authority for the complete subtree digest; this helper adds no Fractional
/// closure receipt or alternate digest.
#[allow(clippy::too_many_arguments)]
pub fn plan_fractional_lifecycle_rent_close_v2(
    retirement: &FractionalRetirementTokenPlanV1,
    credit_key: Pubkey,
    credit_bytes: &[u8],
    credit_lamports: u64,
    wallet_lamports: u64,
    core_receipt_bytes: &[u8],
    current_core_authenticated: bool,
) -> Result<FractionalLifecycleRentClosePlanV2> {
    if credit_key != retirement.rent_credit || retirement.instructions.is_empty() {
        return Err(Error::Rent);
    }
    let credit = LifecycleRentCreditV2::decode(credit_bytes).map_err(|_| Error::Rent)?;
    if credit.market().to_bytes() != retirement.market
        || credit.release_set().to_bytes() != retirement.release_set
    {
        return Err(Error::Rent);
    }
    let core_receipt = RetirementReceiptV1::decode(core_receipt_bytes).map_err(|_| Error::Rent)?;
    let request = CloseLifecycleRentCreditV2::new(core_receipt);
    let credit_id = LifecycleAccountIdV2::new(credit_key.to_bytes()).map_err(|_| Error::Rent)?;
    let core_id = LifecycleAccountIdV2::new(retirement.current_core_program.to_bytes())
        .map_err(|_| Error::Rent)?;
    let plan = LifecycleClosePlanV2::new(
        credit,
        credit_id,
        core_id,
        current_core_authenticated,
        credit_lamports,
        wallet_lamports,
        request,
    )
    .map_err(|_| Error::Rent)?;
    let receipt = plan.receipt(credit, credit_id).map_err(|_| Error::Rent)?;
    Ok(FractionalLifecycleRentClosePlanV2 {
        request,
        plan,
        receipt,
    })
}

fn require_common(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    action: &FractionalActionPlanV1,
    observed: FractionalActionObservationV1<'_>,
) -> Result<()> {
    let input = action.request.input();
    if observed.observation != prepared.observation()
        || observed.revision != prepared.root().input().revision
        || input.release_set != prepared.request_context().release_set
        || input.market != prepared.request_context().market
        || input.product_record != prepared.request_context().product_record
        || input.result_domain != prepared.request_context().result_domain
        || input.terms != prepared.terms().terms_id()
        || input.token_behavior != prepared.terms().token_behavior_selection_id()
        || prepared.checked_release().token_program().to_bytes() != TOKEN_2022_PROGRAM_ID
    {
        return Err(Error::Token);
    }
    Ok(())
}

fn require_token_owner(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    account: FractionalTokenAccountSnapshotV1<'_>,
) -> Result<()> {
    if account.program_owner != prepared.checked_release().token_program() {
        return Err(Error::Token);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn checked_holder<'a>(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    account: Option<FractionalTokenAccountSnapshotV1<'a>>,
    expected_key: [u8; 32],
    expected_mint: [u8; 32],
    expected_owner: [u8; 32],
    expected_amount: u64,
    owner_is_authority: bool,
) -> Result<FractionalTokenAccountSnapshotV1<'a>> {
    let account = account.ok_or(Error::Token)?;
    require_token_owner(prepared, account)?;
    if expected_key == [0; 32] || account.key.to_bytes() != expected_key {
        return Err(Error::Token);
    }
    let parsed = TokenAccount::parse(account.data).map_err(|_| Error::Token)?;
    let owner = if owner_is_authority {
        expected_owner
    } else {
        if parsed.owner == [0; 32] {
            return Err(Error::Token);
        }
        parsed.owner
    };
    let facts = Token2022BehaviorProfileV2::check_account(
        prepared.checked_release().token_program().to_bytes(),
        account.data,
        expected_mint,
        owner,
        expected_amount,
    )
    .map_err(|_| Error::Token)?;
    if facts.base_amount() != expected_amount {
        return Err(Error::Token);
    }
    Ok(account)
}

fn denominator_execution(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    action: &FractionalActionPlanV1,
) -> Result<FractionalDenominatorExecutionV1> {
    if !matches!(
        action.request.action(),
        FractionalActionV1::WholeUnwrap | FractionalActionV1::WinningRedeem
    ) {
        return Ok(FractionalDenominatorExecutionV1::NotApplicable);
    }
    let denominator = prepared.terms().denominator();
    if action.consumed_shards == 0
        || !action.consumed_shards.is_multiple_of(denominator)
        || action
            .consumed_shards
            .checked_div(denominator)
            .is_none_or(|whole| whole != action.native_claims)
        || action
            .consumed_shards
            .checked_add(action.change_shards)
            .is_none_or(|input| input != action.request.input().quantity)
    {
        return Err(Error::Token);
    }
    if action.change_shards == 0 {
        Ok(FractionalDenominatorExecutionV1::ExactWhole { denominator })
    } else if action.change_shards < denominator {
        Ok(FractionalDenominatorExecutionV1::WholeWithSameMintChange {
            denominator,
            change_shards: action.change_shards,
        })
    } else {
        Err(Error::Token)
    }
}
