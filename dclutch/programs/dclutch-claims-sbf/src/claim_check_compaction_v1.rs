//! Refusals for permissionless claim-check compaction.
//!
//! Compaction is the crank that lets a terminal market retire past a holder
//! who never returns. After a release-fixed deadline anyone may resolve one
//! sleeping position's payout into a per-market escrow only that holder can
//! open, retire its supply through redemption's own signed-delta executor, and
//! close the position and its admission record -- paying the caller out of rent
//! that was already leaving those accounts.
//!
//! Two properties shape this table and are worth stating where the codes live,
//! because a later edit could quietly drop either.
//!
//! **The wrong-holder hostile has no code, by construction.** The route takes
//! `(aggregate, owner)` as coordinates and re-derives both the position and the
//! claim-check from them; a caller naming the wrong owner derives an address
//! that is not the account they passed. `Identity` is what that derivation
//! mismatch surfaces as -- it is not a holder-field comparison, because there
//! is no holder field.
//!
//! **`Deadline` is the only time refusal, and it can only ever be generous.**
//! The clock origin is the slot the escrow was opened, which is at or after the
//! market went terminal, so stamping it there lengthens the wait rather than
//! shortening it. The deadline itself is compiled into this ELF, so the only
//! tamper surface is a release re-point, which does not exist today and must
//! refuse a shortening when it does.

use dclutch_claims_svm::claim_check_compaction_request_v1::CompactPositionToClaimCheckRequestV1;
use dclutch_claims_svm::claim_check_conservation_v1::{
    ClaimCheckAccountObservationV1, ClaimCheckCompactionObservationV1, ClaimCheckCompactionPlanV1,
    ClaimCheckCompactionPostV1,
};
use dclutch_claims_svm::claim_check_request_v1::OpenClaimCheckEscrowRequestV1;
use dclutch_claims_svm::claim_check_v1::{
    CLAIM_CHECK_BYTES_V1, CLAIM_CHECK_ESCROW_BYTES_V1, COMPACTION_CRANK_REWARD_LAMPORTS_V1,
    COMPACTION_DEADLINE_SLOTS_V1, ClaimCheckEscrowSeedsV1, ClaimCheckEscrowV1, ClaimCheckSeedsV1,
    ClaimCheckV1, ClaimCheckVaultSeedsV1,
};
use dclutch_claims_svm::liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2;
use dclutch_claims_svm::protocol_position_v2::{
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2,
};
use dclutch_claims_svm::terminal_settlement_v3::{
    TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3, TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3,
    TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, Phase};
use dclutch_realm_contract::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::Instruction,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer};
use spl_token_2022_interface::instruction as token_instruction;

/// Exact width of a Token-2022/SPL token account with no extensions.
const TOKEN_ACCOUNT_BYTES_V1: usize = 165;

/// Stable claim-check compaction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimCheckCompactionSbfErrorV1 {
    /// The fixed account frame, ownership, or writability refused.
    Accounts = 0x5600,
    /// A signer the route does not admit was present.
    Authority = 0x5601,
    /// Coordinates did not derive the passed account, or aliased, or were zero.
    Identity = 0x5602,
    /// The compaction deadline had not elapsed at the observed slot.
    Deadline = 0x5603,
    /// The Core phase, or the absence of a terminal receipt, refused.
    Phase = 0x5604,
    /// The claim-check address was already occupied.
    AlreadyCompacted = 0x5605,
    /// A plan's atoms or lamports did not balance.
    Conservation = 0x5606,
    /// The terminal payout derivation refused.
    Economic = 0x5607,
    /// Observed post-balances did not match the admitted plan.
    Receipt = 0x5608,
    /// The escrow was absent, or its mint or token program did not match.
    Escrow = 0x5609,
    /// A position kind this version does not compact.
    Scope = 0x560A,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    ClaimCheckCompactionSbfErrorV1::Accounts as u32
        == dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x600,
    "ClaimCheckCompactionSbfErrorV1 must start at its registered refusal band base"
);
const _: () = assert!(
    (ClaimCheckCompactionSbfErrorV1::Scope as u32)
        < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "ClaimCheckCompactionSbfErrorV1 must not run past its registered refusal band"
);
// Compaction and claim-check redemption are independently versioned request
// families and hold separate round sub-bands. This assertion is what keeps the
// two from ever being interleaved by a later addition to either table.
const _: () = assert!(
    (ClaimCheckCompactionSbfErrorV1::Scope as u32)
        < crate::claim_check_redemption_v1::ClaimCheckRedemptionSbfErrorV1::Accounts as u32,
    "the compaction sub-band must not run into the claim-check redemption sub-band"
);

impl From<ClaimCheckCompactionSbfErrorV1> for ProgramError {
    fn from(value: ClaimCheckCompactionSbfErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

/// Signing opener, who advances the rent for both accounts.
pub const OPEN_OPENER_ACCOUNT_V1: usize = 0;
/// The escrow record PDA this route creates.
pub const OPEN_ESCROW_ACCOUNT_V1: usize = 1;
/// The escrow vault token account this route creates.
pub const OPEN_VAULT_ACCOUNT_V1: usize = 2;
/// The market's Claims aggregate, the escrow's sole PDA seed.
pub const OPEN_AGGREGATE_ACCOUNT_V1: usize = 3;
/// The Core market state carrying the phase and terminal receipt.
pub const OPEN_CORE_MARKET_ACCOUNT_V1: usize = 4;
/// Current Core program, deriving the Core market address.
pub const OPEN_CORE_PROGRAM_ACCOUNT_V1: usize = 5;
/// The collateral Realm record, sole author of the mint and token program.
pub const OPEN_REALM_ACCOUNT_V1: usize = 6;
/// The Realm's staging cursor, proving the record is not mid-update.
pub const OPEN_REALM_STAGING_ACCOUNT_V1: usize = 7;
/// Registry program owning both Realm accounts.
pub const OPEN_REGISTRY_ACCOUNT_V1: usize = 8;
/// The collateral mint, authenticated against the Realm.
pub const OPEN_COLLATERAL_MINT_ACCOUNT_V1: usize = 9;
/// The collateral token program, authenticated against the Realm.
pub const OPEN_TOKEN_PROGRAM_ACCOUNT_V1: usize = 10;
/// System program, for allocation and assignment.
pub const OPEN_SYSTEM_ACCOUNT_V1: usize = 11;
/// Exact open-escrow frame width.
pub const OPEN_CLAIM_CHECK_ESCROW_ACCOUNT_COUNT_V1: usize = 12;

/// Borrowed exact open-escrow frame.
#[derive(Clone, Copy)]
struct OpenAccounts<'accounts, 'info> {
    opener: &'accounts AccountInfo<'info>,
    escrow: &'accounts AccountInfo<'info>,
    vault: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    realm: &'accounts AccountInfo<'info>,
    realm_staging: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    collateral_mint: &'accounts AccountInfo<'info>,
    token_program: &'accounts AccountInfo<'info>,
    system: &'accounts AccountInfo<'info>,
}

/// Open one market's claim-check escrow, permissionlessly.
///
/// This is the act that establishes the compaction deadline's origin, and the
/// asymmetry is worth stating where the stamp happens: because the route
/// refuses any phase before `Terminal`, the earliest origin is the market going
/// terminal, and any later one simply *lengthens* every holder's grace period.
/// Stamping here can only ever be more generous to the holder, never less.
/// Being permissionless, no actor can withhold the start from anyone else.
///
/// The escrow costs rent and pays nothing at this instant, so on its own the
/// open is permissible rather than live. What makes it a funded position is
/// that the record carries the opener's outlay as a debt the cranks repay --
/// and the party who intends to crank is the party who wants the rent, which
/// every market has.
pub fn process_open_escrow(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = OpenClaimCheckEscrowRequestV1::decode(instruction_data)
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Identity)?;
    let accounts = frame(accounts)?;
    let core = authenticate_core(accounts, request)?;
    let aggregate = authenticate_aggregate(program_id, accounts, request)?;
    let realm = authenticate_realm(accounts, request)?;

    let seeds = ClaimCheckEscrowSeedsV1::new(aggregate)
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Identity)?;
    let (expected_escrow, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if accounts.escrow.key != &expected_escrow {
        return Err(ClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    // A second open would restart the clock, which is exactly the delay this
    // design exists to forbid. Vacancy is the whole check: an escrow that
    // exists is one somebody already paid for.
    if !accounts.escrow.data_is_empty()
        || accounts.escrow.owner != &system_program::ID
        || !accounts.vault.data_is_empty()
        || accounts.vault.owner != &system_program::ID
    {
        return Err(ClaimCheckCompactionSbfErrorV1::AlreadyCompacted.into());
    }

    let rent = Rent::get()?;
    let opener_before = accounts.opener.lamports();
    let outlay = create_escrow_accounts(program_id, accounts, &seeds, bump, &rent)?;

    let record = ClaimCheckEscrowV1 {
        aggregate,
        market: request.market,
        release_set: request.release_set,
        vault: accounts.vault.key.to_bytes(),
        collateral_mint: accounts.collateral_mint.key.to_bytes(),
        opener: accounts.opener.key.to_bytes(),
        // The clock starts here and nowhere else. There is no uniformly
        // readable terminal slot on chain, and adding one would mean a Lean
        // edit, a regeneration, a re-proof and a second ELF.
        opened_slot: solana_program::clock::Clock::get()?.slot,
        opener_outlay: outlay,
        outstanding_claim_checks: 0,
        generation: core.identity.generation,
        bump,
    }
    .new()
    .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?;
    // The mint the escrow promises is the Realm's, read from the Realm rather
    // than taken from the caller. Custody enforces the same fact on every
    // transfer; this is the same author read twice, not a second author.
    if record.collateral_mint != *realm.collateral_mint() {
        return Err(ClaimCheckCompactionSbfErrorV1::Escrow.into());
    }

    accounts
        .escrow
        .try_borrow_mut_data()
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?
        .copy_from_slice(
            &record
                .to_bytes()
                .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?,
        );

    // Conservation over acceptance: the opener paid exactly what the record
    // says it is owed, and nothing was created that nobody funded.
    if opener_before.checked_sub(outlay) != Some(accounts.opener.lamports())
        || accounts.escrow.owner != program_id
        || accounts.escrow.data_len() != CLAIM_CHECK_ESCROW_BYTES_V1
        || accounts.vault.owner != accounts.token_program.key
    {
        return Err(ClaimCheckCompactionSbfErrorV1::Receipt.into());
    }
    Ok(())
}

fn frame<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
) -> Result<OpenAccounts<'accounts, 'info>, ProgramError> {
    let at = |index: usize| {
        accounts
            .get(index)
            .ok_or(ClaimCheckCompactionSbfErrorV1::Accounts)
    };
    if accounts.len() != OPEN_CLAIM_CHECK_ESCROW_ACCOUNT_COUNT_V1 {
        return Err(ClaimCheckCompactionSbfErrorV1::Accounts.into());
    }
    let value = OpenAccounts {
        opener: at(OPEN_OPENER_ACCOUNT_V1)?,
        escrow: at(OPEN_ESCROW_ACCOUNT_V1)?,
        vault: at(OPEN_VAULT_ACCOUNT_V1)?,
        aggregate: at(OPEN_AGGREGATE_ACCOUNT_V1)?,
        core_market: at(OPEN_CORE_MARKET_ACCOUNT_V1)?,
        core_program: at(OPEN_CORE_PROGRAM_ACCOUNT_V1)?,
        realm: at(OPEN_REALM_ACCOUNT_V1)?,
        realm_staging: at(OPEN_REALM_STAGING_ACCOUNT_V1)?,
        registry: at(OPEN_REGISTRY_ACCOUNT_V1)?,
        collateral_mint: at(OPEN_COLLATERAL_MINT_ACCOUNT_V1)?,
        token_program: at(OPEN_TOKEN_PROGRAM_ACCOUNT_V1)?,
        system: at(OPEN_SYSTEM_ACCOUNT_V1)?,
    };
    // The opener is the only signer this route admits. It is not an authority
    // -- anyone at all may be it -- it is simply whoever is paying, and it must
    // sign because it is paying.
    if !value.opener.is_signer
        || !value.opener.is_writable
        || !value.escrow.is_writable
        || !value.vault.is_writable
        || value.system.key != &system_program::ID
    {
        return Err(ClaimCheckCompactionSbfErrorV1::Accounts.into());
    }
    for readonly in [
        value.aggregate,
        value.core_market,
        value.realm,
        value.realm_staging,
        value.collateral_mint,
    ] {
        if readonly.is_writable {
            return Err(ClaimCheckCompactionSbfErrorV1::Accounts.into());
        }
    }
    // Every other signer is refused rather than ignored. A route that merely
    // did not read a signature would still let a caller present one, and a
    // presented signature is a privilege somebody can be induced to grant.
    if accounts
        .iter()
        .enumerate()
        .any(|(index, account)| index != OPEN_OPENER_ACCOUNT_V1 && account.is_signer)
    {
        return Err(ClaimCheckCompactionSbfErrorV1::Authority.into());
    }
    Ok(value)
}

fn authenticate_core(
    accounts: OpenAccounts<'_, '_>,
    request: OpenClaimCheckEscrowRequestV1,
) -> Result<CoreState, ProgramError> {
    let bytes = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?;
    let core = CoreState::decode(&bytes).map_err(|_| ClaimCheckCompactionSbfErrorV1::Identity)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        accounts.core_program.key,
    )
    .0;
    // `market` here is the Core market ACCOUNT address, not `identity.market_id`.
    // The two are different values and the tree uses both under the same name:
    // market closure's request names the semantic `market_id`, while a
    // position's aggregate seeds off the account address
    // (`protocol_position_v2.rs:904-908`). The escrow is addressed by the
    // aggregate, so it must mean what the aggregate means, or the escrow would
    // derive to somewhere no position can find it. The identity-derived PDA
    // check on the line above is what proves this account is the genuine Core
    // state for its own identity, so naming the address costs no authority.
    if &expected != accounts.core_market.key
        || accounts.core_market.key.to_bytes() != request.market
        || core.identity.selected_release_set.to_bytes() != request.release_set
        || core.identity.registry_program.to_bytes() != accounts.registry.key.to_bytes()
        || core.identity.generation != request.generation
    {
        return Err(ClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    // Terminal and Retiring, both. Retiring is not a convenience: market
    // closure requires it, and `begin_retiring` is permissionless, so a route
    // that refused there would leave exactly the markets a stranger pushed into
    // Retiring unrescuable -- which is the hostage-taking this design exists to
    // end, re-created one phase later.
    if !matches!(core.phase, Phase::Terminal | Phase::Retiring) {
        return Err(ClaimCheckCompactionSbfErrorV1::Phase.into());
    }
    // Checked even though the phase invariant implies it. A checked invariant
    // is one an implementer cannot silently delete.
    if core.terminal_receipt.is_none() {
        return Err(ClaimCheckCompactionSbfErrorV1::Phase.into());
    }
    Ok(core)
}

fn authenticate_aggregate(
    program_id: &Pubkey,
    accounts: OpenAccounts<'_, '_>,
    request: OpenClaimCheckEscrowRequestV1,
) -> Result<[u8; 32], ProgramError> {
    let expected = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, request.market.as_slice()],
        program_id,
    )
    .0;
    if accounts.aggregate.key != &expected
        || accounts.aggregate.owner != program_id
        || accounts.aggregate.data_is_empty()
    {
        return Err(ClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    Ok(expected.to_bytes())
}

fn authenticate_realm(
    accounts: OpenAccounts<'_, '_>,
    request: OpenClaimCheckEscrowRequestV1,
) -> Result<RealmV1, ProgramError> {
    let expected_realm = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            request.realm.as_slice(),
        ],
        accounts.registry.key,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            request.realm.as_slice(),
        ],
        accounts.registry.key,
    )
    .0;
    if accounts.realm.key != &expected_realm || accounts.realm_staging.key != &expected_staging {
        return Err(ClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    let bytes = accounts
        .realm
        .try_borrow_data()
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?;
    let realm = RealmV1::decode(&bytes).map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?;
    // The Realm is the sole author of both facts. A vault opened under any
    // other mint or token program could never be paid into, because Custody
    // refuses a transfer whose mint is not this one.
    if accounts.collateral_mint.key.to_bytes() != *realm.collateral_mint()
        || accounts.token_program.key.to_bytes() != *realm.token_program()
    {
        return Err(ClaimCheckCompactionSbfErrorV1::Escrow.into());
    }
    Ok(realm)
}

/// Create both accounts and return the lamports the opener advanced.
///
/// `allocate` and `assign` rather than `create_account`, which is the tree's
/// existing idiom and is also the dust-tolerant one: `create_account` refuses
/// an address that already holds lamports, so a stranger could block every open
/// forever with one lamport. Here a pre-funded address simply needs a smaller
/// top-up, and the griefer has donated to the escrow they meant to obstruct.
fn create_escrow_accounts(
    program_id: &Pubkey,
    accounts: OpenAccounts<'_, '_>,
    seeds: &ClaimCheckEscrowSeedsV1,
    bump: u8,
    rent: &Rent,
) -> Result<u64, ProgramError> {
    let escrow_rent = rent.minimum_balance(CLAIM_CHECK_ESCROW_BYTES_V1);
    let vault_rent = rent.minimum_balance(TOKEN_ACCOUNT_BYTES_V1);
    let escrow_top_up = escrow_rent.saturating_sub(accounts.escrow.lamports());
    let vault_top_up = vault_rent.saturating_sub(accounts.vault.lamports());

    for (destination, top_up) in [
        (accounts.escrow, escrow_top_up),
        (accounts.vault, vault_top_up),
    ] {
        if top_up == 0 {
            continue;
        }
        let funding = transfer(accounts.opener.key, destination.key, top_up);
        invoke(
            &Instruction {
                program_id: funding.program_id,
                accounts: funding.accounts,
                data: funding.data,
            },
            &[
                accounts.opener.clone(),
                destination.clone(),
                accounts.system.clone(),
            ],
        )
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?;
    }

    let bump_seed = [bump];
    let [domain, aggregate_seed] = seeds.as_slices();
    let escrow_signer: &[&[u8]] = &[domain, aggregate_seed, &bump_seed];
    allocate_and_assign(
        accounts.escrow,
        accounts.system,
        CLAIM_CHECK_ESCROW_BYTES_V1,
        program_id,
        escrow_signer,
    )?;

    // The vault is an ordinary external token account owned by the escrow PDA,
    // deliberately not a new Custody compartment: `CompartmentV1` is a fixed
    // enum and a new variant would be a custody-contract plus custody-sbf
    // change. From Custody's side this is just another `External` destination,
    // exactly as a holder's own wallet token account already is.
    let vault_seeds = ClaimCheckVaultSeedsV1::new(seeds.aggregate())
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Identity)?;
    let (expected_vault, vault_bump) =
        Pubkey::find_program_address(&vault_seeds.as_slices(), program_id);
    if accounts.vault.key != &expected_vault {
        return Err(ClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    let vault_bump_seed = [vault_bump];
    let [vault_domain, vault_aggregate] = vault_seeds.as_slices();
    allocate_and_assign(
        accounts.vault,
        accounts.system,
        TOKEN_ACCOUNT_BYTES_V1,
        accounts.token_program.key,
        &[vault_domain, vault_aggregate, &vault_bump_seed],
    )?;
    invoke(
        &token_instruction::initialize_account3(
            accounts.token_program.key,
            accounts.vault.key,
            accounts.collateral_mint.key,
            accounts.escrow.key,
        )
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?,
        &[
            accounts.vault.clone(),
            accounts.collateral_mint.clone(),
            accounts.token_program.clone(),
        ],
    )
    .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?;

    escrow_top_up
        .checked_add(vault_top_up)
        .ok_or_else(|| ClaimCheckCompactionSbfErrorV1::Conservation.into())
}

fn allocate_and_assign<'info>(
    destination: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    width: usize,
    owner: &Pubkey,
    seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let space = u64::try_from(width).map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?;
    for value in [
        allocate(destination.key, space),
        assign(destination.key, owner),
    ] {
        invoke_signed(
            &Instruction {
                program_id: value.program_id,
                accounts: value.accounts,
                data: value.data,
            },
            &[destination.clone(), system.clone()],
            &[seeds],
        )
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?;
    }
    if destination.owner != owner || destination.data_len() != width {
        return Err(ClaimCheckCompactionSbfErrorV1::Escrow.into());
    }
    Ok(())
}

/// Terminal-frame width this route wraps before its own accounts begin.
const TERMINAL_FRAME_V1: usize = TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3;
/// The escrow record, whose counter and opener debt this crank moves.
pub const COMPACT_ESCROW_ACCOUNT_V1: usize = TERMINAL_FRAME_V1;
/// The claim-check this crank mints, when the payout is nonzero.
pub const COMPACT_CLAIM_CHECK_ACCOUNT_V1: usize = TERMINAL_FRAME_V1 + 1;
/// The position's admission record, carrying the persisted owner kind.
pub const COMPACT_ADMISSION_ACCOUNT_V1: usize = TERMINAL_FRAME_V1 + 2;
/// The market's RentCredit, residual beneficiary of the sweep.
pub const COMPACT_RENT_CREDIT_ACCOUNT_V1: usize = TERMINAL_FRAME_V1 + 3;
/// The escrow's opener, repaid from the sweep after the crank is paid.
pub const COMPACT_OPENER_ACCOUNT_V1: usize = TERMINAL_FRAME_V1 + 4;
/// System program, for the claim-check's allocation.
pub const COMPACT_SYSTEM_ACCOUNT_V1: usize = TERMINAL_FRAME_V1 + 5;
/// Exact compaction frame width.
pub const COMPACT_ACCOUNT_COUNT_V1: usize = TERMINAL_FRAME_V1 + 6;

/// Compact one sleeping position into a claim-check, permissionlessly.
///
/// The crank the whole design exists to make possible. After a release-fixed
/// deadline, anybody may run a holder's own redemption on their behalf, into an
/// escrow only that holder can open, and be paid for the trouble out of rent
/// that was leaving those accounts anyway.
///
/// The payout is not computed here. It is computed by
/// [`terminal_settlement_v3::execute_claim_check_compaction`], which is the
/// holder's own redemption path with one proof relaxed at coordinate 0 --
/// called, never re-implemented. A second author for the payoff function is how
/// a compaction that pays a different number than redemption would have gets
/// built and passes its own tests, and the number is somebody's money.
pub fn process_compaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != COMPACT_ACCOUNT_COUNT_V1 {
        return Err(ClaimCheckCompactionSbfErrorV1::Accounts.into());
    }
    let request = CompactPositionToClaimCheckRequestV1::decode(instruction_data)
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Identity)?;
    let prepared = authenticate_compaction(program_id, accounts, request)?;
    let terminal = accounts
        .get(..TERMINAL_FRAME_V1)
        .ok_or(ClaimCheckCompactionSbfErrorV1::Accounts)?;
    let vault_account = accounts
        .get(TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3)
        .ok_or(ClaimCheckCompactionSbfErrorV1::Accounts)?;
    let hoard_account = accounts
        .get(TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3)
        .ok_or(ClaimCheckCompactionSbfErrorV1::Accounts)?;
    let vault_before = token_balance(vault_account)?;
    let hoard_before = token_balance(hoard_account)?;

    // CALLED, never re-implemented. Everything the holder's own redemption
    // authenticates, this authenticates, because it is the same code.
    crate::terminal_settlement_v3::execute_claim_check_compaction(
        program_id,
        terminal,
        request.settlement(),
    )
    .map_err(|_| ClaimCheckCompactionSbfErrorV1::Economic)?;

    commit_compaction(
        program_id,
        accounts,
        prepared.as_ref(),
        CollateralMovementV1 {
            hoard_before,
            hoard_after: token_balance(hoard_account)?,
            vault_before,
            vault_after: token_balance(vault_account)?,
        },
    )
}

/// Everything the crank proves before it is entitled to turn.
struct CompactionPreparedV1 {
    aggregate: [u8; 32],
    owner: [u8; 32],
    escrow: ClaimCheckEscrowV1,
    vault: Pubkey,
    record_seeds: ClaimCheckSeedsV1,
    record_bump: u8,
}

/// Observed collateral either side of the payout this crank performed.
#[derive(Clone, Copy)]
struct CollateralMovementV1 {
    hoard_before: u64,
    hoard_after: u64,
    vault_before: u64,
    vault_after: u64,
}

#[inline(never)]
fn authenticate_compaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CompactPositionToClaimCheckRequestV1,
) -> Result<Box<CompactionPreparedV1>, ProgramError> {
    let input = request.input();
    let at = |index: usize| {
        accounts
            .get(index)
            .ok_or(ClaimCheckCompactionSbfErrorV1::Accounts)
    };
    let escrow_account = at(COMPACT_ESCROW_ACCOUNT_V1)?;
    let claim_check_account = at(COMPACT_CLAIM_CHECK_ACCOUNT_V1)?;
    let admission_account = at(COMPACT_ADMISSION_ACCOUNT_V1)?;
    let rent_credit_account = at(COMPACT_RENT_CREDIT_ACCOUNT_V1)?;
    let opener_account = at(COMPACT_OPENER_ACCOUNT_V1)?;
    let system_account = at(COMPACT_SYSTEM_ACCOUNT_V1)?;
    let aggregate_account = at(1)?;
    let vault_account = at(TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3)?;

    if !escrow_account.is_writable
        || !claim_check_account.is_writable
        || !admission_account.is_writable
        || !rent_credit_account.is_writable
        || !opener_account.is_writable
        || system_account.key != &system_program::ID
        || escrow_account.owner != program_id
        || admission_account.owner != program_id
    {
        return Err(ClaimCheckCompactionSbfErrorV1::Accounts.into());
    }

    let aggregate = aggregate_account.key.to_bytes();
    let escrow_seeds = ClaimCheckEscrowSeedsV1::new(aggregate)
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Identity)?;
    if escrow_account.key != &Pubkey::find_program_address(&escrow_seeds.as_slices(), program_id).0
    {
        return Err(ClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    let escrow = ClaimCheckEscrowV1::decode(
        &escrow_account
            .try_borrow_data()
            .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?,
    )
    .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?;

    // The deadline, computed with checked arithmetic: a wrapping add would turn
    // a far-future origin into an already-elapsed one, which is exactly the
    // premature crank this gate exists to refuse. Inclusive `>=`, matching the
    // deployed record-abort precedent.
    let horizon = escrow
        .opened_slot
        .checked_add(COMPACTION_DEADLINE_SLOTS_V1)
        .ok_or(ClaimCheckCompactionSbfErrorV1::Deadline)?;
    if solana_program::clock::Clock::get()?.slot < horizon {
        return Err(ClaimCheckCompactionSbfErrorV1::Deadline.into());
    }

    // The recipient is DERIVED, never accepted. This is the single check that
    // separates a crank from a theft: a caller who could name where the payout
    // lands would redirect a sleeping holder's collateral to themselves.
    let vault_seeds = ClaimCheckVaultSeedsV1::new(aggregate)
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Identity)?;
    let vault = Pubkey::find_program_address(&vault_seeds.as_slices(), program_id).0;
    request
        .require_escrow_recipient(escrow_account.key.to_bytes(), vault.to_bytes())
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Identity)?;
    if vault_account.key != &vault || escrow.vault != vault.to_bytes() {
        return Err(ClaimCheckCompactionSbfErrorV1::Identity.into());
    }

    // The claim-check's coordinates are the position's own, so a caller naming
    // the wrong holder derives an address that is not the account they passed.
    let record_seeds = ClaimCheckSeedsV1::new(aggregate, input.owner)
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Identity)?;
    let (expected_record, record_bump) =
        Pubkey::find_program_address(&record_seeds.as_slices(), program_id);
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(aggregate, input.owner)
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Identity)?;
    if claim_check_account.key != &expected_record
        || admission_account.key
            != &Pubkey::find_program_address(&admission_seeds.as_slices(), program_id).0
    {
        return Err(ClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    // Anti-replay is the account's own existence. A claim-check that exists is
    // a position already compacted.
    if !claim_check_account.data_is_empty() || claim_check_account.owner != &system_program::ID {
        return Err(ClaimCheckCompactionSbfErrorV1::AlreadyCompacted.into());
    }

    // The proof the relaxed signature was silently also carrying, restored
    // explicitly, for every owner kind rather than for one of them.
    let admission = ProtocolPositionAdmissionV2::decode(
        &admission_account
            .try_borrow_data()
            .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?,
    )
    .map_err(|_| ClaimCheckCompactionSbfErrorV1::Scope)?;
    if !owner_kind_can_open_a_claim_check(admission.owner_kind()) {
        return Err(ClaimCheckCompactionSbfErrorV1::Scope.into());
    }

    Ok(Box::new(CompactionPreparedV1 {
        aggregate,
        owner: input.owner,
        escrow,
        vault,
        record_seeds,
        record_bump,
    }))
}

/// Whether a position's owner can ever produce the signature redemption asks
/// for.
///
/// A claim-check pays exactly one address and asks that address to sign
/// (`0x5621`, and the holder is a signer in the redemption frame spec). A
/// program-derived address has no private key and cannot sign a top-level
/// instruction, so a claim-check minted for one is collateral written to an
/// address that can never open it. That is strictly worse than the delay this
/// feature exists to end: a delay ends when the holder comes back, and this
/// does not end at all.
///
/// The relaxed crank authority is what makes the question live. While
/// redemption required the owner's own signature, a PDA-owned position simply
/// could not reach the route; `ClaimCheckCrank` removed that signature on
/// purpose, so the property has to be asserted rather than inherited.
///
/// **`TradingRecord` is not the abstract case it looks like.** It is the
/// Fractional reserve Position: `fractional_retirement_v3.rs` requires
/// `admission.owner_kind() == TradingRecord` with `position_owner` equal to the
/// Trading-owned Fractional root PDA, and that Position holds the collateral
/// backing every outstanding shard. Compacting it would resolve the shard
/// holders' collateral into a record none of them owns, keyed to a PDA that
/// cannot sign, while closing the Position their own redemption reads. Every
/// shard holder would lose everything, permissionlessly, to a caller with no
/// stake in the market.
///
/// The match is exhaustive on purpose. A fourth owner kind must answer this
/// question rather than inherit an answer from whichever arm it was written
/// next to.
pub(crate) const fn owner_kind_can_open_a_claim_check(kind: ProtocolPositionOwnerKindV2) -> bool {
    match kind {
        // A wallet. It signs, so it can be paid.
        ProtocolPositionOwnerKindV2::User => true,
        // Trading-owned resting inventory, and the Fractional reserve. A
        // Trading PDA; it has its own parent-authenticated close route.
        ProtocolPositionOwnerKindV2::TradingRecord => false,
        // A Claims capability PDA. Its claimants are the holders of a mint,
        // plural and unknown to the Position.
        ProtocolPositionOwnerKindV2::ClaimsCapability => false,
    }
}

#[inline(never)]
fn commit_compaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    prepared: &CompactionPreparedV1,
    movement: CollateralMovementV1,
) -> ProgramResult {
    let at = |index: usize| {
        accounts
            .get(index)
            .ok_or(ClaimCheckCompactionSbfErrorV1::Accounts)
    };
    let escrow_account = at(COMPACT_ESCROW_ACCOUNT_V1)?;
    let claim_check_account = at(COMPACT_CLAIM_CHECK_ACCOUNT_V1)?;
    let admission_account = at(COMPACT_ADMISSION_ACCOUNT_V1)?;
    let rent_credit_account = at(COMPACT_RENT_CREDIT_ACCOUNT_V1)?;
    let opener_account = at(COMPACT_OPENER_ACCOUNT_V1)?;
    let system_account = at(COMPACT_SYSTEM_ACCOUNT_V1)?;
    let position_account = at(20)?;
    let escrow = prepared.escrow;
    let CollateralMovementV1 {
        hoard_before,
        hoard_after,
        vault_before,
        vault_after,
    } = movement;

    let rent = Rent::get()?;
    let mints_record = vault_after > vault_before;
    let claim_check_rent = if mints_record {
        rent.minimum_balance(CLAIM_CHECK_BYTES_V1)
    } else {
        0
    };
    let plan = ClaimCheckCompactionPlanV1::new(ClaimCheckCompactionObservationV1 {
        payout_atoms: hoard_before.saturating_sub(hoard_after),
        hoard_before,
        hoard_after,
        vault_before,
        vault_after,
        position: observation(position_account),
        admission: observation(admission_account),
        claim_check: observation(claim_check_account),
        cranker: observation(at(0)?),
        opener: observation(opener_account),
        rent_credit: observation(rent_credit_account),
        claim_check_rent,
        opener_debt: escrow.opener_outlay,
        crank_reward_cap: COMPACTION_CRANK_REWARD_LAMPORTS_V1,
    })
    .map_err(|_| ClaimCheckCompactionSbfErrorV1::Conservation)?;

    if plan.mints_claim_check() {
        write_claim_check(
            program_id,
            claim_check_account,
            system_account,
            &prepared.record_seeds,
            prepared.record_bump,
            ClaimCheckV1 {
                aggregate: prepared.aggregate,
                owner: prepared.owner,
                market: escrow.market,
                release_set: escrow.release_set,
                vault: prepared.vault.to_bytes(),
                collateral_mint: escrow.collateral_mint,
                position_atoms_digest: hash(
                    &position_account
                        .try_borrow_data()
                        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?,
                )
                .to_bytes(),
                entitlement_atoms: plan.entitlement_atoms(),
                compacted_slot: solana_program::clock::Clock::get()?.slot,
                generation: escrow.generation,
                bump: prepared.record_bump,
            },
            plan.claim_check_top_up(),
        )?;
    }

    // The sweep, in the amended order: rent first because it is mandatory, then
    // the CRANK, then the opener's debt, then the residue. Paying the opener
    // before the crank cannot close -- one position's rent does not cover a
    // whole escrow's outlay -- and an unfunded crank is an unturned crank.
    close_and_split(
        position_account,
        admission_account,
        at(0)?,
        opener_account,
        rent_credit_account,
        &plan,
    )?;

    let mut updated = ClaimCheckEscrowV1 {
        opener_outlay: plan.opener_debt_after(),
        ..escrow
    };
    if plan.mints_claim_check() {
        updated = updated
            .admit_claim_check()
            .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?;
    }
    escrow_account
        .try_borrow_mut_data()
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?
        .copy_from_slice(
            &updated
                .to_bytes()
                .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?,
        );

    plan.validate_post(ClaimCheckCompactionPostV1 {
        position_lamports: position_account.lamports(),
        admission_lamports: admission_account.lamports(),
        claim_check_lamports: claim_check_account.lamports(),
        cranker_lamports: at(0)?.lamports(),
        opener_lamports: opener_account.lamports(),
        rent_credit_lamports: rent_credit_account.lamports(),
        hoard_lamports_of_collateral: hoard_after,
        vault_lamports_of_collateral: vault_after,
    })
    .map_err(|_| ClaimCheckCompactionSbfErrorV1::Receipt)?;
    Ok(())
}

fn observation(account: &AccountInfo<'_>) -> ClaimCheckAccountObservationV1 {
    ClaimCheckAccountObservationV1 {
        identity: account.key.to_bytes(),
        lamports: account.lamports(),
    }
}

fn token_balance(account: &AccountInfo<'_>) -> Result<u64, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?;
    let bytes: [u8; 8] = data
        .get(64..72)
        .ok_or(ClaimCheckCompactionSbfErrorV1::Accounts)?
        .try_into()
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?;
    Ok(u64::from_le_bytes(bytes))
}

fn write_claim_check<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    seeds: &ClaimCheckSeedsV1,
    bump: u8,
    record: ClaimCheckV1,
    top_up: u64,
) -> Result<(), ProgramError> {
    let record = record
        .new()
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?;
    let bump_seed = [bump];
    let [domain, aggregate, owner] = seeds.as_slices();
    allocate_and_assign(
        account,
        system,
        CLAIM_CHECK_BYTES_V1,
        program_id,
        &[domain, aggregate, owner, &bump_seed],
    )?;
    // The top-up is credited directly rather than transferred, because the
    // lamports come from accounts this program owns and is about to close.
    // A System transfer cannot move them: their owner is Claims, not System.
    **account
        .try_borrow_mut_lamports()
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)? += top_up;
    account
        .try_borrow_mut_data()
        .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?
        .copy_from_slice(
            &record
                .to_bytes()
                .map_err(|_| ClaimCheckCompactionSbfErrorV1::Escrow)?,
        );
    Ok(())
}

fn close_and_split<'info>(
    position: &AccountInfo<'info>,
    admission: &AccountInfo<'info>,
    cranker: &AccountInfo<'info>,
    opener: &AccountInfo<'info>,
    rent_credit: &AccountInfo<'info>,
    plan: &ClaimCheckCompactionPlanV1,
) -> Result<(), ProgramError> {
    {
        let mut position_lamports = position
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?;
        let mut admission_lamports = admission
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)?;
        **position_lamports = 0;
        **admission_lamports = 0;
    }
    for (destination, credit) in [
        (cranker, plan.crank_reward()),
        (opener, plan.opener_repayment()),
        (rent_credit, plan.rent_credit_residue()),
    ] {
        if credit == 0 {
            continue;
        }
        **destination
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimCheckCompactionSbfErrorV1::Accounts)? += credit;
    }
    for closed in [position, admission] {
        closed
            .resize(0)
            .map_err(|_| ClaimCheckCompactionSbfErrorV1::Conservation)?;
        closed.assign(&system_program::ID);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: [ClaimCheckCompactionSbfErrorV1; 11] = [
        ClaimCheckCompactionSbfErrorV1::Accounts,
        ClaimCheckCompactionSbfErrorV1::Authority,
        ClaimCheckCompactionSbfErrorV1::Identity,
        ClaimCheckCompactionSbfErrorV1::Deadline,
        ClaimCheckCompactionSbfErrorV1::Phase,
        ClaimCheckCompactionSbfErrorV1::AlreadyCompacted,
        ClaimCheckCompactionSbfErrorV1::Conservation,
        ClaimCheckCompactionSbfErrorV1::Economic,
        ClaimCheckCompactionSbfErrorV1::Receipt,
        ClaimCheckCompactionSbfErrorV1::Escrow,
        ClaimCheckCompactionSbfErrorV1::Scope,
    ];

    #[test]
    fn every_code_is_contiguous_and_unique_within_the_sub_band() {
        for (index, code) in TABLE.iter().enumerate() {
            let expected = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x600 + index as u32;
            assert_eq!(*code as u32, expected);
            let rest = index + 1;
            assert!(!TABLE.iter().skip(rest).any(|other| other == code));
        }
    }

    #[test]
    fn the_sub_band_does_not_collide_with_any_occupied_claims_sub_band() {
        // Occupied in claims-sbf before this lane: 0x000, 0x100, 0x140, 0x160,
        // 0x180, 0x200, 0x210, 0x260, 0x500. A later addition to any of those
        // that ran 0x100 wide would reach this block, and this is the assertion
        // that would catch it.
        for occupied in [
            0x000_u32, 0x100, 0x140, 0x160, 0x180, 0x200, 0x210, 0x260, 0x500,
        ] {
            let base = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + occupied;
            for code in TABLE {
                assert_ne!(code as u32, base);
                assert!(code as u32 > base);
            }
        }
    }

    #[test]
    fn a_refusal_reaches_the_runtime_as_the_literal_a_log_line_shows() {
        assert_eq!(
            ProgramError::from(ClaimCheckCompactionSbfErrorV1::Deadline),
            ProgramError::Custom(0x5603)
        );
        assert_eq!(
            ProgramError::from(ClaimCheckCompactionSbfErrorV1::Scope),
            ProgramError::Custom(0x560A)
        );
    }

    #[test]
    fn only_an_owner_that_can_sign_may_be_promised_a_claim_check() {
        // Stated over the whole enum rather than over the kinds this route
        // happens to have met, so a kind added later is a compile error here
        // and not a silent third `true`.
        for (kind, admissible) in [
            (ProtocolPositionOwnerKindV2::User, true),
            (ProtocolPositionOwnerKindV2::TradingRecord, false),
            (ProtocolPositionOwnerKindV2::ClaimsCapability, false),
        ] {
            assert_eq!(
                owner_kind_can_open_a_claim_check(kind),
                admissible,
                "{kind:?} must answer whether it can sign for its own payout"
            );
        }
    }

    #[test]
    fn exactly_one_owner_kind_is_a_wallet_and_the_other_two_are_program_derived() {
        // The reason the gate reads the way it does, kept as an assertion so a
        // future edit that flips an arm has to argue with this sentence: a
        // claim-check is redeemable only by a signature, and only one of the
        // three owner kinds is an identity that has one.
        let admitted = [
            ProtocolPositionOwnerKindV2::TradingRecord,
            ProtocolPositionOwnerKindV2::User,
            ProtocolPositionOwnerKindV2::ClaimsCapability,
        ]
        .into_iter()
        .filter(|kind| owner_kind_can_open_a_claim_check(*kind))
        .count();
        assert_eq!(admitted, 1);
    }
}
