// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    admit_collateral_account_v2, admit_collateral_mint_v2, admit_realm_collateral_account_v2,
    admit_realm_collateral_mint_v2, market_hoard_binding_v2, BoundCollateralProfileV2,
    BoundRealmCollateralV2, CustodyBindingV2, Error, Id, MintObservationV2, ProgramFamilyV2,
    RealmCollateralContextV2, Result, RuntimeAccountViewV2, TokenAccountObservationV2,
    TokenAccountRoleV2, BASE_TOKEN_ACCOUNT_BYTES, IMMUTABLE_OWNER_ACCOUNT_BYTES,
};

/// Exact checked-transfer instruction-data width for both compiled V2 families.
pub const CHECKED_TRANSFER_DATA_V2_BYTES: usize = 10;
/// Exact `InitializeAccount3` instruction-data width.
pub const INITIALIZE_ACCOUNT3_DATA_V2_BYTES: usize = 33;
const INITIALIZE_ACCOUNT3_DISCRIMINATOR: u8 = 18;
const INITIALIZE_IMMUTABLE_OWNER_DISCRIMINATOR: u8 = 22;

/// Position-local collateral cash accounting used only by Market Endow/Withdraw.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionCashV2 {
    /// Total collateral cash credited to the position.
    pub cash_atoms: u64,
    /// Subset encumbered by open orders or another authenticated owner.
    pub reserved_cash_atoms: u64,
}

impl PositionCashV2 {
    /// Validate the reservation subset.
    pub fn validate(&self) -> Result<()> {
        if self.reserved_cash_atoms > self.cash_atoms {
            Err(Error::InsufficientUnreservedCash)
        } else {
            Ok(())
        }
    }

    /// Credit exact deposited atoms after a successful collateral CPI.
    pub fn after_deposit(self, amount_atoms: u64) -> Result<Self> {
        self.validate()?;
        if amount_atoms == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            cash_atoms: self
                .cash_atoms
                .checked_add(amount_atoms)
                .ok_or(Error::Arithmetic)?,
            reserved_cash_atoms: self.reserved_cash_atoms,
        })
    }

    /// Debit only exact unreserved atoms after a successful collateral CPI.
    pub fn after_withdrawal(self, amount_atoms: u64) -> Result<Self> {
        self.validate()?;
        if amount_atoms == 0 {
            return Err(Error::InvalidParameter);
        }
        let unreserved = self
            .cash_atoms
            .checked_sub(self.reserved_cash_atoms)
            .ok_or(Error::Arithmetic)?;
        if unreserved < amount_atoms {
            return Err(Error::InsufficientUnreservedCash);
        }
        Ok(Self {
            cash_atoms: self
                .cash_atoms
                .checked_sub(amount_atoms)
                .ok_or(Error::Arithmetic)?,
            reserved_cash_atoms: self.reserved_cash_atoms,
        })
    }
}

/// Market-local locked principal and immutable cap facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralBackingV2 {
    /// Principal currently locked behind complete-set liabilities.
    pub locked_atoms: u64,
    /// Per-Market cap frozen in immutable Terms.
    pub cap_atoms: u64,
}

impl CollateralBackingV2 {
    /// Join the accounting cap to the bound Market and visible Hoard balance.
    pub fn validate(
        &self,
        bound: BoundCollateralProfileV2,
        visible_hoard_atoms: u64,
    ) -> Result<()> {
        if self.cap_atoms != bound.market().collateral_cap_atoms {
            return Err(Error::MismatchedBinding);
        }
        if self.cap_atoms == 0 || self.locked_atoms > self.cap_atoms {
            return Err(Error::MarketCapExceeded);
        }
        require_hoard_coverage_v2(self.locked_atoms, visible_hoard_atoms)
    }

    /// Lock exact atoms as backing without moving pooled collateral.
    pub fn after_lock(
        self,
        bound: BoundCollateralProfileV2,
        visible_hoard_atoms: u64,
        amount_atoms: u64,
    ) -> Result<Self> {
        self.validate(bound, visible_hoard_atoms)?;
        if amount_atoms == 0 {
            return Err(Error::InvalidParameter);
        }
        let locked_atoms = self
            .locked_atoms
            .checked_add(amount_atoms)
            .ok_or(Error::Arithmetic)?;
        let next = Self {
            locked_atoms,
            cap_atoms: self.cap_atoms,
        };
        next.validate(bound, visible_hoard_atoms)?;
        Ok(next)
    }

    /// Unlock exact atoms after the liability kernel removes complete sets.
    pub fn after_unlock(
        self,
        bound: BoundCollateralProfileV2,
        visible_hoard_atoms: u64,
        amount_atoms: u64,
    ) -> Result<Self> {
        self.validate(bound, visible_hoard_atoms)?;
        if amount_atoms == 0 {
            return Err(Error::InvalidParameter);
        }
        let next = Self {
            locked_atoms: self
                .locked_atoms
                .checked_sub(amount_atoms)
                .ok_or(Error::Arithmetic)?,
            cap_atoms: self.cap_atoms,
        };
        next.validate(bound, visible_hoard_atoms)?;
        Ok(next)
    }
}

/// Require pooled visible collateral to cover locked principal.
pub fn require_hoard_coverage_v2(locked_atoms: u64, visible_hoard_atoms: u64) -> Result<()> {
    if visible_hoard_atoms < locked_atoms {
        Err(Error::HoardCoverageMismatch)
    } else {
        Ok(())
    }
}

/// Typed exact-atom collateral movement. None of these variants mints claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CustodyTransferKindV2 {
    /// Holder source → Market Hoard; credit Position cash after postcheck.
    HolderDeposit = 1,
    /// Market Hoard → holder destination; debit unreserved Position cash.
    HolderWithdrawal = 2,
    /// Holder source → separately owned segregated funding vault.
    SegregatedFunding = 3,
    /// Segregated vault → exact component-owned custody destination.
    OccurrenceDisbursement = 4,
    /// Segregated vault → immutable payer refund token account.
    PrincipalRefund = 5,
    /// Segregated vault → registry-authenticated neutral disposition account.
    DonationDisposition = 6,
}

/// Typed source or destination plus its singular semantic owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferEndpointV2 {
    /// Token byte-admission role.
    pub token_role: TokenAccountRoleV2,
    /// Artifact that owns the endpoint role, not necessarily its balance facts.
    pub semantic_owner: Id,
    /// Owner-local compartment, zero only for ordinary holder endpoints.
    pub compartment: u16,
}

impl TransferEndpointV2 {
    fn validate(
        self,
        bound: BoundRealmCollateralV2,
        market: Option<BoundCollateralProfileV2>,
    ) -> Result<()> {
        self.semantic_owner.require_live()?;
        match self.token_role {
            TokenAccountRoleV2::Holder { owner } => {
                owner.require_live()?;
                if self.compartment != 0 {
                    return Err(Error::InvalidParameter);
                }
            }
            TokenAccountRoleV2::ReceiveOnly { account } => {
                account.require_live()?;
                if self.compartment != 0 {
                    return Err(Error::InvalidParameter);
                }
            }
            TokenAccountRoleV2::Hoard => {
                let expected = market_hoard_binding_v2(market.ok_or(Error::MismatchedBinding)?);
                if self.semantic_owner != expected.semantic_owner
                    || self.compartment != expected.compartment
                {
                    return Err(Error::MismatchedBinding);
                }
            }
            TokenAccountRoleV2::SegregatedVault(binding) => {
                binding.validate(bound.release())?;
                if self.semantic_owner != binding.semantic_owner
                    || self.compartment != binding.compartment
                {
                    return Err(Error::MismatchedBinding);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransferBoundV2 {
    Realm(BoundRealmCollateralV2),
    Market(BoundCollateralProfileV2),
}

impl TransferBoundV2 {
    const fn realm(self) -> BoundRealmCollateralV2 {
        match self {
            Self::Realm(bound) => bound,
            Self::Market(bound) => bound.realm_bound(),
        }
    }

    const fn market(self) -> Option<BoundCollateralProfileV2> {
        match self {
            Self::Realm(_) => None,
            Self::Market(bound) => Some(bound),
        }
    }
}

/// How the source token owner authority reaches the external token program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferAuthorityKindV2 {
    /// Runtime-authenticated transaction signer propagated into CPI.
    TransactionSigner,
    /// Canonical program-derived owner signed with adapter-authenticated seeds.
    ProgramDerived,
}

/// Runtime facts for the exact source token owner authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferAuthorityV2 {
    /// Exact authority account address.
    pub address: Id,
    /// Signer mechanism selected for CPI.
    pub kind: TransferAuthorityKindV2,
    /// Runtime outer-instruction signer bit.
    pub is_transaction_signer: bool,
    /// Whether the live adapter authenticated the canonical PDA and its bump.
    pub program_address_authenticated: bool,
    /// Authority accounts are read-only in this contract.
    pub is_writable: bool,
    /// Authority accounts must not be executable.
    pub executable: bool,
    /// PDA authority accounts carry no data.
    pub data_is_empty: bool,
}

impl TransferAuthorityV2 {
    pub(crate) fn validate(self) -> Result<()> {
        self.address.require_live()?;
        if self.is_writable || self.executable {
            return Err(Error::WrongAccountRole);
        }
        match self.kind {
            TransferAuthorityKindV2::TransactionSigner => {
                if !self.is_transaction_signer || self.program_address_authenticated {
                    return Err(Error::WrongAccountRole);
                }
            }
            TransferAuthorityKindV2::ProgramDerived => {
                if self.is_transaction_signer
                    || !self.program_address_authenticated
                    || !self.data_is_empty
                {
                    return Err(Error::OwnerGuardUnavailable);
                }
            }
        }
        Ok(())
    }
}

/// Complete pure request for one exact collateral transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferRequestV2 {
    /// Typed collateral movement.
    pub kind: CustodyTransferKindV2,
    /// Source semantic/account role.
    pub source: TransferEndpointV2,
    /// Destination semantic/account role.
    pub destination: TransferEndpointV2,
    /// Exact source token owner authority.
    pub authority: TransferAuthorityV2,
    /// Requested raw collateral atoms; decimals never rescale this value.
    pub amount_atoms: u64,
    /// Position cash state for Market deposit/withdrawal, absent otherwise.
    pub position_cash: Option<PositionCashV2>,
    /// Locked Hoard principal that must remain covered after any Hoard movement.
    pub locked_collateral_atoms: u64,
}

/// Fixed CPI account meta without a Solana SDK dependency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpiAccountMetaV2 {
    /// Exact account address.
    pub address: Id,
    /// CPI writable bit.
    pub writable: bool,
    /// CPI signer bit.
    pub signer: bool,
}

/// Exact checked-transfer CPI intent. The live adapter may only translate it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedTransferCpiV2 {
    /// Release-selected external token program.
    pub token_program: Id,
    /// Ordered source, mint, destination, authority metas.
    pub accounts: [CpiAccountMetaV2; 4],
    /// Exact family-checked instruction bytes.
    pub data: [u8; CHECKED_TRANSFER_DATA_V2_BYTES],
    /// Whether the authority requires `invoke_signed` with authenticated seeds.
    pub program_signed: bool,
}

/// Validated pre-CPI snapshot and the sole CPI intent it authorizes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedCollateralTransferV2 {
    bound: TransferBoundV2,
    request: TransferRequestV2,
    source_before: TokenAccountObservationV2,
    destination_before: TokenAccountObservationV2,
    mint_supply_before: u64,
    next_position_cash: Option<PositionCashV2>,
    cpi: CheckedTransferCpiV2,
}

impl PreparedCollateralTransferV2 {
    /// Sole external invocation this prepared transition permits.
    pub const fn cpi(self) -> CheckedTransferCpiV2 {
        self.cpi
    }

    /// Exact raw atom quantity.
    pub const fn amount_atoms(self) -> u64 {
        self.request.amount_atoms
    }
}

/// Accepted post-CPI facts safe to commit to semantic state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedCollateralTransferV2 {
    /// Typed movement that completed.
    pub kind: CustodyTransferKindV2,
    /// Exact raw atom quantity.
    pub amount_atoms: u64,
    /// Artifact that owned the debited endpoint role.
    pub source_semantic_owner: Id,
    /// Source owner-local compartment discriminant.
    pub source_compartment: u16,
    /// Artifact that owns the credited endpoint role.
    pub destination_semantic_owner: Id,
    /// Destination owner-local compartment discriminant.
    pub destination_compartment: u16,
    /// Admitted source balance after CPI.
    pub source_atoms_after: u64,
    /// Admitted destination balance after CPI.
    pub destination_atoms_after: u64,
    /// Unchanged admitted mint supply after CPI.
    pub mint_supply_after: u64,
    /// Updated Position cash for Market Endow/Withdraw, absent otherwise.
    pub next_position_cash: Option<PositionCashV2>,
    /// Visible Hoard balance after CPI when the Hoard was touched.
    pub hoard_atoms_after: Option<u64>,
}

/// Validate hostile pre-state and prepare one exact checked-transfer CPI.
pub fn prepare_collateral_transfer_v2(
    bound: BoundCollateralProfileV2,
    request: TransferRequestV2,
    mint: RuntimeAccountViewV2<'_>,
    source: RuntimeAccountViewV2<'_>,
    destination: RuntimeAccountViewV2<'_>,
) -> Result<PreparedCollateralTransferV2> {
    prepare_collateral_transfer_inner_v2(
        TransferBoundV2::Market(bound),
        request,
        mint,
        source,
        destination,
    )
}

/// Prepare a non-Hoard transfer from an authenticated Realm collateral profile.
///
/// This is the Series-safe entrypoint before an occurrence Market exists. It
/// admits only holder/segregated-vault shapes; Market deposit, withdrawal, and
/// any Hoard endpoint require [`prepare_collateral_transfer_v2`].
pub fn prepare_realm_collateral_transfer_v2(
    bound: BoundRealmCollateralV2,
    request: TransferRequestV2,
    mint: RuntimeAccountViewV2<'_>,
    source: RuntimeAccountViewV2<'_>,
    destination: RuntimeAccountViewV2<'_>,
) -> Result<PreparedCollateralTransferV2> {
    prepare_collateral_transfer_inner_v2(
        TransferBoundV2::Realm(bound),
        request,
        mint,
        source,
        destination,
    )
}

fn prepare_collateral_transfer_inner_v2(
    bound: TransferBoundV2,
    request: TransferRequestV2,
    mint: RuntimeAccountViewV2<'_>,
    source: RuntimeAccountViewV2<'_>,
    destination: RuntimeAccountViewV2<'_>,
) -> Result<PreparedCollateralTransferV2> {
    if request.amount_atoms == 0 {
        return Err(Error::InvalidParameter);
    }
    let realm = bound.realm();
    request.source.validate(realm, bound.market())?;
    request.destination.validate(realm, bound.market())?;
    request.authority.validate()?;
    validate_transfer_shape(request)?;
    if !source.is_writable
        || !destination.is_writable
        || mint.is_writable
        || source.key == destination.key
        || source.key == mint.key
        || destination.key == mint.key
    {
        return Err(Error::WrongAccountRole);
    }
    let mint_before = admit_mint_for_transfer_v2(bound, mint)?;
    let source_before = admit_account_for_transfer_v2(bound, source, request.source.token_role)?;
    let destination_before =
        admit_account_for_transfer_v2(bound, destination, request.destination.token_role)?;
    if source_before.amount_atoms < request.amount_atoms
        || request.authority.address != source_before.owner_authority
    {
        return Err(Error::WrongAccountRole);
    }
    if let Some(hoard) = hoard_amount(request, source_before, destination_before) {
        require_hoard_coverage_v2(request.locked_collateral_atoms, hoard)?;
    } else if request.locked_collateral_atoms != 0 {
        return Err(Error::NonCanonicalPadding);
    }
    let next_position_cash = match request.kind {
        CustodyTransferKindV2::HolderDeposit => Some(
            request
                .position_cash
                .ok_or(Error::InvalidParameter)?
                .after_deposit(request.amount_atoms)?,
        ),
        CustodyTransferKindV2::HolderWithdrawal => Some(
            request
                .position_cash
                .ok_or(Error::InvalidParameter)?
                .after_withdrawal(request.amount_atoms)?,
        ),
        _ => {
            if request.position_cash.is_some() {
                return Err(Error::NonCanonicalPadding);
            }
            None
        }
    };
    let release = realm.release();
    let mut data = [0; CHECKED_TRANSFER_DATA_V2_BYTES];
    data[0] = release.transfer_checked_discriminator;
    data[1..9].copy_from_slice(&request.amount_atoms.to_le_bytes());
    data[9] = realm.policy().decimals;
    let cpi = CheckedTransferCpiV2 {
        token_program: release.token_program,
        accounts: [
            CpiAccountMetaV2 {
                address: source.key,
                writable: true,
                signer: false,
            },
            CpiAccountMetaV2 {
                address: mint.key,
                writable: false,
                signer: false,
            },
            CpiAccountMetaV2 {
                address: destination.key,
                writable: true,
                signer: false,
            },
            CpiAccountMetaV2 {
                address: request.authority.address,
                writable: false,
                signer: true,
            },
        ],
        data,
        program_signed: request.authority.kind == TransferAuthorityKindV2::ProgramDerived,
    };
    Ok(PreparedCollateralTransferV2 {
        bound,
        request,
        source_before,
        destination_before,
        mint_supply_before: mint_before.supply_atoms,
        next_position_cash,
        cpi,
    })
}

/// Reparse every touched account and accept only the exact postcondition.
///
/// A caller must discard all semantic writes if this returns an error. The
/// release flags and admitted account states exclude fees, withheld balances,
/// hooks, opaque amounts, and foreign invocations; this function independently
/// checks the observable debit, credit, and unchanged supply.
pub fn accept_collateral_transfer_v2(
    prepared: PreparedCollateralTransferV2,
    mint_after: RuntimeAccountViewV2<'_>,
    source_after: RuntimeAccountViewV2<'_>,
    destination_after: RuntimeAccountViewV2<'_>,
) -> Result<AcceptedCollateralTransferV2> {
    let request = prepared.request;
    if !source_after.is_writable || !destination_after.is_writable || mint_after.is_writable {
        return Err(Error::PostAdmissionFailed);
    }
    let mint = admit_mint_for_transfer_v2(prepared.bound, mint_after)
        .map_err(|_| Error::PostAdmissionFailed)?;
    let source =
        admit_account_for_transfer_v2(prepared.bound, source_after, request.source.token_role)
            .map_err(|_| Error::PostAdmissionFailed)?;
    let destination = admit_account_for_transfer_v2(
        prepared.bound,
        destination_after,
        request.destination.token_role,
    )
    .map_err(|_| Error::PostAdmissionFailed)?;
    if source.address != prepared.source_before.address
        || destination.address != prepared.destination_before.address
        || mint.address != prepared.bound.realm().policy().mint
        || prepared
            .source_before
            .amount_atoms
            .checked_sub(source.amount_atoms)
            != Some(request.amount_atoms)
        || destination
            .amount_atoms
            .checked_sub(prepared.destination_before.amount_atoms)
            != Some(request.amount_atoms)
        || mint.supply_atoms != prepared.mint_supply_before
    {
        return Err(Error::TransferDeltaMismatch);
    }
    let hoard_atoms_after = hoard_amount(request, source, destination);
    if let Some(hoard) = hoard_atoms_after {
        require_hoard_coverage_v2(request.locked_collateral_atoms, hoard)?;
    }
    Ok(AcceptedCollateralTransferV2 {
        kind: request.kind,
        amount_atoms: request.amount_atoms,
        source_semantic_owner: request.source.semantic_owner,
        source_compartment: request.source.compartment,
        destination_semantic_owner: request.destination.semantic_owner,
        destination_compartment: request.destination.compartment,
        source_atoms_after: source.amount_atoms,
        destination_atoms_after: destination.amount_atoms,
        mint_supply_after: mint.supply_atoms,
        next_position_cash: prepared.next_position_cash,
        hoard_atoms_after,
    })
}

fn admit_mint_for_transfer_v2(
    bound: TransferBoundV2,
    account: RuntimeAccountViewV2<'_>,
) -> Result<MintObservationV2> {
    match bound {
        TransferBoundV2::Realm(realm) => admit_realm_collateral_mint_v2(realm, account),
        TransferBoundV2::Market(market) => admit_collateral_mint_v2(market, account),
    }
}

fn admit_account_for_transfer_v2(
    bound: TransferBoundV2,
    account: RuntimeAccountViewV2<'_>,
    role: TokenAccountRoleV2,
) -> Result<TokenAccountObservationV2> {
    match bound {
        TransferBoundV2::Realm(realm) => admit_realm_collateral_account_v2(realm, account, role),
        TransferBoundV2::Market(market) => admit_collateral_account_v2(market, account, role),
    }
}

fn validate_transfer_shape(request: TransferRequestV2) -> Result<()> {
    use CustodyTransferKindV2::*;
    use TokenAccountRoleV2::*;

    let source_is_holder = matches!(request.source.token_role, Holder { .. });
    let source_is_hoard = matches!(request.source.token_role, Hoard);
    let source_is_segregated = matches!(request.source.token_role, SegregatedVault(_));
    let destination_is_holder = matches!(request.destination.token_role, Holder { .. });
    let destination_is_receive_only = matches!(request.destination.token_role, ReceiveOnly { .. });
    let destination_is_hoard = matches!(request.destination.token_role, Hoard);
    let destination_is_segregated =
        matches!(request.destination.token_role, Hoard | SegregatedVault(_));
    let transaction_signer = request.authority.kind == TransferAuthorityKindV2::TransactionSigner;
    let program_signer = request.authority.kind == TransferAuthorityKindV2::ProgramDerived;

    let valid = match request.kind {
        HolderDeposit => source_is_holder && destination_is_hoard && transaction_signer,
        HolderWithdrawal => source_is_hoard && destination_is_holder && program_signer,
        SegregatedFunding => {
            source_is_holder
                && matches!(request.destination.token_role, SegregatedVault(_))
                && transaction_signer
        }
        OccurrenceDisbursement => {
            source_is_segregated && destination_is_segregated && program_signer
        }
        PrincipalRefund | DonationDisposition => {
            source_is_segregated
                && (destination_is_holder || destination_is_receive_only)
                && program_signer
        }
    };
    if valid {
        Ok(())
    } else {
        Err(Error::WrongAccountRole)
    }
}

fn hoard_amount(
    request: TransferRequestV2,
    source: TokenAccountObservationV2,
    destination: TokenAccountObservationV2,
) -> Option<u64> {
    match (request.source.token_role, request.destination.token_role) {
        (TokenAccountRoleV2::Hoard, _) => Some(source.amount_atoms),
        (_, TokenAccountRoleV2::Hoard) => Some(destination.amount_atoms),
        _ => None,
    }
}

/// Family-specific collateral-custody initialization action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustodyInitializationStepV2 {
    /// Inactive fixed-array padding.
    None,
    /// Token-2022 `InitializeImmutableOwner` over the new custody account.
    InitializeImmutableOwner {
        /// Exact collateral custody token account.
        account: Id,
        /// One-byte instruction data.
        data: [u8; 1],
    },
    /// `InitializeAccount3` over exact custody, mint, and authority identities.
    InitializeAccount3 {
        /// Exact collateral custody token account.
        account: Id,
        /// Exact collateral mint.
        mint: Id,
        /// Exact canonical custody owner authority.
        owner_authority: Id,
        /// Exact instruction bytes.
        data: [u8; INITIALIZE_ACCOUNT3_DATA_V2_BYTES],
    },
}

/// Smallest family-independent account-creation seam for one collateral vault.
///
/// System-program allocation, rent, payer authority, PDA derivation, and CPI
/// execution remain live-adapter responsibilities. This plan freezes only the
/// token-program owner, exact space, and permitted initialization sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyCreationPlanV2 {
    /// Release-selected external token program.
    pub token_program: Id,
    /// Exact account data size.
    pub account_bytes: u16,
    /// Exact collateral custody token account.
    pub account: Id,
    /// Exact canonical custody authority.
    pub owner_authority: Id,
    /// Exact collateral mint.
    pub mint: Id,
    /// Ordered family-specific initialization steps.
    pub steps: [CustodyInitializationStepV2; 2],
    /// Active prefix length of `steps`.
    pub step_count: u8,
}

/// Prepare the selected family's exact Hoard layout and initialization sequence.
pub fn prepare_hoard_creation_v2(bound: BoundCollateralProfileV2) -> Result<CustodyCreationPlanV2> {
    let binding: CustodyBindingV2 = market_hoard_binding_v2(bound);
    prepare_custody_creation_v2(bound.realm_bound(), binding)
}

/// Prepare a segregated custody vault's exact layout and initialization sequence.
///
/// The supplied binding must already be joined to the state artifact and
/// compartment that own it. This function does not choose PDA seeds or account
/// addresses for Product/Series, dealer, wrapper, or recovery components.
pub fn prepare_custody_creation_v2<B: RealmCollateralContextV2>(
    bound: B,
    binding: CustodyBindingV2,
) -> Result<CustodyCreationPlanV2> {
    let bound = bound.realm_collateral();
    let release = bound.release();
    binding.validate(release)?;
    let mut initialize_account3 = [0; INITIALIZE_ACCOUNT3_DATA_V2_BYTES];
    initialize_account3[0] = INITIALIZE_ACCOUNT3_DISCRIMINATOR;
    initialize_account3[1..].copy_from_slice(&binding.owner_authority.bytes());
    let account3 = CustodyInitializationStepV2::InitializeAccount3 {
        account: binding.account,
        mint: bound.policy().mint,
        owner_authority: binding.owner_authority,
        data: initialize_account3,
    };
    let (account_bytes, steps, step_count) = match release.family {
        ProgramFamilyV2::LegacySpl => (
            BASE_TOKEN_ACCOUNT_BYTES,
            [account3, CustodyInitializationStepV2::None],
            1,
        ),
        ProgramFamilyV2::Token2022Base => (
            IMMUTABLE_OWNER_ACCOUNT_BYTES,
            [
                CustodyInitializationStepV2::InitializeImmutableOwner {
                    account: binding.account,
                    data: [INITIALIZE_IMMUTABLE_OWNER_DISCRIMINATOR],
                },
                account3,
            ],
            2,
        ),
    };
    if account_bytes != release.custody_account_bytes {
        return Err(Error::MismatchedBinding);
    }
    Ok(CustodyCreationPlanV2 {
        token_program: release.token_program,
        account_bytes,
        account: binding.account,
        owner_authority: binding.owner_authority,
        mint: bound.policy().mint,
        steps,
        step_count,
    })
}
