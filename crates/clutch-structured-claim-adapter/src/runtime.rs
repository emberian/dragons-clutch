//! Supply-sensitive canonical wrap and unwind orchestration contracts.

use clutch_structured_claim::{
    BackingVault, DonationDelta, HolderAssets, MarketLedger, StructuredClaimMachine, WrapperState,
};

use crate::{
    prepare_atomic_position_asset_transfer_v1, Amount, AssetTransferPhasePolicyV1,
    AtomicPositionAssetTransferRequestV1, DescriptorIdentityV1, DescriptorStateV1, Error,
    PositionProjectionV1, Result, StructuredClaimDescriptorV1,
};

/// Canonical addresses derived by the SBF adapter from wrapper product identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct StructuredClaimRuntimeAddressesV1 {
    /// Immutable descriptor account.
    pub descriptor: [u8; 32],
    /// Extension-free Token-2022 wrapper mint.
    pub mint: [u8; 32],
    /// PDA holding mint authority.
    pub mint_authority: [u8; 32],
    /// Semantic owner of the base Position holding canonical backing.
    pub vault_owner: [u8; 32],
}

impl StructuredClaimRuntimeAddressesV1 {
    pub(crate) fn validate(&self) -> Result<()> {
        let keys = [
            self.descriptor,
            self.mint,
            self.mint_authority,
            self.vault_owner,
        ];
        let mut left = 0_usize;
        while left < keys.len() {
            if keys[left] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            let mut right = left + 1;
            while right < keys.len() {
                if keys[left] == keys[right] {
                    return Err(Error::InvalidIdentity);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }
}

/// Authenticated extension-free Token-2022 wrapper-mint projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct WrapperMintProjectionV1 {
    /// Canonical mint account.
    pub address: [u8; 32],
    /// Canonical wrapper authority PDA.
    pub mint_authority: [u8; 32],
    /// Actual Token-2022 supply.
    pub supply: Amount,
    /// Must be zero for indivisible wrapper atoms.
    pub decimals: u8,
    /// Must be absent, encoded as zero.
    pub freeze_authority: [u8; 32],
    /// Must be zero: no mint extension is admitted by version one.
    pub extension_mask: u64,
    /// Initialized mint bit from the canonical Token-2022 parser.
    pub initialized: bool,
}

impl WrapperMintProjectionV1 {
    fn validate(&self, addresses: &StructuredClaimRuntimeAddressesV1) -> Result<()> {
        if !self.initialized
            || self.address != addresses.mint
            || self.mint_authority != addresses.mint_authority
            || self.decimals != 0
            || self.freeze_authority != [0; 32]
            || self.extension_mask != 0
        {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }
}

/// Authenticated wrapper-token account participating in one route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct WrapperTokenProjectionV1 {
    /// Token account address.
    pub address: [u8; 32],
    /// Canonical wrapper mint.
    pub mint: [u8; 32],
    /// Bearer authority for the current instruction.
    pub owner: [u8; 32],
    /// Actual token amount.
    pub amount: Amount,
    /// Initialized account bit from the canonical Token-2022 parser.
    pub initialized: bool,
}

impl WrapperTokenProjectionV1 {
    fn validate(&self, addresses: &StructuredClaimRuntimeAddressesV1) -> Result<()> {
        if !self.initialized
            || self.address == [0; 32]
            || self.owner == [0; 32]
            || self.mint != addresses.mint
            || self.address == self.owner
            || self.address == addresses.mint
        {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }
}

/// Canonical-backing wrapper mint request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct CanonicalWrapRequestV1 {
    /// Wrapper atoms to mint.
    pub quantity: Amount,
    /// Expected source semantic owner.
    pub source_owner: [u8; 32],
    /// Exact source generation.
    pub source_generation: u64,
    /// Exact source Replay sequence.
    pub source_replay_sequence: u64,
    /// Exact vault generation.
    pub vault_generation: u64,
    /// Exact vault Replay sequence.
    pub vault_replay_sequence: u64,
}

/// Canonical-backing wrapper burn/unwind request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct CanonicalUnwrapRequestV1 {
    /// Wrapper atoms to burn.
    pub quantity: Amount,
    /// Expected destination semantic owner.
    pub destination_owner: [u8; 32],
    /// Exact destination generation.
    pub destination_generation: u64,
    /// Exact destination Replay sequence.
    pub destination_replay_sequence: u64,
    /// Exact vault generation.
    pub vault_generation: u64,
    /// Exact vault Replay sequence.
    pub vault_replay_sequence: u64,
}

/// Complete prospective state and exact CPI deltas for canonical wrap/unwind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct WrapperTransitionPlanV1 {
    /// Prospective user Position/Replay projection.
    pub user_position: PositionProjectionV1,
    /// Prospective vault Position/Replay projection.
    pub vault_position: PositionProjectionV1,
    /// Prospective actual wrapper-mint supply.
    pub mint_supply: Amount,
    /// Prospective participating wrapper-token balance.
    pub holder_wrapper_atoms: Amount,
    /// Exact wrapper quantity passed to MintToChecked or BurnChecked.
    pub wrapper_quantity: Amount,
    /// Exact canonical backing cash moved by the base CPI.
    pub backing_cash_atoms: Amount,
    /// Exact canonical backing Eggs moved by the base CPI.
    pub backing_internal: [Amount; crate::MAX_OUTCOMES],
}

/// Complete prospective state for a route that also changes base Market
/// supply or Hoard collateral through an exact base-kernel transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct MarketChangingWrapperTransitionPlanV1 {
    /// Prospective authoritative base Market ledger.
    pub market: MarketLedger,
    /// Prospective user Position/Replay projection.
    pub user_position: PositionProjectionV1,
    /// Prospective vault Position/Replay projection.
    pub vault_position: PositionProjectionV1,
    /// Prospective actual wrapper-mint supply.
    pub mint_supply: Amount,
    /// Prospective participating wrapper-token balance.
    pub holder_wrapper_atoms: Amount,
    /// Exact wrapper quantity passed to MintToChecked or BurnChecked.
    pub wrapper_quantity: Amount,
    /// Full native-Egg vector consumed or returned.
    pub full_internal: [Amount; crate::MAX_OUTCOMES],
    /// Hoard collateral before the base transition.
    pub hoard_before_atoms: Amount,
    /// Hoard collateral after the base transition.
    pub hoard_after_atoms: Amount,
}

/// Exact beneficiary-free surplus compaction poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DonationCompactionPlanV1 {
    /// Prospective authoritative base Market ledger.
    pub market: MarketLedger,
    /// Prospective wrapper-vault Position/Replay projection.
    pub vault_position: PositionProjectionV1,
    /// Exact cash and Egg donation deltas returned by the kernel.
    pub donation: DonationDelta,
    /// Actual wrapper supply, unchanged by compaction.
    pub mint_supply: Amount,
}

/// Exact resolved terminal-lot redemption poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct TerminalRedemptionPlanV1 {
    /// Prospective authoritative base Market ledger.
    pub market: MarketLedger,
    /// Prospective beneficiary Position/Replay projection.
    pub user_position: PositionProjectionV1,
    /// Prospective vault Position/Replay projection.
    pub vault_position: PositionProjectionV1,
    /// Prospective actual wrapper-mint supply.
    pub mint_supply: Amount,
    /// Prospective source wrapper-token balance.
    pub holder_wrapper_atoms: Amount,
    /// Exact wrapper quantity passed to BurnChecked.
    pub wrapper_quantity: Amount,
    /// Exact integral terminal payout credited to the Position.
    pub payout_cash_atoms: Amount,
    /// Hoard collateral before residual native redemption.
    pub hoard_before_atoms: Amount,
    /// Hoard collateral after residual native redemption.
    pub hoard_after_atoms: Amount,
}

/// Vault-only request for donation compaction or retirement preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VaultMutationRequestV1 {
    /// Exact vault generation.
    pub vault_generation: u64,
    /// Exact vault Replay sequence.
    pub vault_replay_sequence: u64,
}

/// Base-program Position retirement capability authenticated by the SBF
/// adapter from the exact successor close plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedVaultRetirementV1 {
    /// Content identity of the complete base close plan.
    pub close_receipt: [u8; 32],
    /// Market bound by that plan.
    pub market: [u8; 32],
    /// Wrapper vault's semantic Position owner.
    pub vault_owner: [u8; 32],
    /// Exact Position generation being closed.
    pub generation: u64,
    /// Exact Replay sequence consumed by close.
    pub replay_sequence: u64,
    /// Permanent base tombstone produced by close.
    pub tombstone: [u8; 32],
}

/// Permanent structured-claim retirement plan.
///
/// Descriptor and extension-free mint accounts remain permanent identity
/// tombstones. The mint authority is revoked; the empty base vault Position is
/// closed only through the authenticated base-program capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct DescriptorRetirementPlanV1 {
    /// Prospective permanent descriptor image.
    pub descriptor: StructuredClaimDescriptorV1,
    /// Actual mint supply, necessarily zero.
    pub mint_supply: Amount,
    /// Mint authority before Token-2022 SetAuthority.
    pub mint_authority_before: [u8; 32],
    /// Mint authority after revocation, encoded absent as zero.
    pub mint_authority_after: [u8; 32],
    /// Base close plan that must execute in the same atomic transaction.
    pub vault_close_receipt: [u8; 32],
    /// Permanent base Position tombstone.
    pub vault_tombstone: [u8; 32],
}

/// Prepare canonical backing transfer followed by wrapper minting.
pub fn prepare_wrap_canonical_v1(
    descriptor_state: DescriptorStateV1,
    identity: &DescriptorIdentityV1,
    market: &MarketLedger,
    addresses: StructuredClaimRuntimeAddressesV1,
    mint: WrapperMintProjectionV1,
    destination_token: WrapperTokenProjectionV1,
    source_position: PositionProjectionV1,
    vault_position: PositionProjectionV1,
    request: CanonicalWrapRequestV1,
) -> Result<WrapperTransitionPlanV1> {
    preflight_runtime(
        descriptor_state,
        identity,
        market,
        &addresses,
        &mint,
        &destination_token,
        &source_position,
        &vault_position,
    )?;
    if destination_token.owner != request.source_owner
        || source_position.owner != request.source_owner
        || source_position.generation != request.source_generation
        || source_position.replay_sequence != request.source_replay_sequence
        || vault_position.generation != request.vault_generation
        || vault_position.replay_sequence != request.vault_replay_sequence
    {
        return Err(Error::DifferentPositionDomain);
    }
    let mut machine = restore_machine(identity, market, mint.supply, vault_position)?;
    let source_free_cash = source_position.free_cash_atoms()?;
    let mut holder = HolderAssets {
        cash_atoms: source_free_cash,
        internal: source_position.internal,
        wrapper_atoms: destination_token.amount,
    };
    machine
        .wrap_canonical(market, &mut holder, request.quantity)
        .map_err(|_| Error::EconomicTransitionRefused)?;
    let backing_cash_atoms = request
        .quantity
        .checked_mul(identity.backing.cash_per_wrapper)
        .ok_or(Error::ArithmeticOverflow)?;
    let backing_internal = scaled_residual(identity, request.quantity)?;
    let transfer = prepare_atomic_position_asset_transfer_v1(
        identity.claim.basis.outcome_count,
        market.phase(),
        source_position,
        vault_position,
        AtomicPositionAssetTransferRequestV1 {
            market: identity.claim.basis.market,
            source_owner: request.source_owner,
            destination_owner: addresses.vault_owner,
            source_generation: request.source_generation,
            destination_generation: request.vault_generation,
            source_replay_sequence: request.source_replay_sequence,
            destination_replay_sequence: request.vault_replay_sequence,
            cash_atoms: backing_cash_atoms,
            internal: backing_internal,
            phase_policy: AssetTransferPhasePolicyV1::ActiveOrResolved,
        },
    )?;
    if transfer.source.free_cash_atoms()? != holder.cash_atoms
        || transfer.source.internal != holder.internal
        || transfer.destination.cash_atoms != machine.vault.cash_atoms
        || transfer.destination.internal != machine.vault.internal
        || machine.wrapper.actual_supply
            != mint
                .supply
                .checked_add(request.quantity)
                .ok_or(Error::ArithmeticOverflow)?
        || holder.wrapper_atoms
            != destination_token
                .amount
                .checked_add(request.quantity)
                .ok_or(Error::ArithmeticOverflow)?
    {
        return Err(Error::InvariantViolation);
    }
    Ok(WrapperTransitionPlanV1 {
        user_position: transfer.source,
        vault_position: transfer.destination,
        mint_supply: machine.wrapper.actual_supply,
        holder_wrapper_atoms: holder.wrapper_atoms,
        wrapper_quantity: request.quantity,
        backing_cash_atoms,
        backing_internal,
    })
}

/// Prepare wrapper burning followed by canonical backing return.
pub fn prepare_unwrap_canonical_v1(
    descriptor_state: DescriptorStateV1,
    identity: &DescriptorIdentityV1,
    market: &MarketLedger,
    addresses: StructuredClaimRuntimeAddressesV1,
    mint: WrapperMintProjectionV1,
    source_token: WrapperTokenProjectionV1,
    destination_position: PositionProjectionV1,
    vault_position: PositionProjectionV1,
    request: CanonicalUnwrapRequestV1,
) -> Result<WrapperTransitionPlanV1> {
    preflight_runtime(
        descriptor_state,
        identity,
        market,
        &addresses,
        &mint,
        &source_token,
        &destination_position,
        &vault_position,
    )?;
    if source_token.owner != request.destination_owner
        || destination_position.owner != request.destination_owner
        || destination_position.generation != request.destination_generation
        || destination_position.replay_sequence != request.destination_replay_sequence
        || vault_position.generation != request.vault_generation
        || vault_position.replay_sequence != request.vault_replay_sequence
    {
        return Err(Error::DifferentPositionDomain);
    }
    let mut machine = restore_machine(identity, market, mint.supply, vault_position)?;
    let destination_free_cash = destination_position.free_cash_atoms()?;
    let mut holder = HolderAssets {
        cash_atoms: destination_free_cash,
        internal: destination_position.internal,
        wrapper_atoms: source_token.amount,
    };
    machine
        .unwind_canonical(market, &mut holder, request.quantity)
        .map_err(|_| Error::EconomicTransitionRefused)?;
    let backing_cash_atoms = request
        .quantity
        .checked_mul(identity.backing.cash_per_wrapper)
        .ok_or(Error::ArithmeticOverflow)?;
    let backing_internal = scaled_residual(identity, request.quantity)?;
    let transfer = prepare_atomic_position_asset_transfer_v1(
        identity.claim.basis.outcome_count,
        market.phase(),
        vault_position,
        destination_position,
        AtomicPositionAssetTransferRequestV1 {
            market: identity.claim.basis.market,
            source_owner: addresses.vault_owner,
            destination_owner: request.destination_owner,
            source_generation: request.vault_generation,
            destination_generation: request.destination_generation,
            source_replay_sequence: request.vault_replay_sequence,
            destination_replay_sequence: request.destination_replay_sequence,
            cash_atoms: backing_cash_atoms,
            internal: backing_internal,
            phase_policy: AssetTransferPhasePolicyV1::ActiveOrResolved,
        },
    )?;
    if transfer.destination.free_cash_atoms()? != holder.cash_atoms
        || transfer.destination.internal != holder.internal
        || transfer.source.cash_atoms != machine.vault.cash_atoms
        || transfer.source.internal != machine.vault.internal
        || machine.wrapper.actual_supply
            != mint
                .supply
                .checked_sub(request.quantity)
                .ok_or(Error::ArithmeticUnderflow)?
        || holder.wrapper_atoms
            != source_token
                .amount
                .checked_sub(request.quantity)
                .ok_or(Error::ArithmeticUnderflow)?
    {
        return Err(Error::InvariantViolation);
    }
    Ok(WrapperTransitionPlanV1 {
        user_position: transfer.destination,
        vault_position: transfer.source,
        mint_supply: machine.wrapper.actual_supply,
        holder_wrapper_atoms: holder.wrapper_atoms,
        wrapper_quantity: request.quantity,
        backing_cash_atoms,
        backing_internal,
    })
}

/// Prepare full-vector custody, base complete-set merge, and wrapper minting.
pub fn prepare_wrap_full_v1(
    descriptor_state: DescriptorStateV1,
    identity: &DescriptorIdentityV1,
    market: MarketLedger,
    addresses: StructuredClaimRuntimeAddressesV1,
    mint: WrapperMintProjectionV1,
    destination_token: WrapperTokenProjectionV1,
    source_position: PositionProjectionV1,
    vault_position: PositionProjectionV1,
    request: CanonicalWrapRequestV1,
) -> Result<MarketChangingWrapperTransitionPlanV1> {
    preflight_runtime(
        descriptor_state,
        identity,
        &market,
        &addresses,
        &mint,
        &destination_token,
        &source_position,
        &vault_position,
    )?;
    if destination_token.owner != request.source_owner
        || source_position.owner != request.source_owner
        || source_position.generation != request.source_generation
        || source_position.replay_sequence != request.source_replay_sequence
        || vault_position.generation != request.vault_generation
        || vault_position.replay_sequence != request.vault_replay_sequence
    {
        return Err(Error::DifferentPositionDomain);
    }
    let hoard_before_atoms = market.hoard_atoms();
    let mut next_market = market;
    let mut machine = restore_machine(identity, &next_market, mint.supply, vault_position)?;
    let mut holder = HolderAssets {
        cash_atoms: source_position.free_cash_atoms()?,
        internal: source_position.internal,
        wrapper_atoms: destination_token.amount,
    };
    machine
        .wrap_full(&mut next_market, &mut holder, request.quantity)
        .map_err(|_| Error::EconomicTransitionRefused)?;
    let (user_position, vault_position) = stage_market_changing_positions(
        identity.claim.basis.outcome_count,
        source_position,
        vault_position,
        holder,
        &machine,
    )?;
    let expected_supply = mint
        .supply
        .checked_add(request.quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    let expected_holder = destination_token
        .amount
        .checked_add(request.quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    if machine.wrapper.actual_supply != expected_supply
        || holder.wrapper_atoms != expected_holder
        || user_position.free_cash_atoms()? != source_position.free_cash_atoms()?
    {
        return Err(Error::InvariantViolation);
    }
    Ok(MarketChangingWrapperTransitionPlanV1 {
        market: next_market,
        user_position,
        vault_position,
        mint_supply: expected_supply,
        holder_wrapper_atoms: expected_holder,
        wrapper_quantity: request.quantity,
        full_internal: scaled_full(identity, request.quantity)?,
        hoard_before_atoms,
        hoard_after_atoms: next_market.hoard_atoms(),
    })
}

/// Prepare wrapper burning, base complete-set split, and full-vector return.
pub fn prepare_unwrap_full_v1(
    descriptor_state: DescriptorStateV1,
    identity: &DescriptorIdentityV1,
    market: MarketLedger,
    addresses: StructuredClaimRuntimeAddressesV1,
    mint: WrapperMintProjectionV1,
    source_token: WrapperTokenProjectionV1,
    destination_position: PositionProjectionV1,
    vault_position: PositionProjectionV1,
    request: CanonicalUnwrapRequestV1,
) -> Result<MarketChangingWrapperTransitionPlanV1> {
    preflight_runtime(
        descriptor_state,
        identity,
        &market,
        &addresses,
        &mint,
        &source_token,
        &destination_position,
        &vault_position,
    )?;
    if source_token.owner != request.destination_owner
        || destination_position.owner != request.destination_owner
        || destination_position.generation != request.destination_generation
        || destination_position.replay_sequence != request.destination_replay_sequence
        || vault_position.generation != request.vault_generation
        || vault_position.replay_sequence != request.vault_replay_sequence
    {
        return Err(Error::DifferentPositionDomain);
    }
    let hoard_before_atoms = market.hoard_atoms();
    let mut next_market = market;
    let mut machine = restore_machine(identity, &next_market, mint.supply, vault_position)?;
    let mut holder = HolderAssets {
        cash_atoms: destination_position.free_cash_atoms()?,
        internal: destination_position.internal,
        wrapper_atoms: source_token.amount,
    };
    machine
        .unwind_full(&mut next_market, &mut holder, request.quantity)
        .map_err(|_| Error::EconomicTransitionRefused)?;
    let (user_position, vault_position) = stage_market_changing_positions(
        identity.claim.basis.outcome_count,
        destination_position,
        vault_position,
        holder,
        &machine,
    )?;
    let expected_supply = mint
        .supply
        .checked_sub(request.quantity)
        .ok_or(Error::ArithmeticUnderflow)?;
    let expected_holder = source_token
        .amount
        .checked_sub(request.quantity)
        .ok_or(Error::ArithmeticUnderflow)?;
    if machine.wrapper.actual_supply != expected_supply
        || holder.wrapper_atoms != expected_holder
        || user_position.free_cash_atoms()? != destination_position.free_cash_atoms()?
    {
        return Err(Error::InvariantViolation);
    }
    Ok(MarketChangingWrapperTransitionPlanV1 {
        market: next_market,
        user_position,
        vault_position,
        mint_supply: expected_supply,
        holder_wrapper_atoms: expected_holder,
        wrapper_quantity: request.quantity,
        full_internal: scaled_full(identity, request.quantity)?,
        hoard_before_atoms,
        hoard_after_atoms: next_market.hoard_atoms(),
    })
}

/// Prepare beneficiary-free compaction of every backing atom above the exact
/// current-supply requirement.
pub fn prepare_compact_donation_v1(
    descriptor_state: DescriptorStateV1,
    identity: &DescriptorIdentityV1,
    market: MarketLedger,
    addresses: StructuredClaimRuntimeAddressesV1,
    mint: WrapperMintProjectionV1,
    vault_position: PositionProjectionV1,
    request: VaultMutationRequestV1,
) -> Result<DonationCompactionPlanV1> {
    if descriptor_state != DescriptorStateV1::Active {
        return Err(Error::InvalidState);
    }
    addresses.validate()?;
    mint.validate(&addresses)?;
    if vault_position.market != identity.claim.basis.market
        || vault_position.owner != addresses.vault_owner
        || vault_position.generation != request.vault_generation
        || vault_position.replay_sequence != request.vault_replay_sequence
        || vault_position.reserved_cash_atoms != 0
        || identity.claim.basis != market.basis
    {
        return Err(Error::DifferentPositionDomain);
    }
    vault_position.validate(identity.claim.basis.outcome_count)?;
    let mut next_market = market;
    let mut machine = restore_machine(identity, &next_market, mint.supply, vault_position)?;
    let donation = machine
        .compact_donation(&mut next_market)
        .map_err(|_| Error::EconomicTransitionRefused)?;
    let mut any = donation.cash_to_hoard != 0;
    let mut index = 0_usize;
    while index < crate::MAX_OUTCOMES {
        any |= donation.eggs_destroyed[index] != 0;
        index += 1;
    }
    if !any {
        return Err(Error::ZeroQuantity);
    }
    let mut next_vault = vault_position;
    next_vault.cash_atoms = machine.vault.cash_atoms;
    next_vault.internal = machine.vault.internal;
    next_vault.replay_sequence = next_vault
        .replay_sequence
        .checked_add(1)
        .ok_or(Error::ReplayExhausted)?;
    next_vault.validate(identity.claim.basis.outcome_count)?;
    Ok(DonationCompactionPlanV1 {
        market: next_market,
        vault_position: next_vault,
        donation,
        mint_supply: mint.supply,
    })
}

/// Prepare an exact resolved terminal-lot burn and aggregate cash redemption.
pub fn prepare_redeem_terminal_v1(
    descriptor_state: DescriptorStateV1,
    identity: &DescriptorIdentityV1,
    market: MarketLedger,
    addresses: StructuredClaimRuntimeAddressesV1,
    mint: WrapperMintProjectionV1,
    source_token: WrapperTokenProjectionV1,
    destination_position: PositionProjectionV1,
    vault_position: PositionProjectionV1,
    request: CanonicalUnwrapRequestV1,
) -> Result<TerminalRedemptionPlanV1> {
    preflight_runtime(
        descriptor_state,
        identity,
        &market,
        &addresses,
        &mint,
        &source_token,
        &destination_position,
        &vault_position,
    )?;
    if source_token.owner != request.destination_owner
        || destination_position.owner != request.destination_owner
        || destination_position.generation != request.destination_generation
        || destination_position.replay_sequence != request.destination_replay_sequence
        || vault_position.generation != request.vault_generation
        || vault_position.replay_sequence != request.vault_replay_sequence
    {
        return Err(Error::DifferentPositionDomain);
    }
    let hoard_before_atoms = market.hoard_atoms();
    let mut next_market = market;
    let mut machine = restore_machine(identity, &next_market, mint.supply, vault_position)?;
    let mut holder = HolderAssets {
        cash_atoms: destination_position.free_cash_atoms()?,
        internal: destination_position.internal,
        wrapper_atoms: source_token.amount,
    };
    let payout_cash_atoms = machine
        .redeem_terminal(&mut next_market, &mut holder, request.quantity)
        .map_err(|_| Error::EconomicTransitionRefused)?;
    let (user_position, vault_position) = stage_market_changing_positions(
        identity.claim.basis.outcome_count,
        destination_position,
        vault_position,
        holder,
        &machine,
    )?;
    let expected_supply = mint
        .supply
        .checked_sub(request.quantity)
        .ok_or(Error::ArithmeticUnderflow)?;
    let expected_holder = source_token
        .amount
        .checked_sub(request.quantity)
        .ok_or(Error::ArithmeticUnderflow)?;
    if machine.wrapper.actual_supply != expected_supply
        || holder.wrapper_atoms != expected_holder
        || user_position.free_cash_atoms()?
            != destination_position
                .free_cash_atoms()?
                .checked_add(payout_cash_atoms)
                .ok_or(Error::ArithmeticOverflow)?
    {
        return Err(Error::InvariantViolation);
    }
    Ok(TerminalRedemptionPlanV1 {
        market: next_market,
        user_position,
        vault_position,
        mint_supply: expected_supply,
        holder_wrapper_atoms: expected_holder,
        wrapper_quantity: request.quantity,
        payout_cash_atoms,
        hoard_before_atoms,
        hoard_after_atoms: next_market.hoard_atoms(),
    })
}

/// Prepare permanent zero-supply retirement and mint-authority revocation.
pub fn prepare_retire_descriptor_v1(
    mut descriptor: StructuredClaimDescriptorV1,
    identity: &DescriptorIdentityV1,
    market: &MarketLedger,
    addresses: StructuredClaimRuntimeAddressesV1,
    mint: WrapperMintProjectionV1,
    vault_position: PositionProjectionV1,
    request: VaultMutationRequestV1,
    vault_retirement: AuthenticatedVaultRetirementV1,
) -> Result<DescriptorRetirementPlanV1> {
    descriptor.validate_persisted()?;
    if descriptor.state != DescriptorStateV1::Active {
        return Err(Error::InvalidState);
    }
    addresses.validate()?;
    mint.validate(&addresses)?;
    if descriptor.market != identity.claim.basis.market
        || descriptor.terms_digest != identity.claim.basis.terms
        || descriptor.primitive != identity.claim.vector.coefficients
        || descriptor.base_program != identity.deployment.base_program
        || descriptor.base_program_data != identity.deployment.base_program_data
        || descriptor.base_deployment_slot != identity.deployment.base_deployment_slot
        || descriptor.wrapper_program_data != identity.deployment.wrapper_program_data
        || descriptor.wrapper_deployment_slot != identity.deployment.wrapper_deployment_slot
        || descriptor.token_2022_program != identity.deployment.token_2022_program
        || descriptor.token_2022_program_data != identity.deployment.token_2022_program_data
        || descriptor.token_2022_deployment_slot != identity.deployment.token_2022_deployment_slot
        || identity.claim.basis != market.basis
        || vault_position.market != identity.claim.basis.market
        || vault_position.owner != addresses.vault_owner
        || vault_position.generation != request.vault_generation
        || vault_position.replay_sequence != request.vault_replay_sequence
        || vault_position.reserved_cash_atoms != 0
        || vault_position.cash_atoms != 0
        || vault_position.internal != [0; crate::MAX_OUTCOMES]
        || vault_retirement.close_receipt == [0; 32]
        || vault_retirement.tombstone == [0; 32]
        || vault_retirement.close_receipt == vault_retirement.tombstone
        || vault_retirement.market != identity.claim.basis.market
        || vault_retirement.vault_owner != addresses.vault_owner
        || vault_retirement.generation != request.vault_generation
        || vault_retirement.replay_sequence != request.vault_replay_sequence
    {
        return Err(Error::AuthorityUnavailable);
    }
    vault_position.validate(identity.claim.basis.outcome_count)?;
    let mut machine = restore_machine(identity, market, mint.supply, vault_position)?;
    machine
        .retire(market)
        .map_err(|_| Error::EconomicTransitionRefused)?;
    if machine.wrapper.actual_supply != 0
        || !machine.wrapper.retired
        || machine.vault != BackingVault::EMPTY
    {
        return Err(Error::InvariantViolation);
    }
    descriptor.state = DescriptorStateV1::Retired;
    descriptor.validate_persisted()?;
    Ok(DescriptorRetirementPlanV1 {
        descriptor,
        mint_supply: 0,
        mint_authority_before: addresses.mint_authority,
        mint_authority_after: [0; 32],
        vault_close_receipt: vault_retirement.close_receipt,
        vault_tombstone: vault_retirement.tombstone,
    })
}

fn preflight_runtime(
    descriptor_state: DescriptorStateV1,
    identity: &DescriptorIdentityV1,
    market: &MarketLedger,
    addresses: &StructuredClaimRuntimeAddressesV1,
    mint: &WrapperMintProjectionV1,
    token: &WrapperTokenProjectionV1,
    user_position: &PositionProjectionV1,
    vault_position: &PositionProjectionV1,
) -> Result<()> {
    if descriptor_state != DescriptorStateV1::Active {
        return Err(Error::InvalidState);
    }
    addresses.validate()?;
    mint.validate(addresses)?;
    token.validate(addresses)?;
    if user_position.market != identity.claim.basis.market
        || vault_position.market != identity.claim.basis.market
        || user_position.owner == addresses.vault_owner
        || vault_position.owner != addresses.vault_owner
        || vault_position.reserved_cash_atoms != 0
        || identity.claim.basis != market.basis
    {
        return Err(Error::DifferentPositionDomain);
    }
    Ok(())
}

fn restore_machine(
    identity: &DescriptorIdentityV1,
    market: &MarketLedger,
    actual_supply: Amount,
    vault_position: PositionProjectionV1,
) -> Result<StructuredClaimMachine> {
    StructuredClaimMachine::restore(
        identity.claim,
        WrapperState {
            actual_supply,
            retired: false,
        },
        BackingVault {
            cash_atoms: vault_position.cash_atoms,
            internal: vault_position.internal,
        },
        market,
    )
    .map_err(|_| Error::EconomicTransitionRefused)
}

fn scaled_residual(
    identity: &DescriptorIdentityV1,
    quantity: Amount,
) -> Result<[Amount; crate::MAX_OUTCOMES]> {
    let mut output = [0_u64; crate::MAX_OUTCOMES];
    let width = usize::from(identity.claim.basis.outcome_count);
    let mut index = 0_usize;
    while index < width {
        output[index] = quantity
            .checked_mul(identity.backing.residual_eggs_per_wrapper[index])
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    Ok(output)
}

fn scaled_full(
    identity: &DescriptorIdentityV1,
    quantity: Amount,
) -> Result<[Amount; crate::MAX_OUTCOMES]> {
    let mut output = [0_u64; crate::MAX_OUTCOMES];
    let width = usize::from(identity.claim.basis.outcome_count);
    let mut index = 0_usize;
    while index < width {
        output[index] = quantity
            .checked_mul(identity.claim.vector.coefficients[index])
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    Ok(output)
}

fn stage_market_changing_positions(
    outcome_count: u8,
    user_before: PositionProjectionV1,
    vault_before: PositionProjectionV1,
    holder_after: HolderAssets,
    machine_after: &StructuredClaimMachine,
) -> Result<(PositionProjectionV1, PositionProjectionV1)> {
    user_before.validate(outcome_count)?;
    vault_before.validate(outcome_count)?;
    if user_before.market != vault_before.market
        || user_before.owner == vault_before.owner
        || vault_before.reserved_cash_atoms != 0
    {
        return Err(Error::DifferentPositionDomain);
    }
    let mut user_after = user_before;
    user_after.cash_atoms = holder_after
        .cash_atoms
        .checked_add(user_before.reserved_cash_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    user_after.internal = holder_after.internal;
    user_after.replay_sequence = user_after
        .replay_sequence
        .checked_add(1)
        .ok_or(Error::ReplayExhausted)?;
    let mut vault_after = vault_before;
    vault_after.cash_atoms = machine_after.vault.cash_atoms;
    vault_after.internal = machine_after.vault.internal;
    vault_after.replay_sequence = vault_after
        .replay_sequence
        .checked_add(1)
        .ok_or(Error::ReplayExhausted)?;
    user_after.validate(outcome_count)?;
    vault_after.validate(outcome_count)?;
    if user_after.reserved_cash_atoms != user_before.reserved_cash_atoms
        || vault_after.reserved_cash_atoms != 0
    {
        return Err(Error::InvariantViolation);
    }
    Ok((user_after, vault_after))
}

#[cfg(test)]
mod tests {
    use clutch_structured_claim::{
        BackingPlan, ClaimVector, NativeBasisIdentity, NativeClaim, WrapperState,
    };

    use super::*;

    fn key(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn machine() -> StructuredClaimMachine {
        let claim = NativeClaim {
            basis: NativeBasisIdentity {
                market: key(1),
                terms: key(2),
                basis_degree: 1,
                denominator: 1,
                outcome_count: 2,
            },
            vector: ClaimVector {
                outcome_count: 2,
                coefficients: [1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            },
        };
        StructuredClaimMachine {
            claim,
            backing: BackingPlan {
                outcome_count: 2,
                cash_per_wrapper: 1,
                residual_eggs_per_wrapper: [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            },
            wrapper: WrapperState {
                actual_supply: 0,
                retired: false,
            },
            vault: BackingVault::EMPTY,
        }
    }

    fn position(owner: u8, replay_sequence: u64) -> PositionProjectionV1 {
        PositionProjectionV1 {
            market: key(1),
            owner: key(owner),
            generation: 1,
            replay_sequence,
            cash_atoms: 0,
            reserved_cash_atoms: 0,
            internal: [0; crate::MAX_OUTCOMES],
            closed: false,
        }
    }

    #[test]
    fn market_changing_projection_refuses_alias_and_reserved_vault() {
        let holder = HolderAssets::EMPTY;
        assert_eq!(
            stage_market_changing_positions(2, position(3, 0), position(3, 0), holder, &machine()),
            Err(Error::DifferentPositionDomain)
        );
        let mut reserved = position(4, 0);
        reserved.cash_atoms = 1;
        reserved.reserved_cash_atoms = 1;
        assert_eq!(
            stage_market_changing_positions(2, position(3, 0), reserved, holder, &machine()),
            Err(Error::DifferentPositionDomain)
        );
    }

    #[test]
    fn market_changing_projection_refuses_replay_exhaustion() {
        assert_eq!(
            stage_market_changing_positions(
                2,
                position(3, u64::MAX),
                position(4, 0),
                HolderAssets::EMPTY,
                &machine(),
            ),
            Err(Error::ReplayExhausted)
        );
    }
}
