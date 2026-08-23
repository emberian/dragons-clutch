//! Host-only conformance model for Realm-selected collateral adapters.
//!
//! This crate deliberately contains no Solana SDK, CPI, account parser, RPC,
//! key, signing, or deployable code.  It specifies the facts a concrete
//! adapter must establish before and after it moves collateral.  Egg issuance
//! is a separate Token-2022 plane and is not generalized by this model.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// A 32-byte program, mint, Realm, profile, or release identity.
pub type Identity = [u8; 32];

/// The legacy SPL Token program.
pub const LEGACY_TOKEN_PROGRAM: Identity = [
    0x06, 0xdd, 0xf6, 0xe1, 0xd7, 0x65, 0xa1, 0x93, 0xd9, 0xcb, 0xe1, 0x46, 0xce, 0xeb, 0x79, 0xac,
    0x1c, 0xb4, 0x85, 0xed, 0x5f, 0x5b, 0x37, 0x91, 0x3a, 0x8c, 0xf5, 0x85, 0x7e, 0xff, 0x00, 0xa9,
];

/// The Token-2022 program.
pub const TOKEN_2022_PROGRAM: Identity = [
    0x06, 0xdd, 0xf6, 0xe1, 0xee, 0x75, 0x8f, 0xde, 0x18, 0x42, 0x5d, 0xbc, 0xe4, 0x6c, 0xcd, 0xda,
    0xb6, 0x1a, 0xfc, 0x4d, 0x83, 0xb9, 0x0d, 0x27, 0xfe, 0xbd, 0xf9, 0x28, 0xd8, 0xa1, 0x8b, 0xfc,
];

/// Token-2022 extension discriminants `0..=28`, as pinned by the V1 parser.
pub const TOKEN_2022_KNOWN_EXTENSIONS: u64 = (1_u64 << 29) - 1;
/// Token-2022 `ImmutableOwner`, the only account extension admitted here.
pub const EXT_IMMUTABLE_OWNER: u64 = 1_u64 << 7;
/// Token-2022 `TransferFeeConfig`.
pub const EXT_TRANSFER_FEE_CONFIG: u64 = 1_u64 << 1;
/// Token-2022 `TransferHook`.
pub const EXT_TRANSFER_HOOK: u64 = 1_u64 << 14;

/// Semantic behavior bits understood independently of any token program's
/// extension numbering.
pub mod behavior {
    /// Transfer debits and credits may differ because a fee is withheld.
    pub const FEE_ON_TRANSFER: u16 = 1 << 0;
    /// A transfer can invoke a program outside the selected token adapter.
    pub const TRANSFER_HOOK: u16 = 1 << 1;
    /// Amounts or supply are not fully visible as exact integers.
    pub const CONFIDENTIAL: u16 = 1 << 2;
    /// Ordinary transfer in or out may be prohibited.
    pub const NONTRANSFERABLE: u16 = 1 << 3;
    /// Newly created accounts can default to a frozen state.
    pub const DEFAULT_FROZEN: u16 = 1 << 4;
    /// A third party has permanent transfer or burn authority.
    pub const PERMANENT_DELEGATE: u16 = 1 << 5;
    /// An external authority can pause ordinary token operations.
    pub const PAUSABLE: u16 = 1 << 6;
    /// The program applies a mutable display scaling to raw atoms.
    pub const SCALED_UI: u16 = 1 << 7;

    /// Every semantic behavior the exact-visible-atom profile refuses.
    pub const ALL_REFUSED: u16 = FEE_ON_TRANSFER
        | TRANSFER_HOOK
        | CONFIDENTIAL
        | NONTRANSFERABLE
        | DEFAULT_FROZEN
        | PERMANENT_DELEGATE
        | PAUSABLE
        | SCALED_UI;
}

/// A token-program family.  A new family is not routeable merely because a
/// Realm names a new program id; it needs a compiled, release-identified
/// adapter implementation and conformance evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramFamily {
    /// Fixed-layout legacy SPL Token accounts.
    LegacySpl,
    /// Extensible Token-2022 accounts under a pinned extension universe.
    Token2022,
    /// A future, explicitly implemented family with a stable numeric tag.
    Future(u16),
}

/// How a Hoard token account prevents its owner authority from being changed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HoardOwnerGuard {
    /// The token program enforces an immutable-owner state bit/extension.
    ImmutableOwner,
    /// The owner is a Clutch PDA and the pinned adapter release exposes no
    /// authority-change CPI.  This is weaker than token-enforced immutability
    /// and is sound only inside the checked program-release boundary.
    PdaSoleSigner,
}

/// The semantic contract of one compiled collateral adapter release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterRelease {
    /// Content/release identity selected by a frozen Realm profile.
    pub release_id: Identity,
    /// Token program this adapter authenticates and invokes.
    pub program_id: Identity,
    /// Checked external token-program artifact/deployment identity. A program
    /// address alone is not a behavioral release pin.
    pub program_deployment: Identity,
    /// Parser and CPI family.
    pub family: ProgramFamily,
    /// Extension discriminants this parser recognizes on mints.
    pub known_mint_extensions: u64,
    /// Recognized mint extensions proven compatible with exact visible atoms.
    pub safe_mint_extensions: u64,
    /// Extension discriminants this parser recognizes on token accounts.
    pub known_account_extensions: u64,
    /// Recognized account extensions proven compatible with exact visible atoms.
    pub safe_account_extensions: u64,
    /// Extensions a Hoard account must carry under this release.
    pub required_hoard_extensions: u64,
    /// Owner-authority protection furnished by this family.
    pub owner_guard: HoardOwnerGuard,
    /// Dangerous behavior the release itself necessarily introduces.
    pub intrinsic_behaviors: u16,
    /// Whether Clutch's adapter surface exposes a token-account owner change.
    pub exposes_owner_authority_change: bool,
    /// Whether ordinary amounts and mint supply are exactly visible.
    pub visible_integer_atoms: bool,
    /// Whether `q` debited is guaranteed to be `q` credited before postchecks.
    pub exact_one_to_one_transfer: bool,
}

impl AdapterRelease {
    /// A model release for legacy SPL Token.  Its release id is intentionally a
    /// non-production placeholder; deployment must replace it with a checked
    /// artifact/release identity.
    pub const fn legacy_spl_model() -> Self {
        Self {
            release_id: [0x11; 32],
            program_id: LEGACY_TOKEN_PROGRAM,
            program_deployment: [0x12; 32],
            family: ProgramFamily::LegacySpl,
            known_mint_extensions: 0,
            safe_mint_extensions: 0,
            known_account_extensions: 0,
            safe_account_extensions: 0,
            required_hoard_extensions: 0,
            owner_guard: HoardOwnerGuard::PdaSoleSigner,
            intrinsic_behaviors: 0,
            exposes_owner_authority_change: false,
            visible_integer_atoms: true,
            exact_one_to_one_transfer: true,
        }
    }

    /// A model release for the conservative Token-2022 base-token profile.
    /// Its release id is intentionally a non-production placeholder.
    pub const fn token_2022_model() -> Self {
        Self {
            release_id: [0x22; 32],
            program_id: TOKEN_2022_PROGRAM,
            program_deployment: [0x23; 32],
            family: ProgramFamily::Token2022,
            known_mint_extensions: TOKEN_2022_KNOWN_EXTENSIONS,
            safe_mint_extensions: 0,
            known_account_extensions: TOKEN_2022_KNOWN_EXTENSIONS,
            safe_account_extensions: EXT_IMMUTABLE_OWNER,
            required_hoard_extensions: EXT_IMMUTABLE_OWNER,
            owner_guard: HoardOwnerGuard::ImmutableOwner,
            intrinsic_behaviors: 0,
            exposes_owner_authority_change: false,
            visible_integer_atoms: true,
            exact_one_to_one_transfer: true,
        }
    }
}

/// The collateral portion of one immutable Realm profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralProfile {
    /// Exact adapter release selected by the profile.
    pub adapter_release: Identity,
    /// Exact token program selected by the profile and release.
    pub token_program: Identity,
    /// Exact external token-program deployment/release selected by the Realm.
    pub token_program_deployment: Identity,
    /// Exact collateral mint.
    pub mint: Identity,
    /// Atom exponent.  It authenticates `TransferChecked`; arithmetic remains
    /// entirely in raw `u64` atoms.
    pub decimals: u8,
    /// Maximum admitted current mint supply in atoms.
    pub max_supply_atoms: u64,
    /// Mint extensions the Realm allows, inside the release ceiling.
    pub allowed_mint_extensions: u64,
    /// Allowed mint extensions that must be present.
    pub required_mint_extensions: u64,
    /// Account extensions the Realm allows, inside the release ceiling.
    pub allowed_account_extensions: u64,
    /// Allowed account extensions every collateral account must carry.
    pub required_account_extensions: u64,
}

/// Independent identity of the Egg issuance adapter. This is deliberately not
/// a field of [`CollateralProfile`]: selecting legacy collateral must not make
/// Eggs legacy SPL tokens, and changing claim semantics must not mutate a
/// Realm's collateral identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimIssuanceBinding {
    /// Content/release identity of the claim mint/burn adapter.
    pub adapter_release: Identity,
    /// Claim token program id.
    pub token_program: Identity,
    /// Checked external claim-program artifact/deployment identity.
    pub token_program_deployment: Identity,
}

/// One authenticated mint snapshot produced by a family-specific parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintSnapshot {
    /// Runtime owner of the mint account.
    pub account_owner_program: Identity,
    /// Mint account address.
    pub address: Identity,
    /// Whether the base state is initialized.
    pub initialized: bool,
    /// Exact raw-atom exponent stored by the mint.
    pub decimals: u8,
    /// Exact raw supply.
    pub supply_atoms: u64,
    /// Whether any mint authority remains.
    pub has_mint_authority: bool,
    /// Whether any freeze authority remains.
    pub has_freeze_authority: bool,
    /// Parsed extension discriminants.
    pub extensions: u64,
    /// Program-independent semantic behaviors discovered by the parser.
    pub behaviors: u16,
    /// Whether amount and supply facts are plaintext exact integers.
    pub amounts_visible: bool,
}

/// One authenticated token-account snapshot produced by a family parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountSnapshot {
    /// Runtime owner of the token account.
    pub account_owner_program: Identity,
    /// Mint stored in the token account.
    pub mint: Identity,
    /// Token owner authority stored in the account.
    pub owner_authority: Identity,
    /// Exact spendable raw-atom balance.
    pub amount_atoms: u64,
    /// Whether the base state is initialized.
    pub initialized: bool,
    /// Whether the token account is frozen.
    pub frozen: bool,
    /// Whether a delegate is present.
    pub has_delegate: bool,
    /// Whether a close authority is present.
    pub has_close_authority: bool,
    /// Parsed extension discriminants.
    pub extensions: u64,
    /// Program-independent semantic behaviors discovered by the parser.
    pub behaviors: u16,
    /// Whether the spendable amount and auxiliary balances are fully visible.
    pub amounts_visible: bool,
}

/// Whether a presented token account is custody or a user's source/destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountRole {
    /// The market-local Hoard, which must have no delegate or close authority.
    Hoard,
    /// A user-controlled account.  Delegate/close policy is the user's, while
    /// mint, owner, state, extension, and exact-atom checks remain mandatory.
    Holder,
}

/// Pre/post facts for one collateral transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferTrace {
    /// Quantity requested, in raw atoms.
    pub requested_atoms: u64,
    /// Decimals carried by the checked-transfer instruction.
    pub instruction_decimals: u8,
    /// Source spendable balance before the transfer.
    pub source_before: u64,
    /// Source spendable balance after the transfer.
    pub source_after: u64,
    /// Destination spendable balance before the transfer.
    pub destination_before: u64,
    /// Destination spendable balance after the transfer.
    pub destination_after: u64,
    /// Mint supply before the transfer.
    pub supply_before: u64,
    /// Mint supply after the transfer.
    pub supply_after: u64,
    /// Total withheld/auxiliary amount before the transfer.
    pub withheld_before: u64,
    /// Total withheld/auxiliary amount after the transfer.
    pub withheld_after: u64,
    /// Number of programs invoked by the token program during transfer.
    pub foreign_program_invocations: u16,
}

/// Realm is the single semantic owner of the selected Profile identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmBinding {
    /// Realm identity.
    pub realm: Identity,
    /// Immutable parent Profile identity selected by this Realm.
    pub profile: Identity,
}

/// Profile is the single semantic owner of its canonical collateral-policy
/// digest and adapter-release identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileBinding {
    /// Parent Profile identity.
    pub profile: Identity,
    /// Digest recomputed from the presented canonical policy bytes.
    pub collateral_policy_digest: Identity,
    /// Adapter release committed inside the successor policy.
    pub adapter_release: Identity,
}

/// Markets copy only Realm/Profile references, not collateral DTO fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketBinding {
    /// Realm selected when the market was created.
    pub realm: Identity,
    /// Parent Profile selected when the market was created.
    pub profile: Identity,
}

/// Why a collateral adapter conformance check refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// Realm, Profile, or Market identities disagree.
    BindingMismatch,
    /// The presented policy digest is not the Profile's frozen digest.
    PolicyDigestMismatch,
    /// The Realm-selected adapter release is not the compiled release.
    UnknownAdapterRelease,
    /// Program id or family does not match the release/profile.
    WrongProgram,
    /// A profile or release carries a zero identity or another malformed core field.
    InvalidProfile,
    /// The release itself cannot satisfy the exact-visible-atom contract.
    UnsafeRelease,
    /// Profile extension masks exceed or contradict the release ceiling.
    InvalidExtensionPolicy,
    /// The wrong mint was presented.
    WrongMint,
    /// The mint or token account is not initialized.
    Uninitialized,
    /// Mint decimals or checked-transfer decimals differ from the profile.
    WrongDecimals,
    /// Supply is zero or exceeds the immutable profile ceiling.
    SupplyNotAdmitted,
    /// A forbidden mint/freeze authority remains.
    MintAuthorityNotAdmitted,
    /// An extension is unknown, disallowed, misplaced, or missing.
    ExtensionNotAdmitted,
    /// A dangerous semantic behavior was observed.
    DangerousBehavior,
    /// Exact integer amounts are not observable.
    OpaqueAmount,
    /// A token account is frozen.
    FrozenAccount,
    /// The token owner authority is not the expected signer/PDA.
    WrongAccountOwner,
    /// The Hoard has a delegate.
    DelegatePresent,
    /// The Hoard has a close authority.
    CloseAuthorityPresent,
    /// The release's Hoard owner guard was not established.
    OwnerGuardUnavailable,
    /// Pre/post source, destination, or supply deltas were not exact.
    TransferDeltaMismatch,
    /// A withheld/auxiliary balance was nonzero or changed.
    WithheldBalance,
    /// The token transfer invoked a foreign program.
    ForeignProgramInvocation,
}

/// Validate the semantic contract of a compiled adapter release.
pub fn validate_release(release: &AdapterRelease) -> Result<(), Refusal> {
    if release.release_id == [0; 32]
        || release.program_id == [0; 32]
        || release.program_deployment == [0; 32]
        || release.intrinsic_behaviors & behavior::ALL_REFUSED != 0
        || !release.visible_integer_atoms
        || !release.exact_one_to_one_transfer
        || release.safe_mint_extensions & !release.known_mint_extensions != 0
        || release.safe_account_extensions & !release.known_account_extensions != 0
        || release.required_hoard_extensions & !release.safe_account_extensions != 0
    {
        return Err(Refusal::UnsafeRelease);
    }
    match release.owner_guard {
        HoardOwnerGuard::ImmutableOwner => {
            if release.required_hoard_extensions == 0 {
                return Err(Refusal::OwnerGuardUnavailable);
            }
        }
        HoardOwnerGuard::PdaSoleSigner => {
            if release.exposes_owner_authority_change {
                return Err(Refusal::OwnerGuardUnavailable);
            }
        }
    }
    match release.family {
        ProgramFamily::LegacySpl => {
            if release.program_id != LEGACY_TOKEN_PROGRAM
                || release.known_mint_extensions != 0
                || release.known_account_extensions != 0
                || release.safe_mint_extensions != 0
                || release.safe_account_extensions != 0
                || release.required_hoard_extensions != 0
                || !matches!(release.owner_guard, HoardOwnerGuard::PdaSoleSigner)
            {
                return Err(Refusal::UnsafeRelease);
            }
        }
        ProgramFamily::Token2022 => {
            if release.program_id != TOKEN_2022_PROGRAM
                || release.known_mint_extensions != TOKEN_2022_KNOWN_EXTENSIONS
                || release.known_account_extensions != TOKEN_2022_KNOWN_EXTENSIONS
                || release.safe_mint_extensions != 0
                || release.safe_account_extensions != EXT_IMMUTABLE_OWNER
                || release.required_hoard_extensions != EXT_IMMUTABLE_OWNER
                || !matches!(release.owner_guard, HoardOwnerGuard::ImmutableOwner)
            {
                return Err(Refusal::UnsafeRelease);
            }
        }
        ProgramFamily::Future(_) => {}
    }
    Ok(())
}

/// Resolve a frozen profile only against the exact compiled adapter release it
/// selected.  An arbitrary token program is never treated as a generic token.
pub fn validate_profile(
    profile: &CollateralProfile,
    release: &AdapterRelease,
) -> Result<(), Refusal> {
    validate_release(release)?;
    if profile.adapter_release != release.release_id {
        return Err(Refusal::UnknownAdapterRelease);
    }
    if profile.token_program != release.program_id {
        return Err(Refusal::WrongProgram);
    }
    if profile.token_program_deployment != release.program_deployment {
        return Err(Refusal::UnknownAdapterRelease);
    }
    if profile.mint == [0; 32] {
        return Err(Refusal::InvalidProfile);
    }
    if profile.max_supply_atoms == 0
        || profile.required_mint_extensions & !profile.allowed_mint_extensions != 0
        || profile.required_account_extensions & !profile.allowed_account_extensions != 0
        || profile.allowed_mint_extensions & !release.safe_mint_extensions != 0
        || profile.allowed_account_extensions & !release.safe_account_extensions != 0
        || release.required_hoard_extensions & !profile.allowed_account_extensions != 0
    {
        return Err(Refusal::InvalidExtensionPolicy);
    }
    Ok(())
}

fn check_extensions(present: u64, known: u64, allowed: u64, required: u64) -> Result<(), Refusal> {
    if present & !known != 0 || present & !allowed != 0 || required & !present != 0 {
        Err(Refusal::ExtensionNotAdmitted)
    } else {
        Ok(())
    }
}

/// Admit an authenticated mint snapshot under a resolved profile/release.
/// Concrete adapters must rerun this check at every collateral transition,
/// because initialization-time admission does not prove absence of later drift.
pub fn admit_mint(
    profile: &CollateralProfile,
    release: &AdapterRelease,
    mint: &MintSnapshot,
) -> Result<(), Refusal> {
    validate_profile(profile, release)?;
    if mint.account_owner_program != release.program_id {
        return Err(Refusal::WrongProgram);
    }
    if mint.address != profile.mint {
        return Err(Refusal::WrongMint);
    }
    if !mint.initialized {
        return Err(Refusal::Uninitialized);
    }
    if mint.decimals != profile.decimals {
        return Err(Refusal::WrongDecimals);
    }
    if mint.supply_atoms == 0 || mint.supply_atoms > profile.max_supply_atoms {
        return Err(Refusal::SupplyNotAdmitted);
    }
    if mint.has_mint_authority || mint.has_freeze_authority {
        return Err(Refusal::MintAuthorityNotAdmitted);
    }
    if !mint.amounts_visible {
        return Err(Refusal::OpaqueAmount);
    }
    if mint.behaviors & behavior::ALL_REFUSED != 0 {
        return Err(Refusal::DangerousBehavior);
    }
    check_extensions(
        mint.extensions,
        release.known_mint_extensions,
        profile.allowed_mint_extensions,
        profile.required_mint_extensions,
    )
}

/// Admit an authenticated collateral token-account snapshot.
pub fn admit_account(
    profile: &CollateralProfile,
    release: &AdapterRelease,
    account: &AccountSnapshot,
    expected_owner: Identity,
    expected_owner_is_program_derived: bool,
    role: AccountRole,
) -> Result<(), Refusal> {
    validate_profile(profile, release)?;
    if account.account_owner_program != release.program_id {
        return Err(Refusal::WrongProgram);
    }
    if account.mint != profile.mint {
        return Err(Refusal::WrongMint);
    }
    if !account.initialized {
        return Err(Refusal::Uninitialized);
    }
    if account.frozen {
        return Err(Refusal::FrozenAccount);
    }
    if account.owner_authority != expected_owner {
        return Err(Refusal::WrongAccountOwner);
    }
    if !account.amounts_visible {
        return Err(Refusal::OpaqueAmount);
    }
    if account.behaviors & behavior::ALL_REFUSED != 0 {
        return Err(Refusal::DangerousBehavior);
    }
    let mut required = profile.required_account_extensions;
    if matches!(role, AccountRole::Hoard) {
        if account.has_delegate {
            return Err(Refusal::DelegatePresent);
        }
        if account.has_close_authority {
            return Err(Refusal::CloseAuthorityPresent);
        }
        required |= release.required_hoard_extensions;
        if matches!(release.owner_guard, HoardOwnerGuard::PdaSoleSigner)
            && !expected_owner_is_program_derived
        {
            return Err(Refusal::OwnerGuardUnavailable);
        }
    }
    check_extensions(
        account.extensions,
        release.known_account_extensions,
        profile.allowed_account_extensions,
        required,
    )
}

/// Verify the exact-visible-atom postcondition of one checked collateral
/// transfer.  This is mandatory even after mint/account admission: it catches
/// a parser or semantic-classification mistake before ledger credit is kept.
pub fn verify_transfer(
    profile: &CollateralProfile,
    release: &AdapterRelease,
    trace: &TransferTrace,
) -> Result<(), Refusal> {
    validate_profile(profile, release)?;
    if trace.instruction_decimals != profile.decimals {
        return Err(Refusal::WrongDecimals);
    }
    if trace.withheld_before != 0 || trace.withheld_after != 0 {
        return Err(Refusal::WithheldBalance);
    }
    if trace.foreign_program_invocations != 0 {
        return Err(Refusal::ForeignProgramInvocation);
    }
    let source_debit = trace.source_before.checked_sub(trace.source_after);
    let destination_credit = trace
        .destination_after
        .checked_sub(trace.destination_before);
    if source_debit != Some(trace.requested_atoms)
        || destination_credit != Some(trace.requested_atoms)
        || trace.supply_before != trace.supply_after
    {
        return Err(Refusal::TransferDeltaMismatch);
    }
    Ok(())
}

/// Authenticate the one-owner Realm → Profile → policy → adapter chain.
/// `presented_policy_digest` must be recomputed from canonical bytes by the
/// concrete codec before this function is called.
pub fn authenticate_binding(
    market: &MarketBinding,
    realm: &RealmBinding,
    profile: &ProfileBinding,
    presented_policy_digest: Identity,
    selected_release: Identity,
) -> Result<(), Refusal> {
    if market.realm != realm.realm
        || market.profile != realm.profile
        || realm.profile != profile.profile
    {
        return Err(Refusal::BindingMismatch);
    }
    if presented_policy_digest != profile.collateral_policy_digest {
        return Err(Refusal::PolicyDigestMismatch);
    }
    if selected_release != profile.adapter_release {
        return Err(Refusal::UnknownAdapterRelease);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINT: Identity = [0x44; 32];
    const OWNER: Identity = [0x55; 32];

    fn profile(release: AdapterRelease) -> CollateralProfile {
        CollateralProfile {
            adapter_release: release.release_id,
            token_program: release.program_id,
            token_program_deployment: release.program_deployment,
            mint: MINT,
            decimals: 6,
            max_supply_atoms: 1_000_000_000,
            allowed_mint_extensions: 0,
            required_mint_extensions: 0,
            allowed_account_extensions: release.safe_account_extensions,
            required_account_extensions: 0,
        }
    }

    fn mint(release: AdapterRelease) -> MintSnapshot {
        MintSnapshot {
            account_owner_program: release.program_id,
            address: MINT,
            initialized: true,
            decimals: 6,
            supply_atoms: 9_000_000,
            has_mint_authority: false,
            has_freeze_authority: false,
            extensions: 0,
            behaviors: 0,
            amounts_visible: true,
        }
    }

    fn account(release: AdapterRelease) -> AccountSnapshot {
        AccountSnapshot {
            account_owner_program: release.program_id,
            mint: MINT,
            owner_authority: OWNER,
            amount_atoms: 3_000_000,
            initialized: true,
            frozen: false,
            has_delegate: false,
            has_close_authority: false,
            extensions: release.required_hoard_extensions,
            behaviors: 0,
            amounts_visible: true,
        }
    }

    fn trace() -> TransferTrace {
        TransferTrace {
            requested_atoms: 1,
            instruction_decimals: 6,
            source_before: 10,
            source_after: 9,
            destination_before: 20,
            destination_after: 21,
            supply_before: 100,
            supply_after: 100,
            withheld_before: 0,
            withheld_after: 0,
            foreign_program_invocations: 0,
        }
    }

    #[test]
    fn token_2022_and_legacy_are_distinct_conforming_collateral_profiles() {
        for release in [
            AdapterRelease::token_2022_model(),
            AdapterRelease::legacy_spl_model(),
        ] {
            let policy = profile(release);
            validate_profile(&policy, &release).expect("profile must resolve");
            admit_mint(&policy, &release, &mint(release)).expect("mint must admit");
            admit_account(
                &policy,
                &release,
                &account(release),
                OWNER,
                true,
                AccountRole::Hoard,
            )
            .expect("Hoard must admit");
            verify_transfer(&policy, &release, &trace()).expect("one atom stays one atom");
        }
    }

    #[test]
    fn legacy_is_conditional_on_pda_custody_and_no_authority_change_route() {
        let release = AdapterRelease::legacy_spl_model();
        let policy = profile(release);
        let hoard = account(release);
        assert_eq!(
            admit_account(&policy, &release, &hoard, OWNER, false, AccountRole::Hoard,),
            Err(Refusal::OwnerGuardUnavailable)
        );

        let mut unsafe_release = release;
        unsafe_release.exposes_owner_authority_change = true;
        assert_eq!(
            validate_release(&unsafe_release),
            Err(Refusal::OwnerGuardUnavailable)
        );

        let mut invented_extension = policy;
        invented_extension.allowed_account_extensions = EXT_IMMUTABLE_OWNER;
        assert_eq!(
            validate_profile(&invented_extension, &release),
            Err(Refusal::InvalidExtensionPolicy)
        );
    }

    #[test]
    fn token_2022_hoard_requires_immutable_owner() {
        let release = AdapterRelease::token_2022_model();
        let policy = profile(release);
        let mut hoard = account(release);
        hoard.extensions = 0;
        assert_eq!(
            admit_account(&policy, &release, &hoard, OWNER, true, AccountRole::Hoard,),
            Err(Refusal::ExtensionNotAdmitted)
        );
    }

    #[test]
    fn hostile_token_behaviors_refuse_independently_of_extension_numbers() {
        let release = AdapterRelease::token_2022_model();
        let policy = profile(release);
        for hostile in [
            behavior::FEE_ON_TRANSFER,
            behavior::TRANSFER_HOOK,
            behavior::CONFIDENTIAL,
            behavior::NONTRANSFERABLE,
            behavior::DEFAULT_FROZEN,
            behavior::PERMANENT_DELEGATE,
            behavior::PAUSABLE,
            behavior::SCALED_UI,
        ] {
            let mut observation = mint(release);
            observation.behaviors = hostile;
            assert_eq!(
                admit_mint(&policy, &release, &observation),
                Err(Refusal::DangerousBehavior)
            );
        }
    }

    #[test]
    fn extension_drift_refuses_when_admission_is_rerun() {
        let release = AdapterRelease::token_2022_model();
        let policy = profile(release);
        let before = mint(release);
        admit_mint(&policy, &release, &before).expect("base mint admits");

        let mut after = before;
        after.extensions = EXT_TRANSFER_HOOK;
        assert_eq!(
            admit_mint(&policy, &release, &after),
            Err(Refusal::ExtensionNotAdmitted)
        );
        after.behaviors = behavior::TRANSFER_HOOK;
        assert_eq!(
            admit_mint(&policy, &release, &after),
            Err(Refusal::DangerousBehavior)
        );

        let mut wrongly_widened_release = release;
        wrongly_widened_release.safe_mint_extensions = EXT_TRANSFER_FEE_CONFIG;
        assert_eq!(
            validate_release(&wrongly_widened_release),
            Err(Refusal::UnsafeRelease)
        );
    }

    #[test]
    fn exact_postconditions_catch_fees_hooks_and_supply_changes() {
        let release = AdapterRelease::token_2022_model();
        let policy = profile(release);

        let mut fee = trace();
        fee.destination_after = 20;
        fee.withheld_after = 1;
        assert_eq!(
            verify_transfer(&policy, &release, &fee),
            Err(Refusal::WithheldBalance)
        );

        let mut hook = trace();
        hook.foreign_program_invocations = 1;
        assert_eq!(
            verify_transfer(&policy, &release, &hook),
            Err(Refusal::ForeignProgramInvocation)
        );

        let mut minted = trace();
        minted.supply_after = 101;
        assert_eq!(
            verify_transfer(&policy, &release, &minted),
            Err(Refusal::TransferDeltaMismatch)
        );
    }

    #[test]
    fn frozen_opaque_delegated_or_closeable_hoards_refuse() {
        let release = AdapterRelease::token_2022_model();
        let policy = profile(release);
        for expected in [
            Refusal::FrozenAccount,
            Refusal::OpaqueAmount,
            Refusal::DelegatePresent,
            Refusal::CloseAuthorityPresent,
        ] {
            let mut hoard = account(release);
            match expected {
                Refusal::FrozenAccount => hoard.frozen = true,
                Refusal::OpaqueAmount => hoard.amounts_visible = false,
                Refusal::DelegatePresent => hoard.has_delegate = true,
                Refusal::CloseAuthorityPresent => hoard.has_close_authority = true,
                _ => unreachable!(),
            }
            assert_eq!(
                admit_account(&policy, &release, &hoard, OWNER, true, AccountRole::Hoard,),
                Err(expected)
            );
        }
    }

    #[test]
    fn decimals_authenticate_atoms_but_never_rescale_them() {
        let release = AdapterRelease::token_2022_model();
        let policy = profile(release);
        verify_transfer(&policy, &release, &trace()).expect("one raw atom transfers exactly");

        let mut wrong_instruction_scale = trace();
        wrong_instruction_scale.instruction_decimals = 9;
        assert_eq!(
            verify_transfer(&policy, &release, &wrong_instruction_scale),
            Err(Refusal::WrongDecimals)
        );

        let mut wrong_mint_scale = mint(release);
        wrong_mint_scale.decimals = 9;
        assert_eq!(
            admit_mint(&policy, &release, &wrong_mint_scale),
            Err(Refusal::WrongDecimals)
        );
    }

    #[test]
    fn a_future_family_needs_an_explicit_safe_release() {
        let future_program = [0x77; 32];
        let release = AdapterRelease {
            release_id: [0x88; 32],
            program_id: future_program,
            program_deployment: [0x89; 32],
            family: ProgramFamily::Future(7),
            known_mint_extensions: 0,
            safe_mint_extensions: 0,
            known_account_extensions: 0,
            safe_account_extensions: 0,
            required_hoard_extensions: 0,
            owner_guard: HoardOwnerGuard::PdaSoleSigner,
            intrinsic_behaviors: 0,
            exposes_owner_authority_change: false,
            visible_integer_atoms: true,
            exact_one_to_one_transfer: true,
        };
        let policy = profile(release);
        validate_profile(&policy, &release).expect("explicit future release may conform");

        let mut unknown = policy;
        unknown.adapter_release = [0x99; 32];
        assert_eq!(
            validate_profile(&unknown, &release),
            Err(Refusal::UnknownAdapterRelease)
        );

        let mut wrong_deployment = policy;
        wrong_deployment.token_program_deployment = [0x9a; 32];
        assert_eq!(
            validate_profile(&wrong_deployment, &release),
            Err(Refusal::UnknownAdapterRelease)
        );

        let mut opaque = release;
        opaque.visible_integer_atoms = false;
        assert_eq!(validate_release(&opaque), Err(Refusal::UnsafeRelease));
    }

    #[test]
    fn realm_profile_policy_and_release_are_one_immutable_chain() {
        let realm = RealmBinding {
            realm: [0xa1; 32],
            profile: [0xa2; 32],
        };
        let profile = ProfileBinding {
            profile: [0xa2; 32],
            collateral_policy_digest: [0xa3; 32],
            adapter_release: [0xa4; 32],
        };
        let market = MarketBinding {
            realm: [0xa1; 32],
            profile: [0xa2; 32],
        };
        authenticate_binding(&market, &realm, &profile, [0xa3; 32], [0xa4; 32])
            .expect("exact chain binds");

        let mut other_market = market;
        other_market.profile = [0xb2; 32];
        assert_eq!(
            authenticate_binding(&other_market, &realm, &profile, [0xa3; 32], [0xa4; 32]),
            Err(Refusal::BindingMismatch)
        );
        assert_eq!(
            authenticate_binding(&market, &realm, &profile, [0xb3; 32], [0xa4; 32]),
            Err(Refusal::PolicyDigestMismatch)
        );
        assert_eq!(
            authenticate_binding(&market, &realm, &profile, [0xa3; 32], [0xb4; 32]),
            Err(Refusal::UnknownAdapterRelease)
        );
    }

    #[test]
    fn legacy_collateral_does_not_change_token_2022_egg_issuance_identity() {
        let collateral_release = AdapterRelease::legacy_spl_model();
        let collateral = profile(collateral_release);
        validate_profile(&collateral, &collateral_release).expect("legacy collateral resolves");

        let claims = ClaimIssuanceBinding {
            adapter_release: [0xc1; 32],
            token_program: TOKEN_2022_PROGRAM,
            token_program_deployment: [0xc2; 32],
        };
        assert_eq!(collateral.token_program, LEGACY_TOKEN_PROGRAM);
        assert_eq!(claims.token_program, TOKEN_2022_PROGRAM);
        assert_ne!(collateral.adapter_release, claims.adapter_release);
        assert_ne!(
            collateral.token_program_deployment,
            claims.token_program_deployment
        );
    }
}
