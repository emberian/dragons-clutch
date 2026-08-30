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

use dclutch_claims_svm::claim_check_request_v1::OpenClaimCheckEscrowRequestV1;
use dclutch_claims_svm::claim_check_v1::{
    CLAIM_CHECK_ESCROW_BYTES_V1, ClaimCheckEscrowSeedsV1, ClaimCheckEscrowV1,
    ClaimCheckVaultSeedsV1,
};
use dclutch_claims_svm::liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2;
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, Phase};
use dclutch_realm_contract::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
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
}
