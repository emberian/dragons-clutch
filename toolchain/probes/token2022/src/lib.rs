//! Token-2022 admission checks, run against real Token-2022 account bytes.
//!
//! This crate is a **probe**.  It holds no Dragon's Clutch semantics, depends
//! on no `clutch-*` crate, and must never be linked into a deployable program.
//! What it does hold is the one piece of the future adapter that cannot be
//! written offline: the predicate that turns raw Token-2022 mint / token-account
//! bytes into an accept-or-refuse decision under the V1 collateral profile.
//!
//! The refusal numbering is deliberately identical to `RefusalCode` in
//! `research/collateral-profiles/model.py`, so the offline decision vectors and
//! anything this probe observes on chain can be compared directly.  The Python
//! model decides over a hand-built *snapshot*; this crate decides over bytes an
//! actual Token-2022 program wrote.  Agreement between the two is the point.
//!
//! Not implemented here, and not implied by anything here: PDA derivation,
//! account authentication, CPI construction, rent, or any transition.  Those
//! are the adapter's, and they are unwritten.

#![deny(missing_docs)]

use solana_address::Address;
use solana_program_option::COption;
use solana_program_pack::Pack;
use spl_token_2022_interface::{
    extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions},
    state::{Account as TokenAccount, AccountState, Mint},
};

/// Refusal codes, numbered exactly as `RefusalCode` in
/// `research/collateral-profiles/model.py`.
///
/// `Accept` is zero there and zero here.  A code is a stable name for *why*
/// admission failed; it is not an error type for the runtime and carries no
/// `ProgramError` mapping yet, because the instruction set that would need one
/// does not exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum RefusalCode {
    /// The mint or token account is admitted by the profile.
    Accept = 0,
    /// Owning token program is not the one the profile names.
    WrongProgram = 1,
    /// Mint identity is not the one the profile names.
    WrongMint = 2,
    /// Mint or token account is not initialized.
    Uninitialized = 3,
    /// Mint decimals differ from the profile.
    WrongDecimals = 4,
    /// Supply is zero and the profile requires a positive supply.
    ZeroSupply = 5,
    /// Supply exceeds the profile's immutable ceiling.
    SupplyExceedsProfile = 6,
    /// A mint authority remains and the profile requires none.
    MintAuthorityPresent = 7,
    /// A freeze authority remains and the profile requires none.
    FreezeAuthorityPresent = 8,
    /// An extension discriminant is not one this build knows.
    UnknownExtension = 9,
    /// An extension appeared on the wrong account kind.
    WrongExtensionLocation = 10,
    /// A known extension is outside the profile's allowed set.
    ExtensionNotAllowed = 11,
    /// A required extension is absent.
    RequiredExtensionMissing = 12,
    /// Token account is frozen.
    FrozenAccount = 13,
    /// Token account owner authority is not the expected one.
    WrongAccountOwner = 14,
    /// Token account carries a delegate and the profile requires none.
    DelegatePresent = 15,
    /// Token account carries a close authority and the profile requires none.
    CloseAuthorityPresent = 16,
    /// The TLV extension region did not parse.
    MalformedExtensionSet = 17,
}

/// A refusal: the code, plus the extension that caused it when one did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Refusal {
    /// Why admission failed.
    pub code: RefusalCode,
    /// The offending extension, when the refusal names one.
    pub extension: Option<ExtensionType>,
}

impl Refusal {
    fn plain(code: RefusalCode) -> Self {
        Self {
            code,
            extension: None,
        }
    }

    fn with_extension(code: RefusalCode, extension: ExtensionType) -> Self {
        Self {
            code,
            extension: Some(extension),
        }
    }
}

/// The V1 collateral-mint policy, in the shape the adapter will carry it.
///
/// `allowed_extensions` is the *effective* set: the intersection of the
/// protocol ceiling and the Realm's own bitset, already computed.  V1's
/// protocol mint ceiling is empty, so the effective set is empty for every
/// legal V1 Realm; the field exists so that a later schema widening the
/// ceiling does not require a different predicate.
#[derive(Clone, Debug)]
pub struct MintPolicy {
    /// Token program that must own the mint account.
    pub token_program: Address,
    /// Mint address the profile names.
    pub mint: Address,
    /// Decimals the profile names.
    pub decimals: u8,
    /// Immutable supply ceiling in atoms.
    pub max_supply_atoms: u64,
    /// Refuse a zero supply.
    pub require_nonzero_supply: bool,
    /// Refuse a mint that still has a mint authority.
    pub require_mint_authority_none: bool,
    /// Refuse a mint that still has a freeze authority.
    pub require_freeze_authority_none: bool,
    /// Mint extensions admitted after intersecting ceiling and Realm bitset.
    pub allowed_extensions: &'static [ExtensionType],
    /// Mint extensions the profile requires to be present.
    pub required_extensions: &'static [ExtensionType],
}

impl MintPolicy {
    /// The V1 collateral-mint policy: base fungible mint, no extensions at all,
    /// no mint authority, no freeze authority.
    pub fn v1_collateral(
        token_program: Address,
        mint: Address,
        decimals: u8,
        ceiling: u64,
    ) -> Self {
        Self {
            token_program,
            mint,
            decimals,
            max_supply_atoms: ceiling,
            require_nonzero_supply: true,
            require_mint_authority_none: true,
            require_freeze_authority_none: true,
            allowed_extensions: &[],
            required_extensions: &[],
        }
    }
}

/// The V1 Hoard token-account policy.
#[derive(Clone, Debug)]
pub struct TokenAccountPolicy {
    /// Token program that must own the token account.
    pub token_program: Address,
    /// Mint the token account must be for.
    pub mint: Address,
    /// The authority the Hoard must be owned by (a program address, in the
    /// real adapter).
    pub expected_owner_authority: Address,
    /// Refuse a delegate.
    pub require_delegate_none: bool,
    /// Refuse a close authority.
    pub require_close_authority_none: bool,
    /// Account extensions admitted after intersecting ceiling and Realm bitset.
    pub allowed_extensions: &'static [ExtensionType],
    /// Account extensions the profile requires to be present.
    pub required_extensions: &'static [ExtensionType],
}

impl TokenAccountPolicy {
    /// The V1 Hoard policy: `ImmutableOwner` is the only admitted extension,
    /// and it is admitted rather than required.
    pub fn v1_hoard(
        token_program: Address,
        mint: Address,
        expected_owner_authority: Address,
    ) -> Self {
        Self {
            token_program,
            mint,
            expected_owner_authority,
            require_delegate_none: true,
            require_close_authority_none: true,
            allowed_extensions: &[ExtensionType::ImmutableOwner],
            required_extensions: &[],
        }
    }
}

/// What the probe observed on a mint, kept beside the decision so a test can
/// assert on the facts rather than only on the verdict.
#[derive(Clone, Debug, PartialEq)]
pub struct MintObservation {
    /// Decimals as written by the token program.
    pub decimals: u8,
    /// Supply in atoms.
    pub supply: u64,
    /// Mint authority, if any.
    pub mint_authority: Option<Address>,
    /// Freeze authority, if any.
    pub freeze_authority: Option<Address>,
    /// Every extension discriminant present, in TLV order.
    pub extensions: Vec<ExtensionType>,
}

/// What the probe observed on a token account.
#[derive(Clone, Debug, PartialEq)]
pub struct TokenAccountObservation {
    /// Mint this account is for.
    pub mint: Address,
    /// Owner authority.
    pub owner: Address,
    /// Balance in atoms, excluding anything withheld by an extension.
    pub amount: u64,
    /// Whether the account is frozen.
    pub frozen: bool,
    /// Delegate, if any.
    pub delegate: Option<Address>,
    /// Close authority, if any.
    pub close_authority: Option<Address>,
    /// Every extension discriminant present, in TLV order.
    pub extensions: Vec<ExtensionType>,
}

fn coption(value: COption<Address>) -> Option<Address> {
    match value {
        COption::Some(address) => Some(address),
        COption::None => None,
    }
}

/// Decode a Token-2022 mint account's bytes into an observation.
///
/// An unparseable TLV region, including one carrying a discriminant this build
/// does not know, refuses rather than being skipped: the fail-closed rule of
/// `COLLATERAL_PROFILES.md` is inherited from the decoder, not re-implemented.
pub fn observe_mint(data: &[u8]) -> Result<MintObservation, Refusal> {
    let state = StateWithExtensions::<Mint>::unpack(data)
        .map_err(|_| Refusal::plain(RefusalCode::MalformedExtensionSet))?;
    if !state.base.is_initialized {
        return Err(Refusal::plain(RefusalCode::Uninitialized));
    }
    let extensions = state
        .get_extension_types()
        .map_err(|_| Refusal::plain(RefusalCode::UnknownExtension))?;
    Ok(MintObservation {
        decimals: state.base.decimals,
        supply: state.base.supply,
        mint_authority: coption(state.base.mint_authority),
        freeze_authority: coption(state.base.freeze_authority),
        extensions,
    })
}

/// Decode a Token-2022 token account's bytes into an observation.
pub fn observe_token_account(data: &[u8]) -> Result<TokenAccountObservation, Refusal> {
    let state = StateWithExtensions::<TokenAccount>::unpack(data)
        .map_err(|_| Refusal::plain(RefusalCode::MalformedExtensionSet))?;
    if state.base.state == AccountState::Uninitialized {
        return Err(Refusal::plain(RefusalCode::Uninitialized));
    }
    let extensions = state
        .get_extension_types()
        .map_err(|_| Refusal::plain(RefusalCode::UnknownExtension))?;
    Ok(TokenAccountObservation {
        mint: state.base.mint,
        owner: state.base.owner,
        amount: state.base.amount,
        frozen: state.base.state == AccountState::Frozen,
        delegate: coption(state.base.delegate),
        close_authority: coption(state.base.close_authority),
        extensions,
    })
}

fn check_extension_sets(
    present: &[ExtensionType],
    allowed: &[ExtensionType],
    required: &[ExtensionType],
    expected_location: ExtensionLocation,
) -> Result<(), Refusal> {
    for extension in present {
        if location_of(*extension) != expected_location {
            return Err(Refusal::with_extension(
                RefusalCode::WrongExtensionLocation,
                *extension,
            ));
        }
    }
    // Lowest discriminant first, so the reported extension is deterministic and
    // matches the Python model's `min(denied, key=int)`.
    let mut denied: Vec<ExtensionType> = present
        .iter()
        .copied()
        .filter(|e| !allowed.contains(e))
        .collect();
    denied.sort_by_key(|e| u16::from(*e));
    if let Some(extension) = denied.first() {
        return Err(Refusal::with_extension(
            RefusalCode::ExtensionNotAllowed,
            *extension,
        ));
    }
    let mut missing: Vec<ExtensionType> = required
        .iter()
        .copied()
        .filter(|e| !present.contains(e))
        .collect();
    missing.sort_by_key(|e| u16::from(*e));
    if let Some(extension) = missing.first() {
        return Err(Refusal::with_extension(
            RefusalCode::RequiredExtensionMissing,
            *extension,
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExtensionLocation {
    MintOnly,
    AccountOnly,
    Either,
}

fn location_of(extension: ExtensionType) -> ExtensionLocation {
    use spl_token_2022_interface::extension::AccountType;
    match extension.get_account_type() {
        AccountType::Mint => ExtensionLocation::MintOnly,
        AccountType::Account => ExtensionLocation::AccountOnly,
        // `Uninitialized` is the padding discriminant; it belongs to neither and
        // is refused as not-allowed by every V1 profile anyway.
        AccountType::Uninitialized => ExtensionLocation::Either,
    }
}

/// Admit or refuse a collateral mint from its on-chain bytes.
///
/// `account_owner` is the *runtime* owner of the mint account, which is the
/// token program.  The adapter must read it from `AccountInfo::owner`; passing
/// a caller-asserted value here would defeat the check, which is exactly the
/// mistake obligation 2 names.
pub fn admit_mint(
    account_owner: &Address,
    mint_address: &Address,
    data: &[u8],
    policy: &MintPolicy,
) -> Result<MintObservation, Refusal> {
    if *account_owner != policy.token_program {
        return Err(Refusal::plain(RefusalCode::WrongProgram));
    }
    if *mint_address != policy.mint {
        return Err(Refusal::plain(RefusalCode::WrongMint));
    }
    let observation = observe_mint(data)?;
    if observation.decimals != policy.decimals {
        return Err(Refusal::plain(RefusalCode::WrongDecimals));
    }
    if policy.require_nonzero_supply && observation.supply == 0 {
        return Err(Refusal::plain(RefusalCode::ZeroSupply));
    }
    if observation.supply > policy.max_supply_atoms {
        return Err(Refusal::plain(RefusalCode::SupplyExceedsProfile));
    }
    if policy.require_mint_authority_none && observation.mint_authority.is_some() {
        return Err(Refusal::plain(RefusalCode::MintAuthorityPresent));
    }
    if policy.require_freeze_authority_none && observation.freeze_authority.is_some() {
        return Err(Refusal::plain(RefusalCode::FreezeAuthorityPresent));
    }
    check_extension_sets(
        &observation.extensions,
        policy.allowed_extensions,
        policy.required_extensions,
        ExtensionLocation::MintOnly,
    )?;
    Ok(observation)
}

/// Admit or refuse a Hoard token account from its on-chain bytes.
pub fn admit_token_account(
    account_owner: &Address,
    data: &[u8],
    policy: &TokenAccountPolicy,
) -> Result<TokenAccountObservation, Refusal> {
    if *account_owner != policy.token_program {
        return Err(Refusal::plain(RefusalCode::WrongProgram));
    }
    let observation = observe_token_account(data)?;
    if observation.mint != policy.mint {
        return Err(Refusal::plain(RefusalCode::WrongMint));
    }
    if observation.frozen {
        return Err(Refusal::plain(RefusalCode::FrozenAccount));
    }
    if observation.owner != policy.expected_owner_authority {
        return Err(Refusal::plain(RefusalCode::WrongAccountOwner));
    }
    if policy.require_delegate_none && observation.delegate.is_some() {
        return Err(Refusal::plain(RefusalCode::DelegatePresent));
    }
    if policy.require_close_authority_none && observation.close_authority.is_some() {
        return Err(Refusal::plain(RefusalCode::CloseAuthorityPresent));
    }
    check_extension_sets(
        &observation.extensions,
        policy.allowed_extensions,
        policy.required_extensions,
        ExtensionLocation::AccountOnly,
    )?;
    Ok(observation)
}

/// Byte length of a base (extension-free) Token-2022 mint, for account sizing.
pub const BASE_MINT_LEN: usize = Mint::LEN;

/// Byte length of a base (extension-free) Token-2022 token account.
pub const BASE_TOKEN_ACCOUNT_LEN: usize = TokenAccount::LEN;
