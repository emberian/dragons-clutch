//! Supply-sensitive canonical wrap and unwind orchestration contracts.

use clutch_structured_claim::{
    BackingVault, HolderAssets, MarketLedger, StructuredClaimMachine, WrapperState,
};

use crate::{
    prepare_atomic_position_asset_transfer_v1, Amount, AssetTransferPhasePolicyV1,
    AtomicPositionAssetTransferRequestV1, DescriptorIdentityV1, DescriptorStateV1, Error,
    PositionProjectionV1, Result,
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
    fn validate(&self) -> Result<()> {
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
