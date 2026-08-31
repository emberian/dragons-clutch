//! Refusals for holder-signed claim-check redemption and escrow close.
//!
//! This is the route that outlives the market. The holder signs; nobody else
//! can call it and nobody else needs to. What makes it durable is what is
//! *absent* from its frame -- no Claims aggregate, no Core state, no linked
//! basis record, no composition graph record, no Hoard, no Realm, no Custody
//! authority, and above all no Custody replay cursor. Every one of those has
//! been closed by the retirement the claim-check permitted.
//!
//! That absence is why this table is short, and the shortness is the feature.
//!
//! **Anti-replay is the account's own existence, so it has no code.** The
//! record is created once -- a non-vacant address refuses a second compaction
//! -- and closed on redemption. A closed account cannot be redeemed, and
//! re-creating one would need a compaction crank, which refuses because the
//! position it would have to read is gone. There is no cursor, no revision and
//! no counter to get wrong.
//!
//! **Draining another holder is likewise refused by construction.** The vault
//! is debited only by a redemption closing exactly one record for exactly its
//! own entitlement, and `Conservation` is what an attempt to move any other
//! quantity surfaces as.

use core::convert::TryInto;

use dclutch_claims_svm::claim_check_conservation_v1::{
    ClaimCheckRedemptionObservationV1, ClaimCheckRedemptionPlanV1, ClaimCheckRedemptionPostV1,
};
use dclutch_claims_svm::claim_check_request_v1::{
    CloseClaimCheckEscrowRequestV1, RedeemClaimCheckRequestV1,
};
use dclutch_claims_svm::claim_check_v1::{
    CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1, ClaimCheckEscrowSeedsV1, ClaimCheckEscrowV1,
    ClaimCheckRedemptionRoleV1, ClaimCheckSeedsV1, ClaimCheckV1,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program::invoke_signed,
    program_error::ProgramError, pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use spl_token_2022_interface::instruction as token_instruction;

/// Stable claim-check redemption refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimCheckRedemptionSbfErrorV1 {
    /// The fixed account frame, ownership, or writability refused.
    Accounts = 0x5620,
    /// The signer was not the record's sole entitled holder.
    Authority = 0x5621,
    /// The record was not at its derived address, or the vault did not match.
    Identity = 0x5622,
    /// The vault debit did not equal the record's entitlement.
    Conservation = 0x5623,
    /// Observed post-balances did not match the admitted plan.
    Receipt = 0x5624,
    /// An escrow close was attempted while claim-checks were still live.
    Vault = 0x5625,
}

impl ClaimCheckRedemptionSbfErrorV1 {
    /// Every refusal this request family can raise, in discriminant order.
    ///
    /// This is what the sub-band assertions below read. It is kept honest by
    /// [`ClaimCheckRedemptionSbfErrorV1::ordinal`], whose match is exhaustive: a variant added to
    /// the enum does not compile until its author writes an arm here, and the only arm that
    /// satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 6] = [
        Self::Accounts,
        Self::Authority,
        Self::Identity,
        Self::Conservation,
        Self::Receipt,
        Self::Vault,
    ];

    /// This refusal's position in [`ClaimCheckRedemptionSbfErrorV1::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a seventh variant is a
    /// COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Accounts => 0,
            Self::Authority => 1,
            Self::Identity => 2,
            Self::Conservation => 3,
            Self::Receipt => 4,
            Self::Vault => 5,
        }
    }
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
//
// WHY THIS IS A LIST AND NOT TWO ENDPOINTS. The ceiling assertion used to name
// one variant BY HAND as "the last one". A hand-named ceiling says nothing about
// the variants after it and goes stale silently every single time the family
// grows -- the failure is not that the name is wrong, it is that nothing can
// notice. Claims' own top-level band proved it the expensive way: its bound went
// on naming `ReleaseSuperseded` after a later variant landed, so for as long as
// that stood, the newest refusal in the program was checked by nothing.
//
// So the sub-band is now checked over `ALL`, element by element, and `ALL` is
// welded to the enum by the exhaustive `ordinal` match. A new variant cannot
// join quietly: it does not compile until its author answers for it, and the
// answer they must give is its index here.
const _: () = {
    const SUB_BAND: u32 = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x620;
    assert!(
        ClaimCheckRedemptionSbfErrorV1::ALL[0] as u32 == SUB_BAND,
        "ClaimCheckRedemptionSbfErrorV1 must start at its registered sub-band offset"
    );
    let mut index = 0;
    while index < ClaimCheckRedemptionSbfErrorV1::ALL.len() {
        let variant = ClaimCheckRedemptionSbfErrorV1::ALL[index];
        assert!(
            variant.ordinal() == index,
            "ClaimCheckRedemptionSbfErrorV1::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32 == SUB_BAND + index as u32,
            "ClaimCheckRedemptionSbfErrorV1 discriminants are not the contiguous run from the sub-band offset that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "ClaimCheckRedemptionSbfErrorV1 must not run past its registered refusal band"
        );
        index += 1;
    }
};

impl From<ClaimCheckRedemptionSbfErrorV1> for ProgramError {
    fn from(value: ClaimCheckRedemptionSbfErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

/// The holder, who signs and is paid.
pub const REDEEM_HOLDER_ACCOUNT_V1: usize = 0;
/// The claim-check record, closed into the holder.
pub const REDEEM_RECORD_ACCOUNT_V1: usize = 1;
/// The per-market escrow.
pub const REDEEM_ESCROW_ACCOUNT_V1: usize = 2;
/// The escrow vault.
pub const REDEEM_VAULT_ACCOUNT_V1: usize = 3;
/// The holder's own token account.
pub const REDEEM_HOLDER_TOKENS_ACCOUNT_V1: usize = 4;
/// The collateral mint.
pub const REDEEM_MINT_ACCOUNT_V1: usize = 5;
/// The collateral token program.
pub const REDEEM_TOKEN_PROGRAM_ACCOUNT_V1: usize = 6;

/// Redeem one claim-check, owner-signed, forever.
///
/// The route that outlives the market, and the only one a holder ever touches
/// once everything else is gone. What makes it durable is what is absent from
/// its frame: no Claims aggregate, no Core state, no linked basis record, no
/// composition graph record, no Hoard, no Realm, no Custody authority, and
/// above all no Custody replay cursor. Every one of those was closed by the
/// retirement this claim-check permitted, and a route that needed one would be
/// a promise that stops working the moment its dependency is cleaned up.
///
/// Anti-replay is the account's own existence. The record is created once -- a
/// non-vacant address refuses a second compaction -- and closed here. A closed
/// account cannot be redeemed, and re-creating one would need a compaction
/// crank, which refuses because the position it would have to read is gone. No
/// cursor, no revision, no counter.
pub fn process_redemption(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1 {
        return Err(ClaimCheckRedemptionSbfErrorV1::Accounts.into());
    }
    let request = RedeemClaimCheckRequestV1::decode(instruction_data)
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?;
    let at = |index: usize| {
        accounts
            .get(index)
            .ok_or(ClaimCheckRedemptionSbfErrorV1::Accounts)
    };
    let holder = at(REDEEM_HOLDER_ACCOUNT_V1)?;
    let record_account = at(REDEEM_RECORD_ACCOUNT_V1)?;
    let escrow_account = at(REDEEM_ESCROW_ACCOUNT_V1)?;
    let vault = at(REDEEM_VAULT_ACCOUNT_V1)?;
    let holder_tokens = at(REDEEM_HOLDER_TOKENS_ACCOUNT_V1)?;
    let mint = at(REDEEM_MINT_ACCOUNT_V1)?;
    let token_program = at(REDEEM_TOKEN_PROGRAM_ACCOUNT_V1)?;

    // Privileges come from the declared frame, so the route and the spec cannot
    // drift: adding an account means adding a role, and a role must say whether
    // it outlives the market.
    for (index, role) in ClaimCheckRedemptionRoleV1::frame().iter().enumerate() {
        let account = at(index)?;
        let (signer, writable) = role.privileges();
        if account.is_signer != signer || account.is_writable != writable {
            return Err(ClaimCheckRedemptionSbfErrorV1::Accounts.into());
        }
    }
    if record_account.owner != program_id || escrow_account.owner != program_id {
        return Err(ClaimCheckRedemptionSbfErrorV1::Accounts.into());
    }

    let record = ClaimCheckV1::decode(
        &record_account
            .try_borrow_data()
            .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?,
    )
    .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?;
    let escrow = ClaimCheckEscrowV1::decode(
        &escrow_account
            .try_borrow_data()
            .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?,
    )
    .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?;

    // One signature, proving one thing: that the signer is the holder this
    // record names. There is no holder field on the wire to forge -- the
    // coordinates are PDA seeds, and they must derive the account passed.
    if holder.key.to_bytes() != record.owner {
        return Err(ClaimCheckRedemptionSbfErrorV1::Authority.into());
    }
    let seeds = ClaimCheckSeedsV1::new(request.aggregate, request.owner)
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?;
    if record_account.key != &Pubkey::find_program_address(&seeds.as_slices(), program_id).0
        || request.owner != record.owner
        || request.aggregate != record.aggregate
        || escrow.aggregate != record.aggregate
        || record.vault != vault.key.to_bytes()
        || escrow.vault != vault.key.to_bytes()
        || record.collateral_mint != mint.key.to_bytes()
        || escrow_account.key
            != &Pubkey::find_program_address(
                &ClaimCheckEscrowSeedsV1::new(record.aggregate)
                    .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?
                    .as_slices(),
                program_id,
            )
            .0
    {
        return Err(ClaimCheckRedemptionSbfErrorV1::Identity.into());
    }

    let plan = ClaimCheckRedemptionPlanV1::new(ClaimCheckRedemptionObservationV1 {
        entitlement_atoms: record.entitlement_atoms,
        vault_before: token_balance(vault)?,
        holder_tokens_before: token_balance(holder_tokens)?,
        record_lamports: record_account.lamports(),
        holder_lamports_before: holder.lamports(),
    })
    .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Conservation)?;

    // The escrow PDA is the vault's authority and signs for it. Its bump is the
    // one persisted in the escrow record, so the signer recipe has one author.
    let signer = escrow
        .signer_seeds()
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?;
    let [domain, aggregate, bump] = signer.as_slices();
    let decimals = mint_decimals(mint)?;
    invoke_signed(
        &token_instruction::transfer_checked(
            token_program.key,
            vault.key,
            mint.key,
            holder_tokens.key,
            escrow_account.key,
            &[],
            plan.entitlement_atoms(),
            decimals,
        )
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Conservation)?,
        &[
            vault.clone(),
            mint.clone(),
            holder_tokens.clone(),
            escrow_account.clone(),
            token_program.clone(),
        ],
        &[&[domain, aggregate, bump]],
    )
    .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Conservation)?;

    // The record closes into the holder, and its rent goes home with them.
    let closed_lamports = record_account.lamports();
    {
        let mut record_lamports = record_account
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?;
        let mut holder_lamports = holder
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?;
        **record_lamports = 0;
        **holder_lamports = holder_lamports
            .checked_add(closed_lamports)
            .ok_or(ClaimCheckRedemptionSbfErrorV1::Conservation)?;
    }
    record_account
        .resize(0)
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?;
    record_account.assign(&system_program::ID);

    let settled = escrow
        .retire_claim_check()
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Conservation)?;
    escrow_account
        .try_borrow_mut_data()
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?
        .copy_from_slice(
            &settled
                .to_bytes()
                .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?,
        );

    plan.validate_post(ClaimCheckRedemptionPostV1 {
        vault_atoms: token_balance(vault)?,
        holder_tokens: token_balance(holder_tokens)?,
        record_lamports: record_account.lamports(),
        holder_lamports: holder.lamports(),
    })
    .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Receipt)?;
    Ok(())
}

fn token_balance(account: &AccountInfo<'_>) -> Result<u64, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?;
    let bytes: [u8; 8] = data
        .get(64..72)
        .ok_or(ClaimCheckRedemptionSbfErrorV1::Accounts)?
        .try_into()
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?;
    Ok(u64::from_le_bytes(bytes))
}

fn mint_decimals(mint: &AccountInfo<'_>) -> Result<u8, ProgramError> {
    let data = mint
        .try_borrow_data()
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?;
    data.get(44)
        .copied()
        .ok_or_else(|| ClaimCheckRedemptionSbfErrorV1::Accounts.into())
}

/// The caller, who closes the escrow and is paid the rent for doing it.
pub const CLOSE_CALLER_ACCOUNT_V1: usize = 0;
/// The escrow record being closed.
pub const CLOSE_ESCROW_ACCOUNT_V1: usize = 1;
/// The escrow vault being closed.
pub const CLOSE_VAULT_ACCOUNT_V1: usize = 2;
/// The caller's token account, receiving any residue the vault still holds.
pub const CLOSE_CALLER_TOKENS_ACCOUNT_V1: usize = 3;
/// The collateral mint.
pub const CLOSE_MINT_ACCOUNT_V1: usize = 4;
/// The collateral token program.
pub const CLOSE_TOKEN_PROGRAM_ACCOUNT_V1: usize = 5;
/// Exact escrow-close frame width.
pub const CLOSE_CLAIM_CHECK_ESCROW_ACCOUNT_COUNT_V1: usize = 6;

/// Close one fully redeemed escrow, permissionlessly.
///
/// This is what makes the residue self-liquidating rather than merely small.
/// The design's honest claim is that a compacted market leaves one escrow, one
/// vault and one claim-check per unredeemed holder -- and that this shrinks to
/// zero, with the last redemption enabling the close. This route is the last
/// clause of that sentence.
///
/// The gate is the escrow's own outstanding count, not a deadline. An escrow
/// still holding a live claim-check is holding collateral for somebody who has
/// not come back, and closing it would be taking their money. That it can stay
/// open forever is the ruling working as intended: the claim survives, and
/// collateral has to be somewhere.
///
/// It needs no escrow of its own because both accounts' rent, plus any residue
/// the vault still holds, funds whoever turns it.
pub fn process_escrow_close(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != CLOSE_CLAIM_CHECK_ESCROW_ACCOUNT_COUNT_V1 {
        return Err(ClaimCheckRedemptionSbfErrorV1::Accounts.into());
    }
    let request = CloseClaimCheckEscrowRequestV1::decode(instruction_data)
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?;
    let at = |index: usize| {
        accounts
            .get(index)
            .ok_or(ClaimCheckRedemptionSbfErrorV1::Accounts)
    };
    let caller = at(CLOSE_CALLER_ACCOUNT_V1)?;
    let escrow_account = at(CLOSE_ESCROW_ACCOUNT_V1)?;
    let vault = at(CLOSE_VAULT_ACCOUNT_V1)?;
    let caller_tokens = at(CLOSE_CALLER_TOKENS_ACCOUNT_V1)?;
    let mint = at(CLOSE_MINT_ACCOUNT_V1)?;
    let token_program = at(CLOSE_TOKEN_PROGRAM_ACCOUNT_V1)?;

    if !caller.is_signer
        || !caller.is_writable
        || !escrow_account.is_writable
        || !vault.is_writable
        || !caller_tokens.is_writable
        || mint.is_writable
        || token_program.is_writable
        || escrow_account.owner != program_id
    {
        return Err(ClaimCheckRedemptionSbfErrorV1::Accounts.into());
    }

    let escrow = ClaimCheckEscrowV1::decode(
        &escrow_account
            .try_borrow_data()
            .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?,
    )
    .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?;
    if escrow_account.key
        != &Pubkey::find_program_address(
            &ClaimCheckEscrowSeedsV1::new(request.aggregate)
                .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?
                .as_slices(),
            program_id,
        )
        .0
        || escrow.aggregate != request.aggregate
        || escrow.vault != vault.key.to_bytes()
        || escrow.collateral_mint != mint.key.to_bytes()
    {
        return Err(ClaimCheckRedemptionSbfErrorV1::Identity.into());
    }

    // The one gate. An escrow with a live claim-check is somebody's collateral.
    if !escrow.is_settled() {
        return Err(ClaimCheckRedemptionSbfErrorV1::Vault.into());
    }

    let signer = escrow
        .signer_seeds()
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Identity)?;
    let [domain, aggregate, bump] = signer.as_slices();

    // Residue rather than refusal. A transfer-fee mint can leave atoms behind
    // that no claim-check promises; sweeping them to the caller is what stops a
    // rounding remainder from pinning an empty escrow open forever. Today the
    // terminal executor's exact-equality check means this is always zero, so
    // this branch is insurance, not a code path with a bill attached.
    let residue = token_balance(vault)?;
    if residue != 0 {
        invoke_signed(
            &token_instruction::transfer_checked(
                token_program.key,
                vault.key,
                mint.key,
                caller_tokens.key,
                escrow_account.key,
                &[],
                residue,
                mint_decimals(mint)?,
            )
            .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Conservation)?,
            &[
                vault.clone(),
                mint.clone(),
                caller_tokens.clone(),
                escrow_account.clone(),
                token_program.clone(),
            ],
            &[&[domain, aggregate, bump]],
        )
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Conservation)?;
    }

    let caller_before = caller.lamports();
    let vault_lamports = vault.lamports();
    invoke_signed(
        &token_instruction::close_account(
            token_program.key,
            vault.key,
            caller.key,
            escrow_account.key,
            &[],
        )
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Conservation)?,
        &[
            vault.clone(),
            caller.clone(),
            escrow_account.clone(),
            token_program.clone(),
        ],
        &[&[domain, aggregate, bump]],
    )
    .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Conservation)?;

    let escrow_lamports = escrow_account.lamports();
    {
        let mut escrow_balance = escrow_account
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?;
        let mut caller_balance = caller
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?;
        **escrow_balance = 0;
        **caller_balance = caller_balance
            .checked_add(escrow_lamports)
            .ok_or(ClaimCheckRedemptionSbfErrorV1::Conservation)?;
    }
    escrow_account
        .resize(0)
        .map_err(|_| ClaimCheckRedemptionSbfErrorV1::Accounts)?;
    escrow_account.assign(&system_program::ID);

    // Everything both accounts held reached the caller, and nothing was
    // stranded in an account about to be left at zero length.
    if escrow_account.lamports() != 0
        || !vault.data_is_empty()
        || caller.lamports()
            != caller_before
                .checked_add(vault_lamports)
                .and_then(|value| value.checked_add(escrow_lamports))
                .ok_or(ClaimCheckRedemptionSbfErrorV1::Conservation)?
    {
        return Err(ClaimCheckRedemptionSbfErrorV1::Receipt.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_is_contiguous_and_unique_within_the_sub_band() {
        for (index, code) in ClaimCheckRedemptionSbfErrorV1::ALL.iter().enumerate() {
            let expected = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x620 + index as u32;
            assert_eq!(*code as u32, expected);
            let rest = index + 1;
            assert!(
                !ClaimCheckRedemptionSbfErrorV1::ALL
                    .iter()
                    .skip(rest)
                    .any(|other| other == code)
            );
        }
    }

    #[test]
    fn the_two_claim_check_sub_bands_never_overlap() {
        // Compaction runs 0x5600..=0x560A. Redemption starts at 0x5620, and the
        // gap is deliberate room for compaction to grow without either family
        // renumbering, which would break every hostile test naming a literal.
        use crate::claim_check_compaction_v1::ClaimCheckCompactionSbfErrorV1;
        assert!(
            (ClaimCheckCompactionSbfErrorV1::Scope as u32)
                < ClaimCheckRedemptionSbfErrorV1::Accounts as u32
        );
        for code in ClaimCheckRedemptionSbfErrorV1::ALL {
            assert!(code as u32 > ClaimCheckCompactionSbfErrorV1::Scope as u32);
        }
    }

    #[test]
    fn a_refusal_reaches_the_runtime_as_the_literal_a_log_line_shows() {
        assert_eq!(
            ProgramError::from(ClaimCheckRedemptionSbfErrorV1::Authority),
            ProgramError::Custom(0x5621)
        );
        assert_eq!(
            ProgramError::from(ClaimCheckRedemptionSbfErrorV1::Vault),
            ProgramError::Custom(0x5625)
        );
    }
}
