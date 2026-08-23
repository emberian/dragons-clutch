//! `Intent::CreateMarket` — validated initialization-write.
//!
//! This module brings a market into existence: it authenticates a fixed
//! eleven-account state prefix, re-composes the whole of
//! [`clutch_solana_reference::validate_market_init`] on-chain, writes the seven
//! initial account states, and then re-runs that same validation over the bytes
//! it just wrote.  A refusal anywhere aborts the instruction, and SVM
//! transaction semantics — not this program — discard the partial write.
//!
//! It contains no economic logic.  The founding market is empty by
//! construction, so the only kernel call is
//! [`clutch_kernel::MarketState::check_invariants`] over the state being
//! founded; byte ownership stays in [`clutch_solana_layout`] and in the
//! reference-only codecs of [`clutch_solana_reference`], and metadata
//! authentication stays in [`crate::accounts`].
//!
//! ## Authority model — **PROPOSED**
//!
//! Market creation here is **permissionless**, and that is a proposal this lane
//! is making, not a frozen rule.  Concretely:
//!
//! - the account at index [`IX_CREATOR`] must present an authenticated
//!   signature, and it is the fee payer the runtime already charges;
//! - **there is no privileged key of any kind** — no protocol admin, no Realm
//!   authority, no deploy authority, and no allow-list.  `PROJECT.md` §4 puts
//!   permissionless work at the edge and gives no market-creation gate, and the
//!   frozen [`clutch_solana_layout::RealmAccount`] carries no authority field to
//!   check one against, so inventing an authority here would be inventing an
//!   ABI;
//! - the creator becomes the owner of the founding Position and Replay pair. That is
//!   the same owner interpretation [`crate::seeds`] proposes and
//!   [`super::split`] already enforces: the 32-byte owner identity is the raw
//!   bytes of the signing wallet address;
//! - the only real gates are **structural**: the Realm and Profile accounts
//!   must already exist at their canonical addresses, the Profile must have
//!   **frozen** its collateral policy, and the immutable terms artifact the new
//!   market binds must already exist and already bind this Realm, Profile,
//!   feed, and outcome count.  A Realm that has not decided which collateral
//!   policy it commits to must not mint liabilities, which is
//!   `require_frozen_collateral_policy` in the offline reference adapter.
//!
//! Two alternatives were considered and rejected in the same breath.  A Realm
//! authority signature has nowhere to live in the frozen layout.  A program
//! upgrade-authority gate would centralize creation on the deployer, which is
//! the opposite of the charter.
//!
//! ### Residues this authority model does not close
//!
//! Market identity is `canonical_market_id(realm, profile, market_nonce)` and
//! is **not creator-bound**, so a nonce is a first-come address: an observer of
//! a pending transaction can create the same market first.  It cannot create a
//! *different* market at that address — every field is a function of the
//! identity, the terms artifact, and the signer — but it does become the
//! founding position owner.  Naming it is not fixing it.
//!
//! ## What this instruction creates, and what it still does not
//!
//! It creates the seven-account program state plane, the Token-2022 outcome
//! plane, and the Realm-selected collateral Hoard. The canonical Market,
//! Hoard, founding Position, kernel aggregate,
//! founding Replay, SupplyLedger, and Resolution addresses must be genuinely
//! absent System-owned slots: zero lamports, zero data, writable, and
//! non-executable. All seven are rent-funded by the creator and assigned to
//! this program with signed System CPIs. One outcome mint per active outcome
//! and the Hoard's token account are then created through their independently
//! authenticated token-program roles.
//!
//! This is what closes the optional-leg hole of
//! `docs/implementation/TOKEN2022_PLAN.md` §0.2.  Until a market could be
//! founded *with* its mints, a mandatory token leg on `Materialize` would have
//! named an account nothing could create; now every market founded here has
//! one mint per outcome and one Hoard token account, so
//! [`super::split`]'s legs are mandatory and its `Absent` variant is gone.
//!
//! External claims deliberately have no program-owned per-holder shadow. Their
//! truth is the actual Token-2022 outcome mints and token accounts. Consequently
//! this instruction founds seven program accounts, not the former eight-account
//! plane that included `ExternalAccount`.
//!
//! ## Where the collateral cap comes from, and the field still unsourced
//!
//! - [`MarketAccount::collateral_cap`] is written from
//!   [`clutch_solana_layout::TermsAccount::collateral_cap`] — the immutable,
//!   digest-committed terms field the v3 revision added for exactly this
//!   (`RESOLUTION_EVIDENCE_PLAN.md` §3.5's finding: the cap needs a terms
//!   field, not a policy field).  **Markets founded here are fundable**: the
//!   terms codec refuses a zero cap, so "cap 0 refuses at market init" is
//!   structural — a terms artifact with no cap decision cannot exist, and
//!   the old residue ("a market created today exists and cannot accept
//!   collateral") is closed.  [`validate_initial_plane`] re-checks that the
//!   written cap equals the terms' cap, so a founding write cannot invent
//!   one. The live path takes the canonical `CollateralPolicyV2` account as
//!   evidence and refuses a cap above its mint ceiling. **That evidence
//!   account is in this plane** ([`IX_POLICY`]), joined through Profile V2 to
//!   the exact compiled adapter release, so `admit_collateral` discharges the
//!   ceiling check on chain without caller-shaped deployment facts.
//! - [`MarketAccount::created_slot`] is written **`0`**.  The honest value is
//!   the `Clock` sysvar slot, and this crate has no clock plane: no sysvar
//!   dependency, no sysvar account role, and adding either is a shared-file
//!   decision.  No check in the layout crate, in the reference adapter, or here
//!   reads the field, so a zero is inert rather than load-bearing — but it is a
//!   placeholder and is listed as such.
//!
//! ## Free initial values this lane chose — **PROPOSED**
//!
//! - `position.generation` and replay `position_generation` are **`0`**: a
//!   market's founding owner plane enters at generation zero, and the replay
//!   PDA is seeded on it.
//! - `supply.generation` is **`0`**: one accounting era per ledger lifetime,
//!   per `docs/implementation/MULTI_POSITION_CLOSURE.md` §4.  Nothing writes it
//!   again, so an era bump is structurally impossible.
//! - `hoard.authority` is the **Hoard PDA's own address bytes**.  The frozen
//!   codec only requires it nonzero; making it the account's own address is the
//!   one choice that is checkable from the account list rather than asserted.
//! - The kernel payout set is **copied from the immutable terms artifact**, and
//!   the resolution record is initialized **unresolved**
//!   ([`clutch_solana_layout::PAYOUT_INDEX_UNRESOLVED`], zero window, zero
//!   cursors).
//! - The request envelope's `sequence` must be **`0`**.  Creation consumes no
//!   replay sequence — the founding replay account is written at zero, so the
//!   first `Split` uses sequence zero — and a nonzero creation sequence is a
//!   replay-plane claim this instruction cannot honour.
//!
//! ## What is checked, and where it comes from
//!
//! [`validate_initial_plane`] preserves the state-account checks from
//! `clutch_solana_reference::validate_market_init`, which cannot be called from
//! a program because its SBF frame exceeds 4 KiB. The production token-truth
//! cut intentionally removes the reference model's per-owner ExternalAccount:
//! external truth is instead established by the zero-supply Token-2022 mints
//! this same instruction creates and admits.
//!
//! | divergence | direction | why |
//! | --- | --- | --- |
//! | the resolution record must be present, bound, and unresolved | stricter here | the reference's `validate_market_init` has no resolution account; this instruction writes one, so it validates one |
//! | no per-owner ExternalAccount | production token-truth cut | actual outcome mint supply and token accounts are authoritative; a second program shadow would be a contradictory ledger |
//!
//! ## Refusal codes
//!
//! `error.rs` is unfrozen this wave and carries the four appends this table
//! used to reserve.  What this instruction emits:
//!
//! | check | emitted | code |
//! | --- | --- | --- |
//! | a target address was not genuinely absent (re-initialization or squatting) | [`ClutchError::AlreadyInitialized`] | `0x0040` |
//! | the Profile's collateral policy is not frozen | [`ClutchError::CollateralPolicyNotFrozen`] | `0x0041` |
//! | the kernel payout set is not the terms payout set | [`ClutchError::PayoutSetMismatch`] | `0x0042` |
//! | the terms artifact does not bind this market, or the written cap is not the terms' cap | [`ClutchError::TermsBindingMismatch`] | `0x0043` |
//! | an initial value was nonzero | `Reference(NonEmptyInitialization)` | `0x3010` |
//! | every other check | the [`ClutchError`] the check already has | `0x0001..=0x0017` |
//!
//! ## Frame discipline
//!
//! Every function holding a whole decoded account is `#[inline(never)]`, for
//! the reason [`crate::accounts`] gives: the kernel account and the terms
//! artifact are over a kilobyte each, and the 4 KiB SBF frame does not hold two
//! of them plus a caller. Each of the seven target accounts is decoded exactly
//! once during validation, into a small facts structure; the payout set is
//! never carried into [`process`]'s frame.
//!
//! This is **measured, not reasoned**: `cargo-build-sbf` emits its frame-space
//! diagnostic for `clutch_solana_reference::validate_market_init` (estimated
//! 10496 bytes) and for six other functions in the layout and reference crates
//! that this program never calls, and for **no** function in `clutch_sbf` —
//! this module's included.  Re-running that build is how the discipline stays
//! true; a frame overflow is undefined behaviour the loader will happily
//! execute, so it is not something to discover from a failing transaction.
//!
//! Compute cost is measured by the SVM test that drives this instruction from
//! absent target addresses and reports the transaction's consumed units.

use crate::accounts::{
    self, expect_pda, require, require_signer, require_two_term_closure, MarketFacts, Outcome,
    RealmFacts, StateRole, SupplyFacts, TermsFacts,
};
use crate::collateral_release::authenticate_realm_collateral_v2;
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use crate::token;
use clutch_collateral_adapter_v2::{
    admit_collateral_account_v2, admit_collateral_mint_v2, prepare_hoard_creation_v2,
    refine_market_collateral_v2, BoundCollateralProfileV2, CustodyCreationPlanV2,
    CustodyInitializationStepV2, Id as CollateralId, MarketCollateralBindingV2,
    RuntimeAccountViewV2, TokenAccountRoleV2,
};
use clutch_kernel::{
    BasisMode, MarketState, PayoutSet, PayoutVector, Phase, MAX_OUTCOMES as KERNEL_MAX_OUTCOMES,
};
use clutch_solana_layout::{
    account_len, canonical_market_id, canonical_outcome_id, collateral,
    native_resolution::{NativeResolutionAccount, NATIVE_RESOLUTION_LEN},
    occupation_resolution::{
        is_occupation_statistic, OccupationResolutionAccount, OCCUPATION_RESOLUTION_LEN,
    },
    Hash32, HoardAccount, Intent, MarketAccount, PositionAccount, ProfileAccount,
    ResolutionAccount, SupplyLedgerAccount, TermsAccount, MAX_OUTCOMES, MAX_PAYOUTS,
    PAYOUT_INDEX_UNRESOLVED, PROFILE_FLAG_POLICY_FROZEN,
};
use clutch_solana_reference::{
    Action, Error as ReferenceError, KernelAccount, ReplayAccount, Request, KERNEL_ACCOUNT_LEN,
    REPLAY_ACCOUNT_LEN,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::construction::{self, MarketStateBumps, MarketStateIdentity, MarketStateTargets};
use super::genesis;

/* ------------------------------------------------------------------------ */
/* Account plane                                                             */
/* ------------------------------------------------------------------------ */

/// The state prefix of this instruction's account list, in list order.
///
/// Eleven accounts: creator, three immutable inputs, and seven state targets.
/// The token plane follows and is **not** optional; the exact total is
/// [`account_count`].
pub const ACCOUNT_COUNT: usize = 11;

/// Authenticated creator; pays, signs, and owns the founding position.
pub const IX_CREATOR: usize = 0;
/// Realm configuration account (read-only).
pub const IX_REALM: usize = 1;
/// Profile identity account (read-only).
pub const IX_PROFILE: usize = 2;
/// Immutable terms artifact the new market binds (read-only).
pub const IX_TERMS: usize = 3;
/// Market account to initialize.
pub const IX_MARKET: usize = 4;
/// Hoard collateral account to initialize.
pub const IX_HOARD: usize = 5;
/// Founding position account to initialize.
pub const IX_POSITION: usize = 6;
/// Reference-only kernel-aggregate account to initialize.
pub const IX_KERNEL: usize = 7;
/// Reference-only replay-sequence account to initialize.
pub const IX_REPLAY: usize = 8;
/// Market-wide supply-ledger account to initialize.
pub const IX_SUPPLY: usize = 9;
/// Resolution-record account to initialize, unresolved.
pub const IX_RESOLUTION: usize = 10;

/// Existing program-owned inputs, in account-list order.
///
/// The seven targets are absent System-owned slots and are authenticated by
/// [`construction::create_market_state_plane`], not as existing state.
const INPUT_STATE_ROLES: [StateRole; 3] = [
    StateRole::read_only(IX_REALM, account_len::REALM),
    StateRole::read_only(IX_PROFILE, account_len::PROFILE),
    StateRole::read_only(IX_TERMS, account_len::TERMS),
];

/* --------------------------------------------------------------------- */
/* The token plane, mandatory                                             */
/* --------------------------------------------------------------------- */

/// Accounts this instruction takes before the one-per-outcome mints.
///
/// Eleven state/actor accounts, then the Realm's canonical CollateralPolicyV2,
/// release-selected collateral token program, collateral mint, System program,
/// Rent sysvar, Hoard signing authority, Hoard token account, and the separate
/// Token-2022 outcome-issuance program. The outcome mints follow, one per
/// active outcome, so the exact count is
/// [`account_count`] and not a constant.
pub const ACCOUNT_COUNT_BASE: usize = 19;

/// Canonical sealed collateral-policy PDA (read-only, program-owned).
///
/// Byte recomputation still proves the child digest and parent Profile
/// identity. Program ownership plus `policy(Profile, digest)` additionally
/// proves the bytes came through typed `SealArtifact`; caller-owned copies are
/// not construction evidence.
pub const IX_POLICY: usize = 11;
/// Realm-selected collateral token program (read-only, executable).
pub const IX_COLLATERAL_TOKEN_PROGRAM: usize = 12;
/// Backward-compatible source spelling for the collateral program index.
pub const IX_TOKEN_PROGRAM: usize = IX_COLLATERAL_TOKEN_PROGRAM;
/// The collateral mint the Realm's policy names (read-only).
pub const IX_COLLATERAL_MINT: usize = 13;
/// The System program, which creates every account this instruction founds.
pub const IX_SYSTEM_PROGRAM: usize = 14;
/// The Rent sysvar, read for the rent-exempt minimum of each new account.
pub const IX_RENT: usize = 15;
/// The Hoard's signing authority; holds no data and is never written.
pub const IX_HOARD_AUTHORITY: usize = 16;
/// The Hoard's release-selected collateral token account, created here.
pub const IX_HOARD_TOKEN: usize = 17;
/// Token-2022 program used only for outcome issuance (read-only, executable).
pub const IX_OUTCOME_TOKEN_PROGRAM: usize = 18;
/// First outcome mint; one per active outcome follows, in index order.
pub const IX_OUTCOME_MINT_BASE: usize = 19;

/// The exact account count for a market with `outcome_count` outcomes.
///
/// A fixed count is itself a check, and this one is fixed *given the intent*:
/// the outcome count is a field of `Intent::CreateMarket`, and
/// [`validate_initial_plane`] proves it equals the immutable terms artifact's,
/// so a caller cannot inflate the list by lying about it and then have the
/// market founded.
pub const fn account_count(outcome_count: u8) -> usize {
    ACCOUNT_COUNT_BASE + outcome_count as usize
}

/// Refuse all role aliases except the two independent read-only program roles
/// when this Realm also selects Token-2022 for collateral.
///
/// Solana privilege union means those two instruction positions may resolve to
/// the same runtime account. That is safe: neither is writable or a signer,
/// the collateral release authenticates one, and the outcome role separately
/// authenticates the fixed Token-2022 program.
fn require_market_account_distinctness(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let count = accounts.len();
    let mut left = 0usize;
    while left < count {
        let mut right = left + 1;
        while right < count {
            let allowed_program_alias =
                left == IX_COLLATERAL_TOKEN_PROGRAM && right == IX_OUTCOME_TOKEN_PROGRAM;
            require(
                accounts[left].key != accounts[right].key || allowed_program_alias,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Authenticated Market refinement and exact release-selected Hoard creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdmittedMarketCollateralV2 {
    bound: BoundCollateralProfileV2,
    creation: CustodyCreationPlanV2,
}

fn runtime_account_view<'a>(account: &AccountInfo<'_>, data: &'a [u8]) -> RuntimeAccountViewV2<'a> {
    RuntimeAccountViewV2 {
        key: CollateralId::from_bytes(account.key.to_bytes()),
        owner_program: CollateralId::from_bytes(account.owner.to_bytes()),
        data,
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        executable: account.executable,
    }
}

/// Bind the Realm's Profile V2, policy, and compiled adapter release, refine
/// that authority with this exact Market/Hoard, and admit the collateral mint.
///
/// This is the **market-initialization** half of the two enforcement points
/// `docs/implementation/TOKEN2022_PLAN.md` §3.4 requires; the other half is
/// [`super::split::seam`], which re-runs the extension refusal at every token
/// instruction because a mint address is not a stable description of a mint.
///
/// Four things happen here that nothing else in this program could do:
///
/// 1. the canonical policy bytes are **bound** to Profile V2 by recomputed
///    content identity and to one exact compiled release, so a well-formed
///    impostor policy or caller-shaped deployment cannot decide;
/// 2. the release-selected hostile-byte parser runs the exact extension matrix
///    against the mint rather than re-stating it in SBF;
/// 3. `collateral_cap` is checked against the policy's mint ceiling, which the
///    module docs above record as an obligation this program could not
///    discharge. It can now; and
/// 4. the exact custody owner guard and ordered initialization operations come
///    from the selected release. Token-2022 uses `ImmutableOwner`; a reviewed
///    legacy SPL release instead relies on a sole program-derived signer and
///    the adapter's absence of any owner-authority-change route.
#[inline(never)]
fn admit_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market: Hash32,
    collateral_cap: u64,
    hoard_authority: Pubkey,
) -> Outcome<AdmittedMarketCollateralV2> {
    let realm = authenticate_realm_collateral_v2(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_POLICY],
        &accounts[IX_COLLATERAL_TOKEN_PROGRAM],
    )?;
    let bound = refine_market_collateral_v2(
        realm,
        MarketCollateralBindingV2 {
            market: CollateralId::from_bytes(market.bytes()),
            realm: CollateralId::from_bytes(realm.realm().realm.bytes()),
            profile: CollateralId::from_bytes(realm.realm().profile.bytes()),
            collateral_cap_atoms: collateral_cap,
            hoard_authority: CollateralId::from_bytes(hoard_authority.to_bytes()),
            hoard_token_account: CollateralId::from_bytes(accounts[IX_HOARD_TOKEN].key.to_bytes()),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let mint = &accounts[IX_COLLATERAL_MINT];
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    admit_collateral_mint_v2(bound, runtime_account_view(mint, &mint_data))
        .map_err(|_| Refusal::Adapter(ClutchError::MintNotAdmitted))?;
    let creation = prepare_hoard_creation_v2(bound)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        creation.token_program
            == CollateralId::from_bytes(accounts[IX_COLLATERAL_TOKEN_PROGRAM].key.to_bytes())
            && creation.account
                == CollateralId::from_bytes(accounts[IX_HOARD_TOKEN].key.to_bytes())
            && creation.owner_authority == CollateralId::from_bytes(hoard_authority.to_bytes())
            && creation.mint == CollateralId::from_bytes(mint.key.to_bytes())
            && creation.step_count != 0
            && usize::from(creation.step_count) <= creation.steps.len(),
        ClutchError::MismatchedState,
    )?;
    Ok(AdmittedMarketCollateralV2 { bound, creation })
}

/// Authenticate the sealed policy's content-derived address.
#[inline(never)]
fn require_canonical_policy_pda(program_id: &Pubkey, accounts: &[AccountInfo]) -> Outcome<()> {
    let profile = ProfileAccount::decode(&accounts[IX_PROFILE].data.borrow())?;
    expect_pda(
        accounts[IX_POLICY].key,
        seeds::policy_pda(
            program_id,
            &profile.profile.bytes(),
            &profile.collateral_policy_id.bytes(),
        ),
        None,
    )?;
    Ok(())
}

/// Authenticate the two programs and the sysvar the founding CPIs need.
#[inline(never)]
fn validate_creation_roles(accounts: &[AccountInfo]) -> Outcome<()> {
    let system = &accounts[IX_SYSTEM_PROGRAM];
    require(
        *system.key == token::SYSTEM_PROGRAM_ID && system.executable,
        ClutchError::WrongProgramOwner,
    )?;
    require(!system.is_writable, ClutchError::UnexpectedWritable)?;
    let rent = &accounts[IX_RENT];
    require(*rent.key == token::RENT_SYSVAR_ID, ClutchError::WrongPda)?;
    require(!rent.is_writable, ClutchError::UnexpectedWritable)?;
    require(!rent.executable, ClutchError::ExecutableAccount)?;
    let outcome_token_program = &accounts[IX_OUTCOME_TOKEN_PROGRAM];
    require(
        *outcome_token_program.key == token::TOKEN_2022_PROGRAM_ID
            && outcome_token_program.executable,
        ClutchError::WrongTokenProgram,
    )?;
    require(
        !outcome_token_program.is_writable && !outcome_token_program.is_signer,
        ClutchError::UnexpectedWritable,
    )?;
    Ok(())
}

/// Refuse an account this instruction is about to create that already exists.
///
/// The token-plane half of the idempotence gate: a market founded twice would otherwise reach
/// `system_instruction::create_account` and be refused one frame down with a
/// worse diagnostic.  Zero lamports and zero data is what the runtime hands a
/// program for an address nobody has created. Prior SOL funding is not
/// initialization: predictable token PDAs use the same shortfall-top-up,
/// Allocate, Assign sequence as state PDAs, so prefunding cannot squat them.
fn require_uncreated(account: &AccountInfo) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.data_is_empty() && *account.owner == genesis::SYSTEM_PROGRAM_ID,
        ClutchError::AlreadyInitialized,
    )
}

/// Create the Token-2022 outcome plane and release-selected collateral Hoard.
///
/// **This is the hole `TOKEN2022_PLAN.md` §0.2 named, closed.**  Until this
/// function existed no instruction in this program created an outcome mint or
/// a Hoard token account, so the token legs of `Materialize`, `Dematerialize`,
/// `Split` and `Merge` had to be optional — a mandatory leg would have named
/// accounts nothing could bring into existence.  A market founded here has all
/// of them, so the legs are mandatory and the `Absent` variants are gone.
///
/// Per outcome, in order: `CreateAccount` for an 82-byte Token-2022-owned
/// account at [`seeds::outcome_mint_pda`], `InitializeMint2` with decimals `0`,
/// the Market PDA as mint authority and **no freeze authority**, and then —
/// the step that makes the first two evidence rather than intention — the same
/// [`token::MintPolicy::outcome`] admission every seam instruction will run,
/// over the bytes the token program just wrote.
///
/// Then the Hoard: release-selected allocation and ordered initialization,
/// owned by the Hoard *authority* PDA, followed by admission through the exact
/// same Realm/Profile/release binding. The two Hoard addresses stay distinct for
/// the reason `seeds` gives — collapsing them makes the signing seeds and the
/// account seeds the same bytes — and the founding mirror
/// `HoardAccount::collateral_atoms == hoard_token.amount` therefore holds from
/// birth at zero, which is the invariant every collateral transition then
/// preserves.
#[inline(never)]
fn create_token_plane(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_bytes: &[u8; 32],
    outcome_count: u8,
    collateral: AdmittedMarketCollateralV2,
    rent: &genesis::RentParameters,
) -> Outcome<()> {
    let system = &accounts[IX_SYSTEM_PROGRAM];
    let payer = &accounts[IX_CREATOR];
    let collateral_token_program = &accounts[IX_COLLATERAL_TOKEN_PROGRAM];
    let outcome_token_program = &accounts[IX_OUTCOME_TOKEN_PROGRAM];
    let authority = *accounts[IX_MARKET].key;

    let mut outcome = 0_u8;
    while outcome < outcome_count {
        let mint = &accounts[IX_OUTCOME_MINT_BASE + usize::from(outcome)];
        require_uncreated(mint)?;
        let derived = seeds::outcome_mint_pda(program_id, market_bytes, outcome);
        expect_pda(mint.key, derived, None)?;
        let index = [outcome];
        let bump = [derived.1];
        genesis::create_pda_account(
            &token::TOKEN_2022_PROGRAM_ID,
            payer,
            mint,
            system,
            rent,
            token::MINT_ACCOUNT_LEN,
            &[seeds::SEED_OUTCOME_MINT, market_bytes, &index, &bump],
        )?;
        token::initialize_outcome_mint(outcome_token_program, mint, &authority)?;
        token::admit_mint(mint, &token::MintPolicy::outcome(*mint.key, authority))?;
        outcome += 1;
    }

    let hoard_token = &accounts[IX_HOARD_TOKEN];
    require_uncreated(hoard_token)?;
    let derived = seeds::hoard_token_pda(program_id, market_bytes);
    expect_pda(hoard_token.key, derived, None)?;
    let bump = [derived.1];
    genesis::create_pda_account(
        collateral_token_program.key,
        payer,
        hoard_token,
        system,
        rent,
        usize::from(collateral.creation.account_bytes),
        &[seeds::SEED_HOARD_TOKEN, market_bytes, &bump],
    )?;
    let collateral_mint = &accounts[IX_COLLATERAL_MINT];
    let mut step = 0usize;
    while step < usize::from(collateral.creation.step_count) {
        match collateral.creation.steps[step] {
            CustodyInitializationStepV2::None => return Err(ClutchError::MismatchedState.into()),
            CustodyInitializationStepV2::InitializeImmutableOwner { account, data } => {
                require(
                    account == CollateralId::from_bytes(hoard_token.key.to_bytes()),
                    ClutchError::MismatchedState,
                )?;
                let instruction = Instruction::new_with_bytes(
                    *collateral_token_program.key,
                    &data,
                    vec![AccountMeta::new(*hoard_token.key, false)],
                );
                invoke(
                    &instruction,
                    &[hoard_token.clone(), collateral_token_program.clone()],
                )
                .map_err(|_| Refusal::Adapter(ClutchError::TokenAccountNotAdmitted))?;
            }
            CustodyInitializationStepV2::InitializeAccount3 {
                account,
                mint,
                owner_authority,
                data,
            } => {
                require(
                    account == CollateralId::from_bytes(hoard_token.key.to_bytes())
                        && mint == CollateralId::from_bytes(collateral_mint.key.to_bytes())
                        && owner_authority
                            == CollateralId::from_bytes(
                                accounts[IX_HOARD_AUTHORITY].key.to_bytes(),
                            ),
                    ClutchError::MismatchedState,
                )?;
                let instruction = Instruction::new_with_bytes(
                    *collateral_token_program.key,
                    &data,
                    vec![
                        AccountMeta::new(*hoard_token.key, false),
                        AccountMeta::new_readonly(*collateral_mint.key, false),
                    ],
                );
                invoke(
                    &instruction,
                    &[
                        hoard_token.clone(),
                        collateral_mint.clone(),
                        collateral_token_program.clone(),
                    ],
                )
                .map_err(|_| Refusal::Adapter(ClutchError::TokenAccountNotAdmitted))?;
            }
        }
        step += 1;
    }
    let hoard_data = hoard_token
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observation = admit_collateral_account_v2(
        collateral.bound,
        runtime_account_view(hoard_token, &hoard_data),
        TokenAccountRoleV2::Hoard,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenAccountNotAdmitted))?;
    /* The founding market holds no collateral, so the mirror the collateral
     * transitions preserve is established here at zero rather than assumed. */
    token::require_hoard_mirror(0, observation.amount_atoms)
}

/* ------------------------------------------------------------------------ */
/* Request, bumps, and plane views                                           */
/* ------------------------------------------------------------------------ */

/// One already-matched `CreateMarket` intent.
///
/// [`crate::dispatch`] hands this module the whole envelope, so the match lives
/// here.  The fallback arm is not decoration: it is the same
/// `_ => Err(UnsupportedIntent)` the offline reference adapter keeps, and it is
/// what stops a future routing edit from delivering another intent into an
/// initializer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateMarketIntent {
    /// Realm namespace the market is created under.
    pub realm: Hash32,
    /// Profile identity the Realm commits to.
    pub profile: Hash32,
    /// Nonce distinguishing markets within one `(realm, profile)` pair.
    pub market_nonce: u64,
    /// Active outcome count.
    pub outcome_count: u8,
    /// Immutable terms digest the market binds.
    pub terms: Hash32,
    /// Feed identity the market resolves against.
    pub feed: Hash32,
}

/// The canonical bumps of every account this instruction initializes.
///
/// The reference-only kernel aggregate has no stored bump field, but its bump
/// is still carried because the System CPI must sign for its canonical PDA.
pub type PlaneBumps = MarketStateBumps;

/// Read-only view of the seven initialized accounts.
#[derive(Clone, Copy, Debug)]
pub struct PlaneBytes<'a> {
    /// Market account bytes.
    pub market: &'a [u8],
    /// Hoard account bytes.
    pub hoard: &'a [u8],
    /// Founding position account bytes.
    pub position: &'a [u8],
    /// Reference-only kernel-aggregate bytes.
    pub kernel: &'a [u8],
    /// Reference-only replay-sequence bytes.
    pub replay: &'a [u8],
    /// Supply-ledger bytes.
    pub supply: &'a [u8],
    /// Resolution-record bytes.
    pub resolution: &'a [u8],
}

/// Writable view of the seven initialized accounts.
#[derive(Debug)]
pub struct PlaneWrite<'a> {
    /// Market account bytes.
    pub market: &'a mut [u8],
    /// Hoard account bytes.
    pub hoard: &'a mut [u8],
    /// Founding position account bytes.
    pub position: &'a mut [u8],
    /// Reference-only kernel-aggregate bytes.
    pub kernel: &'a mut [u8],
    /// Reference-only replay-sequence bytes.
    pub replay: &'a mut [u8],
    /// Supply-ledger bytes.
    pub supply: &'a mut [u8],
    /// Resolution-record bytes.
    pub resolution: &'a mut [u8],
}

/// The identities a founding write is parameterized by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingIdentities {
    /// Canonical market identity, derived from the intent.
    pub market: Hash32,
    /// Founding position owner; the creator's raw address bytes.
    pub owner: Hash32,
    /// Hoard authority; the dedicated signing PDA's address bytes.
    pub hoard_authority: Hash32,
}

/* ------------------------------------------------------------------------ */
/* Local refusal helpers                                                     */
/* ------------------------------------------------------------------------ */

/// Refuse with a reference-adapter class unless `condition` holds.
///
/// Two of this instruction's checks are the reference adapter's own named
/// refusals and have no adapter-vocabulary equivalent; see the refusal table in
/// the module docs.
fn require_reference(condition: bool, error: ReferenceError) -> Outcome<()> {
    if condition {
        Ok(())
    } else {
        Err(Refusal::Reference(error))
    }
}

/// Refuse with the terms-binding append unless `condition` holds.
///
/// The layout crate's own `binds_market` raises `Codec(MismatchedBinding)`
/// for the same disagreement; this instruction emits the allocated
/// first-class code [`ClutchError::TermsBindingMismatch`] (`0x0043`) so a
/// transaction log names the check, not the codec that happened to run it.
fn require_binding(condition: bool) -> Outcome<()> {
    require(condition, ClutchError::TermsBindingMismatch)
}

/* ------------------------------------------------------------------------ */
/* Local plane readers                                                       */
/* ------------------------------------------------------------------------ */

/// Profile facts including the freeze discipline.
///
/// [`crate::accounts::read_profile`] carries neither the flags nor the
/// collateral-policy digest, and the freeze gate needs both.  The frozen codec
/// already refuses every combination except "flag set exactly when the digest
/// is nonzero", so the two fields are carried rather than pre-judged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProfileInitFacts {
    profile: Hash32,
    realm: Hash32,
    version: u8,
    collateral_policy_id: Hash32,
    adapter_release_id: Hash32,
    flags: u8,
}

/// Hoard facts a founding write binds against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HoardFacts {
    market: Hash32,
    realm: Hash32,
    authority: Hash32,
    collateral_atoms: u64,
    stored_bump: u8,
}

/// Founding-position facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PositionFacts {
    market: Hash32,
    owner: Hash32,
    generation: u64,
    internal: [u64; MAX_OUTCOMES],
    cash_atoms: u64,
    reserved_cash_atoms: u64,
    stored_bump: u8,
    close_state: u8,
}

/// Replay-sequence facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReplayFacts {
    market: Hash32,
    owner: Hash32,
    position_generation: u64,
    sequence: u64,
    stored_bump: u8,
}

/// ABI-independent facts needed from the freshly written resolution record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InitialResolutionFacts {
    market: Hash32,
    terms: Hash32,
    feed: Hash32,
    resolved: bool,
    stored_bump: u8,
}

#[inline(never)]
fn read_initial_resolution(
    terms_data: &[u8],
    resolution_data: &[u8],
) -> Outcome<InitialResolutionFacts> {
    match resolution_account_len(terms_data)? {
        account_len::RESOLUTION => {
            let value = ResolutionAccount::decode(resolution_data)?;
            Ok(InitialResolutionFacts {
                market: value.market,
                terms: value.terms,
                feed: value.feed,
                resolved: value.is_resolved(),
                stored_bump: value.stored_bump,
            })
        }
        NATIVE_RESOLUTION_LEN => {
            let value = NativeResolutionAccount::decode(resolution_data)?;
            Ok(InitialResolutionFacts {
                market: value.market,
                terms: value.terms,
                feed: value.feed,
                resolved: value.is_resolved(),
                stored_bump: value.stored_bump,
            })
        }
        #[cfg(feature = "profile-full")]
        OCCUPATION_RESOLUTION_LEN => {
            let value = OccupationResolutionAccount::decode(resolution_data)?;
            Ok(InitialResolutionFacts {
                market: value.market,
                terms: value.terms,
                feed: value.feed,
                resolved: value.is_resolved(),
                stored_bump: value.stored_bump,
            })
        }
        _ => Err(ClutchError::WrongDataLength.into()),
    }
}

/// Decode a Profile account, carrying the freeze discipline fields.
#[inline(never)]
fn read_profile_init(data: &[u8]) -> Outcome<ProfileInitFacts> {
    let value = ProfileAccount::decode(data)?;
    Ok(ProfileInitFacts {
        profile: value.profile,
        realm: value.realm,
        version: value.version,
        collateral_policy_id: value.collateral_policy_id,
        adapter_release_id: value.adapter_release_id,
        flags: value.flags,
    })
}

/// Decode a Hoard account.
#[inline(never)]
fn read_hoard(data: &[u8]) -> Outcome<HoardFacts> {
    let value = HoardAccount::decode(data)?;
    Ok(HoardFacts {
        market: value.market,
        realm: value.realm,
        authority: value.authority,
        collateral_atoms: value.collateral_atoms,
        stored_bump: value.stored_bump,
    })
}

/// Decode a Position account.
#[inline(never)]
fn read_position(data: &[u8]) -> Outcome<PositionFacts> {
    let value = PositionAccount::decode(data)?;
    Ok(PositionFacts {
        market: value.market,
        owner: value.owner,
        generation: value.generation,
        internal: value.internal,
        cash_atoms: value.cash_atoms,
        reserved_cash_atoms: value.reserved_cash_atoms,
        stored_bump: value.stored_bump,
        close_state: value.close_state,
    })
}

/// Decode a reference-only replay-sequence account.
#[inline(never)]
fn read_replay(data: &[u8]) -> Outcome<ReplayFacts> {
    let value = ReplayAccount::decode(data)?;
    Ok(ReplayFacts {
        market: value.market,
        owner: value.owner,
        position_generation: value.position_generation,
        sequence: value.sequence,
        stored_bump: value.stored_bump,
    })
}

/* ------------------------------------------------------------------------ */
/* Payout-set plumbing                                                       */
/* ------------------------------------------------------------------------ */

/// Lift the immutable terms payout vectors into the kernel's payout set.
///
/// The terms artifact is the only committed source of "what this market pays":
/// [`MarketAccount::terms`] is the digest of the vectors' body.  The whole set
/// is over a kilobyte, so this is the only place it is materialized and it
/// never crosses into [`process`]'s frame.
///
/// `decode_unchecked`: the terms bytes were already fully decoded — digest
/// recomputation included — earlier in this same instruction (the address
/// plane in [`process`], and `validate_market_wide`'s own full read before
/// `require_payout_set_binding` runs), and the account is presented
/// read-only, so re-paying the SHA-256 here would be a second copy of a fact
/// this transaction already established.
#[inline(never)]
fn terms_payout_set(terms_data: &[u8]) -> Outcome<PayoutSet> {
    let mut terms = TermsAccount::ZEROED;
    TermsAccount::decode_unchecked_into(terms_data, &mut terms)?;
    let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
    let mut index = 0_usize;
    while index < usize::from(terms.payout_count) {
        vectors[index] = PayoutVector::new(
            terms.payouts[index].denominator,
            terms.payouts[index].weights,
        );
        index += 1;
    }
    Ok(PayoutSet::new(
        terms.payout_count,
        terms.outcome_count,
        vectors,
    ))
}

/// The terms' digest-committed collateral cap, in its own frame.
///
/// Same `decode_unchecked` soundness argument as [`terms_payout_set`].
#[inline(never)]
fn terms_collateral_cap(terms_data: &[u8]) -> Outcome<u64> {
    let mut terms = TermsAccount::ZEROED;
    TermsAccount::decode_unchecked_into(terms_data, &mut terms)?;
    Ok(terms.collateral_cap)
}

/// The immutable basis mode selects the sole admitted resolution-account ABI.
#[inline(never)]
fn terms_basis_degree(terms_data: &[u8]) -> Outcome<u8> {
    let mut terms = TermsAccount::ZEROED;
    TermsAccount::decode_unchecked_into(terms_data, &mut terms)?;
    require(terms.basis_degree <= 3, ClutchError::NonCanonical)?;
    Ok(terms.basis_degree)
}

#[inline(never)]
fn terms_statistic(terms_data: &[u8]) -> Outcome<u16> {
    let mut terms = TermsAccount::ZEROED;
    TermsAccount::decode_unchecked_into(terms_data, &mut terms)?;
    Ok(terms.statistic_id)
}

fn basis_mode_for_degree(degree: u8) -> BasisMode {
    if degree == 0 {
        BasisMode::FinitePreset
    } else {
        BasisMode::DerivedBasis
    }
}

fn resolution_account_len(terms_data: &[u8]) -> Outcome<usize> {
    let degree = terms_basis_degree(terms_data)?;
    if degree == 0 {
        return Ok(account_len::RESOLUTION);
    }
    if is_occupation_statistic(terms_statistic(terms_data)?) {
        #[cfg(feature = "profile-full")]
        return Ok(OCCUPATION_RESOLUTION_LEN);
        #[cfg(not(feature = "profile-full"))]
        return Err(ClutchError::UnsupportedInstruction.into());
    } else {
        Ok(NATIVE_RESOLUTION_LEN)
    }
}

/// Compare an encoded kernel account's payout set against an expected set.
#[inline(never)]
fn require_kernel_payouts(
    kernel_data: &[u8],
    expected: &PayoutSet,
    expected_mode: BasisMode,
) -> Outcome<()> {
    let kernel = KernelAccount::decode(kernel_data)?;
    if kernel.basis_mode != expected_mode {
        return Err(Refusal::Reference(ReferenceError::Kernel(
            clutch_kernel::Error::WrongResolutionMode,
        )));
    }
    require(
        kernel.payouts.count == expected.count && kernel.payouts.outcomes == expected.outcomes,
        ClutchError::PayoutSetMismatch,
    )?;
    let mut index = 0_usize;
    while index < MAX_PAYOUTS {
        require(
            kernel.payouts.vectors[index] == expected.vectors[index],
            ClutchError::PayoutSetMismatch,
        )?;
        index += 1;
    }
    Ok(())
}

/// The reference adapter's `require_payout_set_binding`, hoisted to creation.
#[inline(never)]
fn require_payout_set_binding(kernel_data: &[u8], terms_data: &[u8]) -> Outcome<()> {
    let expected = terms_payout_set(terms_data)?;
    let expected_mode = basis_mode_for_degree(terms_basis_degree(terms_data)?);
    require_kernel_payouts(kernel_data, &expected, expected_mode)
}

/// The reference adapter's `kernel_market` plus `check_invariants`.
///
/// The whole `KernelAccount`/`MarketState` working set lives in this frame and
/// nowhere else, exactly as [`super::split`]'s `kernel_split` does.
#[inline(never)]
fn require_kernel_invariants(
    kernel_data: &[u8],
    outcome_count: u8,
    collateral: u64,
    basis_degree: u8,
) -> Outcome<()> {
    let kernel = KernelAccount::decode(kernel_data)?;
    let expected_mode = basis_mode_for_degree(basis_degree);
    if kernel.basis_mode != expected_mode {
        return Err(Refusal::Reference(ReferenceError::Kernel(
            clutch_kernel::Error::WrongResolutionMode,
        )));
    }
    require(
        usize::from(outcome_count) <= KERNEL_MAX_OUTCOMES
            && kernel.payouts.outcomes == outcome_count,
        ClutchError::MismatchedState,
    )?;
    let phase = match kernel.phase {
        0 => Phase::Active,
        1 => Phase::Resolved,
        _ => return Err(ClutchError::NonCanonical.into()),
    };
    let market = MarketState {
        outcomes: outcome_count,
        phase,
        resolved_payout: kernel.resolved_payout,
        basis_mode: kernel.basis_mode,
        resolved_vector: PayoutVector::ZERO,
        collateral,
        total_supply: kernel.total_supply,
        payouts: kernel.payouts,
    };
    market.check_invariants()?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Preconditions                                                             */
/* ------------------------------------------------------------------------ */

/// Match the routed envelope, refusing any intent that is not `CreateMarket`.
pub fn create_market_intent(request: &Request) -> Outcome<CreateMarketIntent> {
    match request.action {
        Action::Layout(Intent::CreateMarket {
            realm,
            profile,
            market_nonce,
            outcome_count,
            terms,
            feed,
        }) => Ok(CreateMarketIntent {
            realm,
            profile,
            market_nonce,
            outcome_count,
            terms,
            feed,
        }),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// Creation consumes no replay sequence, so the envelope must carry zero.
pub fn require_creation_sequence(sequence: u64) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)
}

/// Refuse a Realm whose Profile has not frozen its collateral policy.
///
/// This is the Profile-side freeze discipline. The separately presented policy
/// evidence is content-authenticated by [`admit_collateral`].
fn require_frozen_collateral_policy(profile: &ProfileInitFacts) -> Outcome<()> {
    require(
        profile.flags & PROFILE_FLAG_POLICY_FROZEN != 0
            && profile.collateral_policy_id != Hash32::ZERO
            && profile.adapter_release_id != Hash32::ZERO,
        ClutchError::CollateralPolicyNotFrozen,
    )
}

/* ------------------------------------------------------------------------ */
/* The initialization write                                                  */
/* ------------------------------------------------------------------------ */

/// Encode the initial Market account.
#[inline(never)]
fn write_market(
    data: &mut [u8],
    intent: &CreateMarketIntent,
    market: Hash32,
    bumps: &PlaneBumps,
    collateral_cap: u64,
) -> Outcome<()> {
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    let mut index = 0_usize;
    while index < usize::from(intent.outcome_count) && index < MAX_OUTCOMES {
        outcomes[index] = canonical_outcome_id(market, index as u8);
        index += 1;
    }
    let account = MarketAccount {
        market,
        realm: intent.realm,
        profile: intent.profile,
        terms: intent.terms,
        outcome_count: intent.outcome_count,
        lifecycle: 0,
        stored_bump: bumps.market,
        hoard_bump: bumps.hoard,
        outcomes,
        feed: intent.feed,
        /* The terms' digest-committed cap; `created_slot` stays the named
         * zero placeholder.  See the cap section of the module docs. */
        collateral_cap,
        created_slot: 0,
        reserved: Hash32::ZERO,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the initial Hoard account.
#[inline(never)]
fn write_hoard(
    data: &mut [u8],
    identities: &FoundingIdentities,
    realm: Hash32,
    bump: u8,
) -> Outcome<()> {
    let account = HoardAccount {
        market: identities.market,
        realm,
        authority: identities.hoard_authority,
        collateral_atoms: 0,
        stored_bump: bump,
        flags: 0,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the founding Position account, provably zero (C0).
#[inline(never)]
fn write_position(data: &mut [u8], market: Hash32, owner: Hash32, bump: u8) -> Outcome<()> {
    let account = PositionAccount {
        market,
        owner,
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        stored_bump: bump,
        close_state: 0,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the initial reference-only kernel aggregate.
#[inline(never)]
fn write_kernel(data: &mut [u8], terms_data: &[u8], market: Hash32) -> Outcome<()> {
    let payouts = terms_payout_set(terms_data)?;
    let basis_mode = basis_mode_for_degree(terms_basis_degree(terms_data)?);
    let account = KernelAccount {
        market,
        phase: 0,
        basis_mode,
        resolved_payout: 0,
        payouts,
        total_supply: [0; MAX_OUTCOMES],
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the founding replay sequence, provably zero (C0).
#[inline(never)]
fn write_replay(data: &mut [u8], market: Hash32, owner: Hash32, bump: u8) -> Outcome<()> {
    let account = ReplayAccount {
        market,
        owner,
        position_generation: 0,
        sequence: 0,
        stored_bump: bump,
        flags: 0,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the initial market-wide supply ledger, both terms zero.
#[inline(never)]
fn write_supply(
    data: &mut [u8],
    market: Hash32,
    realm: Hash32,
    outcome_count: u8,
    bump: u8,
) -> Outcome<()> {
    let account = SupplyLedgerAccount {
        market,
        realm,
        generation: 0,
        outcome_count,
        internal_supply: [0; MAX_OUTCOMES],
        external_supply: [0; MAX_OUTCOMES],
        stored_bump: bump,
        flags: 0,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the initial resolution record, unresolved.
#[inline(never)]
fn write_resolution(
    data: &mut [u8],
    terms_data: &[u8],
    market: Hash32,
    intent: &CreateMarketIntent,
    bump: u8,
) -> Outcome<()> {
    match resolution_account_len(terms_data)? {
        account_len::RESOLUTION => {
            require(
                data.len() == account_len::RESOLUTION,
                ClutchError::WrongDataLength,
            )?;
            let account = ResolutionAccount {
                market,
                terms: intent.terms,
                feed: intent.feed,
                window: Hash32::ZERO,
                feed_cursor: 0,
                sealed_end_bucket_exclusive: 0,
                repair_generation: 0,
                resolved_slot: 0,
                payout_index: PAYOUT_INDEX_UNRESOLVED,
                stored_bump: bump,
                flags: 0,
            };
            account.encode(data)?;
        }
        NATIVE_RESOLUTION_LEN => {
            require(
                data.len() == NATIVE_RESOLUTION_LEN,
                ClutchError::WrongDataLength,
            )?;
            NativeResolutionAccount::unresolved(market, intent.terms, intent.feed, bump)
                .encode(data)?;
        }
        #[cfg(feature = "profile-full")]
        OCCUPATION_RESOLUTION_LEN => {
            require(
                data.len() == OCCUPATION_RESOLUTION_LEN,
                ClutchError::WrongDataLength,
            )?;
            OccupationResolutionAccount::unresolved(market, intent.terms, intent.feed, bump)
                .encode(data)?;
        }
        _ => return Err(ClutchError::WrongDataLength.into()),
    }
    Ok(())
}

/// Write all seven initial account states.
///
/// Every `encode` runs the frozen codec's own `validate` first, so a malformed
/// account never reaches an account's data.  Nothing here checks anything
/// *across* accounts; that is [`validate_initial_plane`], which runs afterwards
/// over exactly these bytes.
#[inline(never)]
pub fn write_initial_plane(
    terms_data: &[u8],
    plane: PlaneWrite<'_>,
    intent: &CreateMarketIntent,
    identities: &FoundingIdentities,
    bumps: &PlaneBumps,
) -> Outcome<()> {
    let market = identities.market;
    write_market(
        plane.market,
        intent,
        market,
        bumps,
        terms_collateral_cap(terms_data)?,
    )?;
    write_hoard(plane.hoard, identities, intent.realm, bumps.hoard)?;
    write_position(plane.position, market, identities.owner, bumps.position)?;
    write_kernel(plane.kernel, terms_data, market)?;
    write_replay(plane.replay, market, identities.owner, bumps.replay)?;
    write_supply(
        plane.supply,
        market,
        intent.realm,
        intent.outcome_count,
        bumps.supply,
    )?;
    write_resolution(
        plane.resolution,
        terms_data,
        market,
        intent,
        bumps.resolution,
    )?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* The validation                                                            */
/* ------------------------------------------------------------------------ */

/// The market-wide half of `validate_market_init`.
///
/// Returns the decoded market facts so the founding-triple half does not decode
/// the market a second time.  The check order is the reference's: stored bumps,
/// cross-account linkage, the freeze gate, the intent-to-state identity
/// conjunction, the ledger binding, emptiness, kernel invariants, padding, and
/// the two-term closure.
#[inline(never)]
fn validate_market_wide(
    realm_data: &[u8],
    profile_data: &[u8],
    terms_data: &[u8],
    plane: PlaneBytes<'_>,
    intent: &CreateMarketIntent,
    bumps: &PlaneBumps,
) -> Outcome<MarketFacts> {
    let realm: RealmFacts = accounts::read_realm(realm_data)?;
    let profile = read_profile_init(profile_data)?;
    let terms: TermsFacts = accounts::read_terms(terms_data)?;
    let market: MarketFacts = accounts::read_market(plane.market)?;
    let hoard = read_hoard(plane.hoard)?;
    let kernel = accounts::read_kernel(plane.kernel)?;
    let supply: SupplyFacts = accounts::read_supply(plane.supply)?;
    let resolution = read_initial_resolution(terms_data, plane.resolution)?;

    /* Stored bumps, before anything reads a balance: an account presented at a
     * canonical address but carrying another address's bump is a mislinked
     * account whatever its contents say. */
    require(
        market.stored_bump == bumps.market
            && market.hoard_bump == bumps.hoard
            && hoard.stored_bump == bumps.hoard
            && supply.stored_bump == bumps.supply
            && resolution.stored_bump == bumps.resolution,
        ClutchError::WrongBump,
    )?;

    /* Cross-account linkage, mirroring `validate_links`. */
    require(
        market.market == hoard.market
            && market.realm == hoard.realm
            && market.market == kernel.market
            && market.market == supply.market
            && market.realm == supply.realm
            && market.outcome_count == supply.outcome_count
            && market.lifecycle == 0
            && kernel.phase == 0,
        ClutchError::MismatchedState,
    )?;

    require_frozen_collateral_policy(&profile)?;

    /* The intent-to-state conjunction of `validate_market_init`, including the
     * canonical market identity and the Realm/Profile edges. */
    let expected_market = canonical_market_id(intent.realm, intent.profile, intent.market_nonce);
    require(
        realm.realm == intent.realm
            && realm.profile == intent.profile
            && profile.profile == intent.profile
            && profile.realm == intent.realm
            && realm.profile_version == profile.version
            && usize::from(realm.max_outcomes) == MAX_OUTCOMES
            && intent.outcome_count <= realm.max_outcomes
            && market.market == expected_market
            && market.realm == intent.realm
            && market.profile == intent.profile
            && market.outcome_count == intent.outcome_count
            && market.terms == intent.terms
            && market.feed == intent.feed,
        ClutchError::MismatchedState,
    )?;

    /* NAMED STRENGTHENING: the presented terms artifact must be the one this
     * market's digest binds.  `TermsAccount::binds_market` is exactly this
     * comparison, and its refusal class is reproduced rather than reclassified.
     * The digest is self-certifying inside the codec, so equality of the digest
     * plus these five fields is equality of the whole artifact. */
    require_binding(
        terms.terms == market.terms
            && terms.realm == market.realm
            && terms.profile == market.profile
            && terms.feed == market.feed
            && terms.outcome_count == market.outcome_count,
    )?;

    /* The written cap must be the terms' digest-committed cap — the cap flow
     * of RESOLUTION_EVIDENCE_PLAN §3.5.  The terms codec refuses a zero cap,
     * so a founded market is never the unfundable cap-0 residue. */
    require_binding(market.collateral_cap == terms.collateral_cap)?;

    /* NAMED STRENGTHENING: the resolution record is present, bound, and
     * unresolved.  A market founded beside a record that already selects a
     * payout would be resolved before it existed. */
    require(
        resolution.market == market.market
            && resolution.terms == market.terms
            && resolution.feed == market.feed
            && !resolution.resolved,
        ClutchError::MismatchedState,
    )?;

    /* Emptiness. This is the market-wide half of C0 and of the reference's
     * `NonEmptyInitialization`; the founding owner plane's half follows. */
    let mut outcome = 0_usize;
    while outcome < MAX_OUTCOMES {
        require_reference(
            kernel.total_supply[outcome] == 0
                && supply.internal_supply[outcome] == 0
                && supply.external_supply[outcome] == 0,
            ReferenceError::NonEmptyInitialization,
        )?;
        outcome += 1;
    }
    require_reference(
        hoard.collateral_atoms == 0,
        ReferenceError::NonEmptyInitialization,
    )?;

    /* Kernel invariants over the founded state, then the payout-set binding. */
    require_kernel_invariants(
        plane.kernel,
        market.outcome_count,
        hoard.collateral_atoms,
        terms_basis_degree(terms_data)?,
    )?;
    require_payout_set_binding(plane.kernel, terms_data)?;

    /* C1: the two-term ledger closes against the kernel aggregate. */
    require_two_term_closure(&supply, &kernel, market.outcome_count)?;
    Ok(market)
}

/// The founding owner-plane half of `validate_market_init`: C0.
///
/// Position and Replay must be mutually bound to one market, owner, and
/// generation, and must be provably zero. External claims have no per-owner
/// program shadow: actual Token-2022 mint supply and holder accounts are the
/// composability-boundary truth.
#[inline(never)]
fn validate_founding_owner_plane(
    plane: PlaneBytes<'_>,
    market: &MarketFacts,
    bumps: &PlaneBumps,
) -> Outcome<()> {
    let position = read_position(plane.position)?;
    let replay = read_replay(plane.replay)?;
    let supply: SupplyFacts = accounts::read_supply(plane.supply)?;

    require(
        position.stored_bump == bumps.position && replay.stored_bump == bumps.replay,
        ClutchError::WrongBump,
    )?;
    require(
        market.market == position.market
            && market.market == replay.market
            && position.owner == replay.owner
            && position.generation == replay.position_generation,
        ClutchError::MismatchedState,
    )?;

    /* C0 proper. */
    require_reference(
        position.close_state == 0
            && position.cash_atoms == 0
            && position.reserved_cash_atoms == 0
            && replay.sequence == 0,
        ReferenceError::NonEmptyInitialization,
    )?;
    let mut outcome = 0_usize;
    while outcome < MAX_OUTCOMES {
        require_reference(
            position.internal[outcome] == 0,
            ReferenceError::NonEmptyInitialization,
        )?;
        outcome += 1;
    }

    /* Padding beyond the active outcome count, and C2 against the ledger terms.
     *
     * Both are *redundant here* and neither is load-bearing: the emptiness loop
     * above already proved every one of the `MAX_OUTCOMES` entries zero, which
     * implies canonical padding, and an all-zero triple is bounded by any
     * ledger.  They are kept because `validate_market_init` keeps them --
     * `validate_padding` and `validate_aggregate_closure` run on the same
     * initial state there -- and this function's contract is to be that
     * function, not a minimized version of it.  Claiming they catch something
     * at initialization would be false. */
    let count = usize::from(market.outcome_count);
    let mut padding = count;
    while padding < MAX_OUTCOMES {
        require(position.internal[padding] == 0, ClutchError::NonCanonical)?;
        padding += 1;
    }
    let mut represented = 0_usize;
    while represented < count {
        require(
            position.internal[represented] <= supply.internal_supply[represented],
            ClutchError::AggregateClosureMismatch,
        )?;
        represented += 1;
    }
    Ok(())
}

/// Re-compose `clutch_solana_reference::validate_market_init` over one plane.
///
/// The oracle for every check here is that function; the oracle for every byte
/// [`write_initial_plane`] produces is that function plus the frozen layout
/// codecs.  It is deliberately callable on already-encoded bytes with
/// separately supplied bumps, exactly as [`crate::accounts::expect_pda`] takes
/// an already-derived address: that is what lets the whole refusal table be
/// exercised on a host where program-address derivation is not compiled.
pub fn validate_initial_plane(
    realm_data: &[u8],
    profile_data: &[u8],
    terms_data: &[u8],
    plane: PlaneBytes<'_>,
    intent: &CreateMarketIntent,
    bumps: &PlaneBumps,
) -> Outcome<()> {
    let market = validate_market_wide(realm_data, profile_data, terms_data, plane, intent, bumps)?;
    validate_founding_owner_plane(plane, &market, bumps)
}

/// Create the seven-account state plane with a term-selected resolution width.
///
/// The shared categorical constructor remains the exact v2 path. Native
/// degree-1..=3 terms select the v3 point or v4 occupation width, so this path
/// reuses the shared address and zero-prestate preflight, creates the first six
/// accounts with the same seeds and widths, and varies only Resolution.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn create_native_market_state_plane<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    rent: &genesis::RentParameters,
    targets: &MarketStateTargets<'_, 'info>,
    identity: &MarketStateIdentity,
    bumps: &PlaneBumps,
    resolution_len: usize,
) -> Outcome<()> {
    construction::validate_market_state_addresses(program_id, targets, identity, bumps)?;
    construction::preflight_absent_market_state(targets)?;

    genesis::create_pda_account(
        program_id,
        payer,
        targets.market,
        system_program,
        rent,
        account_len::MARKET,
        &[
            seeds::SEED_MARKET,
            &identity.realm,
            &identity.market,
            &[bumps.market],
        ],
    )?;
    genesis::create_pda_account(
        program_id,
        payer,
        targets.hoard,
        system_program,
        rent,
        account_len::HOARD,
        &[seeds::SEED_HOARD, &identity.market, &[bumps.hoard]],
    )?;
    genesis::create_pda_account(
        program_id,
        payer,
        targets.position,
        system_program,
        rent,
        account_len::POSITION,
        &[
            seeds::SEED_POSITION,
            &identity.market,
            &identity.owner,
            &[bumps.position],
        ],
    )?;
    genesis::create_pda_account(
        program_id,
        payer,
        targets.kernel,
        system_program,
        rent,
        KERNEL_ACCOUNT_LEN,
        &[seeds::SEED_KERNEL, &identity.market, &[bumps.kernel]],
    )?;
    let generation = identity.generation.to_le_bytes();
    genesis::create_pda_account(
        program_id,
        payer,
        targets.replay,
        system_program,
        rent,
        REPLAY_ACCOUNT_LEN,
        &[
            seeds::SEED_REPLAY,
            &identity.market,
            &identity.owner,
            &generation,
            &[bumps.replay],
        ],
    )?;
    genesis::create_pda_account(
        program_id,
        payer,
        targets.supply,
        system_program,
        rent,
        account_len::SUPPLY_LEDGER,
        &[seeds::SEED_SUPPLY, &identity.market, &[bumps.supply]],
    )?;
    genesis::create_pda_account(
        program_id,
        payer,
        targets.resolution,
        system_program,
        rent,
        resolution_len,
        &[
            seeds::SEED_RESOLUTION,
            &identity.market,
            &[bumps.resolution],
        ],
    )
}

/* ------------------------------------------------------------------------ */
/* The instruction                                                           */
/* ------------------------------------------------------------------------ */

/// Validate hostile accounts and initialize exactly one market.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    let intent = create_market_intent(request)?;

    /* The outcome count decides the account count, so it is bounded before it
     * is used as one.  A market with no outcomes is not a market. */
    require(
        intent.outcome_count > 0 && usize::from(intent.outcome_count) <= MAX_OUTCOMES,
        ClutchError::NonCanonical,
    )?;
    require(
        accounts.len() == account_count(intent.outcome_count),
        ClutchError::AccountCount,
    )?;

    /* The authority model, in three lines: the creator signs, nothing else is
     * privileged, and the creator's address is the founding position owner.
     * The creator is also the rent payer for every account founded below, so
     * the runtime must have been told its lamports may fall. */
    let creator = &accounts[IX_CREATOR];
    require_signer(creator)?;
    require(creator.is_writable, ClutchError::NotWritable)?;

    require_market_account_distinctness(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &INPUT_STATE_ROLES)?;
    require_creation_sequence(request.sequence)?;

    /* Identities.  The market identity is a function of the intent alone, and
     * caller-supplied expected keys are never accepted: every address below is
     * recomputed from the frozen seed schema and compared. */
    let market_id = canonical_market_id(intent.realm, intent.profile, intent.market_nonce);
    let realm_bytes = intent.realm.bytes();
    let profile_bytes = intent.profile.bytes();
    let terms_bytes = intent.terms.bytes();
    let market_bytes = market_id.bytes();
    let owner_bytes = creator.key.to_bytes();

    let realm_stored_bump = accounts::read_realm(&accounts[IX_REALM].data.borrow())?.stored_bump;
    let (terms_stored_bump, terms_collateral_cap) = {
        let terms = accounts::read_terms(&accounts[IX_TERMS].data.borrow())?;
        (terms.stored_bump, terms.collateral_cap)
    };
    expect_pda(
        accounts[IX_REALM].key,
        seeds::realm_pda(program_id, &realm_bytes),
        Some(realm_stored_bump),
    )?;
    expect_pda(
        accounts[IX_PROFILE].key,
        seeds::profile_pda(program_id, &realm_bytes, &profile_bytes),
        None,
    )?;
    expect_pda(
        accounts[IX_TERMS].key,
        seeds::terms_pda(program_id, &realm_bytes, &terms_bytes),
        Some(terms_stored_bump),
    )?;

    /* The seven target accounts are absent System-owned slots. Their canonical
     * bumps are written after the construction module derives, authenticates,
     * rent-funds, allocates, and assigns every one. */
    let market_derived = seeds::market_pda(program_id, &realm_bytes, &market_bytes);
    let hoard_derived = seeds::hoard_pda(program_id, &market_bytes);
    let position_derived = seeds::position_pda(program_id, &market_bytes, &owner_bytes);
    let kernel_derived = seeds::kernel_pda(program_id, &market_bytes);
    let replay_derived = seeds::replay_pda(program_id, &market_bytes, &owner_bytes, 0);
    let supply_derived = seeds::supply_pda(program_id, &market_bytes);
    let resolution_derived = seeds::resolution_pda(program_id, &market_bytes);

    let bumps = PlaneBumps {
        market: market_derived.1,
        hoard: hoard_derived.1,
        position: position_derived.1,
        kernel: kernel_derived.1,
        replay: replay_derived.1,
        supply: supply_derived.1,
        resolution: resolution_derived.1,
    };

    /* The token plane's own addresses and roles.  The Hoard *authority* is a
     * signing address that holds nothing, so it is proved derived and proved
     * empty and never written; the Hoard *token account* is created below. */
    validate_creation_roles(accounts)?;
    let hoard_authority = &accounts[IX_HOARD_AUTHORITY];
    require(
        !hoard_authority.is_writable,
        ClutchError::UnexpectedWritable,
    )?;
    require(!hoard_authority.executable, ClutchError::ExecutableAccount)?;
    require(
        hoard_authority.data_is_empty(),
        ClutchError::WrongDataLength,
    )?;
    expect_pda(
        hoard_authority.key,
        seeds::hoard_authority_pda(program_id, &market_bytes),
        None,
    )?;

    /* Collateral admission, before the first write.  The cap compared against
     * the policy ceiling is the *terms'* cap, which is the same value
     * `write_initial_plane` is about to put in `MarketAccount::collateral_cap`
     * and which `validate_initial_plane` then re-checks equal to it -- so a
     * market cannot be founded above its Realm's ceiling by writing one cap and
     * admitting another. */
    require_canonical_policy_pda(program_id, accounts)?;
    let collateral = admit_collateral(
        program_id,
        accounts,
        market_id,
        terms_collateral_cap,
        *hoard_authority.key,
    )?;

    /* Authenticate and create every program-owned state target before writing
     * any founding bytes. `create_market_state_plane` checks all seven absent
     * before its first CPI; a later token/encode refusal rolls all of them back
     * with the transaction. */
    let rent = genesis::read_rent(&accounts[IX_RENT])?;
    let targets = MarketStateTargets {
        market: &accounts[IX_MARKET],
        hoard: &accounts[IX_HOARD],
        position: &accounts[IX_POSITION],
        kernel: &accounts[IX_KERNEL],
        replay: &accounts[IX_REPLAY],
        supply: &accounts[IX_SUPPLY],
        resolution: &accounts[IX_RESOLUTION],
    };
    let state_identity = MarketStateIdentity {
        realm: realm_bytes,
        market: market_bytes,
        owner: owner_bytes,
        generation: 0,
    };
    let resolution_len = resolution_account_len(&accounts[IX_TERMS].data.borrow())?;
    if resolution_len == account_len::RESOLUTION {
        construction::create_market_state_plane(
            program_id,
            creator,
            &accounts[IX_SYSTEM_PROGRAM],
            &rent,
            &targets,
            &state_identity,
            &bumps,
        )?;
    } else {
        create_native_market_state_plane(
            program_id,
            creator,
            &accounts[IX_SYSTEM_PROGRAM],
            &rent,
            &targets,
            &state_identity,
            &bumps,
            resolution_len,
        )?;
    }

    /* The outcome mints and Hoard token account follow and are re-admitted
     * through the policies every later instruction applies. */
    create_token_plane(
        program_id,
        accounts,
        &market_bytes,
        intent.outcome_count,
        collateral,
        &rent,
    )?;

    let identities = FoundingIdentities {
        market: market_id,
        owner: Hash32::from_bytes(owner_bytes),
        hoard_authority: Hash32::from_bytes(accounts[IX_HOARD_AUTHORITY].key.to_bytes()),
    };

    {
        let borrow = |index: usize| {
            accounts[index]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
        };
        let mut market_data = borrow(IX_MARKET)?;
        let mut hoard_data = borrow(IX_HOARD)?;
        let mut position_data = borrow(IX_POSITION)?;
        let mut kernel_data = borrow(IX_KERNEL)?;
        let mut replay_data = borrow(IX_REPLAY)?;
        let mut supply_data = borrow(IX_SUPPLY)?;
        let mut resolution_data = borrow(IX_RESOLUTION)?;
        write_initial_plane(
            &accounts[IX_TERMS].data.borrow(),
            PlaneWrite {
                market: &mut market_data,
                hoard: &mut hoard_data,
                position: &mut position_data,
                kernel: &mut kernel_data,
                replay: &mut replay_data,
                supply: &mut supply_data,
                resolution: &mut resolution_data,
            },
            &intent,
            &identities,
            &bumps,
        )?;
    }

    /* ...and this re-reads exactly what was written and runs the whole of the
     * offline `validate_market_init` over it.  A refusal here aborts the
     * instruction; SVM transaction semantics discard the write. */
    validate_initial_plane(
        &accounts[IX_REALM].data.borrow(),
        &accounts[IX_PROFILE].data.borrow(),
        &accounts[IX_TERMS].data.borrow(),
        PlaneBytes {
            market: &accounts[IX_MARKET].data.borrow(),
            hoard: &accounts[IX_HOARD].data.borrow(),
            position: &accounts[IX_POSITION].data.borrow(),
            kernel: &accounts[IX_KERNEL].data.borrow(),
            replay: &accounts[IX_REPLAY].data.borrow(),
            supply: &accounts[IX_SUPPLY].data.borrow(),
            resolution: &accounts[IX_RESOLUTION].data.borrow(),
        },
        &intent,
        &bumps,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        canonical_profile_hash, canonical_realm_id, CodecError, PayoutVectorBytes, RealmAccount,
        PROFILE_PARENT_BYTES,
    };

    /* These tests run on the host, where `seeds::find` is deliberately not
     * compiled (see the module docs of `crate::seeds`).  They therefore cover
     * everything this instruction is except the one thing an address syscall
     * decides: the initialization write, byte for byte, and every refusal in
     * `validate_market_init`.  `process` itself is exercised only up to its
     * first derivation; the SVM leg is a follow-up wave. */

    const REALM_NONCE: u64 = 7;
    const MARKET_NONCE: u64 = 9;
    const OUTCOME_COUNT: u8 = 2;
    const PAYOUT_COUNT: u8 = 2;
    /// The terms' digest-committed collateral cap the founding write copies.
    const FIXTURE_CAP: u64 = 5_000;

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    /* ------------------------------------------------------------------ */
    /* The optional collateral-admission leg                               */
    /* ------------------------------------------------------------------ */

    /// Owned backing store for one host-side `AccountInfo`.
    struct Cell {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
        is_writable: bool,
        executable: bool,
    }

    impl Cell {
        fn inert() -> Self {
            Self {
                key: Pubkey::new_from_array([0; 32]),
                owner: Pubkey::new_from_array([0; 32]),
                lamports: 1,
                data: Vec::new(),
                is_writable: false,
                executable: false,
            }
        }

        fn info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                false,
                self.is_writable,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                self.executable,
            )
        }
    }

    /// The four accounts [`admit_collateral`] actually reads, in a
    /// fifteen-slot list.
    ///
    /// The other eleven are inert: this exercises the admission decision
    /// itself, which is the half of `process` that does not derive an address
    /// and therefore *can* run on a host where `seeds::find` is
    /// `unimplemented!()`. The derivation half is covered by the SVM
    /// workspace.
    struct CollateralCase {
        profile: Vec<u8>,
        policy: Vec<u8>,
        policy_len_override: Option<usize>,
        token_program: Pubkey,
        token_program_executable: bool,
        mint_key: Pubkey,
        mint_owner: Pubkey,
        mint_data: Vec<u8>,
        cap: u64,
    }

    fn fixture_policy(mint: [u8; 32], decimals: u8) -> collateral::CollateralPolicy {
        let backing = collateral::CurrencyRef::spl(collateral::TOKEN_2022_PROGRAM, mint, decimals);
        collateral::CollateralPolicy {
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
        }
    }

    impl CollateralCase {
        /// A case that passes every check, so a test can break exactly one.
        fn admitted() -> Self {
            let mint = [0x6d_u8; 32];
            let policy = fixture_policy(mint, 6);
            let policy_bytes = policy
                .canonical_bytes()
                .expect("the fixture policy must encode")
                .to_vec();
            /* The Profile identity is the *parent* hash over this policy's own
             * digest, recomputed rather than chosen: `verify_profile_identity`
             * refuses any other pairing, which is the whole point of binding
             * by digest instead of by address. */
            let parent = collateral::ParentProfile::from_policy(&policy)
                .expect("the parent profile must compose");
            let profile_id = parent.identity().expect("the parent identity must derive");
            let profile = ProfileAccount {
                profile: profile_id,
                realm: h(0x11),
                collateral_policy_id: policy.digest().expect("digest"),
                adapter_release_id: h(0x52),
                version: 2,
                flags: PROFILE_FLAG_POLICY_FROZEN,
            };
            let mut profile_bytes = vec![0_u8; account_len::PROFILE];
            profile
                .encode(&mut profile_bytes)
                .expect("the fixture profile must encode");
            Self {
                profile: profile_bytes,
                policy: policy_bytes,
                policy_len_override: None,
                token_program: crate::token::TOKEN_2022_PROGRAM_ID,
                token_program_executable: true,
                mint_key: Pubkey::new_from_array(mint),
                mint_owner: crate::token::TOKEN_2022_PROGRAM_ID,
                mint_data: crate::token::fixtures::mint_bytes(6, 5_000, None, None),
                cap: 5_000,
            }
        }

        fn run(&self) -> Outcome<()> {
            self.admit().map(|_| ())
        }

        fn admit(&self) -> Outcome<AdmittedMarketCollateralV2> {
            let program_id = Pubkey::new_from_array([0xc1; 32]);
            let mut cells: Vec<Cell> = (0..account_count(2)).map(|_| Cell::inert()).collect();
            cells[IX_PROFILE].data = self.profile.clone();
            cells[IX_POLICY].data = self.policy.clone();
            cells[IX_POLICY].owner = program_id;
            if let Some(len) = self.policy_len_override {
                cells[IX_POLICY].data.resize(len, 0);
            }
            cells[IX_TOKEN_PROGRAM].key = self.token_program;
            cells[IX_TOKEN_PROGRAM].executable = self.token_program_executable;
            cells[IX_COLLATERAL_MINT].key = self.mint_key;
            cells[IX_COLLATERAL_MINT].owner = self.mint_owner;
            cells[IX_COLLATERAL_MINT].data = self.mint_data.clone();
            let infos: Vec<AccountInfo<'_>> = cells.iter_mut().map(Cell::info).collect();
            admit_collateral(
                &program_id,
                &infos,
                h(0x73),
                self.cap,
                Pubkey::new_from_array([0xa5; 32]),
            )
        }
    }

    #[test]
    fn the_realms_own_frozen_policy_admits_its_own_collateral_mint() {
        CollateralCase::admitted()
            .run()
            .expect("a base Token-2022 mint under a V1 policy is admitted");
    }

    #[test]
    fn a_policy_the_profile_is_not_frozen_to_cannot_decide_admission() {
        /* The load-bearing check. Without the recomputed digest a well-formed
         * policy and a well-formed Profile could be paired freely, and an
         * adapter that merely decoded both would have checked nothing. */
        let mut case = CollateralCase::admitted();
        let other = fixture_policy([0x21; 32], 6);
        case.policy = other.canonical_bytes().expect("encodes").to_vec();
        assert!(
            case.run().is_err(),
            "a policy the Profile does not commit to must not decide anything"
        );

        // And a policy account of the wrong length never reaches the decoder.
        let mut short = CollateralCase::admitted();
        short.policy_len_override = Some(collateral::COLLATERAL_POLICY_BYTES - 1);
        assert_eq!(
            short.run().unwrap_err(),
            ClutchError::WrongDataLength.into()
        );
    }

    #[test]
    fn the_collateral_matrix_refuses_a_hostile_mint_at_market_initialization() {
        /* The market-initialization half of `TOKEN2022_PLAN.md` §3.4, over the
         * Realm's actual bitsets rather than a re-stated matrix. Every
         * discriminant here is a row of `COLLATERAL_PROFILES.md`. */
        for (discriminant, label) in [
            (1_u16, "TransferFeeConfig"),
            (3, "MintCloseAuthority"),
            (6, "DefaultAccountState"),
            (9, "NonTransferable"),
            (12, "PermanentDelegate"),
            (14, "TransferHook"),
            (26, "Pausable"),
        ] {
            let mut case = CollateralCase::admitted();
            case.mint_data = crate::token::fixtures::with_extension(
                &crate::token::fixtures::mint_bytes(6, 5_000, None, None),
                crate::token::BASE_MINT_LEN,
                1,
                discriminant,
            );
            assert_eq!(
                case.run().unwrap_err(),
                ClutchError::TokenExtensionNotAllowed.into(),
                "{label} must be refused at market initialization"
            );
        }
    }

    #[test]
    fn a_live_mint_authority_or_freeze_authority_refuses_the_collateral() {
        let mut live = CollateralCase::admitted();
        live.mint_data = crate::token::fixtures::mint_bytes(6, 5_000, Some([0x33; 32]), None);
        assert_eq!(
            live.run().unwrap_err(),
            ClutchError::MintNotAdmitted.into(),
            "a live mint authority means the collateral supply is not fixed"
        );

        let mut freezable = CollateralCase::admitted();
        freezable.mint_data = crate::token::fixtures::mint_bytes(6, 5_000, None, Some([0x44; 32]));
        assert_eq!(
            freezable.run().unwrap_err(),
            ClutchError::MintNotAdmitted.into()
        );

        // Another mint entirely, presented for this Realm's policy.
        let mut elsewhere = CollateralCase::admitted();
        elsewhere.mint_key = Pubkey::new_from_array([0x9e; 32]);
        assert_eq!(
            elsewhere.run().unwrap_err(),
            ClutchError::MintNotAdmitted.into()
        );

        // And the right mint under some other program.
        let mut wrong_program = CollateralCase::admitted();
        wrong_program.mint_owner = Pubkey::new_from_array([0x5a; 32]);
        assert_eq!(
            wrong_program.run().unwrap_err(),
            ClutchError::WrongTokenProgram.into()
        );
    }

    #[test]
    fn the_market_cap_ceiling_is_finally_checkable_on_chain() {
        /* The module docs above record this as "an obligation on whoever adds a
         * policy-bytes account to the schema". This is that obligation
         * discharged: the fixture policy's mint ceiling is 10 000 atoms, so a
         * market founded with a larger cap is refused rather than founded. */
        let mut over = CollateralCase::admitted();
        over.cap = 10_001;
        assert!(over.run().is_err(), "a cap above the Realm ceiling refuses");

        let mut at_ceiling = CollateralCase::admitted();
        at_ceiling.cap = 10_000;
        at_ceiling
            .run()
            .expect("a cap exactly at the ceiling is admitted");
    }

    #[test]
    fn a_realm_that_does_not_admit_immutable_owner_gets_no_hoard() {
        /* Open decision 4 of `TOKEN2022_PLAN.md`, taken.  The Hoard's whole
         * security story is that its owner authority is a program address;
         * `SetAuthority(AccountOwner)` is the instruction that would break
         * that and `ImmutableOwner` is the extension that forbids it.  V1
         * merely *allows* the extension, so a Realm can write a policy that
         * does not admit it — and this program will not found a Hoard it
         * cannot make immutable. */
        let mut narrow = CollateralCase::admitted();
        let mut policy = fixture_policy([0x6d; 32], 6);
        policy.allowed_account_extensions = 0;
        let parent = collateral::ParentProfile::from_policy(&policy).expect("the parent composes");
        let profile = ProfileAccount {
            profile: parent.identity().expect("the parent identity derives"),
            realm: h(0x11),
            collateral_policy_id: policy.digest().expect("digest"),
            adapter_release_id: h(0x52),
            version: 2,
            flags: PROFILE_FLAG_POLICY_FROZEN,
        };
        let mut profile_bytes = vec![0_u8; account_len::PROFILE];
        profile
            .encode(&mut profile_bytes)
            .expect("the narrowed profile encodes");
        narrow.profile = profile_bytes;
        narrow.policy = policy.canonical_bytes().expect("encodes").to_vec();
        assert_eq!(
            narrow.run(),
            Err(ClutchError::TokenAccountNotAdmitted.into()),
            "a Realm that forbids ImmutableOwner cannot have a Hoard here"
        );
    }

    #[test]
    fn the_admitted_case_yields_the_policy_the_hoard_is_created_under() {
        /* The returned value is not decoration: it is what
         * `create_token_plane` checks the account it just created against, so
         * the mint the Hoard holds and the authority that owns it are the
         * Realm's decision rather than this module's. */
        let authority = Pubkey::new_from_array([0xa5; 32]);
        let policy = CollateralCase::admitted()
            .admit()
            .expect("the fixture policy admits its own mint");
        assert_eq!(policy.expected_owner_authority, authority);
        assert_eq!(policy.mint, Pubkey::new_from_array([0x6d; 32]));
        assert_eq!(
            policy.allowed_extensions,
            collateral::EXTENSION_IMMUTABLE_OWNER
        );
        assert!(policy.require_delegate_none && policy.require_close_authority_none);
    }

    #[test]
    fn the_token_program_role_is_authenticated_at_initialization_too() {
        let mut impostor = CollateralCase::admitted();
        impostor.token_program = Pubkey::new_from_array([0x4d; 32]);
        assert_eq!(
            impostor.run().unwrap_err(),
            ClutchError::WrongTokenProgram.into()
        );

        let mut inert = CollateralCase::admitted();
        inert.token_program_executable = false;
        assert_eq!(
            inert.run().unwrap_err(),
            ClutchError::WrongTokenProgram.into()
        );
    }

    fn plane_bumps() -> PlaneBumps {
        PlaneBumps {
            market: 3,
            hoard: 4,
            position: 5,
            kernel: 6,
            replay: 7,
            supply: 10,
            resolution: 9,
        }
    }

    fn profile_hash() -> Hash32 {
        canonical_profile_hash(&[0xc0; PROFILE_PARENT_BYTES]).expect("exact parent preimage")
    }

    fn realm_hash() -> Hash32 {
        canonical_realm_id(profile_hash(), REALM_NONCE)
    }

    fn market_id() -> Hash32 {
        canonical_market_id(realm_hash(), profile_hash(), MARKET_NONCE)
    }

    fn unit_vector(index: usize) -> [u64; MAX_OUTCOMES] {
        let mut weights = [0; MAX_OUTCOMES];
        weights[index] = 1;
        weights
    }

    /// The immutable terms artifact the fixture market binds.
    ///
    /// Its window policy is the offline reference adapter's own resolution
    /// fixture, so the payout set this instruction lifts into the kernel is the
    /// set the reference would have expected to find there.
    fn terms_account(profile: Hash32) -> TermsAccount {
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        payouts[0] = PayoutVectorBytes {
            denominator: 1,
            weights: unit_vector(0),
        };
        payouts[1] = PayoutVectorBytes {
            denominator: 1,
            weights: unit_vector(1),
        };
        let mut knots = [0u128; clutch_solana_layout::MAX_KNOTS];
        knots[0] = 1;
        let mut payout_map = [clutch_solana_layout::PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        payout_map[0] = 0;
        payout_map[1] = 1;
        let mut value = TermsAccount {
            terms: Hash32::ZERO,
            realm: realm_hash(),
            profile,
            feed: h(9),
            price_grid: h(0x9a),
            outcome_count: OUTCOME_COUNT,
            payout_count: PAYOUT_COUNT,
            payouts,
            grid_family_id: 7,
            grid_version: 1,
            bucket_seconds: 60,
            expected_start_bucket: 100,
            expected_end_bucket_exclusive: 130,
            maturity_horizon_buckets: 30,
            coverage_policy_id: 11,
            repair_policy_id: 12,
            failure_policy_id: 13,
            statistic_id: 1,
            ambiguity_policy_id: 1,
            edge_policy_id: 1,
            basis_degree: 0,
            knot_count: 1,
            uniform_log2_spacing: clutch_solana_layout::UNIFORM_SPACING_NONE,
            failure_payout_index: 0,
            coverage_policy_parameter: 0,
            repair_generation: 0,
            source_version: 1,
            evaluator_version: 1,
            source_adapter_id: h(9),
            payout_map,
            knots,
            collateral_cap: FIXTURE_CAP,
            stored_bump: 8,
            flags: 0,
        };
        value.terms = value.recomputed_terms_digest().expect("terms body digests");
        value
    }

    fn realm_account() -> RealmAccount {
        RealmAccount {
            realm: realm_hash(),
            profile: profile_hash(),
            max_outcomes: MAX_OUTCOMES as u8,
            profile_version: 2,
            stored_bump: 200,
            flags: 0,
        }
    }

    fn profile_account() -> ProfileAccount {
        ProfileAccount {
            profile: profile_hash(),
            realm: realm_hash(),
            collateral_policy_id: h(0xd0),
            adapter_release_id: h(0x52),
            version: 2,
            flags: PROFILE_FLAG_POLICY_FROZEN,
        }
    }

    fn encoded<F>(len: usize, encode: F) -> Vec<u8>
    where
        F: FnOnce(&mut [u8]) -> core::result::Result<usize, CodecError>,
    {
        let mut bytes = vec![0; len];
        encode(&mut bytes).expect("fixture encodes");
        bytes
    }

    /// One founded market, as [`write_initial_plane`] actually writes it.
    struct Founded {
        realm: Vec<u8>,
        profile: Vec<u8>,
        terms: Vec<u8>,
        intent: CreateMarketIntent,
        bumps: PlaneBumps,
        market: Vec<u8>,
        hoard: Vec<u8>,
        position: Vec<u8>,
        kernel: Vec<u8>,
        replay: Vec<u8>,
        supply: Vec<u8>,
        resolution: Vec<u8>,
    }

    impl Founded {
        fn plane(&self) -> PlaneBytes<'_> {
            PlaneBytes {
                market: &self.market,
                hoard: &self.hoard,
                position: &self.position,
                kernel: &self.kernel,
                replay: &self.replay,
                supply: &self.supply,
                resolution: &self.resolution,
            }
        }

        fn validate(&self) -> Outcome<()> {
            validate_initial_plane(
                &self.realm,
                &self.profile,
                &self.terms,
                self.plane(),
                &self.intent,
                &self.bumps,
            )
        }
    }

    fn owner() -> Hash32 {
        h(31)
    }

    fn hoard_authority() -> Hash32 {
        h(0x40)
    }

    fn founded() -> Founded {
        let profile = profile_hash();
        let terms_value = terms_account(profile);
        let intent = CreateMarketIntent {
            realm: realm_hash(),
            profile,
            market_nonce: MARKET_NONCE,
            outcome_count: OUTCOME_COUNT,
            terms: terms_value.terms,
            feed: terms_value.feed,
        };
        let identities = FoundingIdentities {
            market: market_id(),
            owner: owner(),
            hoard_authority: hoard_authority(),
        };
        let bumps = plane_bumps();

        let terms = encoded(account_len::TERMS, |out| terms_value.encode(out));
        let mut market = vec![0; account_len::MARKET];
        let mut hoard = vec![0; account_len::HOARD];
        let mut position = vec![0; account_len::POSITION];
        let mut kernel = vec![0; KERNEL_ACCOUNT_LEN];
        let mut replay = vec![0; REPLAY_ACCOUNT_LEN];
        let mut supply = vec![0; account_len::SUPPLY_LEDGER];
        let mut resolution = vec![0; account_len::RESOLUTION];
        write_initial_plane(
            &terms,
            PlaneWrite {
                market: &mut market,
                hoard: &mut hoard,
                position: &mut position,
                kernel: &mut kernel,
                replay: &mut replay,
                supply: &mut supply,
                resolution: &mut resolution,
            },
            &intent,
            &identities,
            &bumps,
        )
        .expect("the founding write must succeed");

        Founded {
            realm: encoded(account_len::REALM, |out| realm_account().encode(out)),
            profile: encoded(account_len::PROFILE, |out| profile_account().encode(out)),
            terms,
            intent,
            bumps,
            market,
            hoard,
            position,
            kernel,
            replay,
            supply,
            resolution,
        }
    }

    /* -------------------------------------------------------------------- */
    /* Happy path: byte-exact initialization                                 */
    /* -------------------------------------------------------------------- */

    #[test]
    fn the_founding_write_is_byte_exact_against_independently_encoded_accounts() {
        /* The expectation is the *structs*, encoded by the frozen codecs, not a
         * hex transcript: a change to any field this lane chose shows up here
         * as a struct that no longer matches, with the field named. */
        let founded = founded();
        let market = market_id();
        let realm = realm_hash();
        let profile = profile_hash();
        let terms_value = terms_account(profile);
        let bumps = plane_bumps();

        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        outcomes[0] = canonical_outcome_id(market, 0);
        outcomes[1] = canonical_outcome_id(market, 1);
        let expected_market = MarketAccount {
            market,
            realm,
            profile,
            terms: terms_value.terms,
            outcome_count: OUTCOME_COUNT,
            lifecycle: 0,
            stored_bump: bumps.market,
            hoard_bump: bumps.hoard,
            outcomes,
            feed: terms_value.feed,
            // The terms' digest-committed cap; the slot stays the named zero
            // placeholder.  See the module docs.
            collateral_cap: FIXTURE_CAP,
            created_slot: 0,
            reserved: Hash32::ZERO,
        };
        let expected_hoard = HoardAccount {
            market,
            realm,
            authority: hoard_authority(),
            collateral_atoms: 0,
            stored_bump: bumps.hoard,
            flags: 0,
        };
        let expected_position = PositionAccount {
            market,
            owner: owner(),
            generation: 0,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 0,
            reserved_cash_atoms: 0,
            stored_bump: bumps.position,
            close_state: 0,
        };
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        vectors[0] = PayoutVector::new(1, unit_vector(0));
        vectors[1] = PayoutVector::new(1, unit_vector(1));
        let expected_kernel = KernelAccount {
            market,
            phase: 0,
            basis_mode: BasisMode::FinitePreset,
            resolved_payout: 0,
            payouts: PayoutSet::new(PAYOUT_COUNT, OUTCOME_COUNT, vectors),
            total_supply: [0; MAX_OUTCOMES],
        };
        let expected_replay = ReplayAccount {
            market,
            owner: owner(),
            position_generation: 0,
            sequence: 0,
            stored_bump: bumps.replay,
            flags: 0,
        };
        let expected_supply = SupplyLedgerAccount {
            market,
            realm,
            generation: 0,
            outcome_count: OUTCOME_COUNT,
            internal_supply: [0; MAX_OUTCOMES],
            external_supply: [0; MAX_OUTCOMES],
            stored_bump: bumps.supply,
            flags: 0,
        };
        let expected_resolution = ResolutionAccount {
            market,
            terms: terms_value.terms,
            feed: terms_value.feed,
            window: Hash32::ZERO,
            feed_cursor: 0,
            sealed_end_bucket_exclusive: 0,
            repair_generation: 0,
            resolved_slot: 0,
            payout_index: PAYOUT_INDEX_UNRESOLVED,
            stored_bump: bumps.resolution,
            flags: 0,
        };

        assert_eq!(
            founded.market,
            encoded(account_len::MARKET, |out| expected_market.encode(out))
        );
        assert_eq!(
            founded.hoard,
            encoded(account_len::HOARD, |out| expected_hoard.encode(out))
        );
        assert_eq!(
            founded.position,
            encoded(account_len::POSITION, |out| expected_position.encode(out))
        );
        let mut kernel_bytes = vec![0; KERNEL_ACCOUNT_LEN];
        expected_kernel
            .encode(&mut kernel_bytes)
            .expect("kernel encodes");
        assert_eq!(founded.kernel, kernel_bytes);
        let mut replay_bytes = vec![0; REPLAY_ACCOUNT_LEN];
        expected_replay
            .encode(&mut replay_bytes)
            .expect("replay encodes");
        assert_eq!(founded.replay, replay_bytes);
        assert_eq!(
            founded.supply,
            encoded(account_len::SUPPLY_LEDGER, |out| expected_supply
                .encode(out))
        );
        assert_eq!(
            founded.resolution,
            encoded(account_len::RESOLUTION, |out| expected_resolution
                .encode(out))
        );
        assert_eq!(founded.validate(), Ok(()));
    }

    #[test]
    fn the_founded_resolution_record_is_unresolved_and_the_market_is_active() {
        let founded = founded();
        let resolution = accounts::read_resolution(&founded.resolution).expect("decodes");
        assert!(!resolution.resolved);
        assert_eq!(resolution.payout_index, PAYOUT_INDEX_UNRESOLVED);
        let market = accounts::read_market(&founded.market).expect("decodes");
        assert_eq!(market.lifecycle, 0);
        /* Fundable by construction: the cap is the terms' own, never zero. */
        assert_eq!(market.collateral_cap, FIXTURE_CAP);
        assert_eq!(
            accounts::read_terms(&founded.terms)
                .expect("decodes")
                .collateral_cap,
            FIXTURE_CAP
        );
        let kernel = accounts::read_kernel(&founded.kernel).expect("decodes");
        assert_eq!(kernel.phase, 0);
        assert_eq!(kernel.total_supply, [0; MAX_OUTCOMES]);
    }

    /* -------------------------------------------------------------------- */
    /* Envelope discipline                                                   */
    /* -------------------------------------------------------------------- */

    fn layout_request(sequence: u64, intent: Intent) -> Vec<u8> {
        let mut body = [0_u8; clutch_solana_layout::MAX_INTENT_BYTES];
        let len = intent.encode(&mut body).expect("intent encodes");
        let mut out = vec![0xd1, 1];
        out.extend_from_slice(&sequence.to_le_bytes());
        out.push(0);
        out.extend_from_slice(&(len as u16).to_le_bytes());
        out.extend_from_slice(&body[..len]);
        out
    }

    fn create_request(sequence: u64) -> Request {
        let terms_value = terms_account(profile_hash());
        let bytes = layout_request(
            sequence,
            Intent::CreateMarket {
                realm: realm_hash(),
                profile: profile_hash(),
                market_nonce: MARKET_NONCE,
                outcome_count: OUTCOME_COUNT,
                terms: terms_value.terms,
                feed: terms_value.feed,
            },
        );
        Request::decode(&bytes).expect("the create envelope decodes")
    }

    #[test]
    fn the_intent_match_accepts_only_create_market() {
        let request = create_request(0);
        let intent = create_market_intent(&request).expect("create matches");
        assert_eq!(intent.market_nonce, MARKET_NONCE);
        assert_eq!(intent.outcome_count, OUTCOME_COUNT);

        let split = Request::decode(&layout_request(
            0,
            Intent::Split {
                market: market_id(),
                owner: owner(),
                quantity: 1,
            },
        ))
        .expect("split envelope decodes");
        assert_eq!(
            create_market_intent(&split),
            Err(ClutchError::UnsupportedInstruction.into())
        );
        /* And `process` refuses it before touching an account, which is why it
         * can be called here with no accounts at all. */
        assert_eq!(
            process(&Pubkey::new_from_array([1; 32]), &[], &split),
            Err(ClutchError::UnsupportedInstruction.into())
        );
    }

    #[test]
    fn creation_consumes_no_replay_sequence() {
        assert_eq!(require_creation_sequence(0), Ok(()));
        assert_eq!(
            require_creation_sequence(1),
            Err(ClutchError::Replay.into())
        );
        assert_eq!(
            require_creation_sequence(u64::MAX),
            Err(ClutchError::Replay.into())
        );
    }

    /* -------------------------------------------------------------------- */
    /* The refusal table                                                     */
    /* -------------------------------------------------------------------- */

    /// Rewrite one plane account from a mutated struct.
    macro_rules! rewrite {
        ($founded:expr, $field:ident, $ty:ty, $len:expr, $mutate:expr) => {{
            let mut value = <$ty>::decode(&$founded.$field).expect("plane account decodes");
            #[allow(clippy::redundant_closure_call)]
            ($mutate)(&mut value);
            let mut bytes = vec![0; $len];
            value.encode(&mut bytes).expect("mutated account encodes");
            $founded.$field = bytes;
        }};
    }

    #[test]
    fn non_canonical_outcome_ids_refuse() {
        /* The outcome identities are `canonical_outcome_id(market, i)` and the
         * frozen codec owns that rule, so the tamper has to be at the byte
         * level: `MarketAccount::encode` would refuse to produce it. */
        let mut founded = founded();
        let offset = 2 + (4 * 32) + 4;
        founded.market[offset] ^= 0xff;
        assert_eq!(
            founded.validate(),
            Err(Refusal::Codec(CodecError::NonCanonicalIdentity))
        );
    }

    #[test]
    fn a_non_canonical_market_identity_refuses() {
        /* A market whose identity is not `canonical_market_id(realm, profile,
         * nonce)`, but whose outcome identities are canonical *for that wrong
         * identity*, so the codec is satisfied and only the initializer's own
         * derivation check can catch it. */
        let mut founded = founded();
        let forged = canonical_market_id(realm_hash(), profile_hash(), MARKET_NONCE + 1);
        rewrite!(
            founded,
            market,
            MarketAccount,
            account_len::MARKET,
            |market: &mut MarketAccount| {
                market.market = forged;
                market.outcomes[0] = canonical_outcome_id(forged, 0);
                market.outcomes[1] = canonical_outcome_id(forged, 1);
            }
        );
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn every_stored_bump_is_compared_against_the_derivation() {
        for (name, mutate) in [
            ("market", 0_usize),
            ("hoard", 1),
            ("supply", 2),
            ("resolution", 3),
            ("position", 4),
            ("replay", 5),
        ] {
            let mut founded = founded();
            match mutate {
                0 => rewrite!(
                    founded,
                    market,
                    MarketAccount,
                    account_len::MARKET,
                    |v: &mut MarketAccount| v.stored_bump ^= 1
                ),
                1 => rewrite!(
                    founded,
                    hoard,
                    HoardAccount,
                    account_len::HOARD,
                    |v: &mut HoardAccount| v.stored_bump ^= 1
                ),
                2 => rewrite!(
                    founded,
                    supply,
                    SupplyLedgerAccount,
                    account_len::SUPPLY_LEDGER,
                    |v: &mut SupplyLedgerAccount| v.stored_bump ^= 1
                ),
                3 => rewrite!(
                    founded,
                    resolution,
                    ResolutionAccount,
                    account_len::RESOLUTION,
                    |v: &mut ResolutionAccount| v.stored_bump ^= 1
                ),
                4 => rewrite!(
                    founded,
                    position,
                    PositionAccount,
                    account_len::POSITION,
                    |v: &mut PositionAccount| v.stored_bump ^= 1
                ),
                _ => rewrite!(
                    founded,
                    replay,
                    ReplayAccount,
                    REPLAY_ACCOUNT_LEN,
                    |v: &mut ReplayAccount| v.stored_bump ^= 1
                ),
            }
            assert_eq!(
                founded.validate(),
                Err(ClutchError::WrongBump.into()),
                "{name} bump must be compared"
            );
        }
    }

    #[test]
    fn a_hoard_bound_to_another_realm_refuses() {
        let mut founded = founded();
        rewrite!(
            founded,
            hoard,
            HoardAccount,
            account_len::HOARD,
            |v: &mut HoardAccount| v.realm = h(0x77)
        );
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn a_supply_ledger_bound_to_another_market_refuses() {
        let mut founded = founded();
        rewrite!(
            founded,
            supply,
            SupplyLedgerAccount,
            account_len::SUPPLY_LEDGER,
            |v: &mut SupplyLedgerAccount| v.market = h(0x78)
        );
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn a_founding_owner_plane_that_is_not_mutually_bound_refuses() {
        for index in 0..2_usize {
            let mut founded = founded();
            match index {
                0 => rewrite!(
                    founded,
                    replay,
                    ReplayAccount,
                    REPLAY_ACCOUNT_LEN,
                    |v: &mut ReplayAccount| v.position_generation = 1
                ),
                _ => rewrite!(
                    founded,
                    position,
                    PositionAccount,
                    account_len::POSITION,
                    |v: &mut PositionAccount| v.market = h(0x7a)
                ),
            }
            assert_eq!(
                founded.validate(),
                Err(ClutchError::MismatchedState.into()),
                "owner-plane linkage case {index}"
            );
        }
    }

    #[test]
    fn nonzero_initial_supplies_refuse() {
        let empty = Err(Refusal::Reference(ReferenceError::NonEmptyInitialization));

        let mut ledger = founded();
        rewrite!(
            ledger,
            supply,
            SupplyLedgerAccount,
            account_len::SUPPLY_LEDGER,
            |v: &mut SupplyLedgerAccount| v.internal_supply[0] = 1
        );
        assert_eq!(ledger.validate(), empty);

        let mut external_term = founded();
        rewrite!(
            external_term,
            supply,
            SupplyLedgerAccount,
            account_len::SUPPLY_LEDGER,
            |v: &mut SupplyLedgerAccount| v.external_supply[1] = 1
        );
        assert_eq!(external_term.validate(), empty);

        let mut aggregate = founded();
        rewrite!(
            aggregate,
            kernel,
            KernelAccount,
            KERNEL_ACCOUNT_LEN,
            |v: &mut KernelAccount| v.total_supply[0] = 1
        );
        assert_eq!(aggregate.validate(), empty);

        let mut collateral = founded();
        rewrite!(
            collateral,
            hoard,
            HoardAccount,
            account_len::HOARD,
            |v: &mut HoardAccount| v.collateral_atoms = 1
        );
        assert_eq!(collateral.validate(), empty);
    }

    #[test]
    fn c0_refuses_a_founding_owner_plane_that_is_not_provably_zero() {
        let empty = Err(Refusal::Reference(ReferenceError::NonEmptyInitialization));

        let mut claims = founded();
        rewrite!(
            claims,
            position,
            PositionAccount,
            account_len::POSITION,
            |v: &mut PositionAccount| v.internal[0] = 1
        );
        assert_eq!(claims.validate(), empty);

        let mut cash = founded();
        rewrite!(
            cash,
            position,
            PositionAccount,
            account_len::POSITION,
            |v: &mut PositionAccount| v.cash_atoms = 1
        );
        assert_eq!(cash.validate(), empty);

        let mut reserved = founded();
        rewrite!(
            reserved,
            position,
            PositionAccount,
            account_len::POSITION,
            |v: &mut PositionAccount| {
                v.cash_atoms = 5;
                v.reserved_cash_atoms = 5;
            }
        );
        assert_eq!(reserved.validate(), empty);

        let mut closing = founded();
        rewrite!(
            closing,
            position,
            PositionAccount,
            account_len::POSITION,
            |v: &mut PositionAccount| v.close_state = 1
        );
        assert_eq!(closing.validate(), empty);

        let mut replayed = founded();
        rewrite!(
            replayed,
            replay,
            ReplayAccount,
            REPLAY_ACCOUNT_LEN,
            |v: &mut ReplayAccount| v.sequence = 1
        );
        assert_eq!(replayed.validate(), empty);
    }

    #[test]
    fn an_unfrozen_collateral_policy_refuses_market_initialization() {
        let mut founded = founded();
        let unfrozen = ProfileAccount {
            profile: profile_hash(),
            realm: realm_hash(),
            collateral_policy_id: Hash32::ZERO,
            adapter_release_id: Hash32::ZERO,
            version: 2,
            flags: 0,
        };
        founded.profile = encoded(account_len::PROFILE, |out| unfrozen.encode(out));
        assert_eq!(
            founded.validate(),
            Err(Refusal::Adapter(ClutchError::CollateralPolicyNotFrozen))
        );
    }

    #[test]
    fn the_realm_width_gate_is_unreachable_because_the_frozen_codec_pins_it() {
        /* `validate_initial_plane` carries the reference's two width checks --
         * `realm.max_outcomes == MAX_OUTCOMES` and
         * `intent.outcome_count <= realm.max_outcomes` -- and neither can fire,
         * because the frozen codecs refuse both halves first.  That is worth an
         * assertion rather than a comment: if either codec ever loosens, this
         * test fails and the checks above it stop being dead weight. */
        let narrow = RealmAccount {
            realm: realm_hash(),
            profile: profile_hash(),
            max_outcomes: 8,
            profile_version: 2,
            stored_bump: 200,
            flags: 0,
        };
        let mut realm_bytes = [0; account_len::REALM];
        assert_eq!(
            narrow.encode(&mut realm_bytes),
            Err(CodecError::InvalidCount)
        );
        let mut intent_bytes = [0; clutch_solana_layout::MAX_INTENT_BYTES];
        assert_eq!(
            Intent::CreateMarket {
                realm: realm_hash(),
                profile: profile_hash(),
                market_nonce: MARKET_NONCE,
                outcome_count: (MAX_OUTCOMES as u8) + 1,
                terms: h(1),
                feed: h(2),
            }
            .encode(&mut intent_bytes),
            Err(CodecError::InvalidCount)
        );
    }

    #[test]
    fn a_profile_version_the_realm_does_not_expect_refuses() {
        let mut founded = founded();
        let drifted = RealmAccount {
            realm: realm_hash(),
            profile: profile_hash(),
            max_outcomes: MAX_OUTCOMES as u8,
            profile_version: 2,
            stored_bump: 200,
            flags: 0,
        };
        founded.realm = encoded(account_len::REALM, |out| drifted.encode(out));
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn the_kernel_invariant_checker_runs_over_the_founded_state() {
        /* At initialization the only reachable `check_invariants` failure is the
         * outcome-count disagreement: the frozen kernel codec already refuses a
         * malformed payout set at decode, and required collateral over a zero
         * supply is zero.  Both halves are asserted, so neither can silently
         * stop being checked. */
        let mut disagreeing = founded();
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        vectors[0] = PayoutVector::new(1, unit_vector(0));
        vectors[1] = PayoutVector::new(1, unit_vector(1));
        rewrite!(
            disagreeing,
            kernel,
            KernelAccount,
            KERNEL_ACCOUNT_LEN,
            |v: &mut KernelAccount| v.payouts = PayoutSet::new(PAYOUT_COUNT, 3, vectors)
        );
        assert_eq!(
            disagreeing.validate(),
            Err(ClutchError::MismatchedState.into())
        );

        /* A payout set that does not sum to its denominator never reaches the
         * invariant checker: `KernelAccount::encode` does not validate, so the
         * bytes exist, and `decode` is what refuses them. */
        let mut malformed = founded();
        let mut bad = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut weights = [0; MAX_OUTCOMES];
        weights[0] = 2;
        bad[0] = PayoutVector::new(1, weights);
        bad[1] = PayoutVector::new(1, unit_vector(1));
        let broken = KernelAccount {
            market: market_id(),
            phase: 0,
            basis_mode: BasisMode::FinitePreset,
            resolved_payout: 0,
            payouts: PayoutSet::new(PAYOUT_COUNT, OUTCOME_COUNT, bad),
            total_supply: [0; MAX_OUTCOMES],
        };
        let mut bytes = vec![0; KERNEL_ACCOUNT_LEN];
        broken.encode(&mut bytes).expect("unvalidated encode");
        malformed.kernel = bytes;
        /* The reference-only kernel codec raises the kernel's own class through
         * its own error type; `error.rs` maps both to the same `0x2004`. */
        assert_eq!(
            malformed.validate(),
            Err(Refusal::Reference(ReferenceError::Kernel(
                clutch_kernel::Error::InvalidPayoutWeights
            )))
        );
    }

    #[test]
    fn a_kernel_payout_set_that_is_not_the_terms_payout_set_refuses() {
        /* Valid on its own, and a valid set for this outcome count -- it is
         * simply not the set the market's own terms digest commits to. */
        let mut founded = founded();
        let mut swapped = [PayoutVector::ZERO; MAX_PAYOUTS];
        swapped[0] = PayoutVector::new(1, unit_vector(1));
        swapped[1] = PayoutVector::new(1, unit_vector(0));
        rewrite!(
            founded,
            kernel,
            KernelAccount,
            KERNEL_ACCOUNT_LEN,
            |v: &mut KernelAccount| v.payouts =
                PayoutSet::new(PAYOUT_COUNT, OUTCOME_COUNT, swapped)
        );
        assert_eq!(
            founded.validate(),
            Err(ClutchError::PayoutSetMismatch.into())
        );
    }

    #[test]
    fn degree_zero_through_three_select_exactly_one_immutable_kernel_mode() {
        assert_eq!(basis_mode_for_degree(0), BasisMode::FinitePreset);
        for degree in 1..=3 {
            assert_eq!(basis_mode_for_degree(degree), BasisMode::DerivedBasis);
        }
    }

    #[test]
    fn occupation_statistics_select_only_the_v4_resolution_width() {
        let mut terms = terms_account(profile_hash());
        terms.basis_degree = 1;
        terms.knot_count = 2;
        terms.knots = [0; clutch_solana_layout::MAX_KNOTS];
        terms.knots[1] = 1;
        terms.payout_map = [clutch_solana_layout::PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        for statistic in [
            clutch_solana_layout::occupation_resolution::STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
            clutch_solana_layout::occupation_resolution::STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07,
        ] {
            terms.statistic_id = statistic;
            terms.terms = Hash32::ZERO;
            terms.terms = terms.recomputed_terms_digest().unwrap();
            let mut bytes = vec![0_u8; account_len::TERMS];
            terms.encode(&mut bytes).unwrap();
            #[cfg(feature = "profile-full")]
            assert_eq!(
                resolution_account_len(&bytes),
                Ok(OCCUPATION_RESOLUTION_LEN)
            );
            #[cfg(not(feature = "profile-full"))]
            assert_eq!(
                resolution_account_len(&bytes),
                Err(ClutchError::UnsupportedInstruction.into())
            );
        }
        terms.statistic_id = 1;
        terms.terms = Hash32::ZERO;
        terms.terms = terms.recomputed_terms_digest().unwrap();
        let mut bytes = vec![0_u8; account_len::TERMS];
        terms.encode(&mut bytes).unwrap();
        assert_eq!(resolution_account_len(&bytes), Ok(NATIVE_RESOLUTION_LEN));
    }

    #[test]
    fn a_hostile_kernel_mode_flip_refuses_the_exact_wrong_mode_class() {
        let mut state = founded();
        let before_terms = state.terms.clone();
        rewrite!(
            state,
            kernel,
            KernelAccount,
            KERNEL_ACCOUNT_LEN,
            |value: &mut KernelAccount| value.basis_mode = BasisMode::DerivedBasis
        );
        assert_eq!(
            state.validate(),
            Err(Refusal::Reference(ReferenceError::Kernel(
                clutch_kernel::Error::WrongResolutionMode
            )))
        );
        assert_eq!(
            state.terms, before_terms,
            "immutable terms are not rewritten"
        );
    }

    #[test]
    fn a_terms_artifact_that_does_not_bind_this_market_refuses() {
        let mut founded = founded();
        let other = terms_account(h(0x51));
        founded.terms = encoded(account_len::TERMS, |out| other.encode(out));
        assert_eq!(
            founded.validate(),
            Err(Refusal::Adapter(ClutchError::TermsBindingMismatch))
        );
    }

    #[test]
    fn a_resolution_record_that_is_already_resolved_refuses() {
        let mut founded = founded();
        rewrite!(
            founded,
            resolution,
            ResolutionAccount,
            account_len::RESOLUTION,
            |v: &mut ResolutionAccount| {
                v.payout_index = 0;
                v.window = h(0x30);
                v.feed_cursor = 130;
                v.sealed_end_bucket_exclusive = 130;
                v.repair_generation = 1;
                v.resolved_slot = 900;
            }
        );
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn a_resolution_record_bound_to_other_terms_refuses() {
        let mut founded = founded();
        rewrite!(
            founded,
            resolution,
            ResolutionAccount,
            account_len::RESOLUTION,
            |v: &mut ResolutionAccount| v.terms = h(0x52)
        );
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn a_market_cap_that_is_not_the_terms_cap_refuses() {
        /* The cap flow: the founding write copies the terms'
         * digest-committed cap, and the re-validation refuses any other
         * value — a writer cannot invent a risk limit.  The zero case is not
         * writable at all: the terms codec refuses a zero cap, so the old
         * "exists and cannot accept collateral" residue is unfoundable. */
        let mut mismatched = founded();
        rewrite!(
            mismatched,
            market,
            MarketAccount,
            account_len::MARKET,
            |v: &mut MarketAccount| v.collateral_cap = FIXTURE_CAP + 1
        );
        assert_eq!(
            mismatched.validate(),
            Err(Refusal::Adapter(ClutchError::TermsBindingMismatch))
        );

        let mut zeroed = founded();
        rewrite!(
            zeroed,
            market,
            MarketAccount,
            account_len::MARKET,
            |v: &mut MarketAccount| v.collateral_cap = 0
        );
        assert_eq!(
            zeroed.validate(),
            Err(Refusal::Adapter(ClutchError::TermsBindingMismatch))
        );

        let mut undecided = terms_account(profile_hash());
        undecided.collateral_cap = 0;
        undecided.terms = undecided.recomputed_terms_digest().expect("digest");
        assert_eq!(undecided.validate(), Err(CodecError::ZeroValue));
    }

    #[test]
    fn an_intent_that_does_not_describe_the_written_market_refuses() {
        for index in 0..4_usize {
            let mut founded = founded();
            match index {
                0 => founded.intent.market_nonce += 1,
                1 => founded.intent.outcome_count = 3,
                2 => founded.intent.feed = h(0x53),
                _ => founded.intent.realm = h(0x54),
            }
            assert_eq!(
                founded.validate(),
                Err(ClutchError::MismatchedState.into()),
                "intent case {index}"
            );
        }
    }
}
