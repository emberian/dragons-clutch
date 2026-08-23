//! Fully staged route plans and exact CPI/post-state reconciliation.

use clutch_kernel::{BasisMode, MarketState, PayoutSet, PayoutVector, Phase};
use clutch_solana_layout::{Hash32, PositionAccount, SupplyLedgerAccount};
use clutch_structured_claim::{
    BackingPlan, BackingVault, ClaimVector, HolderAssets, MarketLedger, NativeBasisIdentity,
    NativeClaim, StructuredClaimMachine, WrapperState,
};

use crate::codec::{DESCRIPTOR_LIVE, DESCRIPTOR_RETIRED};
use crate::identity::{
    canonical_replay_namespace, AddressBinding, PdaVerifier, RuntimeDeployments, REPLAY_SEED,
};
use crate::projection::{check_position, AccountRole};
use crate::{
    bind_descriptor, check_market_closure, AccountSet, Action, AuthenticatedMarket, Error, Key,
    MintProjection, RequestV1, Result, StructuredClaimDescriptorV1, TokenAccountProjection,
    WrapperReplayV1, MAX_OUTCOMES,
};

/// Maximum outer CPIs in any V1 route.
pub const MAX_CPI_STEPS: usize = 3;

/// Authenticated projection of the base program's per-Position replay anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaseReplayProjection {
    /// Replay account address.
    pub key: Key,
    /// Market identity.
    pub market: Key,
    /// Position owner identity.
    pub owner: Key,
    /// Position generation namespace.
    pub position_generation: u64,
    /// Exact next base request sequence.
    pub sequence: u64,
    /// Stored replay PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; must be zero.
    pub flags: u8,
}

impl BaseReplayProjection {
    const EMPTY: Self = Self {
        key: [0; 32],
        market: [0; 32],
        owner: [0; 32],
        position_generation: 0,
        sequence: 0,
        stored_bump: 0,
        flags: 0,
    };

    fn validate(&self, market: Key, owner: Key, generation: u64, sequence: u64) -> Result<()> {
        if self.key == [0; 32]
            || self.market != market
            || self.owner != owner
            || self.position_generation != generation
            || self.sequence != sequence
            || self.flags != 0
        {
            return Err(Error::ReplayMismatch);
        }
        Ok(())
    }
}

/// One exact outer CPI operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CpiStepKind {
    /// Fixed-capacity sentinel.
    None,
    /// Base atomic Position transfer from holder to wrapper vault.
    TransferIntoVault,
    /// Base atomic Position transfer from wrapper vault to beneficiary.
    TransferOutOfVault,
    /// Existing base complete-set Merge in the vault Position.
    MergeCompleteSet,
    /// Existing base complete-set Split in the vault Position.
    SplitCompleteSet,
    /// Base beneficiary-free collateral donation.
    DonateCollateral,
    /// Base beneficiary-free internal-vector donation.
    DonateInternalVector,
    /// Base exact aggregate-vector redemption from vault to beneficiary.
    RedeemInternalVector,
    /// Token-2022 `MintToChecked` with decimals zero.
    TokenMintChecked,
    /// Token-2022 `BurnChecked` with decimals zero.
    TokenBurnChecked,
}

/// Exact CPI target, arguments, and replay expectations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpiStep {
    /// Invoked executable.
    pub program: Key,
    /// Frozen operation family.
    pub kind: CpiStepKind,
    /// Wrapper or complete-set quantity, depending on `kind`.
    pub quantity: u64,
    /// Exact cash argument or expected cash consequence.
    pub cash_atoms: u64,
    /// Exact canonical native Egg vector argument/consequence.
    pub internal: [u64; MAX_OUTCOMES],
    /// Exact current source replay sequence, or zero when inapplicable.
    pub source_sequence: u64,
    /// Exact current destination/vault replay sequence, or zero when inapplicable.
    pub destination_sequence: u64,
}

impl CpiStep {
    /// Empty fixed-capacity sentinel.
    pub const EMPTY: Self = Self {
        program: [0; 32],
        kind: CpiStepKind::None,
        quantity: 0,
        cash_atoms: 0,
        internal: [0; MAX_OUTCOMES],
        source_sequence: 0,
        destination_sequence: 0,
    };
}

/// Dispatcher-produced receipt for one successful CPI.
///
/// The final accounts remain authoritative. A receipt proves that the staged
/// program and exact arguments were invoked successfully; [`reconcile_post_state`]
/// independently checks every resulting account field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpiReceipt {
    /// Exact successful call, byte-for-byte equal to the staged step.
    pub executed: CpiStep,
    /// CPI returned success.
    pub success: bool,
}

impl CpiReceipt {
    /// Empty fixed-capacity sentinel.
    pub const EMPTY: Self = Self {
        executed: CpiStep::EMPTY,
        success: false,
    };
}

/// Fully staged post-state. Every field is reconstructed from authoritative
/// account owners; there is no descriptor-maintained backing or supply shadow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedPostState {
    /// Live or retired descriptor state byte.
    pub descriptor_state: u8,
    /// Base Hoard collateral atoms.
    pub hoard_atoms: u64,
    /// Base kernel total native Egg supply.
    pub total_supply: [u64; MAX_OUTCOMES],
    /// Optional source/beneficiary Position after the route.
    pub holder_position: Option<PositionAccount>,
    /// Wrapper-vault Position after the route.
    pub vault_position: PositionAccount,
    /// Market-wide internal/external SupplyLedger after the route.
    pub supply: SupplyLedgerAccount,
    /// Actual Token-2022 mint after the route.
    pub mint: MintProjection,
    /// Optional holder token account after the route.
    pub holder_token: Option<TokenAccountProjection>,
    /// Wrapper replay after the route.
    pub wrapper_replay: WrapperReplayV1,
    /// Optional source/beneficiary base replay after the route.
    pub source_replay: Option<BaseReplayProjection>,
    /// Vault base replay after the route.
    pub vault_replay: BaseReplayProjection,
}

impl ExpectedPostState {
    const EMPTY: Self = Self {
        descriptor_state: DESCRIPTOR_LIVE,
        hoard_atoms: 0,
        total_supply: [0; MAX_OUTCOMES],
        holder_position: None,
        vault_position: empty_position(),
        supply: empty_supply(),
        mint: MintProjection {
            key: [0; 32],
            token_program: [0; 32],
            supply: 0,
            mint_authority: [0; 32],
            initialized: false,
            decimals: 0,
            freeze_authority_present: false,
            extension_mask: 0,
        },
        holder_token: None,
        wrapper_replay: WrapperReplayV1 {
            descriptor: [0; 32],
            actor: [0; 32],
            sequence: 0,
            stored_bump: 0,
        },
        source_replay: None,
        vault_replay: BaseReplayProjection::EMPTY,
    };
}

/// Fully checked outer execution plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoutePlan {
    /// Active CPI prefix length.
    pub step_count: u8,
    /// Active CPI prefix followed by [`CpiStep::EMPTY`].
    pub steps: [CpiStep; MAX_CPI_STEPS],
    /// Exact final account projections.
    pub post: ExpectedPostState,
}

impl RoutePlan {
    /// Empty caller-owned output slot for [`plan_route_into`].
    pub const EMPTY: Self = Self {
        step_count: 0,
        steps: [CpiStep::EMPTY; MAX_CPI_STEPS],
        post: ExpectedPostState::EMPTY,
    };
}

/// Caller-owned fixed workspace for the SBF planner.
///
/// This is roughly 5.2 KiB on the current host ABI of invalid sentinel state until planning overwrites
/// it. The dispatcher must place it in its requested heap or another owned
/// scratch region, never in one SBF frame. No field is persisted.
#[derive(Clone, Copy, Debug)]
pub struct RouteScratch {
    market: MarketLedger,
    machine: StructuredClaimMachine,
    holder: Option<HolderAssets>,
    pre_total_supply: [u64; MAX_OUTCOMES],
    pre_collateral: u64,
    donation: Option<clutch_structured_claim::DonationDelta>,
    redemption_payout: u64,
    builder: PlanBuilder,
    plan: RoutePlan,
}

impl RouteScratch {
    /// Invalid-but-initialized scratch sentinel; no semantic API reads a field
    /// before [`plan_route_into`] overwrites it.
    pub const EMPTY: Self = Self {
        market: empty_market_ledger(),
        machine: empty_machine(),
        holder: None,
        pre_total_supply: [0; MAX_OUTCOMES],
        pre_collateral: 0,
        donation: None,
        redemption_payout: 0,
        builder: PlanBuilder::new([0; 32], [0; 32], 0, 0),
        plan: RoutePlan::EMPTY,
    };

    /// Borrow the completed plan after [`plan_route_into`] returns `Ok(())`.
    pub const fn plan(&self) -> &RoutePlan {
        &self.plan
    }
}

/// Authenticated account projections for one planning call.
#[derive(Clone, Copy, Debug)]
pub struct AdapterContext<'a> {
    /// Immutable wrapper descriptor.
    pub descriptor: &'a StructuredClaimDescriptorV1,
    /// Canonical native portfolio claim id from the live layout owner.
    pub native_claim_id: Key,
    /// Canonical deployment-bound wrapper product id.
    pub product_id: Key,
    /// Authenticated executable deployments.
    pub deployments: &'a RuntimeDeployments,
    /// Wrapper-owned PDA addresses.
    pub addresses: &'a AddressBinding,
    /// Market/Terms/Hoard/Supply/kernel projection.
    pub market: AuthenticatedMarket<'a>,
    /// Wrapper-vault base Position.
    pub vault_position: &'a PositionAccount,
    /// Actual wrapper mint.
    pub mint: &'a MintProjection,
    /// Source or beneficiary Position for holder routes.
    pub holder_position: Option<&'a PositionAccount>,
    /// Holder wrapper-token account for mint/burn routes.
    pub holder_token: Option<&'a TokenAccountProjection>,
    /// Per-actor wrapper replay.
    pub wrapper_replay: &'a WrapperReplayV1,
    /// Optional source/beneficiary base replay.
    pub source_replay: Option<&'a BaseReplayProjection>,
    /// Vault base replay.
    pub vault_replay: &'a BaseReplayProjection,
    /// Exact role/address/privilege projection.
    pub accounts: &'a AccountSet,
}

const fn empty_position() -> PositionAccount {
    PositionAccount {
        market: Hash32::ZERO,
        owner: Hash32::ZERO,
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        stored_bump: 0,
        close_state: 0,
    }
}

const fn empty_supply() -> SupplyLedgerAccount {
    SupplyLedgerAccount {
        market: Hash32::ZERO,
        realm: Hash32::ZERO,
        generation: 0,
        outcome_count: 0,
        internal_supply: [0; MAX_OUTCOMES],
        external_supply: [0; MAX_OUTCOMES],
        stored_bump: 0,
        flags: 0,
    }
}

const fn empty_market_ledger() -> MarketLedger {
    MarketLedger {
        basis: NativeBasisIdentity {
            market: [0; 32],
            terms: [0; 32],
            basis_degree: 0,
            denominator: 0,
            outcome_count: 0,
        },
        base: MarketState {
            outcomes: 0,
            phase: Phase::Active,
            resolved_payout: 0,
            basis_mode: BasisMode::FinitePreset,
            resolved_vector: PayoutVector::ZERO,
            collateral: 0,
            total_supply: [0; MAX_OUTCOMES],
            payouts: PayoutSet::EMPTY,
        },
    }
}

const fn empty_machine() -> StructuredClaimMachine {
    StructuredClaimMachine {
        claim: NativeClaim {
            basis: NativeBasisIdentity {
                market: [0; 32],
                terms: [0; 32],
                basis_degree: 0,
                denominator: 0,
                outcome_count: 0,
            },
            vector: ClaimVector {
                outcome_count: 0,
                coefficients: [0; MAX_OUTCOMES],
            },
        },
        backing: BackingPlan {
            outcome_count: 0,
            cash_per_wrapper: 0,
            residual_eggs_per_wrapper: [0; MAX_OUTCOMES],
        },
        wrapper: WrapperState {
            actual_supply: 0,
            retired: false,
        },
        vault: BackingVault::EMPTY,
    }
}

/// Validate every precondition, run the semantic core on copies, and stage all
/// exact CPI arguments plus the complete expected post-state before any CPI.
#[cfg(not(target_os = "solana"))]
pub fn plan_route<P: PdaVerifier>(
    context: &AdapterContext<'_>,
    request: &RequestV1,
    pda_verifier: &P,
) -> Result<RoutePlan> {
    let mut scratch = RouteScratch::EMPTY;
    plan_route_into(context, request, pda_verifier, &mut scratch)?;
    Ok(scratch.plan)
}

/// Stack-bounded planner writing into a caller-owned scratch region.
///
/// The SBF dispatcher should allocate [`RouteScratch`] from its requested heap
/// and pass it here. No CPI may occur until this returns successfully.
#[inline(never)]
pub fn plan_route_into<P: PdaVerifier>(
    context: &AdapterContext<'_>,
    request: &RequestV1,
    pda_verifier: &P,
    scratch: &mut RouteScratch,
) -> Result<()> {
    scratch.plan = RoutePlan::EMPTY;
    preflight_route_into(context, request, pda_verifier, scratch)?;
    apply_semantics_into(context, request, scratch)?;
    finish_route_into(context, request, scratch)
}

#[inline(never)]
fn preflight_route_into<P: PdaVerifier>(
    context: &AdapterContext<'_>,
    request: &RequestV1,
    pda_verifier: &P,
    scratch: &mut RouteScratch,
) -> Result<()> {
    validate_route_context(context, request)?;
    bind_claim_into(context, pda_verifier, scratch)?;
    load_market_into(context, scratch)?;
    load_machine_into(context, scratch)?;
    load_holder_into(context, request, scratch)
}

#[inline(never)]
fn validate_route_context(context: &AdapterContext<'_>, request: &RequestV1) -> Result<()> {
    request.validate()?;
    context.accounts.validate_for(request.action)?;
    check_market_closure(&context.market)?;
    check_account_bindings(context, request.action)?;
    check_replays(context, request)?;

    context.mint.validate(
        context.addresses.mint,
        context.descriptor.token_2022_program,
        context.addresses.vault_owner,
    )?;
    if request.expected_mint_supply != context.mint.supply {
        return Err(Error::TokenDeltaMismatch);
    }
    check_position(
        context.vault_position,
        context.market.market,
        context.market.supply,
        context.addresses.vault_owner,
        request.vault_generation,
    )?;
    // Wrapper authority never places orders. A nonzero reserved-cash field is
    // therefore a direct invariant violation; seller-reserved Eggs would have
    // left `internal` and make the core's exact coverage check fail.
    if context.vault_position.reserved_cash_atoms != 0 {
        return Err(Error::InvalidPosition);
    }
    Ok(())
}

#[inline(never)]
fn bind_claim_into<P: PdaVerifier>(
    context: &AdapterContext<'_>,
    pda_verifier: &P,
    scratch: &mut RouteScratch,
) -> Result<()> {
    let wrapper_program = context.accounts.get(AccountRole::WrapperProgram)?.key;
    scratch.machine.claim = bind_descriptor(
        context.descriptor,
        wrapper_program,
        context.market.market,
        context.market.terms,
        context.deployments,
        context.native_claim_id,
        context.product_id,
        context.addresses,
        pda_verifier,
    )?;
    let replay_namespace =
        canonical_replay_namespace(context.product_id, context.wrapper_replay.actor)?;
    if !pda_verifier.verify(
        &wrapper_program,
        &context.accounts.get(AccountRole::WrapperReplay)?.key,
        REPLAY_SEED,
        &replay_namespace,
        context.wrapper_replay.stored_bump,
    ) {
        return Err(Error::PdaMismatch);
    }
    Ok(())
}

#[inline(never)]
fn load_market_into(context: &AdapterContext<'_>, scratch: &mut RouteScratch) -> Result<()> {
    scratch.market.basis = scratch.machine.claim.basis;
    scratch.market.base = *context.market.base;
    scratch.market.validate()?;
    scratch.pre_total_supply = scratch.market.base.total_supply;
    scratch.pre_collateral = scratch.market.base.collateral;
    Ok(())
}

#[inline(never)]
fn load_machine_into(context: &AdapterContext<'_>, scratch: &mut RouteScratch) -> Result<()> {
    scratch.machine = StructuredClaimMachine::restore(
        scratch.machine.claim,
        WrapperState {
            actual_supply: context.mint.supply,
            retired: context.descriptor.state == DESCRIPTOR_RETIRED,
        },
        BackingVault {
            cash_atoms: context.vault_position.cash_atoms,
            internal: context.vault_position.internal,
        },
        &scratch.market,
    )?;
    Ok(())
}

#[inline(never)]
fn load_holder_into(
    context: &AdapterContext<'_>,
    request: &RequestV1,
    scratch: &mut RouteScratch,
) -> Result<()> {
    scratch.holder = holder_assets(context, request)?;
    scratch.donation = None;
    scratch.redemption_payout = 0;
    Ok(())
}

#[inline(never)]
fn apply_semantics_into(
    context: &AdapterContext<'_>,
    request: &RequestV1,
    scratch: &mut RouteScratch,
) -> Result<()> {
    match request.action {
        Action::WrapCanonical => {
            scratch.machine.wrap_canonical(
                &scratch.market,
                scratch.holder.as_mut().ok_or(Error::InvalidPosition)?,
                request.quantity,
            )?;
        }
        Action::WrapFull => {
            scratch.machine.wrap_full(
                &mut scratch.market,
                scratch.holder.as_mut().ok_or(Error::InvalidPosition)?,
                request.quantity,
            )?;
        }
        Action::UnwindCanonical => {
            scratch.machine.unwind_canonical(
                &scratch.market,
                scratch.holder.as_mut().ok_or(Error::InvalidPosition)?,
                request.quantity,
            )?;
        }
        Action::UnwindFull => {
            scratch.machine.unwind_full(
                &mut scratch.market,
                scratch.holder.as_mut().ok_or(Error::InvalidPosition)?,
                request.quantity,
            )?;
        }
        Action::CompactDonation => {
            scratch.donation = Some(scratch.machine.compact_donation(&mut scratch.market)?);
        }
        Action::RedeemVector => {
            scratch.redemption_payout = scratch.machine.redeem_terminal(
                &mut scratch.market,
                scratch.holder.as_mut().ok_or(Error::InvalidPosition)?,
                request.quantity,
            )?;
        }
        Action::Retire => scratch.machine.retire(&scratch.market)?,
    }
    if scratch.market.base.collateral > context.market.market.collateral_cap
        || scratch.market.base.collateral > context.market.terms.collateral_cap
    {
        return Err(Error::CollateralCapExceeded);
    }
    Ok(())
}

#[inline(never)]
fn finish_route_into(
    context: &AdapterContext<'_>,
    request: &RequestV1,
    scratch: &mut RouteScratch,
) -> Result<()> {
    scratch.builder = PlanBuilder::new(
        context.deployments.binding.base_program,
        context.deployments.binding.token_2022_program,
        request.source_base_sequence,
        request.vault_base_sequence,
    );
    stage_steps(
        &mut scratch.builder,
        request,
        &scratch.machine,
        request.expected_mint_supply,
        scratch.donation,
        scratch.redemption_payout,
    )?;
    check_required_access(context, request.action, &scratch.builder)?;
    scratch.plan.step_count = scratch.builder.count;
    scratch.plan.steps = scratch.builder.steps;
    stage_post_state_into(
        context,
        request,
        scratch.pre_total_supply,
        scratch.pre_collateral,
        &scratch.market,
        &scratch.machine,
        scratch.holder,
        scratch.builder.source_sequence,
        scratch.builder.vault_sequence,
        &mut scratch.plan.post,
    )?;
    Ok(())
}

/// Onchain monomorphization using Solana's program-address syscall.
///
/// Kept as a named, non-inlined SBF boundary so stack evidence covers the same
/// verifier implementation the dispatcher will call.
#[cfg(target_os = "solana")]
#[inline(never)]
pub fn plan_route_solana(
    context: &AdapterContext<'_>,
    request: &RequestV1,
    scratch: &mut RouteScratch,
) -> Result<()> {
    plan_route_into(
        context,
        request,
        &crate::identity::SolanaPdaVerifier,
        scratch,
    )
}

fn holder_assets(
    context: &AdapterContext<'_>,
    request: &RequestV1,
) -> Result<Option<HolderAssets>> {
    if matches!(request.action, Action::CompactDonation | Action::Retire) {
        if context.holder_position.is_some()
            || context.holder_token.is_some()
            || context.source_replay.is_some()
            || request.expected_holder_amount != 0
            || request.source_generation != 0
            || request.source_base_sequence != 0
        {
            return Err(Error::NonCanonical);
        }
        return Ok(None);
    }
    let position = context.holder_position.ok_or(Error::InvalidPosition)?;
    let token = context.holder_token.ok_or(Error::InvalidTokenProjection)?;
    token.validate(
        context.addresses.mint,
        context.descriptor.token_2022_program,
    )?;
    if token.amount != request.expected_holder_amount {
        return Err(Error::TokenDeltaMismatch);
    }
    check_position(
        position,
        context.market.market,
        context.market.supply,
        position.owner.bytes(),
        request.source_generation,
    )?;
    Ok(Some(HolderAssets {
        cash_atoms: position
            .free_cash_atoms()
            .map_err(|_| Error::InvalidPosition)?,
        internal: position.internal,
        wrapper_atoms: token.amount,
    }))
}

fn check_replays(context: &AdapterContext<'_>, request: &RequestV1) -> Result<()> {
    let actor = context.accounts.get(AccountRole::Actor)?;
    let descriptor = context.accounts.get(AccountRole::Descriptor)?;
    context.wrapper_replay.validate()?;
    if context.wrapper_replay.descriptor != descriptor.key
        || context.wrapper_replay.actor != actor.key
        || context.wrapper_replay.sequence != request.wrapper_sequence
    {
        return Err(Error::ReplayMismatch);
    }
    let market = context.market.market.market.bytes();
    context.vault_replay.validate(
        market,
        context.addresses.vault_owner,
        request.vault_generation,
        request.vault_base_sequence,
    )?;
    if let (Some(position), Some(replay)) = (context.holder_position, context.source_replay) {
        replay.validate(
            market,
            position.owner.bytes(),
            request.source_generation,
            request.source_base_sequence,
        )?;
    }
    Ok(())
}

fn check_account_bindings(context: &AdapterContext<'_>, action: Action) -> Result<()> {
    let accounts = context.accounts;
    let wrapper_program = context.deployments.binding.wrapper_program;
    let base_program = context.deployments.binding.base_program;
    let token_program = context.deployments.binding.token_2022_program;
    for (role, expected, executable) in [
        (AccountRole::WrapperProgram, wrapper_program, true),
        (AccountRole::BaseProgram, base_program, true),
        (AccountRole::TokenProgram, token_program, true),
        (AccountRole::Descriptor, context.addresses.descriptor, false),
        (AccountRole::Mint, context.addresses.mint, false),
        (
            AccountRole::VaultPosition,
            context.accounts.get(AccountRole::VaultPosition)?.key,
            false,
        ),
        (
            AccountRole::WrapperReplay,
            context.wrapper_replay_key()?,
            false,
        ),
        (AccountRole::VaultReplay, context.vault_replay.key, false),
    ] {
        let account = accounts.get(role)?;
        if account.key != expected || account.executable != executable {
            return Err(Error::InvalidAccountSet);
        }
    }
    let owner_checks = [
        (AccountRole::Descriptor, wrapper_program),
        (AccountRole::WrapperReplay, wrapper_program),
        (AccountRole::Mint, token_program),
        (AccountRole::VaultPosition, base_program),
        (AccountRole::VaultReplay, base_program),
        (AccountRole::Market, base_program),
        (AccountRole::Terms, base_program),
        (AccountRole::Hoard, base_program),
        (AccountRole::SupplyLedger, base_program),
        (AccountRole::Kernel, base_program),
    ];
    let mut i = 0;
    while i < owner_checks.len() {
        if accounts.get(owner_checks[i].0)?.owner != owner_checks[i].1 {
            return Err(Error::InvalidAccountSet);
        }
        i += 1;
    }
    if accounts.get(AccountRole::Mint)?.key != context.mint.key
        || !accounts.get(AccountRole::Actor)?.signer
    {
        return Err(Error::InvalidAccountSet);
    }
    if !matches!(action, Action::CompactDonation | Action::Retire) {
        let position = context.holder_position.ok_or(Error::InvalidPosition)?;
        let token = context.holder_token.ok_or(Error::InvalidTokenProjection)?;
        let replay = context.source_replay.ok_or(Error::ReplayMismatch)?;
        if accounts.get(AccountRole::HolderPosition)?.owner != base_program
            || accounts.get(AccountRole::HolderToken)?.owner != token_program
            || accounts.get(AccountRole::SourceReplay)?.owner != base_program
            || accounts.get(AccountRole::HolderToken)?.key != token.key
            || accounts.get(AccountRole::SourceReplay)?.key != replay.key
            || accounts.get(AccountRole::HolderPosition)?.key
                == accounts.get(AccountRole::VaultPosition)?.key
            || position.owner.bytes() == context.addresses.vault_owner
        {
            return Err(Error::InvalidAccountSet);
        }
        let actor = accounts.get(AccountRole::Actor)?.key;
        match action {
            Action::WrapCanonical | Action::WrapFull if actor != position.owner.bytes() => {
                return Err(Error::InvalidAccountSet);
            }
            Action::UnwindCanonical | Action::UnwindFull | Action::RedeemVector
                if actor != token.authority =>
            {
                return Err(Error::InvalidAccountSet);
            }
            _ => {}
        }
    }
    Ok(())
}

impl AdapterContext<'_> {
    fn wrapper_replay_key(&self) -> Result<Key> {
        Ok(self.accounts.get(AccountRole::WrapperReplay)?.key)
    }
}

#[derive(Clone, Copy, Debug)]
struct PlanBuilder {
    base_program: Key,
    token_program: Key,
    source_sequence: u64,
    vault_sequence: u64,
    count: u8,
    steps: [CpiStep; MAX_CPI_STEPS],
}

impl PlanBuilder {
    const fn new(
        base_program: Key,
        token_program: Key,
        source_sequence: u64,
        vault_sequence: u64,
    ) -> Self {
        Self {
            base_program,
            token_program,
            source_sequence,
            vault_sequence,
            count: 0,
            steps: [CpiStep::EMPTY; MAX_CPI_STEPS],
        }
    }

    fn push(&mut self, step: CpiStep) -> Result<()> {
        let slot = self
            .steps
            .get_mut(usize::from(self.count))
            .ok_or(Error::Arithmetic)?;
        *slot = step;
        self.count = self.count.checked_add(1).ok_or(Error::Arithmetic)?;
        Ok(())
    }

    fn token(&mut self, kind: CpiStepKind, quantity: u64) -> Result<()> {
        self.push(CpiStep {
            program: self.token_program,
            kind,
            quantity,
            cash_atoms: 0,
            internal: [0; MAX_OUTCOMES],
            source_sequence: 0,
            destination_sequence: 0,
        })
    }

    fn transfer(
        &mut self,
        kind: CpiStepKind,
        cash_atoms: u64,
        internal: [u64; MAX_OUTCOMES],
    ) -> Result<()> {
        let source = self.source_sequence;
        let vault = self.vault_sequence;
        self.source_sequence = self
            .source_sequence
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        self.vault_sequence = self
            .vault_sequence
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        self.push(CpiStep {
            program: self.base_program,
            kind,
            quantity: 0,
            cash_atoms,
            internal,
            source_sequence: if kind == CpiStepKind::TransferIntoVault {
                source
            } else {
                vault
            },
            destination_sequence: if kind == CpiStepKind::TransferIntoVault {
                vault
            } else {
                source
            },
        })
    }

    fn vault_base(
        &mut self,
        kind: CpiStepKind,
        quantity: u64,
        cash_atoms: u64,
        internal: [u64; MAX_OUTCOMES],
    ) -> Result<()> {
        let vault = self.vault_sequence;
        self.vault_sequence = self
            .vault_sequence
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        self.push(CpiStep {
            program: self.base_program,
            kind,
            quantity,
            cash_atoms,
            internal,
            source_sequence: vault,
            destination_sequence: 0,
        })
    }

    fn redeem(
        &mut self,
        quantity: u64,
        cash_atoms: u64,
        internal: [u64; MAX_OUTCOMES],
    ) -> Result<()> {
        let vault = self.vault_sequence;
        let destination = self.source_sequence;
        self.vault_sequence = self
            .vault_sequence
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        self.source_sequence = self
            .source_sequence
            .checked_add(1)
            .ok_or(Error::Arithmetic)?;
        self.push(CpiStep {
            program: self.base_program,
            kind: CpiStepKind::RedeemInternalVector,
            quantity,
            cash_atoms,
            internal,
            source_sequence: vault,
            destination_sequence: destination,
        })
    }
}

fn stage_steps(
    builder: &mut PlanBuilder,
    request: &RequestV1,
    machine: &StructuredClaimMachine,
    pre_wrapper_supply: u64,
    donation: Option<clutch_structured_claim::DonationDelta>,
    redemption_payout: u64,
) -> Result<()> {
    let backing = machine.backing;
    let quantity = request.quantity;
    let floor_cash = quantity
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(Error::Arithmetic)?;
    let residual = scaled(&backing.residual_eggs_per_wrapper, quantity)?;
    let full = scaled(&machine.claim.vector.coefficients, quantity)?;
    let mut complete_sets = [0; MAX_OUTCOMES];
    let mut i = 0;
    while i < usize::from(machine.claim.basis.outcome_count) {
        complete_sets[i] = floor_cash;
        i += 1;
    }
    match request.action {
        Action::WrapCanonical => {
            builder.transfer(CpiStepKind::TransferIntoVault, floor_cash, residual)?;
            builder.token(CpiStepKind::TokenMintChecked, quantity)?;
        }
        Action::WrapFull => {
            builder.transfer(CpiStepKind::TransferIntoVault, 0, full)?;
            if floor_cash != 0 {
                builder.vault_base(
                    CpiStepKind::MergeCompleteSet,
                    floor_cash,
                    floor_cash,
                    complete_sets,
                )?;
            }
            builder.token(CpiStepKind::TokenMintChecked, quantity)?;
        }
        Action::UnwindCanonical => {
            builder.token(CpiStepKind::TokenBurnChecked, quantity)?;
            builder.transfer(CpiStepKind::TransferOutOfVault, floor_cash, residual)?;
        }
        Action::UnwindFull => {
            builder.token(CpiStepKind::TokenBurnChecked, quantity)?;
            if floor_cash != 0 {
                builder.vault_base(
                    CpiStepKind::SplitCompleteSet,
                    floor_cash,
                    floor_cash,
                    complete_sets,
                )?;
            }
            builder.transfer(CpiStepKind::TransferOutOfVault, 0, full)?;
        }
        Action::CompactDonation => {
            let delta = donation.ok_or(Error::PostStateMismatch)?;
            if delta.cash_to_hoard != 0 {
                builder.vault_base(
                    CpiStepKind::DonateCollateral,
                    delta.cash_to_hoard,
                    delta.cash_to_hoard,
                    [0; MAX_OUTCOMES],
                )?;
            }
            if delta.eggs_destroyed.iter().any(|amount| *amount != 0) {
                builder.vault_base(
                    CpiStepKind::DonateInternalVector,
                    0,
                    0,
                    delta.eggs_destroyed,
                )?;
            }
        }
        Action::RedeemVector => {
            builder.token(CpiStepKind::TokenBurnChecked, quantity)?;
            builder.redeem(floor_cash, redemption_payout, residual)?;
        }
        Action::Retire => {}
    }
    let expected_supply = match request.action {
        Action::WrapCanonical | Action::WrapFull => pre_wrapper_supply
            .checked_add(quantity)
            .ok_or(Error::Arithmetic)?,
        Action::UnwindCanonical | Action::UnwindFull | Action::RedeemVector => pre_wrapper_supply
            .checked_sub(quantity)
            .ok_or(Error::Arithmetic)?,
        Action::CompactDonation | Action::Retire => pre_wrapper_supply,
    };
    if machine.wrapper.actual_supply != expected_supply {
        return Err(Error::PostStateMismatch);
    }
    Ok(())
}

fn scaled(values: &[u64; MAX_OUTCOMES], quantity: u64) -> Result<[u64; MAX_OUTCOMES]> {
    let mut output = [0; MAX_OUTCOMES];
    let mut i = 0;
    while i < MAX_OUTCOMES {
        output[i] = values[i].checked_mul(quantity).ok_or(Error::Arithmetic)?;
        i += 1;
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn stage_post_state_into(
    context: &AdapterContext<'_>,
    request: &RequestV1,
    pre_total_supply: [u64; MAX_OUTCOMES],
    pre_collateral: u64,
    post_market: &MarketLedger,
    post_machine: &StructuredClaimMachine,
    holder: Option<HolderAssets>,
    source_sequence: u64,
    vault_sequence: u64,
    output: &mut ExpectedPostState,
) -> Result<()> {
    let mut vault = *context.vault_position;
    vault.cash_atoms = post_machine.vault.cash_atoms;
    vault.internal = post_machine.vault.internal;
    vault.reserved_cash_atoms = 0;

    let holder_position = match (context.holder_position, holder) {
        (Some(pre), Some(assets)) => {
            let mut post = *pre;
            post.cash_atoms = assets
                .cash_atoms
                .checked_add(pre.reserved_cash_atoms)
                .ok_or(Error::Arithmetic)?;
            post.internal = assets.internal;
            Some(post)
        }
        (None, None) => None,
        _ => return Err(Error::PostStateMismatch),
    };
    let holder_token = match (context.holder_token, holder) {
        (Some(pre), Some(assets)) => {
            let mut post = *pre;
            post.amount = assets.wrapper_atoms;
            Some(post)
        }
        (None, None) => None,
        _ => return Err(Error::PostStateMismatch),
    };
    let mut mint = *context.mint;
    mint.supply = post_machine.wrapper.actual_supply;

    let mut supply = *context.market.supply;
    let active = usize::from(context.market.market.outcome_count);
    let mut i = 0;
    while i < active {
        supply.internal_supply[i] = apply_delta(
            supply.internal_supply[i],
            pre_total_supply[i],
            post_market.base.total_supply[i],
        )?;
        let aggregate = supply.internal_supply[i]
            .checked_add(supply.external_supply[i])
            .ok_or(Error::Arithmetic)?;
        if aggregate != post_market.base.total_supply[i] {
            return Err(Error::SupplyClosureMismatch);
        }
        i += 1;
    }

    check_presented_conservation(
        context.holder_position,
        holder_position.as_ref(),
        context.vault_position,
        &vault,
        pre_collateral,
        post_market.base.collateral,
        context.market.supply,
        &supply,
    )?;
    supply
        .validate()
        .map_err(|_| Error::SupplyClosureMismatch)?;
    supply
        .check_position_bound(&vault)
        .map_err(|_| Error::SupplyClosureMismatch)?;
    if let Some(position) = holder_position.as_ref() {
        supply
            .check_position_bound(position)
            .map_err(|_| Error::SupplyClosureMismatch)?;
    }

    let mut wrapper_replay = *context.wrapper_replay;
    wrapper_replay.sequence = wrapper_replay
        .sequence
        .checked_add(1)
        .ok_or(Error::Arithmetic)?;
    let mut vault_replay = *context.vault_replay;
    vault_replay.sequence = vault_sequence;
    let source_replay = match context.source_replay {
        Some(value) => {
            let mut post = *value;
            post.sequence = source_sequence;
            Some(post)
        }
        None => None,
    };
    let descriptor_state = if post_machine.wrapper.retired {
        DESCRIPTOR_RETIRED
    } else {
        DESCRIPTOR_LIVE
    };
    if request.action == Action::Retire && descriptor_state != DESCRIPTOR_RETIRED {
        return Err(Error::PostStateMismatch);
    }
    *output = ExpectedPostState {
        descriptor_state,
        hoard_atoms: post_market.base.collateral,
        total_supply: post_market.base.total_supply,
        holder_position,
        vault_position: vault,
        supply,
        mint,
        holder_token,
        wrapper_replay,
        source_replay,
        vault_replay,
    };
    Ok(())
}

fn apply_delta(current: u64, pre: u64, post: u64) -> Result<u64> {
    if post >= pre {
        current.checked_add(post - pre).ok_or(Error::Arithmetic)
    } else {
        current.checked_sub(pre - post).ok_or(Error::Arithmetic)
    }
}

#[allow(clippy::too_many_arguments)]
fn check_presented_conservation(
    pre_holder: Option<&PositionAccount>,
    post_holder: Option<&PositionAccount>,
    pre_vault: &PositionAccount,
    post_vault: &PositionAccount,
    pre_hoard: u64,
    post_hoard: u64,
    pre_supply: &SupplyLedgerAccount,
    post_supply: &SupplyLedgerAccount,
) -> Result<()> {
    let pre_cash = u128::from(pre_hoard)
        + u128::from(pre_vault.cash_atoms)
        + u128::from(pre_holder.map_or(0, |position| position.cash_atoms));
    let post_cash = u128::from(post_hoard)
        + u128::from(post_vault.cash_atoms)
        + u128::from(post_holder.map_or(0, |position| position.cash_atoms));
    if pre_cash != post_cash {
        return Err(Error::SupplyClosureMismatch);
    }
    let mut i = 0;
    while i < MAX_OUTCOMES {
        let pre_positions = u128::from(pre_vault.internal[i])
            + u128::from(pre_holder.map_or(0, |position| position.internal[i]));
        let post_positions = u128::from(post_vault.internal[i])
            + u128::from(post_holder.map_or(0, |position| position.internal[i]));
        let pre_ledger = u128::from(pre_supply.internal_supply[i]);
        let post_ledger = u128::from(post_supply.internal_supply[i]);
        if pre_positions + post_ledger != post_positions + pre_ledger {
            return Err(Error::SupplyClosureMismatch);
        }
        i += 1;
    }
    Ok(())
}

fn check_required_access(
    context: &AdapterContext<'_>,
    action: Action,
    builder: &PlanBuilder,
) -> Result<()> {
    let accounts = context.accounts;
    if !accounts.get(AccountRole::WrapperReplay)?.writable {
        return Err(Error::InvalidAccountSet);
    }
    if action == Action::Retire && !accounts.get(AccountRole::Descriptor)?.writable {
        return Err(Error::InvalidAccountSet);
    }
    let mut needs_token = false;
    let mut needs_positions = false;
    let mut needs_market_write = false;
    let mut i = 0;
    while i < usize::from(builder.count) {
        match builder.steps[i].kind {
            CpiStepKind::TokenMintChecked | CpiStepKind::TokenBurnChecked => needs_token = true,
            CpiStepKind::TransferIntoVault | CpiStepKind::TransferOutOfVault => {
                needs_positions = true
            }
            CpiStepKind::MergeCompleteSet
            | CpiStepKind::SplitCompleteSet
            | CpiStepKind::DonateCollateral
            | CpiStepKind::DonateInternalVector
            | CpiStepKind::RedeemInternalVector => {
                needs_positions = true;
                needs_market_write = true;
            }
            CpiStepKind::None => return Err(Error::NonCanonical),
        }
        i += 1;
    }
    if needs_token
        && (!accounts.get(AccountRole::Mint)?.writable
            || !accounts.get(AccountRole::HolderToken)?.writable)
    {
        return Err(Error::InvalidAccountSet);
    }
    if needs_positions {
        if !accounts.get(AccountRole::VaultPosition)?.writable
            || !accounts.get(AccountRole::VaultReplay)?.writable
        {
            return Err(Error::InvalidAccountSet);
        }
        if !matches!(action, Action::CompactDonation)
            && (!accounts.get(AccountRole::HolderPosition)?.writable
                || !accounts.get(AccountRole::SourceReplay)?.writable)
        {
            return Err(Error::InvalidAccountSet);
        }
    }
    if needs_market_write
        && (!accounts.get(AccountRole::Hoard)?.writable
            || !accounts.get(AccountRole::SupplyLedger)?.writable
            || !accounts.get(AccountRole::Kernel)?.writable)
    {
        return Err(Error::InvalidAccountSet);
    }
    Ok(())
}

/// Check an exact active receipt prefix and canonical empty padding.
pub fn reconcile_receipts(
    plan: &RoutePlan,
    receipt_count: u8,
    receipts: &[CpiReceipt; MAX_CPI_STEPS],
) -> Result<()> {
    if receipt_count != plan.step_count {
        return Err(Error::CpiReceiptMismatch);
    }
    let mut i = 0;
    while i < MAX_CPI_STEPS {
        if i < usize::from(plan.step_count) {
            if !receipts[i].success || receipts[i].executed != plan.steps[i] {
                return Err(Error::CpiReceiptMismatch);
            }
        } else if receipts[i] != CpiReceipt::EMPTY || plan.steps[i] != CpiStep::EMPTY {
            return Err(Error::NonCanonical);
        }
        i += 1;
    }
    Ok(())
}

/// Check every authenticated final account against the pre-CPI staged state.
pub fn reconcile_post_state(plan: &RoutePlan, observed: &ExpectedPostState) -> Result<()> {
    if plan.post == *observed {
        Ok(())
    } else {
        Err(Error::PostStateMismatch)
    }
}
