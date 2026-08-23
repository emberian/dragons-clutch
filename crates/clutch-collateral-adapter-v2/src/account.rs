// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    AdapterReleaseV2, BoundCollateralProfileV2, BoundRealmCollateralV2, Error, Id, OwnerGuardV2,
    ProgramFamilyV2, Result, BASE_MINT_BYTES, BASE_TOKEN_ACCOUNT_BYTES, EXTENSION_IMMUTABLE_OWNER,
    IMMUTABLE_OWNER_ACCOUNT_BYTES,
};

const ACCOUNT_TYPE_TOKEN_ACCOUNT: u8 = 2;
const TOKEN_ACCOUNT_STATE_UNINITIALIZED: u8 = 0;
const TOKEN_ACCOUNT_STATE_INITIALIZED: u8 = 1;
const TOKEN_ACCOUNT_STATE_FROZEN: u8 = 2;

/// Runtime account facts and borrowed hostile data.
///
/// The live adapter must populate these fields from `AccountInfo`; this type
/// does not authenticate a caller-supplied projection by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAccountViewV2<'a> {
    /// Runtime account address.
    pub key: Id,
    /// Runtime owner program.
    pub owner_program: Id,
    /// Exact current data bytes.
    pub data: &'a [u8],
    /// Runtime transaction signer bit.
    pub is_signer: bool,
    /// Runtime writable bit.
    pub is_writable: bool,
    /// Runtime executable bit.
    pub executable: bool,
}

/// Immutable identity of a separately owned collateral custody compartment.
///
/// `semantic_owner` owns the vault's identity/role namespace; it need not own
/// mutable balance facts. For example, a SeriesPlan may own the five component
/// roles while SeriesFunding state solely owns principal/donation amounts.
/// `compartment` is the namespace-local typed discriminant. PDA address/seed
/// allocation is deliberately left to the consuming adapter until frozen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyBindingV2 {
    /// Exact collateral token account.
    pub account: Id,
    /// Exact token owner authority stored in the account.
    pub owner_authority: Id,
    /// Artifact that owns this custody endpoint's identity and role namespace.
    pub semantic_owner: Id,
    /// Nonzero owner-local compartment discriminant.
    pub compartment: u16,
    /// Owner guard selected by the resolved collateral release.
    pub owner_guard: OwnerGuardV2,
    /// Whether the live adapter authenticated the owner as a canonical PDA.
    pub owner_authority_is_program_derived: bool,
}

impl CustodyBindingV2 {
    /// Validate live identities and the release-selected guard.
    pub fn validate(&self, release: AdapterReleaseV2) -> Result<()> {
        self.account.require_live()?;
        self.owner_authority.require_live()?;
        self.semantic_owner.require_live()?;
        if self.compartment == 0
            || self.owner_guard != release.owner_guard
            || !self.owner_authority_is_program_derived
        {
            return Err(Error::OwnerGuardUnavailable);
        }
        Ok(())
    }
}

/// Exact role under which token-account bytes are admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenAccountRoleV2 {
    /// User-controlled source or destination with an exact owner authority.
    Holder {
        /// Required token owner authority.
        owner: Id,
    },
    /// Exact receive-only token account whose mutable owner is discovered from
    /// hostile account bytes rather than copied into immutable protocol Terms.
    ///
    /// This role can never authorize a debit. Transfer-shape validation admits
    /// it only as the destination of principal-refund or neutral-disposition
    /// movements.
    ReceiveOnly {
        /// Exact token-account address frozen by the semantic owner.
        account: Id,
    },
    /// Market-local pooled Hoard owned by the canonical Hoard authority.
    Hoard,
    /// Separately identified program-owned custody compartment.
    SegregatedVault(CustodyBindingV2),
}

/// Parsed and admitted mint facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintObservationV2 {
    /// Exact mint account address.
    pub address: Id,
    /// Raw-atom exponent.
    pub decimals: u8,
    /// Current raw mint supply.
    pub supply_atoms: u64,
    /// Parsed mint extensions.
    pub extensions: u64,
}

/// Parsed and admitted collateral token-account facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenAccountObservationV2 {
    /// Exact token-account address.
    pub address: Id,
    /// Collateral mint stored in the account.
    pub mint: Id,
    /// Token owner authority stored in the account.
    pub owner_authority: Id,
    /// Exact visible spendable balance in raw atoms.
    pub amount_atoms: u64,
    /// Parsed token-account extensions.
    pub extensions: u64,
    /// Semantic owner of custody, or the holder authority for a holder account.
    pub semantic_owner: Id,
    /// Nonzero only for a segregated custody compartment.
    pub compartment: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawMint {
    decimals: u8,
    supply_atoms: u64,
    mint_authority: Option<Id>,
    freeze_authority: Option<Id>,
    extensions: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawTokenAccount {
    mint: Id,
    owner_authority: Id,
    amount_atoms: u64,
    initialized: bool,
    frozen: bool,
    delegate: Option<Id>,
    is_native: bool,
    close_authority: Option<Id>,
    extensions: u64,
}

/// Construct the Market Hoard's typed custody binding without copying policy
/// facts into Market state.
pub fn market_hoard_binding_v2(bound: BoundCollateralProfileV2) -> CustodyBindingV2 {
    let market = bound.market();
    CustodyBindingV2 {
        account: market.hoard_token_account,
        owner_authority: market.hoard_authority,
        semantic_owner: market.market,
        compartment: 1,
        owner_guard: bound.release().owner_guard,
        owner_authority_is_program_derived: true,
    }
}

/// Parse and admit the exact collateral mint selected by the bound Realm.
pub fn admit_realm_collateral_mint_v2(
    bound: BoundRealmCollateralV2,
    account: RuntimeAccountViewV2<'_>,
) -> Result<MintObservationV2> {
    admit_collateral_mint_inner_v2(bound, account)
}

/// Parse and admit the exact collateral mint through a concrete Market refinement.
pub fn admit_collateral_mint_v2(
    bound: BoundCollateralProfileV2,
    account: RuntimeAccountViewV2<'_>,
) -> Result<MintObservationV2> {
    admit_collateral_mint_inner_v2(bound.realm_bound(), account)
}

fn admit_collateral_mint_inner_v2(
    bound: BoundRealmCollateralV2,
    account: RuntimeAccountViewV2<'_>,
) -> Result<MintObservationV2> {
    let policy = bound.policy();
    let release = bound.release();
    require_token_owned_view(account, release)?;
    if account.key != policy.mint {
        return Err(Error::WrongMint);
    }
    if account.is_writable || account.data.len() != usize::from(release.mint_account_bytes) {
        return Err(Error::WrongAccountRole);
    }
    let raw = parse_mint(release, account.data)?;
    if raw.decimals != policy.decimals {
        return Err(Error::WrongDecimals);
    }
    if raw.supply_atoms == 0 || raw.supply_atoms > policy.max_supply_atoms {
        return Err(Error::SupplyNotAdmitted);
    }
    if raw.mint_authority.is_some() || raw.freeze_authority.is_some() {
        return Err(Error::MintAuthorityNotAdmitted);
    }
    admit_extensions(
        raw.extensions,
        release.known_mint_extensions,
        policy.allowed_mint_extensions,
        policy.required_mint_extensions,
    )?;
    Ok(MintObservationV2 {
        address: account.key,
        decimals: raw.decimals,
        supply_atoms: raw.supply_atoms,
        extensions: raw.extensions,
    })
}

/// Parse and admit a holder or segregated custody account before any Market exists.
///
/// A Hoard role refuses here because it requires a concrete Market refinement.
pub fn admit_realm_collateral_account_v2(
    bound: BoundRealmCollateralV2,
    account: RuntimeAccountViewV2<'_>,
    role: TokenAccountRoleV2,
) -> Result<TokenAccountObservationV2> {
    admit_collateral_account_inner_v2(bound, None, account, role)
}

/// Parse and admit one holder, Hoard, or segregated custody token account.
pub fn admit_collateral_account_v2(
    bound: BoundCollateralProfileV2,
    account: RuntimeAccountViewV2<'_>,
    role: TokenAccountRoleV2,
) -> Result<TokenAccountObservationV2> {
    admit_collateral_account_inner_v2(bound.realm_bound(), Some(bound), account, role)
}

fn admit_collateral_account_inner_v2(
    bound: BoundRealmCollateralV2,
    market_bound: Option<BoundCollateralProfileV2>,
    account: RuntimeAccountViewV2<'_>,
    role: TokenAccountRoleV2,
) -> Result<TokenAccountObservationV2> {
    let policy = bound.policy();
    let release = bound.release();
    require_token_owned_view(account, release)?;
    let custody = match role {
        TokenAccountRoleV2::Holder { owner } => {
            owner.require_live()?;
            if account.data.len() != usize::from(release.holder_account_bytes)
                && !(release.family == ProgramFamilyV2::Token2022Base
                    && account.data.len() == usize::from(IMMUTABLE_OWNER_ACCOUNT_BYTES))
            {
                return Err(Error::WrongAccountRole);
            }
            None
        }
        TokenAccountRoleV2::ReceiveOnly {
            account: exact_account,
        } => {
            exact_account.require_live()?;
            if account.key != exact_account
                || (account.data.len() != usize::from(release.holder_account_bytes)
                    && !(release.family == ProgramFamilyV2::Token2022Base
                        && account.data.len() == usize::from(IMMUTABLE_OWNER_ACCOUNT_BYTES)))
            {
                return Err(Error::WrongAccountRole);
            }
            None
        }
        TokenAccountRoleV2::Hoard => {
            let binding = market_hoard_binding_v2(market_bound.ok_or(Error::MismatchedBinding)?);
            binding.validate(release)?;
            if account.key != binding.account
                || account.data.len() != usize::from(release.custody_account_bytes)
            {
                return Err(Error::WrongAccountRole);
            }
            Some(binding)
        }
        TokenAccountRoleV2::SegregatedVault(binding) => {
            binding.validate(release)?;
            if account.key != binding.account
                || account.data.len() != usize::from(release.custody_account_bytes)
            {
                return Err(Error::WrongAccountRole);
            }
            Some(binding)
        }
    };
    let raw = parse_token_account(release, account.data)?;
    if raw.mint != policy.mint {
        return Err(Error::WrongMint);
    }
    if !raw.initialized {
        return Err(Error::Uninitialized);
    }
    if raw.frozen || raw.is_native {
        return Err(Error::TokenAccountNotTransferable);
    }
    let (expected_owner, semantic_owner, compartment) = match (role, custody) {
        (TokenAccountRoleV2::Holder { owner }, None) => (owner, owner, 0),
        (TokenAccountRoleV2::ReceiveOnly { .. }, None) => {
            (raw.owner_authority, raw.owner_authority, 0)
        }
        (_, Some(binding)) => {
            if raw.delegate.is_some() || raw.close_authority.is_some() {
                return Err(Error::CustodyAuthorityNotAdmitted);
            }
            (
                binding.owner_authority,
                binding.semantic_owner,
                binding.compartment,
            )
        }
        _ => return Err(Error::WrongAccountRole),
    };
    if raw.owner_authority != expected_owner {
        return Err(Error::WrongAccountRole);
    }
    let required = if custody.is_some() {
        policy.required_account_extensions | release.required_custody_extensions
    } else {
        policy.required_account_extensions
    };
    admit_extensions(
        raw.extensions,
        release.known_account_extensions,
        policy.allowed_account_extensions,
        required,
    )?;
    Ok(TokenAccountObservationV2 {
        address: account.key,
        mint: raw.mint,
        owner_authority: raw.owner_authority,
        amount_atoms: raw.amount_atoms,
        extensions: raw.extensions,
        semantic_owner,
        compartment,
    })
}

fn require_token_owned_view(
    account: RuntimeAccountViewV2<'_>,
    release: AdapterReleaseV2,
) -> Result<()> {
    account.key.require_live()?;
    account.owner_program.require_live()?;
    if account.executable || account.is_signer {
        return Err(Error::WrongAccountRole);
    }
    if account.owner_program != release.token_program {
        return Err(Error::WrongProgram);
    }
    Ok(())
}

fn parse_mint(release: AdapterReleaseV2, data: &[u8]) -> Result<RawMint> {
    if data.len() != usize::from(BASE_MINT_BYTES) {
        return Err(Error::MalformedTokenState);
    }
    let initialized = match data[45] {
        0 => false,
        1 => true,
        _ => return Err(Error::MalformedTokenState),
    };
    if !initialized {
        return Err(Error::Uninitialized);
    }
    Ok(RawMint {
        decimals: data[44],
        supply_atoms: read_u64(data, 36),
        mint_authority: read_coption_id(data, 0)?,
        freeze_authority: read_coption_id(data, 46)?,
        extensions: match release.family {
            ProgramFamilyV2::LegacySpl | ProgramFamilyV2::Token2022Base => 0,
        },
    })
}

fn parse_token_account(release: AdapterReleaseV2, data: &[u8]) -> Result<RawTokenAccount> {
    if data.len() < usize::from(BASE_TOKEN_ACCOUNT_BYTES) {
        return Err(Error::MalformedTokenState);
    }
    let extensions = match release.family {
        ProgramFamilyV2::LegacySpl => {
            if data.len() != usize::from(BASE_TOKEN_ACCOUNT_BYTES) {
                return Err(Error::MalformedTokenState);
            }
            0
        }
        ProgramFamilyV2::Token2022Base => parse_token_2022_account_extensions(data)?,
    };
    let state = data[108];
    let initialized = state != TOKEN_ACCOUNT_STATE_UNINITIALIZED;
    let frozen = match state {
        TOKEN_ACCOUNT_STATE_UNINITIALIZED | TOKEN_ACCOUNT_STATE_INITIALIZED => false,
        TOKEN_ACCOUNT_STATE_FROZEN => true,
        _ => return Err(Error::MalformedTokenState),
    };
    let is_native = match read_coption_u64(data, 109)? {
        Some(_) => true,
        None => false,
    };
    Ok(RawTokenAccount {
        mint: read_id(data, 0),
        owner_authority: read_id(data, 32),
        amount_atoms: read_u64(data, 64),
        initialized,
        frozen,
        delegate: read_coption_id(data, 72)?,
        is_native,
        close_authority: read_coption_id(data, 129)?,
        extensions,
    })
}

fn parse_token_2022_account_extensions(data: &[u8]) -> Result<u64> {
    match data.len() {
        length if length == usize::from(BASE_TOKEN_ACCOUNT_BYTES) => Ok(0),
        length if length == usize::from(IMMUTABLE_OWNER_ACCOUNT_BYTES) => {
            if data[165] != ACCOUNT_TYPE_TOKEN_ACCOUNT
                || data[166..168] != 7_u16.to_le_bytes()
                || data[168..170] != 0_u16.to_le_bytes()
            {
                return Err(Error::MalformedTokenState);
            }
            Ok(EXTENSION_IMMUTABLE_OWNER)
        }
        _ => Err(Error::ExtensionNotAdmitted),
    }
}

fn admit_extensions(present: u64, known: u64, allowed: u64, required: u64) -> Result<()> {
    if present & !known != 0 || present & !allowed != 0 || required & !present != 0 {
        Err(Error::ExtensionNotAdmitted)
    } else {
        Ok(())
    }
}

fn read_id(data: &[u8], at: usize) -> Id {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(&data[at..at + 32]);
    Id::from_bytes(bytes)
}

fn read_u64(data: &[u8], at: usize) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&data[at..at + 8]);
    u64::from_le_bytes(bytes)
}

fn read_coption_id(data: &[u8], at: usize) -> Result<Option<Id>> {
    match &data[at..at + 4] {
        [0, 0, 0, 0] => Ok(None),
        [1, 0, 0, 0] => {
            let identity = read_id(data, at + 4);
            identity.require_live()?;
            Ok(Some(identity))
        }
        _ => Err(Error::MalformedTokenState),
    }
}

fn read_coption_u64(data: &[u8], at: usize) -> Result<Option<u64>> {
    match &data[at..at + 4] {
        [0, 0, 0, 0] => Ok(None),
        [1, 0, 0, 0] => Ok(Some(read_u64(data, at + 4))),
        _ => Err(Error::MalformedTokenState),
    }
}
