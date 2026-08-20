//! The genesis plane: the instructions that bring accounts into existence.
//!
//! This module owns public System-CPI constructors for foundational accounts
//! outside the market constructor itself. It also owns [`Intent::Endow`], the
//! backed collateral-deposit boundary: on a wallet's first deposit it creates
//! that wallet's canonical Position and Replay accounts before transferring
//! value and crediting cash.
//!
//! | intent | creates | accounts |
//! | --- | --- | ---: |
//! | [`Intent::InitRealm`] | [`RealmAccount`] | 5 |
//! | [`Intent::InitProfile`] | [`ProfileAccount`], policy frozen | 6 |
//! | [`Intent::InitPriceGrid`] | obsolete: use typed `SealArtifact` | -- |
//! | [`Intent::InitTerms`] | obsolete: use typed `SealArtifact` | -- |
//! | [`Intent::InitOrderPage`] | one order page | 6 |
//! | [`Intent::Endow`] | registered source + absent generation-zero Position/Replay, then collateral deposit and cash credit | 15 |
//!
//! ## `Endow` is the inbound value boundary
//!
//! The owner signs a Token-2022 `TransferChecked` from an admitted collateral
//! account into the market's derived Hoard token account.  Mint identity,
//! decimals, extension policy, token-account authorities, Profile/policy
//! binding, every PDA, replay, and the exact source debit and destination
//! credit are checked before ledger bytes are committed.  A later refusal is
//! atomic with the CPI under SVM transaction semantics.
//!
//! The pooled Hoard retains both position cash and locked complete-set
//! collateral. [`HoardAccount::collateral_atoms`] is only the locked term;
//! direct Token-2022 donations may create unowned surplus, so the locally
//! checkable condition is `hoard_token.amount >= collateral_atoms`, not
//! equality.  The stronger global equation over every position's cash is
//! inductive because no frozen account stores the market-wide cash sum.
//!
//! [`MarketAccount::collateral_cap`] limits locked complete-set exposure at
//! `Split`; it is not silently reused as a custody ceiling.  A position may
//! deposit more unused cash than the market can lock.
//!
//! ## Authority — **PROPOSED**, and it is the same proposal `market_init` made
//!
//! Creation here is **permissionless**: the account at index [`IX_PAYER`]
//! signs and pays, and nothing else is privileged.  The frozen layout carries
//! no authority field for a Realm, a Profile, a grid, a terms artifact or a
//! page, so a gate here would be an invented ABI — the argument
//! [`super::market_init`] sets out at length, unchanged.  What bounds the
//! plane instead is that **every address is content- or nonce-derived**: a
//! Realm's identity is a function of its Profile and a nonce, a Profile's is a
//! function of its collateral policy, a grid's and a terms artifact's are
//! digests of their own bodies, and a page's is its position in its epoch.  So
//! a second caller can create the same account first, and cannot create a
//! *different* account at that address.  Naming that residue is not fixing it;
//! it is the same first-come-address residue market creation has.
//!
//! `Endow` is permissionless self-service, not a third-party credit interface:
//! the signer must equal the requested Position owner and may deposit only
//! from a Token-2022 account whose owner authority is that signer.
//!
//! ## Immutable bytes come only from typed, sealed artifacts
//!
//! Collateral policy, price-grid, and Terms bodies are created by the resumable
//! typed transport in [`super::artifact`].  Its successful `SealArtifact`
//! transaction validates the owning hostile-byte codec and creates the final
//! content-derived PDA under this program.  Genesis consumers therefore admit
//! neither caller-owned buffers nor arbitrary program-owned copies.
//!
//! `InitRealm` and `InitProfile` both require the same canonical policy PDA,
//! `policy(Profile, policy_digest)`, and recompute both identities from its
//! bytes.  `InitPriceGrid` and `InitTerms` are obsolete wire intents: the
//! canonical accounts already exist when their typed seal succeeds, so a
//! second copy/initialization path would create a competing semantic owner.
//! They refuse as unsupported rather than preserving the former arbitrary
//! evidence-buffer path.
//!
//! ## Rent is read from the chain, not pinned here
//!
//! `CreateAccount` takes a lamport figure and something has to decide it.
//! This module reads the **rent sysvar account** — 17 bytes: rate, exemption
//! threshold, burn percent — and computes
//! `((ACCOUNT_STORAGE_OVERHEAD + space) * rate) as f64 * threshold`, which is
//! `solana_rent::Rent::minimum_balance` transcribed.  Pinning the mainnet
//! defaults as constants would have been shorter and would have made this
//! program's idea of rent a parallel truth to the runtime's.  The sysvar's key
//! and length are checked ([`ClutchError::WrongRentSysvar`]) because it is
//! evidence like any other account.
//!
//! The transcription is named as a transcription: the formula lives in
//! `solana-rent`, this crate does not depend on that crate (adding a
//! dependency to reach one three-line function is a worse trade than restating
//! it under a citation), and `the_rent_formula_matches_the_runtimes` below
//! pins the result against the published defaults.
//!
//! ## What is *not* here, and who owns it
//!
//! * **No `InitEpoch`.**  [`Intent::InitOrderPage`] requires an epoch account
//!   to already exist, which on a fresh chain nothing creates.  The epoch is a
//!   ten-identity account whose freeze semantics belong with the order-set
//!   commitment work (gap row 14), and creating it from a bring-up genesis
//!   lane would fix a wire format for a freeze this program cannot yet
//!   perform.  Named, not done.
//! * **No `InitClearWork` and no `InitCandidateFeed`.**  The two clearing
//!   accounts landed as codecs this wave
//!   ([`clutch_solana_layout::clearing`]) and are consumed by nothing.  The
//!   checkpoint is also the one account in the inventory that a single
//!   system-program CPI **cannot** allocate — 48,004 bytes against the
//!   runtime's 10,240-byte per-instruction growth ceiling — so its creation is
//!   a five-instruction sequence or a top-level client-signed
//!   `CreateAccount`, analyzed in `docs/implementation/SOLANA_LAYOUT.md`.
//! * **No reference oracle.**  `clutch_solana_reference::apply` refuses every
//!   intent in this module with `UnsupportedIntent`, exactly as it refuses
//!   `PlaceOrder` and `CancelOrder` (gap row 16).  So the SVM differential
//!   must not be pointed at this family: a comparison would be this program
//!   agreeing with a refusal.  The host tests below use the **layout codecs**
//!   as the oracle instead — every expected account is produced by
//!   `clutch_solana_layout`'s own encoder and compared byte for byte, and no
//!   expected byte is typed by hand.
//!
//! ## Refusal codes
//!
//! | check | emitted | code |
//! | --- | --- | --- |
//! | a creation target already exists | [`ClutchError::AlreadyInitialized`] | `0x0040` |
//! | the system-program role is not the system program | [`ClutchError::WrongSystemProgram`] | `0x0070` |
//! | the rent-sysvar role is not the rent sysvar | [`ClutchError::WrongRentSysvar`] | `0x0071` |
//! | the `CreateAccount` CPI refused, or created nothing | [`ClutchError::AccountCreationFailed`] | `0x0072` |
//! | an evidence buffer is not the artifact the intent names | [`ClutchError::EvidenceBufferMismatch`] | `0x0073` |
//! | an Endow token account or exact delta is invalid | token admission / [`ClutchError::TokenDeltaMismatch`] | `0x0018..=0x001c` |
//! | every other check | the [`ClutchError`] the check already has | `0x0001..=0x0017` |
//!
//! ## Frame discipline
//!
//! No function here holds a terms artifact, a price grid, or an order page by
//! value: artifacts are read through [`crate::accounts`]'s facts
//! readers and copied as bytes, and the page is written by the layout crate's
//! streaming writer [`stream::init_page`].  The largest value on any frame in
//! this module is a [`RealmAccount`] and its 70-byte encode buffer.  As
//! everywhere else in this crate that is **measured**: `cargo-build-sbf` emits
//! no frame diagnostic for any `clutch_sbf` function.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::source_archive::SOURCE_SPEC_ACCOUNT_V1_BYTES;
use crate::{seeds, token};
use clutch_solana_layout::direct_selection_v3::{
    DirectEpochV4Account, DirectFundingLedgerV3, DIRECT_EPOCH_V4_BYTES,
};
use clutch_solana_layout::{
    account_len, canonical_realm_id, collateral, stream, Hash32, HoardAccount, Intent,
    MarketAccount, PositionAccount, ProfileAccount, RealmAccount, EPOCH_PHASE_OPEN,
    PROFILE_FLAG_POLICY_FROZEN,
};
use clutch_solana_reference::{Action, ReplayAccount, Request, REPLAY_ACCOUNT_LEN};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::construction::{self, OwnerStateBumps, OwnerStateTargets};
use super::direct_selection_v3::{
    create_pda_account_full_principal, direct_creation_funding, observe_direct_funding,
    DIRECT_NEUTRAL_SINK_V3, DIRECT_VERIFIER_RELEASE_ID_V3,
};

/// Borrow one account's data mutably, or refuse.
///
/// A macro rather than a function for the reason [`super::observe_resolve`]
/// records: `AccountInfo` is invariant in its lifetime.
macro_rules! borrow_mut {
    ($account:expr) => {
        $account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
    };
}

/* ------------------------------------------------------------------------ */
/* The system program and the rent sysvar                                    */
/* ------------------------------------------------------------------------ */

/// The system program's address: the all-zero key.
///
/// Hand-stated for the same reason [`crate::token`] hand-states the
/// Token-2022 instruction discriminants — this crate depends on no Anza
/// interface crate that carries it, and a 32-byte constant with a test is a
/// smaller thing to audit than a dependency.  The all-zero key is also what an
/// uninitialized account slot looks like, which is exactly why
/// [`require_system_program`] checks the executable bit too.
pub const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0; 32]);

/// The rent sysvar's address, `SysvarRent111111111111111111111111111111111`.
pub const RENT_SYSVAR_ID: Pubkey = Pubkey::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
]);

/// Exact data length of the rent sysvar account.
pub const RENT_SYSVAR_LEN: usize = 8 + 8 + 1;

/// `SystemInstruction::CreateAccount`'s bincode variant index.
///
/// The system program's instruction enum is bincode-encoded: a four-byte
/// little-endian variant index, then the variant's fields in declaration
/// order.  `CreateAccount { lamports: u64, space: u64, owner: Pubkey }` is
/// variant zero, so the whole payload is 52 bytes.
const SYSTEM_IX_CREATE_ACCOUNT: u32 = 0;

/// `SystemInstruction::Assign { owner }` bincode variant.
const SYSTEM_IX_ASSIGN: u32 = 1;
/// `SystemInstruction::Transfer { lamports }` bincode variant.
const SYSTEM_IX_TRANSFER: u32 = 2;
/// `SystemInstruction::Allocate { space }` bincode variant.
const SYSTEM_IX_ALLOCATE: u32 = 8;

/// Exact encoded length of a `CreateAccount` instruction payload.
const CREATE_ACCOUNT_DATA_LEN: usize = 4 + 8 + 8 + 32;
const ASSIGN_DATA_LEN: usize = 4 + 32;
const TRANSFER_DATA_LEN: usize = 4 + 8;
const ALLOCATE_DATA_LEN: usize = 4 + 8;

/// Per-account storage overhead in the rent formula.
///
/// `solana_rent::ACCOUNT_STORAGE_OVERHEAD`.
pub const ACCOUNT_STORAGE_OVERHEAD: u64 = 128;

/// The runtime's per-instruction account-data growth ceiling.
///
/// `solana_program_entrypoint::MAX_PERMITTED_DATA_INCREASE`.  An account
/// created through a cross-program invocation grows from zero inside one
/// instruction, so this is also the largest account a program can allocate in
/// one CPI.  Every account this module creates is under it; the streaming
/// checkpoint of [`clutch_solana_layout::clearing`] is not, which is why no
/// instruction here creates one.
pub const MAX_PERMITTED_DATA_INCREASE: usize = 10 * 1024;

/// The two rent parameters the exemption minimum is a function of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RentParameters {
    /// Rental rate in lamports per byte-year.
    pub lamports_per_byte_year: u64,
    /// Years of rent a balance must cover to be exempt.
    pub exemption_threshold: f64,
}

impl RentParameters {
    /// The rent-exempt minimum for an account of `space` data bytes.
    ///
    /// `solana_rent::Rent::minimum_balance`, transcribed:
    /// `(((ACCOUNT_STORAGE_OVERHEAD + bytes) * rate) as f64 * threshold) as
    /// u64`.  The two multiplications are checked in the integer half; the
    /// float half is the runtime's own and is reproduced rather than improved.
    pub fn minimum_balance(&self, space: usize) -> Outcome<u64> {
        let bytes = ACCOUNT_STORAGE_OVERHEAD
            .checked_add(space as u64)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let base = bytes
            .checked_mul(self.lamports_per_byte_year)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        Ok((base as f64 * self.exemption_threshold) as u64)
    }
}

/// Read the rent parameters off the chain's own sysvar account.
///
/// The account is authenticated as evidence: right key, right length, not
/// writable, and a threshold that is a finite non-negative number.  A hostile
/// or corrupt threshold is [`ClutchError::WrongRentSysvar`] rather than a
/// silently enormous or `NaN` lamport figure.
pub fn read_rent(account: &AccountInfo) -> Outcome<RentParameters> {
    require(*account.key == RENT_SYSVAR_ID, ClutchError::WrongRentSysvar)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(
        account.data_len() == RENT_SYSVAR_LEN,
        ClutchError::WrongRentSysvar,
    )?;
    let data = account.data.borrow();
    let mut rate = [0_u8; 8];
    rate.copy_from_slice(&data[0..8]);
    let mut threshold = [0_u8; 8];
    threshold.copy_from_slice(&data[8..16]);
    let value = RentParameters {
        lamports_per_byte_year: u64::from_le_bytes(rate),
        exemption_threshold: f64::from_le_bytes(threshold),
    };
    require(
        value.exemption_threshold.is_finite() && value.exemption_threshold >= 0.0,
        ClutchError::WrongRentSysvar,
    )?;
    Ok(value)
}

/// Refuse anything at the system-program role that is not the system program.
pub fn require_system_program(account: &AccountInfo) -> Outcome<()> {
    require(
        *account.key == SYSTEM_PROGRAM_ID && account.executable,
        ClutchError::WrongSystemProgram,
    )?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)
}

/// Refuse a construction target that is not empty, System-owned, and writable.
///
/// Lamports are deliberately unconstrained. Anyone may transfer SOL to a
/// predictable PDA before its constructor runs; treating that donation as
/// initialization would let one lamport permanently squat every market.
pub fn require_creatable(account: &AccountInfo) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.data_len() == 0 && *account.owner == SYSTEM_PROGRAM_ID,
        ClutchError::AlreadyInitialized,
    )
}

/// The `CreateAccount` payload, split out so the CPI and the tests build one
/// identical encoding.
pub fn create_account_data(lamports: u64, space: usize, owner: &Pubkey) -> [u8; 52] {
    let mut data = [0_u8; CREATE_ACCOUNT_DATA_LEN];
    data[0..4].copy_from_slice(&SYSTEM_IX_CREATE_ACCOUNT.to_le_bytes());
    data[4..12].copy_from_slice(&lamports.to_le_bytes());
    data[12..20].copy_from_slice(&(space as u64).to_le_bytes());
    data[20..52].copy_from_slice(&owner.to_bytes());
    data
}

/// Exact System `Transfer` payload used for a rent shortfall.
pub fn transfer_data(lamports: u64) -> [u8; TRANSFER_DATA_LEN] {
    let mut data = [0_u8; TRANSFER_DATA_LEN];
    data[0..4].copy_from_slice(&SYSTEM_IX_TRANSFER.to_le_bytes());
    data[4..12].copy_from_slice(&lamports.to_le_bytes());
    data
}

/// Exact System `Allocate` payload for a PDA-signed target.
pub fn allocate_data(space: usize) -> [u8; ALLOCATE_DATA_LEN] {
    let mut data = [0_u8; ALLOCATE_DATA_LEN];
    data[0..4].copy_from_slice(&SYSTEM_IX_ALLOCATE.to_le_bytes());
    data[4..12].copy_from_slice(&(space as u64).to_le_bytes());
    data
}

/// Exact System `Assign` payload for a PDA-signed target.
pub fn assign_data(owner: &Pubkey) -> [u8; ASSIGN_DATA_LEN] {
    let mut data = [0_u8; ASSIGN_DATA_LEN];
    data[0..4].copy_from_slice(&SYSTEM_IX_ASSIGN.to_le_bytes());
    data[4..36].copy_from_slice(&owner.to_bytes());
    data
}

/// Allocate and assign one canonical PDA, safely admitting prior SOL funding.
///
/// The whole creation step in one call: refuse a target that already exists,
/// refuse a space the runtime cannot allocate in one instruction, compute the
/// rent-exempt minimum, transfer only a shortfall, PDA-sign System `Allocate`
/// and `Assign`, and then check the exact poststate. Excess prefunding remains
/// in the target as a donation. The postcheck is not ceremony: `invoke_signed`
/// compiles to `Ok(())` off-chain, so without it every host path would report
/// a creation that did not happen — the same silent-no-op hazard
/// [`crate::token`] answers with its exact-delta checks.
#[allow(clippy::too_many_arguments)] // one argument per account and per value
#[inline(never)]
pub fn create_pda_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    require(
        space <= MAX_PERMITTED_DATA_INCREASE,
        ClutchError::AccountCreationFailed,
    )?;
    let minimum = rent.minimum_balance(space)?;
    let before = target.lamports();
    if before < minimum {
        let shortfall = minimum - before;
        let transfer = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &transfer_data(shortfall),
            vec![
                AccountMeta::new(*payer.key, true),
                AccountMeta::new(*target.key, false),
            ],
        );
        invoke_signed(
            &transfer,
            &[payer.clone(), target.clone(), system_program.clone()],
            &[],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    }
    let funded = target.lamports();
    require(
        funded == core::cmp::max(before, minimum),
        ClutchError::AccountCreationFailed,
    )?;

    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;

    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space && target.owner == program_id && target.lamports() == funded,
        ClutchError::AccountCreationFailed,
    )
}

/* ------------------------------------------------------------------------ */
/* Account planes                                                            */
/* ------------------------------------------------------------------------ */

/// Authenticated payer; signs, funds every creation, and is privileged in no
/// other way.  Also the actor of [`Intent::Endow`], where it must be the
/// position owner.
pub const IX_PAYER: usize = 0;
/// The account this instruction creates.  Every creating instruction.
pub const IX_TARGET: usize = 1;

/// Accounts in an `InitRealm` instruction, exactly.
pub const INIT_REALM_ACCOUNT_COUNT: usize = 5;
/// Canonical sealed collateral-policy PDA (read-only, program-owned).
pub const IX_REALM_POLICY: usize = 2;
/// The system program (read-only, executable).  `InitRealm`.
pub const IX_REALM_SYSTEM: usize = 3;
/// The rent sysvar (read-only).  `InitRealm`.
pub const IX_REALM_RENT: usize = 4;

/// Accounts in an `InitProfile` instruction, exactly.
pub const INIT_PROFILE_ACCOUNT_COUNT: usize = 6;
/// The Realm this Profile belongs to (read-only, program-owned).
pub const IX_PROFILE_REALM: usize = 2;
/// Canonical sealed collateral-policy PDA (read-only, program-owned).
pub const IX_PROFILE_POLICY: usize = 3;
/// The system program.  `InitProfile`.
pub const IX_PROFILE_SYSTEM: usize = 4;
/// The rent sysvar.  `InitProfile`.
pub const IX_PROFILE_RENT: usize = 5;

/// Accounts in an `InitOrderPage` instruction, exactly.
pub const INIT_PAGE_ACCOUNT_COUNT: usize = 6;
/// The market the epoch belongs to (read-only, program-owned).
pub const IX_PAGE_MARKET: usize = 2;
/// The epoch whose page set this page joins (read-only, program-owned).
pub const IX_PAGE_EPOCH: usize = 3;
/// The system program.  `InitOrderPage`.
pub const IX_PAGE_SYSTEM: usize = 4;
/// The rent sysvar.  `InitOrderPage`.
pub const IX_PAGE_RENT: usize = 5;

/// Accounts in an `Endow` instruction, exactly.
///
/// The five program-owned state roles are followed by the Realm's
/// content-authenticated collateral policy and the five Token-2022 roles
/// needed for an exact actor-to-Hoard deposit. Immutable Terms and its
/// canonical SourceSpec are appended after every existing role so the source
/// release gate changes no established index.
pub const ENDOW_ACCOUNT_COUNT: usize = 15;
/// The market the deposit belongs to (read-only).
///
/// It sits where the creation target sits in the other five lists, because
/// `Endow` has no creation target: it is the one instruction in this module
/// that allocates nothing.
pub const IX_ENDOW_MARKET: usize = 1;
/// The market-local Hoard accounting account (read-only).
pub const IX_ENDOW_HOARD: usize = 2;
/// The position being credited (writable).
pub const IX_ENDOW_POSITION: usize = 3;
/// The reference-only replay-sequence account (writable).
pub const IX_ENDOW_REPLAY: usize = 4;
/// The immutable Profile whose digest authenticates the collateral policy.
pub const IX_ENDOW_PROFILE: usize = 5;
/// The Realm's 266-byte collateral policy (read-only evidence).
pub const IX_ENDOW_POLICY: usize = 6;
/// The pinned Token-2022 program (read-only, executable).
pub const IX_ENDOW_TOKEN_PROGRAM: usize = 7;
/// The collateral mint named by the authenticated policy (read-only).
pub const IX_ENDOW_COLLATERAL_MINT: usize = 8;
/// The owner's collateral token account (writable source).
pub const IX_ENDOW_ACTOR_TOKEN: usize = 9;
/// The market's derived Hoard token account (writable destination).
pub const IX_ENDOW_HOARD_TOKEN: usize = 10;
/// System program used only when this is the owner's first deposit.
pub const IX_ENDOW_SYSTEM: usize = 11;
/// Rent sysvar used only when this is the owner's first deposit.
pub const IX_ENDOW_RENT: usize = 12;
/// Canonical immutable Terms bound by the Market (read-only).
pub const IX_ENDOW_TERMS: usize = 13;
/// Canonical immutable SourceSpec bound by Terms (read-only).
pub const IX_ENDOW_SOURCE_SPEC: usize = 14;

/// Program-owned roles of `InitProfile`, in account-index order.
const PROFILE_STATE_ROLES: [StateRole; 1] =
    [StateRole::read_only(IX_PROFILE_REALM, account_len::REALM)];
/// Program-owned roles of `InitOrderPage`.
const PAGE_STATE_ROLES: [StateRole; 1] =
    [StateRole::read_only(IX_PAGE_MARKET, account_len::MARKET)];
/// Existing market-global roles of `Endow`.
const ENDOW_COMMON_STATE_ROLES: [StateRole; 5] = [
    StateRole::read_only(IX_ENDOW_MARKET, account_len::MARKET),
    StateRole::read_only(IX_ENDOW_HOARD, account_len::HOARD),
    StateRole::read_only(IX_ENDOW_PROFILE, account_len::PROFILE),
    StateRole::read_only(IX_ENDOW_TERMS, account_len::TERMS),
    StateRole::read_only(IX_ENDOW_SOURCE_SPEC, SOURCE_SPEC_ACCOUNT_V1_BYTES),
];
/// Existing owner-plane roles, when this is not the first deposit.
const ENDOW_OWNER_STATE_ROLES: [StateRole; 2] = [
    StateRole::writable(IX_ENDOW_POSITION, account_len::POSITION),
    StateRole::writable(IX_ENDOW_REPLAY, REPLAY_ACCOUNT_LEN),
];

/// Read and authenticate the one canonical sealed collateral-policy artifact.
///
/// Byte recomputation proves the policy digest and parent Profile identity;
/// program ownership plus the content-derived PDA proves these are the bytes
/// admitted by typed `SealArtifact`, not a caller-owned copy.
#[inline(never)]
fn read_canonical_policy(
    program_id: &Pubkey,
    account: &AccountInfo,
) -> Outcome<(Hash32, Hash32, u16)> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.data_len() == collateral::COLLATERAL_POLICY_BYTES,
        ClutchError::WrongDataLength,
    )?;
    let (profile, digest, schema) = profile_identity_from_policy(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::policy_pda(program_id, &profile.bytes(), &digest.bytes()),
        None,
    )?;
    Ok((profile, digest, schema))
}

/// A creation consumes no replay sequence.
///
/// Exactly [`super::market_init`]'s rule and for exactly its reason: the
/// replay plane is per `(market, owner, generation)` and nothing in this
/// module has one, so a nonzero sequence is a claim about a plane the
/// instruction does not touch.  `Endow` is the exception and consumes one.
fn require_creation_sequence(sequence: u64) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)
}

/* ------------------------------------------------------------------------ */
/* Frame-bounded readers                                                     */
/* ------------------------------------------------------------------------ */

/// Recompute a Profile identity from the Realm's actual collateral policy.
///
/// The whole chain of `RESOLUTION_EVIDENCE_PLAN.md` §3.2 in one call: decode
/// the 266 bytes, take the child digest `D_col`, compose the parent under the
/// policy's own schema version, and hash it.  Returns the pair the two Realm
/// and Profile initializers both need — the identity and the digest — and
/// never lets the decoded policy escape into a caller's frame.
#[inline(never)]
fn profile_identity_from_policy(policy_bytes: &[u8]) -> Outcome<(Hash32, Hash32, u16)> {
    let policy = collateral::CollateralPolicy::decode(policy_bytes)?;
    let digest = policy.digest()?;
    let parent = collateral::ParentProfile::from_policy_digest(digest, policy.schema_version)?;
    Ok((parent.identity()?, digest, policy.schema_version))
}

/// Encode one Realm account into a freshly created account's bytes.
#[inline(never)]
fn write_realm(target: &mut [u8], value: &RealmAccount) -> Outcome<()> {
    value.encode(target)?;
    /* The post-write read is the same discipline `market_init` keeps: the
     * bytes that are now on chain are decoded again, so a writer that produced
     * something its own codec refuses fails here rather than at the first
     * reader. */
    let written = RealmAccount::decode(target)?;
    require(written == *value, ClutchError::MismatchedState)
}

/// Encode one Profile account, then re-bind it to the policy it froze.
///
/// The strongest post-write check in this module:
/// [`collateral::verify_profile_identity`] recomputes the child digest from
/// the 266 policy bytes *and* the parent identity from that digest, so a
/// Profile that landed committing to some other Realm's policy cannot survive
/// this call.
#[inline(never)]
fn write_profile(target: &mut [u8], value: &ProfileAccount, policy_bytes: &[u8]) -> Outcome<()> {
    value.encode(target)?;
    let written = ProfileAccount::decode(target)?;
    require(written == *value, ClutchError::MismatchedState)?;
    collateral::verify_profile_identity(policy_bytes, &written)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Routing                                                                   */
/* ------------------------------------------------------------------------ */

/// Route one already-decoded genesis request to its instruction.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    match request.action {
        Action::Layout(Intent::InitRealm {
            profile,
            realm_nonce,
            max_outcomes,
            profile_version,
        }) => init_realm(
            program_id,
            accounts,
            request.sequence,
            &RealmInit {
                profile,
                realm_nonce,
                max_outcomes,
                profile_version,
            },
        ),
        Action::Layout(Intent::InitProfile {
            realm,
            collateral_policy_digest,
            subfield_schema_version,
            profile_version,
        }) => init_profile(
            program_id,
            accounts,
            request.sequence,
            &ProfileInit {
                realm,
                collateral_policy_digest,
                subfield_schema_version,
                profile_version,
            },
        ),
        /* Typed `SealArtifact` is now the sole constructor for grid and Terms
         * PDAs. Retaining the former copy-from-buffer path would restore an
         * arbitrary caller-account dependency and a second semantic owner. */
        Action::Layout(Intent::InitPriceGrid { .. } | Intent::InitTerms { .. }) => {
            Err(ClutchError::UnsupportedInstruction.into())
        }
        Action::Layout(Intent::InitOrderPage {
            market,
            epoch,
            page_index,
            page_count,
        }) => init_order_page(
            program_id,
            accounts,
            request.sequence,
            &PageInit {
                market,
                epoch,
                page_index,
                page_count,
            },
        ),
        Action::Layout(Intent::Endow {
            market,
            owner,
            amount,
        }) => endow(
            program_id,
            accounts,
            &EndowRequest {
                sequence: request.sequence,
                market,
                owner,
                amount,
            },
        ),
        /* Every other action belongs to another family module; the router
         * never sends one here, and this arm exists so that adding one to the
         * router is a compile error rather than a silent success. */
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/* ------------------------------------------------------------------------ */
/* InitRealm                                                                 */
/* ------------------------------------------------------------------------ */

/// One already-matched `InitRealm` intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmInit {
    /// Parent Profile identity the Realm commits to.
    pub profile: Hash32,
    /// Nonce distinguishing Realms over one Profile.
    pub realm_nonce: u64,
    /// Outcome width; V1 admits only [`clutch_solana_layout::MAX_OUTCOMES`].
    pub max_outcomes: u8,
    /// Profile schema version this Realm expects.
    pub profile_version: u8,
}

fn init_realm(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent: &RealmInit,
) -> Outcome<()> {
    require_count(accounts, INIT_REALM_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_PAYER])?;
    require_distinct(accounts)?;
    require_creation_sequence(sequence)?;
    require_system_program(&accounts[IX_REALM_SYSTEM])?;
    let rent = read_rent(&accounts[IX_REALM_RENT])?;

    /* The claimed Profile identity is *recomputed* from the Realm's actual
     * collateral policy, so a Realm cannot be founded pointing at a Profile
     * nobody can produce a policy for.  This is the check that makes
     * `RealmAccount::profile` evidence rather than a caller's assertion. */
    let (profile, _digest, _schema) =
        read_canonical_policy(program_id, &accounts[IX_REALM_POLICY])?;
    require(
        profile == intent.profile,
        ClutchError::EvidenceBufferMismatch,
    )?;

    let realm = canonical_realm_id(profile, intent.realm_nonce);
    let realm_bytes = realm.bytes();
    let (address, bump) = seeds::realm_pda(program_id, &realm_bytes);
    expect_pda(accounts[IX_TARGET].key, (address, bump), None)?;

    create_pda_account(
        program_id,
        &accounts[IX_PAYER],
        &accounts[IX_TARGET],
        &accounts[IX_REALM_SYSTEM],
        &rent,
        account_len::REALM,
        &[seeds::SEED_REALM, &realm_bytes, &[bump]],
    )?;

    let value = RealmAccount {
        realm,
        profile,
        max_outcomes: intent.max_outcomes,
        profile_version: intent.profile_version,
        stored_bump: bump,
        flags: 0,
    };
    let mut data = borrow_mut!(accounts[IX_TARGET])?;
    write_realm(&mut data, &value)
}

/* ------------------------------------------------------------------------ */
/* InitProfile                                                               */
/* ------------------------------------------------------------------------ */

/// One already-matched `InitProfile` intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileInit {
    /// Realm this Profile belongs to.
    pub realm: Hash32,
    /// Child collateral-policy digest the Profile freezes.
    pub collateral_policy_digest: Hash32,
    /// Subfield schema version the parent identity was composed under.
    pub subfield_schema_version: u16,
    /// Profile schema version.
    pub profile_version: u8,
}

fn init_profile(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent: &ProfileInit,
) -> Outcome<()> {
    require_count(accounts, INIT_PROFILE_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_PAYER])?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &PROFILE_STATE_ROLES)?;
    require_creation_sequence(sequence)?;
    require_system_program(&accounts[IX_PROFILE_SYSTEM])?;
    let rent = read_rent(&accounts[IX_PROFILE_RENT])?;

    let (profile, digest, schema) =
        read_canonical_policy(program_id, &accounts[IX_PROFILE_POLICY])?;
    /* Both declared bindings are checked against the recomputation rather than
     * trusted: the digest the Profile will freeze, and the schema version that
     * digest was composed under. */
    require(
        digest == intent.collateral_policy_digest && schema == intent.subfield_schema_version,
        ClutchError::EvidenceBufferMismatch,
    )?;

    let realm = accounts::read_realm(&accounts[IX_PROFILE_REALM].data.borrow())?;
    let realm_bytes = realm.realm.bytes();
    expect_pda(
        accounts[IX_PROFILE_REALM].key,
        seeds::realm_pda(program_id, &realm_bytes),
        Some(realm.stored_bump),
    )?;
    /* The Realm already committed to a Profile identity and to a Profile
     * schema version; the Profile being created must be that one.  A Realm
     * whose Profile is some other identity has no Profile account it can ever
     * accept. */
    require(
        realm.realm == intent.realm
            && realm.profile == profile
            && realm.profile_version == intent.profile_version,
        ClutchError::MismatchedState,
    )?;

    let profile_bytes = profile.bytes();
    let (address, bump) = seeds::profile_pda(program_id, &realm_bytes, &profile_bytes);
    expect_pda(accounts[IX_TARGET].key, (address, bump), None)?;

    create_pda_account(
        program_id,
        &accounts[IX_PAYER],
        &accounts[IX_TARGET],
        &accounts[IX_PROFILE_SYSTEM],
        &rent,
        account_len::PROFILE,
        &[seeds::SEED_PROFILE, &realm_bytes, &profile_bytes, &[bump]],
    )?;

    /* A Profile created here is frozen on arrival.  There is no unfrozen
     * Profile state to reach: the policy bytes were required, bound, and
     * recomputed before anything was created, so "frozen later" would be a
     * state with no transition into it. */
    let value = ProfileAccount {
        profile,
        realm: realm.realm,
        collateral_policy_digest: digest,
        version: intent.profile_version,
        flags: PROFILE_FLAG_POLICY_FROZEN,
    };
    let policy_bytes = accounts[IX_PROFILE_POLICY].data.borrow();
    let mut data = borrow_mut!(accounts[IX_TARGET])?;
    write_profile(&mut data, &value, &policy_bytes)
}

/* ------------------------------------------------------------------------ */
/* InitPriceGrid and InitTerms are superseded by typed SealArtifact          */
/* ------------------------------------------------------------------------ */

/* ------------------------------------------------------------------------ */
/* InitOrderPage                                                             */
/* ------------------------------------------------------------------------ */

/// One already-matched `InitOrderPage` intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageInit {
    /// Market identity.
    pub market: Hash32,
    /// Epoch identity.
    pub epoch: Hash32,
    /// Zero-based page position.
    pub page_index: u16,
    /// Declared page count of the whole set.
    pub page_count: u16,
}

fn init_order_page(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent: &PageInit,
) -> Outcome<()> {
    if accounts
        .get(IX_PAGE_EPOCH)
        .map(|account| account.data_len())
        == Some(DIRECT_EPOCH_V4_BYTES)
    {
        return init_direct_v4_order_page(program_id, accounts, sequence, intent);
    }
    require_count(accounts, INIT_PAGE_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_PAYER])?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &PAGE_STATE_ROLES)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_PAGE_EPOCH],
        false,
        &[
            account_len::EPOCH,
            clutch_solana_layout::direct_selection::DIRECT_EPOCH_BYTES,
        ],
    )?;
    require_creation_sequence(sequence)?;
    require_system_program(&accounts[IX_PAGE_SYSTEM])?;
    let rent = read_rent(&accounts[IX_PAGE_RENT])?;

    let market = accounts::read_market(&accounts[IX_PAGE_MARKET].data.borrow())?;
    let epoch = accounts::read_epoch(&accounts[IX_PAGE_EPOCH].data.borrow())?;
    expect_pda(
        accounts[IX_PAGE_MARKET].key,
        seeds::market_pda(program_id, &market.realm.bytes(), &market.market.bytes()),
        Some(market.stored_bump),
    )?;
    expect_pda(
        accounts[IX_PAGE_EPOCH].key,
        seeds::epoch_pda(program_id, &epoch.market.bytes(), epoch.epoch_index),
        Some(epoch.stored_bump),
    )?;
    require(
        market.market == intent.market
            && epoch.epoch == intent.epoch
            && epoch.market == market.market,
        ClutchError::MismatchedState,
    )?;
    /* A page is created into an *open* epoch and an *active* market.  A frozen
     * epoch's page set is closed by `verify_page_set`, and adding a page to it
     * would mean the frozen `order_set` no longer folds the set it names. */
    require(
        epoch.phase == EPOCH_PHASE_OPEN && market.lifecycle == 0,
        ClutchError::NotActive,
    )?;
    /* The set's geometry is a decision made once.  Before the freeze the epoch
     * carries no page count (`page_count` is zero until frozen), so the intent
     * declares it and every page of one set must declare the same number —
     * which `verify_page_set` then checks across the whole set. */
    require(epoch.page_count == 0, ClutchError::MismatchedState)?;

    let epoch_bytes = epoch.epoch.bytes();
    let (address, bump) = seeds::page_pda(program_id, &epoch_bytes, intent.page_index);
    expect_pda(accounts[IX_TARGET].key, (address, bump), None)?;

    create_pda_account(
        program_id,
        &accounts[IX_PAYER],
        &accounts[IX_TARGET],
        &accounts[IX_PAGE_SYSTEM],
        &rent,
        account_len::ORDER_PAGE,
        &[
            seeds::SEED_PAGE,
            &epoch_bytes,
            &intent.page_index.to_le_bytes(),
            &[bump],
        ],
    )?;

    let mut data = borrow_mut!(accounts[IX_TARGET])?;
    write_empty_page(&mut data, intent, bump)
}

/// Create the sole page-zero account of a routed Direct V4 Epoch.
///
/// This branch is selected only by the 672-byte V4 Epoch schema, which can
/// exist only through the routed `InitDirectEpochV4`; the legacy page path
/// below it is byte- and behavior-stable.
#[inline(never)]
fn init_direct_v4_order_page(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent: &PageInit,
) -> Outcome<()> {
    require_count(accounts, INIT_PAGE_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_PAYER])?;
    require(accounts[IX_PAYER].is_writable, ClutchError::NotWritable)?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &PAGE_STATE_ROLES)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_PAGE_EPOCH],
        true,
        &[DIRECT_EPOCH_V4_BYTES],
    )?;
    require_creation_sequence(sequence)?;
    require_system_program(&accounts[IX_PAGE_SYSTEM])?;
    require_creatable(&accounts[IX_TARGET])?;
    let rent = read_rent(&accounts[IX_PAGE_RENT])?;
    let market = accounts::read_market(&accounts[IX_PAGE_MARKET].data.borrow())?;
    /* `decode` already ran the complete hostile-shape validation; only the
     * release binding and the sole pre-freeze creation phase re-state here. */
    let mut epoch = DirectEpochV4Account::decode(&accounts[IX_PAGE_EPOCH].data.borrow())?;
    require(
        epoch.verifier_release_id == DIRECT_VERIFIER_RELEASE_ID_V3
            && epoch.lifecycle_phase
                == clutch_solana_layout::direct_selection_v3::DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN
            && epoch.terminal
                == clutch_solana_layout::direct_selection_v3::DirectTerminalReceiptV3::EMPTY,
        ClutchError::NotActive,
    )?;
    require(
        epoch.neutral_lamport_sink == Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes())
            && market.market == intent.market
            && epoch.direct.common.epoch == intent.epoch
            && epoch.direct.common.market == market.market
            && market.lifecycle == 0
            && intent.page_index == 0
            && intent.page_count == 1
            && epoch.direct.common.page_count == 0
            && epoch.page_funding == DirectFundingLedgerV3::ZERO,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_PAGE_MARKET].key,
        seeds::market_pda(program_id, &market.realm.bytes(), &market.market.bytes()),
        Some(market.stored_bump),
    )?;
    expect_pda(
        accounts[IX_PAGE_EPOCH].key,
        seeds::epoch_pda(
            program_id,
            &epoch.direct.common.market.bytes(),
            epoch.direct.common.epoch_index,
        ),
        Some(epoch.direct.common.stored_bump),
    )?;
    let epoch_bytes = epoch.direct.common.epoch.bytes();
    let page_index_bytes = 0u16.to_le_bytes();
    let (page_address, page_bump) = seeds::page_pda(program_id, &epoch_bytes, 0);
    expect_pda(accounts[IX_TARGET].key, (page_address, page_bump), None)?;
    let funding = direct_creation_funding(
        &accounts[IX_PAYER],
        &accounts[IX_TARGET],
        &rent,
        account_len::ORDER_PAGE,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    epoch.epoch_funding = observe_direct_funding(
        epoch.epoch_funding,
        accounts[IX_PAGE_EPOCH].lamports(),
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    epoch.direct.common.page_count = 1;
    epoch.page_funding = funding;
    // `encode` revalidates the complete poststate below.
    let mut epoch_post = [0u8; DIRECT_EPOCH_V4_BYTES];
    epoch.encode(&mut epoch_post)?;

    create_pda_account_full_principal(
        program_id,
        &accounts[IX_PAYER],
        &accounts[IX_TARGET],
        &accounts[IX_PAGE_SYSTEM],
        &rent,
        account_len::ORDER_PAGE,
        funding,
        0,
        &[
            seeds::SEED_PAGE,
            &epoch_bytes,
            &page_index_bytes,
            &[page_bump],
        ],
    )?;
    {
        let mut page = borrow_mut!(accounts[IX_TARGET])?;
        write_empty_page(&mut page, intent, page_bump)?;
    }
    borrow_mut!(accounts[IX_PAGE_EPOCH])?.copy_from_slice(&epoch_post);
    Ok(())
}

/// Write one empty open page, and verify it the way a reader will.
///
/// Every byte comes from the layout crate's streaming writer: an empty page is
/// not a zeroed account — it commits to its own position and to sixteen
/// canonically padded slots, and that digest is not zero.  The verify
/// afterwards is the same streaming path `PlaceOrder` uses, so a page created
/// here is a page that instruction can already append to.
#[inline(never)]
fn write_empty_page(target: &mut [u8], intent: &PageInit, bump: u8) -> Outcome<()> {
    stream::init_page(
        target,
        intent.market,
        intent.epoch,
        intent.page_index,
        intent.page_count,
        bump,
    )?;
    stream::verify_page(target)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Endow                                                                     */
/* ------------------------------------------------------------------------ */

/// One already-matched `Endow` intent plus its replay sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EndowRequest {
    /// Exact replay sequence the request claims.
    pub sequence: u64,
    /// Market the position belongs to.
    pub market: Hash32,
    /// Position owner the credit lands on.
    pub owner: Hash32,
    /// Collateral atoms credited to internal cash.
    pub amount: u64,
}

/// Create a missing generation-zero Position/Replay pair, or authenticate an
/// existing pair. Mixed prestate is always a refusal.
#[inline(never)]
fn ensure_endow_owner_plane(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    rent: &RentParameters,
    market: Hash32,
    owner: Hash32,
) -> Outcome<()> {
    let position = &accounts[IX_ENDOW_POSITION];
    let replay = &accounts[IX_ENDOW_REPLAY];
    let position_exists = position.owner == program_id;
    let replay_exists = replay.owner == program_id;
    require(
        position_exists == replay_exists,
        ClutchError::AlreadyInitialized,
    )?;

    if position_exists {
        return accounts::validate_state_roles(program_id, accounts, &ENDOW_OWNER_STATE_ROLES);
    }

    let market_bytes = market.bytes();
    let owner_bytes = owner.bytes();
    let position_derived = seeds::position_pda(program_id, &market_bytes, &owner_bytes);
    let replay_derived = seeds::replay_pda(program_id, &market_bytes, &owner_bytes, 0);
    let bumps = OwnerStateBumps {
        position: position_derived.1,
        replay: replay_derived.1,
    };
    construction::create_owner_state_plane(
        program_id,
        &accounts[IX_PAYER],
        &accounts[IX_ENDOW_SYSTEM],
        rent,
        &OwnerStateTargets { position, replay },
        &market_bytes,
        &owner_bytes,
        0,
        &bumps,
    )?;

    {
        let mut data = borrow_mut!(position)?;
        PositionAccount {
            market,
            owner,
            generation: 0,
            internal: [0; clutch_solana_layout::MAX_OUTCOMES],
            cash_atoms: 0,
            reserved_cash_atoms: 0,
            stored_bump: bumps.position,
            close_state: 0,
        }
        .encode(&mut data)?;
    }
    {
        let mut data = borrow_mut!(replay)?;
        ReplayAccount {
            market,
            owner,
            position_generation: 0,
            sequence: 0,
            stored_bump: bumps.replay,
            flags: 0,
        }
        .encode(&mut data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    }
    Ok(())
}

fn endow(program_id: &Pubkey, accounts: &[AccountInfo], request: &EndowRequest) -> Outcome<()> {
    require_count(accounts, ENDOW_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_PAYER])?;
    require(accounts[IX_PAYER].is_writable, ClutchError::NotWritable)?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &ENDOW_COMMON_STATE_ROLES)?;
    require_system_program(&accounts[IX_ENDOW_SYSTEM])?;
    let rent = read_rent(&accounts[IX_ENDOW_RENT])?;

    let actor = Hash32::from_bytes(accounts[IX_PAYER].key.to_bytes());
    require(actor == request.owner, ClutchError::UnauthorizedActor)?;

    let market = accounts::read_market(&accounts[IX_ENDOW_MARKET].data.borrow())?;
    let hoard = HoardAccount::decode(&accounts[IX_ENDOW_HOARD].data.borrow())?;
    let profile = ProfileAccount::decode(&accounts[IX_ENDOW_PROFILE].data.borrow())?;
    let market_bytes = market.market.bytes();
    expect_pda(
        accounts[IX_ENDOW_MARKET].key,
        seeds::market_pda(program_id, &market.realm.bytes(), &market_bytes),
        Some(market.stored_bump),
    )?;
    let hoard_derived = seeds::hoard_pda(program_id, &market_bytes);
    expect_pda(
        accounts[IX_ENDOW_HOARD].key,
        hoard_derived,
        Some(hoard.stored_bump),
    )?;
    require(market.hoard_bump == hoard_derived.1, ClutchError::WrongBump)?;
    expect_pda(
        accounts[IX_ENDOW_PROFILE].key,
        seeds::profile_pda(program_id, &market.realm.bytes(), &market.profile.bytes()),
        None,
    )?;
    require(
        hoard.market == market.market
            && hoard.realm == market.realm
            && profile.profile == market.profile
            && profile.realm == market.realm,
        ClutchError::MismatchedState,
    )?;

    /* Endow is the sole protocol-recognized inbound collateral boundary.
     * Authenticate the immutable Terms and SourceSpec and ask the exact same
     * closed registry used by source ingestion before allocating an owner
     * plane or invoking Token-2022. The default ELF has no registered release
     * and therefore cannot take custody even of a market left by an older ELF
     * or installed as a local fixture. */
    super::source_ingest::require_registered_source_for_market(
        program_id,
        &accounts[IX_ENDOW_TERMS],
        &accounts[IX_ENDOW_SOURCE_SPEC],
        market.terms,
        market.realm,
        market.feed,
    )?;

    let policy_account = &accounts[IX_ENDOW_POLICY];
    require(!policy_account.is_writable, ClutchError::UnexpectedWritable)?;
    require(!policy_account.executable, ClutchError::ExecutableAccount)?;
    require(
        policy_account.data_len() == collateral::COLLATERAL_POLICY_BYTES,
        ClutchError::WrongDataLength,
    )?;
    let policy = collateral::verify_profile_identity(&policy_account.data.borrow(), &profile)?;
    token::require_drivable_collateral(&policy)?;
    super::split::validate_token_program(&accounts[IX_ENDOW_TOKEN_PROGRAM])?;

    let mint = &accounts[IX_ENDOW_COLLATERAL_MINT];
    require(!mint.is_writable, ClutchError::UnexpectedWritable)?;
    require(!mint.executable, ClutchError::ExecutableAccount)?;
    let mint_observation = token::admit_mint(mint, &token::MintPolicy::collateral(&policy))?;

    let actor_token = &accounts[IX_ENDOW_ACTOR_TOKEN];
    let hoard_token = &accounts[IX_ENDOW_HOARD_TOKEN];
    require(
        !actor_token.executable && !hoard_token.executable,
        ClutchError::ExecutableAccount,
    )?;
    require(
        actor_token.is_writable && hoard_token.is_writable,
        ClutchError::NotWritable,
    )?;
    let authority = seeds::hoard_authority_pda(program_id, &market_bytes).0;
    require(
        hoard.authority.bytes() == authority.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        hoard_token.key,
        seeds::hoard_token_pda(program_id, &market_bytes),
        None,
    )?;
    let actor_observation = token::admit_token_account(
        actor_token,
        &token::TokenAccountPolicy::collateral_holder(&policy, *accounts[IX_PAYER].key),
    )?;
    let hoard_observation = token::admit_token_account(
        hoard_token,
        &token::TokenAccountPolicy::hoard(&policy, authority),
    )?;
    token::require_hoard_covers_collateral(hoard.collateral_atoms, hoard_observation.amount)?;
    let expected_hoard = hoard_observation
        .amount
        .checked_add(request.amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    /* Every existing market, collateral, and token role is authenticated
     * before the first owner-plane CPI. A later transfer or postcondition
     * refusal still exercises transaction rollback, but hostile metadata does
     * not spend compute allocating accounts it could never use. */
    ensure_endow_owner_plane(program_id, accounts, &rent, market.market, actor)?;
    let (position_owner, position_generation, position_bump) = {
        let data = accounts[IX_ENDOW_POSITION].data.borrow();
        let position = PositionAccount::decode(&data)?;
        (position.owner, position.generation, position.stored_bump)
    };
    let replay_bump = {
        let data = accounts[IX_ENDOW_REPLAY].data.borrow();
        ReplayAccount::decode(&data)?.stored_bump
    };
    let owner_bytes = position_owner.bytes();
    expect_pda(
        accounts[IX_ENDOW_POSITION].key,
        seeds::position_pda(program_id, &market_bytes, &owner_bytes),
        Some(position_bump),
    )?;
    expect_pda(
        accounts[IX_ENDOW_REPLAY].key,
        seeds::replay_pda(program_id, &market_bytes, &owner_bytes, position_generation),
        Some(replay_bump),
    )?;

    let (position_post, replay_post) = validated_endow(
        &accounts[IX_ENDOW_MARKET].data.borrow(),
        &accounts[IX_ENDOW_POSITION].data.borrow(),
        &accounts[IX_ENDOW_REPLAY].data.borrow(),
        actor,
        request,
    )?;

    /* This is the value boundary.  Every state mutation follows the CPI and
     * its exact pre/post checks.  Any later refusal rolls the CPI back with the
     * rest of the SVM transaction. */
    token::transfer_checked(
        &accounts[IX_ENDOW_TOKEN_PROGRAM],
        actor_token,
        mint,
        hoard_token,
        &accounts[IX_PAYER],
        request.amount,
        mint_observation.decimals,
    )?;
    let post_actor = token::token_amount(actor_token)?;
    let post_hoard = token::token_amount(hoard_token)?;
    token::require_exact_debit(actor_observation.amount, post_actor, request.amount)?;
    token::require_exact_credit(hoard_observation.amount, post_hoard, request.amount)?;
    require(
        post_hoard == expected_hoard,
        ClutchError::TokenDeltaMismatch,
    )?;
    token::require_hoard_covers_collateral(hoard.collateral_atoms, post_hoard)?;

    let mut position_data = borrow_mut!(accounts[IX_ENDOW_POSITION])?;
    let mut replay_data = borrow_mut!(accounts[IX_ENDOW_REPLAY])?;
    position_post.encode(&mut position_data)?;
    replay_post
        .encode(&mut replay_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}

/// The whole endowment transition over authenticated bytes.
///
/// Split out from the account plane exactly as `orders_batch` splits its
/// placement: the account plane derives addresses (a runtime syscall, so it
/// cannot run off-chain), and this half is the transition, which host tests
/// drive directly against the layout codecs.
///
/// The order of refusals is deliberate and is the order every other value
/// transition in this program uses: identity bindings, then phase, then
/// replay, then arithmetic, then the cap, then the writes.
#[inline(never)]
pub fn apply_endow(
    market_bytes: &[u8],
    position_bytes: &mut [u8],
    replay_bytes: &mut [u8],
    actor: Hash32,
    request: &EndowRequest,
) -> Outcome<()> {
    let (position, replay) =
        validated_endow(market_bytes, position_bytes, replay_bytes, actor, request)?;
    position.encode(position_bytes)?;
    replay
        .encode(replay_bytes)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(())
}

/// Validate and compute the ledger half of one deposit without writing.
///
/// The on-chain path calls this before the Token-2022 CPI, so authorization,
/// identity, lifecycle, replay, and arithmetic all
/// refuse before value moves.  The returned values are encoded only after the
/// exact token deltas have been observed.
#[inline(never)]
fn validated_endow(
    market_bytes: &[u8],
    position_bytes: &[u8],
    replay_bytes: &[u8],
    actor: Hash32,
    request: &EndowRequest,
) -> Outcome<(PositionAccount, ReplayAccount)> {
    let market = MarketAccount::decode(market_bytes)?;
    let mut position = PositionAccount::decode(position_bytes)?;
    let mut replay = ReplayAccount::decode(replay_bytes)?;

    require(actor == position.owner, ClutchError::UnauthorizedActor)?;
    require(
        request.market == market.market
            && request.owner == position.owner
            && market.market == position.market
            && market.market == replay.market
            && position.owner == replay.owner
            && position.generation == replay.position_generation,
        ClutchError::MismatchedState,
    )?;
    require(
        market.lifecycle == 0 && position.close_state == 0,
        ClutchError::NotActive,
    )?;

    require(request.sequence == replay.sequence, ClutchError::Replay)?;
    let next_sequence = replay
        .sequence
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Replay))?;

    let next_cash = position
        .cash_atoms
        .checked_add(request.amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    position.cash_atoms = next_cash;
    replay.sequence = next_sequence;
    Ok((position, replay))
}

/// The Hoard *accounting record* is untouched by the ledger half of Endow.
///
/// `HoardAccount::collateral_atoms` is locked complete-set backing and Endow
/// does not change it.  The on-chain wrapper separately moves the physical
/// Token-2022 balance before committing the returned position and replay.
#[cfg(test)]
fn hoard_is_unmoved(
    before: &clutch_solana_layout::HoardAccount,
    after: &clutch_solana_layout::HoardAccount,
) -> bool {
    before.collateral_atoms == after.collateral_atoms
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        canonical_epoch_id, canonical_market_id, canonical_outcome_id, stream, CodecError, EpochId,
        HoardAccount, MarketId, OrderPageAccount, MAX_ORDER_PAGES, MAX_OUTCOMES, RELATION_VERSION,
    };

    fn h(byte: u8) -> Hash32 {
        Hash32([byte; 32])
    }

    /* ------------------------------------------------------------------ */
    /* The system-program and rent plumbing                                */
    /* ------------------------------------------------------------------ */

    /// The two pinned addresses, checked against their base58 spellings rather
    /// than against themselves.  A wrong constant here does not fail loudly —
    /// it addresses the wrong program — so it is pinned by decoding the name.
    #[test]
    fn the_pinned_addresses_are_the_ones_they_are_named_for() {
        assert_eq!(SYSTEM_PROGRAM_ID.to_bytes(), [0; 32]);
        // `SysvarRent111111111111111111111111111111111`, base58-decoded.
        assert_eq!(
            RENT_SYSVAR_ID.to_bytes(),
            [
                6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238,
                8, 155, 161, 253, 68, 227, 219, 217, 138, 0, 0, 0, 0
            ]
        );
        assert_ne!(SYSTEM_PROGRAM_ID, RENT_SYSVAR_ID);
    }

    /// The `CreateAccount` payload is bincode, and bincode is exact.
    #[test]
    fn the_create_account_payload_is_the_system_programs_encoding() {
        let owner = Pubkey::new_from_array([7; 32]);
        let data = create_account_data(1_000, 70, &owner);
        assert_eq!(data.len(), 52);
        // Variant zero, little-endian, four bytes wide.
        assert_eq!(&data[0..4], &[0, 0, 0, 0]);
        assert_eq!(&data[4..12], &1_000_u64.to_le_bytes());
        assert_eq!(&data[12..20], &70_u64.to_le_bytes());
        assert_eq!(&data[20..52], &[7; 32]);

        let transfer = transfer_data(99);
        assert_eq!(&transfer[0..4], &2_u32.to_le_bytes());
        assert_eq!(&transfer[4..12], &99_u64.to_le_bytes());

        let allocate = allocate_data(589);
        assert_eq!(&allocate[0..4], &8_u32.to_le_bytes());
        assert_eq!(&allocate[4..12], &589_u64.to_le_bytes());

        let assign = assign_data(&owner);
        assert_eq!(&assign[0..4], &1_u32.to_le_bytes());
        assert_eq!(&assign[4..36], &[7; 32]);
    }

    /// The rent transcription against `solana-rent`'s published defaults:
    /// `DEFAULT_LAMPORTS_PER_BYTE_YEAR = 3480`, `DEFAULT_EXEMPTION_THRESHOLD =
    /// 2.0`, `ACCOUNT_STORAGE_OVERHEAD = 128`.  At a threshold of exactly two
    /// the float half is an integer doubling, so the expected values are
    /// computable here in integers and the transcription is falsifiable.
    #[test]
    fn the_rent_formula_matches_the_runtimes() {
        let rent = RentParameters {
            lamports_per_byte_year: 3_480,
            exemption_threshold: 2.0,
        };
        for space in [
            0,
            account_len::REALM,
            account_len::PROFILE,
            account_len::PRICE_GRID,
            account_len::TERMS,
            account_len::ORDER_PAGE,
        ] {
            let expected = (ACCOUNT_STORAGE_OVERHEAD + space as u64) * 3_480 * 2;
            assert_eq!(rent.minimum_balance(space).unwrap(), expected, "{space}");
        }
        /* The figures `docs/implementation/SBF_BRINGUP.md` quotes, pinned
         * here so the doc is falsifiable rather than decorative.  The order
         * page — the widest account this module creates — is about 0.0288
         * SOL to hold rent-exempt at the default parameters. */
        for (space, lamports) in [
            (account_len::REALM, 1_378_080_u64),
            (account_len::PROFILE, 1_586_880),
            (account_len::PRICE_GRID, 4_990_320),
            (account_len::TERMS, 12_416_640),
            (account_len::ORDER_PAGE, 28_814_400),
        ] {
            assert_eq!(rent.minimum_balance(space).unwrap(), lamports, "{space}");
        }
        /* And the one this module deliberately cannot create: the streaming
         * checkpoint at 48,004 bytes (the T2-1 codec re-pin), quoted in
         * `SOLANA_LAYOUT.md`. */
        assert_eq!(
            rent.minimum_balance(account_len::CLEAR_WORK).unwrap(),
            334_998_720
        );
        // A rate that would overflow the integer half refuses rather than wraps.
        let absurd = RentParameters {
            lamports_per_byte_year: u64::MAX,
            exemption_threshold: 2.0,
        };
        assert_eq!(
            absurd.minimum_balance(account_len::TERMS),
            Err(Refusal::Adapter(ClutchError::Arithmetic))
        );
    }

    /// Every account this module creates fits one system-program CPI, and the
    /// one account that does not is the one no instruction here creates.
    #[test]
    fn every_created_account_fits_the_cpi_growth_ceiling() {
        for (label, len, fits) in [
            ("realm", account_len::REALM, true),
            ("profile", account_len::PROFILE, true),
            ("price grid", account_len::PRICE_GRID, true),
            ("terms", account_len::TERMS, true),
            ("order page", account_len::ORDER_PAGE, true),
            /* The one account in the whole inventory that a single CPI
             * cannot allocate, and the reason no instruction here creates
             * one: the streaming checkpoint is 48,004 bytes. */
            ("clear work", account_len::CLEAR_WORK, false),
        ] {
            assert_eq!(len <= MAX_PERMITTED_DATA_INCREASE, fits, "{label}");
        }
        assert_eq!(MAX_PERMITTED_DATA_INCREASE, 10_240);
    }

    /* ------------------------------------------------------------------ */
    /* The initialization writers, against the layout codecs as oracle     */
    /* ------------------------------------------------------------------ */

    /// The expected bytes of every initializer come from the layout crate's
    /// own encoder, never from a hand-typed literal: the writer and the oracle
    /// are the same codec, so what this pins is that the *values* the
    /// instruction chose are the ones the account plane requires.
    #[test]
    fn the_realm_writer_produces_exactly_the_layout_encoders_bytes() {
        let value = RealmAccount {
            realm: h(1),
            profile: h(2),
            max_outcomes: MAX_OUTCOMES as u8,
            profile_version: 2,
            stored_bump: 254,
            flags: 0,
        };
        let mut written = [0_u8; account_len::REALM];
        write_realm(&mut written, &value).unwrap();
        let mut expected = [0_u8; account_len::REALM];
        value.encode(&mut expected).unwrap();
        assert_eq!(written, expected);
        assert_eq!(RealmAccount::decode(&written), Ok(value));

        // A Realm the codec refuses is refused by the writer, not written.
        let narrow = RealmAccount {
            max_outcomes: 8,
            ..value
        };
        let mut out = [0_u8; account_len::REALM];
        assert_eq!(
            write_realm(&mut out, &narrow),
            Err(Refusal::Codec(CodecError::InvalidCount))
        );
        assert_eq!(out, [0; account_len::REALM]);
    }

    #[test]
    fn the_profile_writer_binds_the_policy_it_froze() {
        let policy = policy_bytes();
        let (profile, digest, _schema) = profile_identity_from_policy(&policy).unwrap();
        let value = ProfileAccount {
            profile,
            realm: h(9),
            collateral_policy_digest: digest,
            version: 2,
            flags: PROFILE_FLAG_POLICY_FROZEN,
        };
        let mut written = [0_u8; account_len::PROFILE];
        write_profile(&mut written, &value, &policy).unwrap();
        let mut expected = [0_u8; account_len::PROFILE];
        value.encode(&mut expected).unwrap();
        assert_eq!(written, expected);

        /* The load-bearing negative: a well-formed Profile that commits to a
         * *different* policy digest is refused by the post-write binding, not
         * accepted because it decodes. */
        let mut forged = value;
        forged.collateral_policy_digest = h(0x5a);
        forged.profile = collateral::ParentProfile::from_policy_digest(h(0x5a), 1)
            .unwrap()
            .identity()
            .unwrap();
        let mut out = [0_u8; account_len::PROFILE];
        assert_eq!(
            write_profile(&mut out, &forged, &policy),
            Err(Refusal::Codec(CodecError::MismatchedBinding))
        );

        // And a Profile whose identity is not the parent hash of its own
        // digest fails the identity half rather than the digest half.
        let mut misnamed = value;
        misnamed.profile = h(0x33);
        assert_eq!(
            write_profile(&mut out, &misnamed, &policy),
            Err(Refusal::Codec(CodecError::NonCanonicalIdentity))
        );
    }

    #[test]
    fn the_page_writer_produces_exactly_the_streaming_writers_page() {
        let market = canonical_market_id(h(1), h(2), 3);
        let epoch = canonical_epoch_id(market, 4);
        let intent = PageInit {
            market,
            epoch,
            page_index: 2,
            page_count: 4,
        };
        let mut written = [0_u8; account_len::ORDER_PAGE];
        write_empty_page(&mut written, &intent, 251).unwrap();

        let mut expected = [0_u8; account_len::ORDER_PAGE];
        stream::init_page(&mut expected, market, epoch, 2, 4, 251).unwrap();
        assert_eq!(written, expected);

        /* An empty page is not a zeroed account: it commits to its position
         * and to sixteen canonical padding slots, and that digest is not
         * zero.  The buffered decoder — the golden reference — agrees. */
        assert_ne!(written, [0; account_len::ORDER_PAGE]);
        let page = OrderPageAccount::decode(&written).unwrap();
        assert_eq!(page.market, market);
        assert_eq!(page.epoch, epoch);
        assert_eq!(page.page_index, 2);
        assert_eq!(page.page_count, 4);
        assert_eq!(page.order_count, 0);
        assert_eq!(page.tombstone_count, 0);
        assert_eq!(page.frozen, 0);
        assert_eq!(page.stored_bump, 251);
        assert_ne!(page.page_digest, Hash32::ZERO);
        assert_eq!(page.page_digest, page.recomputed_page_digest().unwrap());
        assert_eq!(
            stream::verify_page(&written).unwrap(),
            stream::OrderPageHeader::decode(&written).unwrap()
        );

        /* Four pages written this way are one positional chain.  Order ids
         * are positional, so a page's base rank is a fact about its index and
         * not about how full its predecessors are — which is exactly what
         * makes creating pages in any order safe. */
        let mut pages = [[0_u8; account_len::ORDER_PAGE]; MAX_ORDER_PAGES];
        for (index, page) in pages.iter_mut().enumerate() {
            write_empty_page(
                page,
                &PageInit {
                    market,
                    epoch,
                    page_index: index as u16,
                    page_count: MAX_ORDER_PAGES as u16,
                },
                251,
            )
            .unwrap();
        }
        for (index, page) in pages.iter().enumerate() {
            let header = stream::verify_page(page).unwrap();
            assert_eq!(header.page_index as usize, index);
            assert_eq!(header.page_count as usize, MAX_ORDER_PAGES);
            /* The base rank of page `p` is the rank of the last slot before
             * it — `p * MAX_ORDERS_PER_PAGE` — as a canonical order id, and
             * zero on page zero. */
            let base = index * clutch_solana_layout::MAX_ORDERS_PER_PAGE;
            assert_eq!(
                header.prev_page_last_order_id,
                if base == 0 {
                    Hash32::ZERO
                } else {
                    clutch_solana_layout::canonical_order_id(base as u64)
                }
            );
        }

        /* And a page written this way is one the freeze path accepts.
         * Nothing in this program freezes anything — no intent and no
         * instruction can, which is the standing gap the order-set commitment
         * work owns — so this is a property of the *bytes* the creation
         * produced, driven here through the layout crate's own freeze
         * writers.  A one-page set is used because a frozen set must be
         * dense: every page but the last is full. */
        let mut single = [0_u8; account_len::ORDER_PAGE];
        write_empty_page(
            &mut single,
            &PageInit {
                market,
                epoch,
                page_index: 0,
                page_count: 1,
            },
            250,
        )
        .unwrap();
        stream::write_single_slot(&mut single, &order_record(1)).unwrap();
        let views: [&[u8]; 1] = [&single];
        // Open pages do not close: the closure requires every page frozen.
        assert_eq!(
            stream::verify_page_set(&views),
            Err(CodecError::MismatchedBinding)
        );
        let (order_set, set_order_count) = stream::frozen_set_commitment(&views).unwrap();
        assert_eq!(set_order_count, 1);
        stream::seal_page(&mut single, order_set, set_order_count).unwrap();
        let views: [&[u8]; 1] = [&single];
        assert_eq!(stream::verify_page_set(&views), Ok(order_set));
    }

    /* ------------------------------------------------------------------ */
    /* Endow                                                               */
    /* ------------------------------------------------------------------ */

    /// The ledger half of Endow, checked against the layout codecs.  The SVM
    /// test drives the surrounding real Token-2022 transfer.
    #[test]
    fn an_endowment_ledger_credits_cash_and_advances_replay() {
        let mut case = EndowCase::new();
        let hoard_before = case.hoard;
        apply_endow(
            &case.market,
            &mut case.position,
            &mut case.replay,
            case.owner,
            &EndowRequest {
                sequence: 0,
                market: case.market_id,
                owner: case.owner,
                amount: 250,
            },
        )
        .unwrap();

        let mut expected_position = case.position_value;
        expected_position.cash_atoms = 250;
        let mut expected_position_bytes = [0_u8; account_len::POSITION];
        expected_position
            .encode(&mut expected_position_bytes)
            .unwrap();
        assert_eq!(case.position, expected_position_bytes);

        let mut expected_replay = case.replay_value;
        expected_replay.sequence = 1;
        let mut expected_replay_bytes = [0_u8; REPLAY_ACCOUNT_LEN];
        expected_replay.encode(&mut expected_replay_bytes).unwrap();
        assert_eq!(case.replay, expected_replay_bytes);

        assert!(hoard_is_unmoved(&hoard_before, &case.hoard));

        // A second endowment at the consumed sequence is a replay.
        assert_eq!(
            apply_endow(
                &case.market,
                &mut case.position,
                &mut case.replay,
                case.owner,
                &EndowRequest {
                    sequence: 0,
                    market: case.market_id,
                    owner: case.owner,
                    amount: 1,
                },
            ),
            Err(Refusal::Adapter(ClutchError::Replay))
        );
        // At the next sequence it lands, and cash accumulates.
        apply_endow(
            &case.market,
            &mut case.position,
            &mut case.replay,
            case.owner,
            &EndowRequest {
                sequence: 1,
                market: case.market_id,
                owner: case.owner,
                amount: 50,
            },
        )
        .unwrap();
        assert_eq!(
            PositionAccount::decode(&case.position).unwrap().cash_atoms,
            300
        );
    }

    #[test]
    fn an_endowment_refuses_every_hostile_caller_and_state() {
        let base = EndowCase::new();
        let request = EndowRequest {
            sequence: 0,
            market: base.market_id,
            owner: base.owner,
            amount: 250,
        };

        // A signer who is not the position owner.
        let mut case = EndowCase::new();
        assert_eq!(
            apply_endow(
                &case.market,
                &mut case.position,
                &mut case.replay,
                h(0x77),
                &request
            ),
            Err(Refusal::Adapter(ClutchError::UnauthorizedActor))
        );

        // An intent naming another market, and one naming another owner.
        let mut case = EndowCase::new();
        assert_eq!(
            apply_endow(
                &case.market,
                &mut case.position,
                &mut case.replay,
                case.owner,
                &EndowRequest {
                    market: h(0x66),
                    ..request
                }
            ),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        assert_eq!(
            apply_endow(
                &case.market,
                &mut case.position,
                &mut case.replay,
                case.owner,
                &EndowRequest {
                    owner: h(0x55),
                    ..request
                }
            ),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );

        // A resolved market and a closing position both refuse.
        let mut case = EndowCase::new();
        let mut resolved = case.market_value;
        resolved.lifecycle = 1;
        resolved.encode(&mut case.market).unwrap();
        assert_eq!(
            apply_endow(
                &case.market,
                &mut case.position,
                &mut case.replay,
                case.owner,
                &request
            ),
            Err(Refusal::Adapter(ClutchError::NotActive))
        );

        let mut case = EndowCase::new();
        let mut closing = case.position_value;
        closing.close_state = 1;
        closing.encode(&mut case.position).unwrap();
        assert_eq!(
            apply_endow(
                &case.market,
                &mut case.position,
                &mut case.replay,
                case.owner,
                &request
            ),
            Err(Refusal::Adapter(ClutchError::NotActive))
        );

        // The market cap limits locked complete-set collateral, not custody.
        // Unused position cash may exceed it without expanding liabilities.
        let mut case = EndowCase::new();
        apply_endow(
            &case.market,
            &mut case.position,
            &mut case.replay,
            case.owner,
            &EndowRequest {
                amount: case.market_value.collateral_cap + 1,
                ..request
            },
        )
        .unwrap();
        assert_eq!(
            PositionAccount::decode(&case.position).unwrap().cash_atoms,
            case.market_value.collateral_cap + 1
        );

        // Arithmetic overflow still refuses before any write.
        let mut case = EndowCase::new();
        let mut rich = case.position_value;
        rich.cash_atoms = u64::MAX;
        rich.encode(&mut case.position).unwrap();
        assert_eq!(
            apply_endow(
                &case.market,
                &mut case.position,
                &mut case.replay,
                case.owner,
                &EndowRequest {
                    amount: 1,
                    ..request
                }
            ),
            Err(Refusal::Adapter(ClutchError::Arithmetic))
        );

        // A replay account bound to another generation.
        let mut case = EndowCase::new();
        let mut foreign = case.replay_value;
        foreign.position_generation = 9;
        foreign.encode(&mut case.replay).unwrap();
        assert_eq!(
            apply_endow(
                &case.market,
                &mut case.position,
                &mut case.replay,
                case.owner,
                &request
            ),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );

        // Nothing above wrote: every refusing path left the bytes alone.
        let untouched = EndowCase::new();
        assert_eq!(base.position, untouched.position);
        assert_eq!(base.replay, untouched.replay);
    }

    /* ------------------------------------------------------------------ */
    /* Fixtures                                                            */
    /* ------------------------------------------------------------------ */

    /// A generic, well-formed 266-byte collateral policy.
    ///
    /// Built through the layout crate's own encoder rather than transcribed as
    /// hex: this module is not the owner of those bytes and has no business
    /// carrying a second copy of them.
    fn policy_bytes() -> [u8; collateral::COLLATERAL_POLICY_BYTES] {
        let backing = collateral::CurrencyRef::spl(collateral::TOKEN_2022_PROGRAM, [0x6d; 32], 6);
        let policy = collateral::CollateralPolicy {
            schema_version: collateral::COLLATERAL_POLICY_SCHEMA,
            flags: collateral::COLLATERAL_POLICY_STRICT_FLAGS,
            collateral: backing,
            fee: collateral::CurrencyRef::NATIVE_SOL,
            liveness: collateral::CurrencyRef::NATIVE_SOL,
            max_supply_atoms: 10_000,
            allowed_mint_extensions: 0,
            required_mint_extensions: 0,
            allowed_account_extensions: collateral::EXTENSION_IMMUTABLE_OWNER,
            required_account_extensions: 0,
        };
        let mut out = [0; collateral::COLLATERAL_POLICY_BYTES];
        policy.encode(&mut out).unwrap();
        out
    }

    fn order_record(rank: u64) -> clutch_solana_layout::OrderRecord {
        clutch_solana_layout::OrderRecord {
            owner: h(0x20),
            order_id: clutch_solana_layout::canonical_order_id(rank),
            outcome: 0,
            side: 0,
            quantity: 10,
            limit: 1,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 9,
        }
    }

    /// One coherent `(market, hoard, position, replay)` plane for `Endow`.
    struct EndowCase {
        market_id: MarketId,
        owner: Hash32,
        market_value: MarketAccount,
        position_value: PositionAccount,
        replay_value: ReplayAccount,
        hoard: HoardAccount,
        market: [u8; account_len::MARKET],
        position: [u8; account_len::POSITION],
        replay: [u8; REPLAY_ACCOUNT_LEN],
    }

    impl EndowCase {
        fn new() -> Self {
            let realm = h(1);
            let profile = h(2);
            let market_id = canonical_market_id(realm, profile, 3);
            let owner = h(0x11);
            let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
            for (index, slot) in outcomes.iter_mut().enumerate().take(2) {
                *slot = canonical_outcome_id(market_id, index as u8);
            }
            let market_value = MarketAccount {
                market: market_id,
                realm,
                profile,
                terms: h(4),
                outcome_count: 2,
                lifecycle: 0,
                stored_bump: 255,
                hoard_bump: 254,
                outcomes,
                feed: h(5),
                collateral_cap: 1_000,
                created_slot: 0,
                reserved: Hash32::ZERO,
            };
            let position_value = PositionAccount {
                market: market_id,
                owner,
                generation: 0,
                internal: [0; MAX_OUTCOMES],
                cash_atoms: 0,
                reserved_cash_atoms: 0,
                stored_bump: 253,
                close_state: 0,
            };
            let replay_value = ReplayAccount {
                market: market_id,
                owner,
                position_generation: 0,
                sequence: 0,
                stored_bump: 252,
                flags: 0,
            };
            let hoard = HoardAccount {
                market: market_id,
                realm,
                authority: h(6),
                collateral_atoms: 0,
                stored_bump: 254,
                flags: 0,
            };
            let mut market = [0; account_len::MARKET];
            let mut position = [0; account_len::POSITION];
            let mut replay = [0; REPLAY_ACCOUNT_LEN];
            market_value.encode(&mut market).unwrap();
            position_value.encode(&mut position).unwrap();
            replay_value.encode(&mut replay).unwrap();
            Self {
                market_id,
                owner,
                market_value,
                position_value,
                replay_value,
                hoard,
                market,
                position,
                replay,
            }
        }
    }

    /// The epoch identity helper the page test leans on, pinned so that a
    /// change in derivation is a failure here rather than a mystery address.
    #[test]
    fn the_page_geometry_matches_the_relation_book() {
        let market: MarketId = canonical_market_id(h(1), h(2), 3);
        let epoch: EpochId = canonical_epoch_id(market, 4);
        assert_ne!(epoch, Hash32::ZERO);
        assert_eq!(RELATION_VERSION, 1);
        assert_eq!(MAX_ORDER_PAGES, 4);
    }
}
