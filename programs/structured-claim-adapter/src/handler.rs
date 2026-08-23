//! Canonical runtime-contract handlers and exact outer execution plans.

use crate::runtime_contract::{
    prepare_compact_donation_v1, prepare_permanent_identity_funding_v1, prepare_redeem_terminal_v1,
    prepare_retire_descriptor_v1, prepare_unwrap_canonical_v1, prepare_unwrap_full_v1,
    prepare_wrap_canonical_v1, prepare_wrap_full_v1, AuthenticatedVaultRetirementV1,
    CanonicalUnwrapRequestV1, CanonicalWrapRequestV1, CreateDescriptorPayloadV1,
    DescriptorRetirementPlanV1, DescriptorStateV1, DonationCompactionPlanV1,
    MarketChangingWrapperTransitionPlanV1, PermanentIdentityFundingPlanV1,
    PermanentTargetProjectionV1, StructuredClaimActionV1, StructuredClaimDescriptorV2,
    StructuredClaimPayloadV1, TerminalRedemptionPlanV1, VaultMutationRequestV1,
    WrapperTransitionPlanV1,
};

use crate::{
    is_zero, AuthenticatedBaseMarketV1, AuthenticatedBasePositionV3,
    AuthenticatedStructuredCustodyCallV1, AuthenticatedTokenMintV1, AuthenticatedTokenV1,
    BasePositionTransferCpiV1, BoundDescriptorV1, Error, Key, Result,
};

/// Maximum staged outer operations in any version-one route.
pub const MAX_EXECUTION_STEPS: usize = 5;

/// Exact base-program construction evidence supplied by its semantic owner.
///
/// The prefund amounts are never refundable to the creator or a caller. The
/// named `rent_transition_id` must be consumed by a later base close capability
/// that returns creator-funded shortfalls to `payer` and sends these prefunds
/// to `neutral_sink`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaseVaultCreationEvidenceV1 {
    /// Content identity of the complete base construction plan.
    pub creation_receipt: Key,
    /// Wrapper product receiving the dedicated base vault.
    pub wrapper_product_id: Key,
    /// Canonical base Market.
    pub market: Key,
    /// Semantic owner of the dedicated Position.
    pub vault_owner: Key,
    /// Canonical base Position account.
    pub position: Key,
    /// Canonical current-generation Replay account.
    pub replay: Key,
    /// Initial Position generation, necessarily one.
    pub generation: u64,
    /// Initial replay sequence, necessarily zero.
    pub replay_sequence: u64,
    /// Construction payer whose exact shortfalls remain refundable.
    pub payer: Key,
    /// Beneficiary-free sink for all hostile or benevolent prefunding.
    pub neutral_sink: Key,
    /// Existing Position lamports before construction.
    pub position_prefund_lamports: u64,
    /// Existing Replay lamports before construction.
    pub replay_prefund_lamports: u64,
    /// Full payer-funded Position live-plus-tombstone principal. Prefunding
    /// never discounts this amount.
    pub position_principal_lamports: u64,
    /// Full payer-funded Replay principal. Prefunding never discounts this
    /// amount.
    pub replay_principal_lamports: u64,
    /// Exact Position lamports after construction.
    pub position_final_lamports: u64,
    /// Exact Replay lamports after construction.
    pub replay_final_lamports: u64,
    /// Content identity binding the later close-time rent split.
    pub rent_transition_id: Key,
}

/// Named trust boundary for base vault construction and retirement authority.
pub trait BaseCapabilityVerifierV1 {
    /// Verify the complete base vault creation capability.
    fn verify_creation(&self, evidence: &BaseVaultCreationEvidenceV1) -> bool;

    /// Verify the exact base Position close capability.
    fn verify_retirement(&self, evidence: &AuthenticatedVaultRetirementV1) -> bool;
}

/// Base vault construction capability minted only by the named verifier seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundBaseVaultCreationV1(BaseVaultCreationEvidenceV1);

impl BoundBaseVaultCreationV1 {
    /// Exact authenticated creation evidence.
    pub const fn evidence(&self) -> BaseVaultCreationEvidenceV1 {
        self.0
    }
}

/// Base vault retirement capability minted only by the named verifier seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundBaseVaultRetirementV1(AuthenticatedVaultRetirementV1);

impl BoundBaseVaultRetirementV1 {
    /// Canonical runtime-contract retirement evidence.
    pub const fn evidence(&self) -> AuthenticatedVaultRetirementV1 {
        self.0
    }
}

/// Authenticate a pre-fund-safe, empty base Position/Replay construction plan.
pub fn authenticate_base_vault_creation_v1<V: BaseCapabilityVerifierV1>(
    evidence: BaseVaultCreationEvidenceV1,
    verifier: &V,
) -> Result<BoundBaseVaultCreationV1> {
    let identities = [
        evidence.creation_receipt,
        evidence.wrapper_product_id,
        evidence.market,
        evidence.vault_owner,
        evidence.position,
        evidence.replay,
        evidence.payer,
        evidence.neutral_sink,
        evidence.rent_transition_id,
    ];
    let mut left = 0_usize;
    while left < identities.len() {
        if is_zero(&identities[left]) {
            return Err(Error::BaseCapabilityUnavailable);
        }
        let mut right = left + 1;
        while right < identities.len() {
            if identities[left] == identities[right] {
                return Err(Error::BaseCapabilityUnavailable);
            }
            right += 1;
        }
        left += 1;
    }
    if evidence.generation != 1
        || evidence.replay_sequence != 0
        || evidence.position_final_lamports
            != evidence
                .position_prefund_lamports
                .checked_add(evidence.position_principal_lamports)
                .ok_or(Error::Arithmetic)?
        || evidence.replay_final_lamports
            != evidence
                .replay_prefund_lamports
                .checked_add(evidence.replay_principal_lamports)
                .ok_or(Error::Arithmetic)?
        || evidence.position_final_lamports == 0
        || evidence.replay_final_lamports == 0
        || !verifier.verify_creation(&evidence)
    {
        return Err(Error::BaseCapabilityUnavailable);
    }
    Ok(BoundBaseVaultCreationV1(evidence))
}

/// Authenticate the base-program close capability required by retirement.
pub fn authenticate_base_vault_retirement_v1<V: BaseCapabilityVerifierV1>(
    evidence: AuthenticatedVaultRetirementV1,
    verifier: &V,
) -> Result<BoundBaseVaultRetirementV1> {
    if is_zero(&evidence.close_receipt)
        || is_zero(&evidence.market)
        || is_zero(&evidence.vault_owner)
        || is_zero(&evidence.tombstone)
        || evidence.close_receipt == evidence.tombstone
        || !verifier.verify_retirement(&evidence)
    {
        return Err(Error::BaseCapabilityUnavailable);
    }
    Ok(BoundBaseVaultRetirementV1(evidence))
}

/// Construction inputs after descriptor/deployment/PDA and base capability checks.
#[derive(Clone, Copy, Debug)]
pub struct CreateDescriptorContextV1<'a> {
    /// Fully bound canonical descriptor.
    pub descriptor: &'a BoundDescriptorV1,
    /// Construction payer.
    pub payer: Key,
    /// System executable.
    pub system_program: Key,
    /// Hostile pre-allocation descriptor target projection.
    pub descriptor_target: PermanentTargetProjectionV1,
    /// Hostile pre-allocation mint target projection.
    pub mint_target: PermanentTargetProjectionV1,
    /// Exact current-bank descriptor rent minimum.
    pub descriptor_rent_minimum: u64,
    /// Exact current-bank extension-free mint rent minimum.
    pub mint_rent_minimum: u64,
    /// Authenticated base Position/Replay creation plan.
    pub base_vault: BoundBaseVaultCreationV1,
}

/// Mutation inputs reconstructed from authoritative base and Token-2022 state.
#[derive(Clone, Copy, Debug)]
pub struct MutationContextV1<'a> {
    /// Fully bound canonical descriptor.
    pub descriptor: &'a BoundDescriptorV1,
    /// Authenticated base Market join.
    pub market: &'a AuthenticatedBaseMarketV1,
    /// Actual extension-free Token-2022 mint.
    pub mint: AuthenticatedTokenMintV1,
    /// Actual wrapper-vault Position and current-generation Replay.
    pub vault: AuthenticatedBasePositionV3,
    /// Optional user Position/Replay for quantity routes.
    pub user: Option<AuthenticatedBasePositionV3>,
    /// Optional actual holder token account for quantity routes.
    pub holder: Option<AuthenticatedTokenV1>,
    /// Transaction signer; quantity routes bind it to both user authorities.
    pub actor: Key,
    /// Exact typed General action-35 bridge, required only by canonical wrap/unwind.
    pub canonical_custody: Option<AuthenticatedStructuredCustodyCallV1>,
    /// Authenticated base close capability, present only for retirement.
    pub vault_retirement: Option<BoundBaseVaultRetirementV1>,
}

/// One predictable-PDA System allocation/assignment operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemPdaOperationV1 {
    /// Construction payer.
    pub payer: Key,
    /// Predictable target PDA.
    pub target: Key,
    /// Target owner after assignment.
    pub owner_after: Key,
    /// Exact allocation width.
    pub data_len: u32,
    /// Exact shortfall transferred from the payer.
    pub shortfall_lamports: u64,
    /// Existing lamports that remain locked and grant no authority.
    pub locked_prefund_lamports: u64,
    /// Exact post-funding balance before allocate/assign.
    pub final_lamports: u64,
}

/// Exact Token-2022 CPI operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Token2022CpiV1 {
    /// Initialize an extension-free zero-decimal mint.
    InitializeMint {
        /// Pinned Token-2022 executable.
        token_program: Key,
        /// Canonical wrapper mint.
        mint: Key,
        /// Canonical mint-authority PDA.
        mint_authority: Key,
    },
    /// Mint the exact wrapper quantity after backing enters custody.
    MintChecked {
        /// Canonical wrapper mint.
        mint: Key,
        /// Holder token account.
        token: Key,
        /// Mint-authority PDA.
        authority: Key,
        /// Exact zero-decimal quantity.
        quantity: u64,
        /// Actual supply before CPI.
        supply_before: u64,
        /// Required supply after CPI.
        supply_after: u64,
        /// Actual holder balance before CPI.
        holder_before: u64,
        /// Required holder balance after CPI.
        holder_after: u64,
    },
    /// Burn the exact wrapper quantity before backing leaves custody.
    BurnChecked {
        /// Canonical wrapper mint.
        mint: Key,
        /// Holder token account.
        token: Key,
        /// Signing token owner.
        authority: Key,
        /// Exact zero-decimal quantity.
        quantity: u64,
        /// Actual supply before CPI.
        supply_before: u64,
        /// Required supply after CPI.
        supply_after: u64,
        /// Actual holder balance before CPI.
        holder_before: u64,
        /// Required holder balance after CPI.
        holder_after: u64,
    },
    /// Permanently revoke mint authority after the empty vault closes.
    RevokeMintAuthority {
        /// Canonical wrapper mint.
        mint: Key,
        /// Current mint-authority PDA.
        authority_before: Key,
        /// Absent authority, canonically zero.
        authority_after: Key,
    },
}

/// Exact base-program CPI operation.
///
/// Every canonical custody variant carries the exact outer ExtensionRequest
/// and 23-account contract. Runtime-owned semantics remain in the route's
/// separate [`PreparedStructuredClaimSemanticV1`] and are checked against the
/// private-field custody authority before either step can be staged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseCpiV1 {
    /// Create the dedicated empty Position/Replay pair under the base program.
    CreateVault(BaseVaultCreationEvidenceV1),
    /// Execute the supply-neutral canonical backing transfer.
    CanonicalWrap(BasePositionTransferCpiV1),
    /// Execute the supply-neutral canonical backing return.
    CanonicalUnwrap(BasePositionTransferCpiV1),
    /// Execute full-vector custody and exact complete-set compression.
    FullWrap(MarketChangingWrapperTransitionPlanV1),
    /// Execute exact complete-set expansion and full-vector return.
    FullUnwrap(MarketChangingWrapperTransitionPlanV1),
    /// Donate every surplus atom with no beneficiary.
    CompactDonation(DonationCompactionPlanV1),
    /// Redeem the exact terminal aggregate vector into beneficiary cash.
    RedeemTerminal(TerminalRedemptionPlanV1),
    /// Close the empty vault through the authenticated base capability.
    CloseVault(AuthenticatedVaultRetirementV1),
}

/// Exact wrapper-owned descriptor write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorWriteV1 {
    /// Canonical descriptor account address.
    pub address: Key,
    /// Complete expected descriptor before this write, absent for creation.
    pub before: Option<StructuredClaimDescriptorV2>,
    /// Complete canonical 384-byte image after this write.
    pub after: StructuredClaimDescriptorV2,
}

/// One staged outer execution operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionStepV1 {
    /// Canonical empty fixed-capacity sentinel.
    None,
    /// System transfer plus PDA-signed allocate/assign.
    SystemPda(SystemPdaOperationV1),
    /// Exact Token-2022 CPI.
    Token2022(Token2022CpiV1),
    /// Exact base-program CPI.
    Base(BaseCpiV1),
    /// Wrapper-owned descriptor byte write.
    Descriptor(DescriptorWriteV1),
}

/// Canonical semantic output produced by the runtime-contract owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedStructuredClaimSemanticV1 {
    /// Permanent descriptor/mint funding plan.
    Create(PermanentIdentityFundingPlanV1),
    /// Canonical wrap.
    WrapCanonical(WrapperTransitionPlanV1),
    /// Full-vector wrap.
    WrapFull(MarketChangingWrapperTransitionPlanV1),
    /// Canonical unwind.
    UnwrapCanonical(WrapperTransitionPlanV1),
    /// Full-vector unwind.
    UnwrapFull(MarketChangingWrapperTransitionPlanV1),
    /// Beneficiary-free donation compaction.
    CompactDonation(DonationCompactionPlanV1),
    /// Exact terminal redemption.
    RedeemTerminal(TerminalRedemptionPlanV1),
    /// Permanent descriptor retirement.
    RetireDescriptor(DescriptorRetirementPlanV1),
}

/// Completely staged action with canonical empty step padding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedStructuredClaimRouteV1 {
    /// Family-local action.
    pub action: StructuredClaimActionV1,
    /// Canonical runtime-contract semantic result.
    pub semantic: PreparedStructuredClaimSemanticV1,
    /// Active prefix length.
    pub step_count: u8,
    /// Exact active operations followed by `None` sentinels.
    pub steps: [ExecutionStepV1; MAX_EXECUTION_STEPS],
}

/// Dispatcher-observed result for one staged outer operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepReceiptV1 {
    /// Exact operation submitted by the dispatcher.
    pub executed: ExecutionStepV1,
    /// CPI or local write returned success.
    pub success: bool,
}

impl StepReceiptV1 {
    /// Canonical inactive padding receipt.
    pub const EMPTY: Self = Self {
        executed: ExecutionStepV1::None,
        success: false,
    };
}

impl PreparedStructuredClaimRouteV1 {
    /// Reconcile exact successful operation receipts and canonical padding.
    pub fn reconcile_receipts(
        &self,
        receipt_count: u8,
        receipts: &[StepReceiptV1; MAX_EXECUTION_STEPS],
    ) -> Result<()> {
        if receipt_count != self.step_count {
            return Err(Error::ReceiptMismatch);
        }
        let mut index = 0_usize;
        while index < MAX_EXECUTION_STEPS {
            if index < usize::from(self.step_count) {
                if !receipts[index].success || receipts[index].executed != self.steps[index] {
                    return Err(Error::ReceiptMismatch);
                }
            } else if receipts[index] != StepReceiptV1::EMPTY
                || self.steps[index] != ExecutionStepV1::None
            {
                return Err(Error::ReceiptMismatch);
            }
            index += 1;
        }
        Ok(())
    }
}

/// Prepare atomic descriptor, mint, and empty base-vault construction.
pub fn prepare_create_descriptor_v1(
    context: &CreateDescriptorContextV1<'_>,
    payload: CreateDescriptorPayloadV1,
) -> Result<PreparedStructuredClaimRouteV1> {
    let descriptor = *context.descriptor.descriptor();
    let addresses = context.descriptor.addresses();
    let deployment = context.descriptor.identity().deployment;
    let base = context.base_vault.evidence();
    if descriptor.state != DescriptorStateV1::Active
        || payload.native_claim_id != context.descriptor.native_claim_id()
        || payload.wrapper_product_id != context.descriptor.wrapper_product_id()
        || payload.primitive != descriptor.primitive
        || base.wrapper_product_id != payload.wrapper_product_id
        || base.market != descriptor.market
        || base.vault_owner != addresses.vault_owner
        || base.payer != context.payer
        || context.descriptor_target.address != addresses.descriptor
        || context.mint_target.address != addresses.mint
    {
        return Err(Error::BaseCapabilityUnavailable);
    }
    let funding = prepare_permanent_identity_funding_v1(
        context.payer,
        context.system_program,
        deployment.wrapper_program,
        deployment.token_2022_program,
        addresses,
        context.descriptor_target,
        context.mint_target,
        context.descriptor_rent_minimum,
        context.mint_rent_minimum,
    )?;
    let mut builder = StepBuilder::new();
    builder.push(ExecutionStepV1::SystemPda(SystemPdaOperationV1 {
        payer: context.payer,
        target: addresses.descriptor,
        owner_after: deployment.wrapper_program,
        data_len: funding.descriptor_data_len,
        shortfall_lamports: funding.descriptor_shortfall_lamports,
        locked_prefund_lamports: funding.descriptor_locked_prefund_lamports,
        final_lamports: funding.descriptor_final_lamports,
    }))?;
    builder.push(ExecutionStepV1::SystemPda(SystemPdaOperationV1 {
        payer: context.payer,
        target: addresses.mint,
        owner_after: deployment.token_2022_program,
        data_len: funding.mint_data_len,
        shortfall_lamports: funding.mint_shortfall_lamports,
        locked_prefund_lamports: funding.mint_locked_prefund_lamports,
        final_lamports: funding.mint_final_lamports,
    }))?;
    builder.push(ExecutionStepV1::Token2022(Token2022CpiV1::InitializeMint {
        token_program: deployment.token_2022_program,
        mint: addresses.mint,
        mint_authority: addresses.mint_authority,
    }))?;
    builder.push(ExecutionStepV1::Base(BaseCpiV1::CreateVault(base)))?;
    builder.push(ExecutionStepV1::Descriptor(DescriptorWriteV1 {
        address: addresses.descriptor,
        before: None,
        after: descriptor,
    }))?;
    Ok(builder.finish(
        StructuredClaimActionV1::CreateDescriptor,
        PreparedStructuredClaimSemanticV1::Create(funding),
    ))
}

/// Prepare one exact supply-sensitive or lifecycle mutation.
pub fn prepare_mutation_v1(
    context: &MutationContextV1<'_>,
    payload: StructuredClaimPayloadV1,
) -> Result<PreparedStructuredClaimRouteV1> {
    if is_zero(&context.actor) {
        return Err(Error::InvalidAccounts);
    }
    let market = context.market.ledger(context.descriptor)?;
    let addresses = context.descriptor.addresses();
    let descriptor_state = context.descriptor.descriptor().state;
    let mint = context.mint.projection();
    let vault = context.vault.projection();
    match payload {
        StructuredClaimPayloadV1::CreateDescriptor(_) => Err(Error::InvalidInstruction),
        StructuredClaimPayloadV1::WrapCanonical(request) => {
            require_product(context, request.wrapper_product_id)?;
            let (holder, user) = quantity_accounts(context)?;
            require_quantity_actor(context.actor, holder, user)?;
            let plan = prepare_wrap_canonical_v1(
                descriptor_state,
                context.descriptor.identity(),
                &market,
                addresses,
                mint,
                holder,
                user,
                vault,
                CanonicalWrapRequestV1 {
                    quantity: request.quantity,
                    source_owner: context.actor,
                    source_generation: request.user_generation,
                    source_replay_sequence: request.user_replay_sequence,
                    vault_generation: request.vault_generation,
                    vault_replay_sequence: request.vault_replay_sequence,
                },
            )?;
            stage_wrap(context, plan, false)
        }
        StructuredClaimPayloadV1::WrapFull(request) => {
            require_no_canonical_custody(context)?;
            require_product(context, request.wrapper_product_id)?;
            let (holder, user) = quantity_accounts(context)?;
            require_quantity_actor(context.actor, holder, user)?;
            let plan = prepare_wrap_full_v1(
                descriptor_state,
                context.descriptor.identity(),
                market,
                addresses,
                mint,
                holder,
                user,
                vault,
                CanonicalWrapRequestV1 {
                    quantity: request.quantity,
                    source_owner: context.actor,
                    source_generation: request.user_generation,
                    source_replay_sequence: request.user_replay_sequence,
                    vault_generation: request.vault_generation,
                    vault_replay_sequence: request.vault_replay_sequence,
                },
            )?;
            stage_market_wrap(context, plan)
        }
        StructuredClaimPayloadV1::UnwrapCanonical(request) => {
            require_product(context, request.wrapper_product_id)?;
            let (holder, user) = quantity_accounts(context)?;
            require_quantity_actor(context.actor, holder, user)?;
            let plan = prepare_unwrap_canonical_v1(
                descriptor_state,
                context.descriptor.identity(),
                &market,
                addresses,
                mint,
                holder,
                user,
                vault,
                CanonicalUnwrapRequestV1 {
                    quantity: request.quantity,
                    destination_owner: context.actor,
                    destination_generation: request.user_generation,
                    destination_replay_sequence: request.user_replay_sequence,
                    vault_generation: request.vault_generation,
                    vault_replay_sequence: request.vault_replay_sequence,
                },
            )?;
            stage_wrap(context, plan, true)
        }
        StructuredClaimPayloadV1::UnwrapFull(request) => {
            require_no_canonical_custody(context)?;
            require_product(context, request.wrapper_product_id)?;
            let (holder, user) = quantity_accounts(context)?;
            require_quantity_actor(context.actor, holder, user)?;
            let plan = prepare_unwrap_full_v1(
                descriptor_state,
                context.descriptor.identity(),
                market,
                addresses,
                mint,
                holder,
                user,
                vault,
                CanonicalUnwrapRequestV1 {
                    quantity: request.quantity,
                    destination_owner: context.actor,
                    destination_generation: request.user_generation,
                    destination_replay_sequence: request.user_replay_sequence,
                    vault_generation: request.vault_generation,
                    vault_replay_sequence: request.vault_replay_sequence,
                },
            )?;
            stage_market_unwrap(context, plan)
        }
        StructuredClaimPayloadV1::CompactDonation(request) => {
            require_vault_only(context, request.wrapper_product_id)?;
            let plan = prepare_compact_donation_v1(
                descriptor_state,
                context.descriptor.identity(),
                market,
                addresses,
                mint,
                vault,
                VaultMutationRequestV1 {
                    vault_generation: request.vault_generation,
                    vault_replay_sequence: request.vault_replay_sequence,
                },
            )?;
            let mut builder = StepBuilder::new();
            builder.push(ExecutionStepV1::Base(BaseCpiV1::CompactDonation(plan)))?;
            Ok(builder.finish(
                StructuredClaimActionV1::CompactDonation,
                PreparedStructuredClaimSemanticV1::CompactDonation(plan),
            ))
        }
        StructuredClaimPayloadV1::RedeemTerminal(request) => {
            require_no_canonical_custody(context)?;
            require_product(context, request.wrapper_product_id)?;
            let (holder, user) = quantity_accounts(context)?;
            require_quantity_actor(context.actor, holder, user)?;
            let plan = prepare_redeem_terminal_v1(
                descriptor_state,
                context.descriptor.identity(),
                market,
                addresses,
                mint,
                holder,
                user,
                vault,
                CanonicalUnwrapRequestV1 {
                    quantity: request.quantity,
                    destination_owner: context.actor,
                    destination_generation: request.user_generation,
                    destination_replay_sequence: request.user_replay_sequence,
                    vault_generation: request.vault_generation,
                    vault_replay_sequence: request.vault_replay_sequence,
                },
            )?;
            let mut builder = StepBuilder::new();
            builder.push(burn_step(
                context,
                plan.wrapper_quantity,
                plan.mint_supply,
                plan.holder_wrapper_atoms,
            )?)?;
            builder.push(ExecutionStepV1::Base(BaseCpiV1::RedeemTerminal(plan)))?;
            Ok(builder.finish(
                StructuredClaimActionV1::RedeemTerminal,
                PreparedStructuredClaimSemanticV1::RedeemTerminal(plan),
            ))
        }
        StructuredClaimPayloadV1::RetireDescriptor(request) => {
            require_vault_only(context, request.wrapper_product_id)?;
            let retirement = context
                .vault_retirement
                .ok_or(Error::BaseCapabilityUnavailable)?
                .evidence();
            let plan = prepare_retire_descriptor_v1(
                *context.descriptor.descriptor(),
                context.descriptor.identity(),
                &market,
                addresses,
                mint,
                vault,
                VaultMutationRequestV1 {
                    vault_generation: request.vault_generation,
                    vault_replay_sequence: request.vault_replay_sequence,
                },
                retirement,
            )?;
            let mut builder = StepBuilder::new();
            builder.push(ExecutionStepV1::Base(BaseCpiV1::CloseVault(retirement)))?;
            builder.push(ExecutionStepV1::Token2022(
                Token2022CpiV1::RevokeMintAuthority {
                    mint: addresses.mint,
                    authority_before: plan.mint_authority_before,
                    authority_after: plan.mint_authority_after,
                },
            ))?;
            builder.push(ExecutionStepV1::Descriptor(DescriptorWriteV1 {
                address: addresses.descriptor,
                before: Some(*context.descriptor.descriptor()),
                after: plan.descriptor,
            }))?;
            Ok(builder.finish(
                StructuredClaimActionV1::RetireDescriptor,
                PreparedStructuredClaimSemanticV1::RetireDescriptor(plan),
            ))
        }
    }
}

fn stage_wrap(
    context: &MutationContextV1<'_>,
    plan: WrapperTransitionPlanV1,
    unwind: bool,
) -> Result<PreparedStructuredClaimRouteV1> {
    let custody = canonical_custody_cpi(context, plan, unwind)?;
    let mut builder = StepBuilder::new();
    if unwind {
        builder.push(burn_step(
            context,
            plan.wrapper_quantity,
            plan.mint_supply,
            plan.holder_wrapper_atoms,
        )?)?;
        builder.push(ExecutionStepV1::Base(BaseCpiV1::CanonicalUnwrap(custody)))?;
        Ok(builder.finish(
            StructuredClaimActionV1::UnwrapCanonical,
            PreparedStructuredClaimSemanticV1::UnwrapCanonical(plan),
        ))
    } else {
        builder.push(ExecutionStepV1::Base(BaseCpiV1::CanonicalWrap(custody)))?;
        builder.push(mint_step(
            context,
            plan.wrapper_quantity,
            plan.mint_supply,
            plan.holder_wrapper_atoms,
        )?)?;
        Ok(builder.finish(
            StructuredClaimActionV1::WrapCanonical,
            PreparedStructuredClaimSemanticV1::WrapCanonical(plan),
        ))
    }
}

fn canonical_custody_cpi(
    context: &MutationContextV1<'_>,
    plan: WrapperTransitionPlanV1,
    unwind: bool,
) -> Result<BasePositionTransferCpiV1> {
    let custody = context
        .canonical_custody
        .ok_or(Error::CustodyAuthorityMismatch)?;
    if context.vault_retirement.is_some() {
        return Err(Error::InvalidAccounts);
    }
    let user = context.user.ok_or(Error::BaseClosureMismatch)?;
    let transfer = custody.transfer();
    let poststate = custody.poststate();
    let (action, source, destination, source_after, destination_after) = if unwind {
        (
            StructuredClaimActionV1::UnwrapCanonical,
            context.vault,
            user,
            plan.vault_position,
            plan.user_position,
        )
    } else {
        (
            StructuredClaimActionV1::WrapCanonical,
            user,
            context.vault,
            plan.user_position,
            plan.vault_position,
        )
    };
    let source_projection = source.projection();
    let destination_projection = destination.projection();
    if custody.local_action() != action
        || transfer.authority_kind
            != crate::runtime_contract::PositionAssetTransferAuthorityKindV1::StructuredCustody
        || transfer.phase_policy
            != crate::runtime_contract::AssetTransferPhasePolicyV1::ActiveOrResolved
        || custody.authority_id() != transfer.authority_id
        || custody.cpi().program_id != context.descriptor.identity().deployment.base_program
        || transfer.market != context.descriptor.descriptor().market
        || transfer.source_owner != source_projection.owner
        || transfer.destination_owner != destination_projection.owner
        || transfer.source_generation != source_projection.generation
        || transfer.destination_generation != destination_projection.generation
        || transfer.source_replay_sequence != source_projection.replay_sequence
        || transfer.destination_replay_sequence != destination_projection.replay_sequence
        || transfer.cash_atoms != plan.backing_cash_atoms
        || transfer.internal != plan.backing_internal
        || custody.source_after() != source_after
        || custody.destination_after() != destination_after
        || poststate.source_position.address != source.position_address()
        || poststate.source_replay.address != source.replay_address()
        || poststate.destination_position.address != destination.position_address()
        || poststate.destination_replay.address != destination.replay_address()
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    Ok(custody.cpi())
}

fn require_no_canonical_custody(context: &MutationContextV1<'_>) -> Result<()> {
    if context.canonical_custody.is_some() {
        return Err(Error::InvalidAccounts);
    }
    Ok(())
}

fn stage_market_wrap(
    context: &MutationContextV1<'_>,
    plan: MarketChangingWrapperTransitionPlanV1,
) -> Result<PreparedStructuredClaimRouteV1> {
    let mut builder = StepBuilder::new();
    builder.push(ExecutionStepV1::Base(BaseCpiV1::FullWrap(plan)))?;
    builder.push(mint_step(
        context,
        plan.wrapper_quantity,
        plan.mint_supply,
        plan.holder_wrapper_atoms,
    )?)?;
    Ok(builder.finish(
        StructuredClaimActionV1::WrapFull,
        PreparedStructuredClaimSemanticV1::WrapFull(plan),
    ))
}

fn stage_market_unwrap(
    context: &MutationContextV1<'_>,
    plan: MarketChangingWrapperTransitionPlanV1,
) -> Result<PreparedStructuredClaimRouteV1> {
    let mut builder = StepBuilder::new();
    builder.push(burn_step(
        context,
        plan.wrapper_quantity,
        plan.mint_supply,
        plan.holder_wrapper_atoms,
    )?)?;
    builder.push(ExecutionStepV1::Base(BaseCpiV1::FullUnwrap(plan)))?;
    Ok(builder.finish(
        StructuredClaimActionV1::UnwrapFull,
        PreparedStructuredClaimSemanticV1::UnwrapFull(plan),
    ))
}

fn mint_step(
    context: &MutationContextV1<'_>,
    quantity: u64,
    supply_after: u64,
    holder_after: u64,
) -> Result<ExecutionStepV1> {
    let holder = context.holder.ok_or(Error::Token2022Boundary)?.projection();
    Ok(ExecutionStepV1::Token2022(Token2022CpiV1::MintChecked {
        mint: context.descriptor.addresses().mint,
        token: holder.address,
        authority: context.descriptor.addresses().mint_authority,
        quantity,
        supply_before: context.mint.projection().supply,
        supply_after,
        holder_before: holder.amount,
        holder_after,
    }))
}

fn burn_step(
    context: &MutationContextV1<'_>,
    quantity: u64,
    supply_after: u64,
    holder_after: u64,
) -> Result<ExecutionStepV1> {
    let holder = context.holder.ok_or(Error::Token2022Boundary)?.projection();
    Ok(ExecutionStepV1::Token2022(Token2022CpiV1::BurnChecked {
        mint: context.descriptor.addresses().mint,
        token: holder.address,
        authority: context.actor,
        quantity,
        supply_before: context.mint.projection().supply,
        supply_after,
        holder_before: holder.amount,
        holder_after,
    }))
}

fn quantity_accounts(
    context: &MutationContextV1<'_>,
) -> Result<(
    crate::runtime_contract::WrapperTokenProjectionV1,
    crate::runtime_contract::PositionProjectionV1,
)> {
    let holder = context.holder.ok_or(Error::Token2022Boundary)?.projection();
    let user = context.user.ok_or(Error::BaseClosureMismatch)?.projection();
    if context.vault_retirement.is_some() {
        return Err(Error::InvalidAccounts);
    }
    Ok((holder, user))
}

fn require_quantity_actor(
    actor: Key,
    holder: crate::runtime_contract::WrapperTokenProjectionV1,
    user: crate::runtime_contract::PositionProjectionV1,
) -> Result<()> {
    if actor != holder.owner || actor != user.owner {
        return Err(Error::InvalidAccounts);
    }
    Ok(())
}

fn require_product(context: &MutationContextV1<'_>, product: Key) -> Result<()> {
    if product != context.descriptor.wrapper_product_id() {
        return Err(Error::DigestMismatch);
    }
    Ok(())
}

fn require_vault_only(context: &MutationContextV1<'_>, product: Key) -> Result<()> {
    require_product(context, product)?;
    if context.user.is_some() || context.holder.is_some() || context.canonical_custody.is_some() {
        return Err(Error::InvalidAccounts);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct StepBuilder {
    count: u8,
    steps: [ExecutionStepV1; MAX_EXECUTION_STEPS],
}

impl StepBuilder {
    const fn new() -> Self {
        Self {
            count: 0,
            steps: [ExecutionStepV1::None; MAX_EXECUTION_STEPS],
        }
    }

    fn push(&mut self, step: ExecutionStepV1) -> Result<()> {
        if step == ExecutionStepV1::None {
            return Err(Error::PostStateMismatch);
        }
        let slot = self
            .steps
            .get_mut(usize::from(self.count))
            .ok_or(Error::Arithmetic)?;
        *slot = step;
        self.count = self.count.checked_add(1).ok_or(Error::Arithmetic)?;
        Ok(())
    }

    const fn finish(
        self,
        action: StructuredClaimActionV1,
        semantic: PreparedStructuredClaimSemanticV1,
    ) -> PreparedStructuredClaimRouteV1 {
        PreparedStructuredClaimRouteV1 {
            action,
            semantic,
            step_count: self.count,
            steps: self.steps,
        }
    }
}
