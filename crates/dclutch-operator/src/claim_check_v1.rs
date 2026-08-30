//! What a holder needs to find and redeem a claim-check.
//!
//! Everything else in this feature is written for the protocol: a crank anyone
//! may turn, a deadline nobody may shorten, a conservation plan that refuses to
//! exist unless the movement balances. This module is written for the one
//! person the feature is actually for — somebody who bought a claim on a market
//! that has since been cleaned up, and who now has to be able to find their
//! money without knowing any of that happened.
//!
//! # The whole problem, and why it is small
//!
//! A holder returning to a retired market cannot look the market up: the
//! aggregate is closed, the Core state is closed, and there is no registry of
//! who held what. What survives is an address they can *derive*. A claim-check
//! lives at `[CLAIM_CHECK_SEED_V1, aggregate, owner]`, and both coordinates are
//! things a holder's own client already knows — the market they traded and the
//! wallet they traded from. So discovery is a derivation, not a search, and it
//! works offline, forever, with no index and no server.
//!
//! That is the practical payoff of resolving the payout at compaction time. If
//! the record stored raw per-outcome atoms, this module would have to
//! reconstruct a payoff function out of accounts that no longer exist. Instead
//! it reads one number.
//!
//! # What this module refuses to do
//!
//! It does not sign, and it does not submit. Every function here returns an
//! unsigned instruction plus the exact facts the caller should expect
//! afterwards, so the wallet that holds the key decides whether to send it and
//! can check what happened. A holder's key is the one thing the whole design
//! keeps between a person and their collateral, and an operator crate is the
//! wrong place to start making exceptions to that.

use dclutch_claims_svm::claim_check_request_v1::{
    CloseClaimCheckEscrowRequestV1, RedeemClaimCheckRequestV1,
};
use dclutch_claims_svm::claim_check_v1::{
    CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1, ClaimCheckEscrowSeedsV1, ClaimCheckEscrowV1,
    ClaimCheckRedemptionRoleV1, ClaimCheckSeedsV1, ClaimCheckV1, ClaimCheckVaultSeedsV1,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

/// Stable refusal from claim-check operator construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimCheckOperatorErrorV1 {
    /// A coordinate was zero, or two that must differ aliased.
    Coordinate,
    /// Persisted record bytes did not decode.
    Record,
    /// The record does not live at the address these coordinates derive.
    Address,
    /// The record, escrow, and vault did not describe one market.
    Binding,
}

/// Where a holder's claim-check and its escrow live.
///
/// Derived from coordinates alone. No account is read to produce this, which is
/// what lets a client tell a holder "there may be something here for you"
/// before it has fetched anything, and lets it work against a market whose
/// every other account is gone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckCoordinatesV1 {
    /// The Claims aggregate the position was admitted against.
    pub aggregate: Pubkey,
    /// The holder.
    pub owner: Pubkey,
    /// Where the claim-check record lives, if one was ever minted.
    pub record: Pubkey,
    /// Where the market's escrow lives.
    pub escrow: Pubkey,
    /// Where the escrow's collateral vault lives.
    pub vault: Pubkey,
}

/// Derive every address a holder needs, from the two things they know.
///
/// The two coordinates are the market's Claims aggregate and the holder's own
/// wallet — nothing a retired market has to still be alive to answer.
pub fn project_claim_check_coordinates_v1(
    claims_program: &Pubkey,
    aggregate: &Pubkey,
    owner: &Pubkey,
) -> Result<ClaimCheckCoordinatesV1, ClaimCheckOperatorErrorV1> {
    let record_seeds = ClaimCheckSeedsV1::new(aggregate.to_bytes(), owner.to_bytes())
        .map_err(|_| ClaimCheckOperatorErrorV1::Coordinate)?;
    let escrow_seeds = ClaimCheckEscrowSeedsV1::new(aggregate.to_bytes())
        .map_err(|_| ClaimCheckOperatorErrorV1::Coordinate)?;
    let vault_seeds = ClaimCheckVaultSeedsV1::new(aggregate.to_bytes())
        .map_err(|_| ClaimCheckOperatorErrorV1::Coordinate)?;
    Ok(ClaimCheckCoordinatesV1 {
        aggregate: *aggregate,
        owner: *owner,
        record: Pubkey::find_program_address(&record_seeds.as_slices(), claims_program).0,
        escrow: Pubkey::find_program_address(&escrow_seeds.as_slices(), claims_program).0,
        vault: Pubkey::find_program_address(&vault_seeds.as_slices(), claims_program).0,
    })
}

/// What a holder is owed, in the plainest terms the chain can support.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckStatementV1 {
    /// Where this claim-check lives.
    pub coordinates: ClaimCheckCoordinatesV1,
    /// Collateral atoms owed, already resolved. Not a formula: a number.
    pub entitlement_atoms: u64,
    /// The mint those atoms are denominated in.
    pub collateral_mint: Pubkey,
    /// The market this claim came from, for a holder who wants to know which.
    pub market: Pubkey,
    /// The slot at which a crank resolved this payout on the holder's behalf.
    pub compacted_slot: u64,
    /// Lamports the holder also recovers when the record closes.
    pub recoverable_lamports: u64,
}

/// Read one persisted claim-check into a statement a person can act on.
///
/// The record's own address is checked against the coordinates it claims, so a
/// client cannot be handed somebody else's bytes and shown them as the reader's
/// own balance.
pub fn read_claim_check_statement_v1(
    claims_program: &Pubkey,
    record_account: &Pubkey,
    record_bytes: &[u8],
    record_lamports: u64,
) -> Result<ClaimCheckStatementV1, ClaimCheckOperatorErrorV1> {
    let record =
        ClaimCheckV1::decode(record_bytes).map_err(|_| ClaimCheckOperatorErrorV1::Record)?;
    let coordinates = project_claim_check_coordinates_v1(
        claims_program,
        &Pubkey::new_from_array(record.aggregate),
        &Pubkey::new_from_array(record.owner),
    )?;
    if &coordinates.record != record_account {
        return Err(ClaimCheckOperatorErrorV1::Address);
    }
    if coordinates.vault.to_bytes() != record.vault {
        return Err(ClaimCheckOperatorErrorV1::Binding);
    }
    Ok(ClaimCheckStatementV1 {
        coordinates,
        entitlement_atoms: record.entitlement_atoms,
        collateral_mint: Pubkey::new_from_array(record.collateral_mint),
        market: Pubkey::new_from_array(record.market),
        compacted_slot: record.compacted_slot,
        recoverable_lamports: record_lamports,
    })
}

/// An unsigned redemption and the facts that should hold once it lands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimCheckRedemptionReportV1 {
    /// The unsigned instruction. The holder's wallet signs it, or nobody does.
    pub instruction: Instruction,
    /// What the holder is owed, restated so a caller can show it before sending.
    pub statement: ClaimCheckStatementV1,
    /// Atoms the holder's token account should gain, absent a transfer fee.
    pub expected_token_credit: u64,
    /// Lamports the holder's wallet should gain from the closed record.
    pub expected_lamport_credit: u64,
    /// Accounts that should not exist afterwards.
    pub expected_vacant: [Pubkey; 1],
}

/// Build the unsigned instruction that pays a holder their claim-check.
///
/// The frame is generated from [`ClaimCheckRedemptionRoleV1`], the same
/// declaration the on-chain route reads its privileges from, so an operator
/// cannot construct a frame the program would reject on shape — and if the
/// route's frame ever changes, this stops compiling rather than silently
/// building something that refuses.
pub fn build_claim_check_redemption_v1(
    claims_program: &Pubkey,
    token_program: &Pubkey,
    holder_token_account: &Pubkey,
    statement: ClaimCheckStatementV1,
) -> Result<ClaimCheckRedemptionReportV1, ClaimCheckOperatorErrorV1> {
    let coordinates = statement.coordinates;
    let request = RedeemClaimCheckRequestV1 {
        aggregate: coordinates.aggregate.to_bytes(),
        owner: coordinates.owner.to_bytes(),
    }
    .new()
    .map_err(|_| ClaimCheckOperatorErrorV1::Coordinate)?;
    let addresses = [
        coordinates.owner,
        coordinates.record,
        coordinates.escrow,
        coordinates.vault,
        *holder_token_account,
        statement.collateral_mint,
        *token_program,
    ];
    let accounts = ClaimCheckRedemptionRoleV1::frame()
        .iter()
        .zip(addresses)
        .map(|(role, address)| {
            let (signer, writable) = role.privileges();
            AccountMeta {
                pubkey: address,
                is_signer: signer,
                is_writable: writable,
            }
        })
        .collect::<Vec<_>>();
    if accounts.len() != CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1 {
        return Err(ClaimCheckOperatorErrorV1::Coordinate);
    }
    Ok(ClaimCheckRedemptionReportV1 {
        instruction: Instruction {
            program_id: *claims_program,
            accounts,
            data: request
                .to_bytes()
                .map_err(|_| ClaimCheckOperatorErrorV1::Coordinate)?
                .to_vec(),
        },
        statement,
        expected_token_credit: statement.entitlement_atoms,
        expected_lamport_credit: statement.recoverable_lamports,
        expected_vacant: [coordinates.record],
    })
}

/// Whether a market's escrow can be closed, and by whom.
///
/// Anyone may close a settled escrow and keep the rent. An escrow still owing
/// somebody cannot be closed at all, and that is the ruling working as
/// intended: the claim survives, so the collateral has to be somewhere.
pub fn escrow_is_closeable_v1(escrow_bytes: &[u8]) -> Result<bool, ClaimCheckOperatorErrorV1> {
    Ok(ClaimCheckEscrowV1::decode(escrow_bytes)
        .map_err(|_| ClaimCheckOperatorErrorV1::Record)?
        .is_settled())
}

/// Build the unsigned instruction that closes a fully redeemed escrow.
pub fn build_claim_check_escrow_close_v1(
    claims_program: &Pubkey,
    token_program: &Pubkey,
    caller: &Pubkey,
    caller_token_account: &Pubkey,
    coordinates: ClaimCheckCoordinatesV1,
    collateral_mint: &Pubkey,
) -> Result<Instruction, ClaimCheckOperatorErrorV1> {
    let request = CloseClaimCheckEscrowRequestV1 {
        aggregate: coordinates.aggregate.to_bytes(),
    }
    .new()
    .map_err(|_| ClaimCheckOperatorErrorV1::Coordinate)?;
    Ok(Instruction {
        program_id: *claims_program,
        accounts: Vec::from([
            AccountMeta {
                pubkey: *caller,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: coordinates.escrow,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: coordinates.vault,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *caller_token_account,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *collateral_mint,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: *token_program,
                is_signer: false,
                is_writable: false,
            },
        ]),
        data: request
            .to_bytes()
            .map_err(|_| ClaimCheckOperatorErrorV1::Coordinate)?
            .to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims() -> Pubkey {
        Pubkey::new_from_array([9; 32])
    }

    fn record_bytes(entitlement: u64) -> (Pubkey, Pubkey, Vec<u8>) {
        let aggregate = Pubkey::new_from_array([1; 32]);
        let owner = Pubkey::new_from_array([2; 32]);
        let coordinates =
            project_claim_check_coordinates_v1(&claims(), &aggregate, &owner).expect("coordinates");
        let record = ClaimCheckV1 {
            aggregate: aggregate.to_bytes(),
            owner: owner.to_bytes(),
            market: [3; 32],
            release_set: [4; 32],
            vault: coordinates.vault.to_bytes(),
            collateral_mint: [6; 32],
            position_atoms_digest: [7; 32],
            entitlement_atoms: entitlement,
            compacted_slot: 12_345,
            generation: 9,
            bump: 254,
        }
        .new()
        .expect("record");
        (
            coordinates.record,
            aggregate,
            record.to_bytes().expect("bytes").to_vec(),
        )
    }

    #[test]
    fn a_holder_finds_their_claim_check_from_coordinates_alone() {
        // The practical point of the whole design: no index, no server, no live
        // market. Two things the holder already knows.
        let aggregate = Pubkey::new_from_array([1; 32]);
        let owner = Pubkey::new_from_array([2; 32]);
        let first =
            project_claim_check_coordinates_v1(&claims(), &aggregate, &owner).expect("coordinates");
        let again =
            project_claim_check_coordinates_v1(&claims(), &aggregate, &owner).expect("coordinates");
        assert_eq!(first, again, "derivation is deterministic and offline");
        assert_ne!(first.record, first.escrow);
        assert_ne!(first.escrow, first.vault);

        // A different holder derives a different record against the same market.
        let other = Pubkey::new_from_array([5; 32]);
        let theirs =
            project_claim_check_coordinates_v1(&claims(), &aggregate, &other).expect("coordinates");
        assert_ne!(first.record, theirs.record);
        assert_eq!(first.escrow, theirs.escrow, "one escrow serves the market");
        assert_eq!(first.vault, theirs.vault);
    }

    #[test]
    fn a_statement_says_one_number_rather_than_a_formula() {
        let (account, _, bytes) = record_bytes(750_000);
        let statement =
            read_claim_check_statement_v1(&claims(), &account, &bytes, 2_895_360).expect("read");
        assert_eq!(statement.entitlement_atoms, 750_000);
        assert_eq!(statement.recoverable_lamports, 2_895_360);
        assert_eq!(statement.compacted_slot, 12_345);
    }

    #[test]
    fn a_record_read_at_the_wrong_address_is_refused() {
        // A client handed somebody else's bytes must not show them to a reader
        // as their own balance.
        let (_, _, bytes) = record_bytes(750_000);
        let elsewhere = Pubkey::new_from_array([42; 32]);
        assert_eq!(
            read_claim_check_statement_v1(&claims(), &elsewhere, &bytes, 0),
            Err(ClaimCheckOperatorErrorV1::Address)
        );
    }

    #[test]
    fn the_built_redemption_matches_the_routes_own_frame_declaration() {
        let (account, _, bytes) = record_bytes(750_000);
        let statement =
            read_claim_check_statement_v1(&claims(), &account, &bytes, 2_895_360).expect("read");
        let token_program = Pubkey::new_from_array([8; 32]);
        let holder_tokens = Pubkey::new_from_array([11; 32]);
        let report =
            build_claim_check_redemption_v1(&claims(), &token_program, &holder_tokens, statement)
                .expect("report");

        assert_eq!(
            report.instruction.accounts.len(),
            CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1
        );
        // Exactly one signer, and it is the holder: the operator cannot build a
        // frame that asks anybody else to sign.
        let signers = report
            .instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_signer)
            .collect::<Vec<_>>();
        assert_eq!(signers.len(), 1);
        assert_eq!(signers[0].pubkey, statement.coordinates.owner);
        assert_eq!(report.expected_token_credit, 750_000);
        assert_eq!(report.expected_lamport_credit, 2_895_360);
        assert_eq!(report.expected_vacant, [statement.coordinates.record]);
    }

    #[test]
    fn an_escrow_still_owing_somebody_is_not_closeable() {
        let escrow = ClaimCheckEscrowV1 {
            aggregate: [1; 32],
            market: [3; 32],
            release_set: [4; 32],
            vault: [5; 32],
            collateral_mint: [6; 32],
            opener: [8; 32],
            opened_slot: 12_000,
            opener_outlay: 4_711_920,
            outstanding_claim_checks: 1,
            generation: 9,
            bump: 253,
        }
        .new()
        .expect("escrow");
        assert_eq!(
            escrow_is_closeable_v1(&escrow.to_bytes().expect("bytes")),
            Ok(false)
        );
        let settled = ClaimCheckEscrowV1 {
            outstanding_claim_checks: 0,
            ..escrow
        }
        .new()
        .expect("settled");
        assert_eq!(
            escrow_is_closeable_v1(&settled.to_bytes().expect("bytes")),
            Ok(true)
        );
    }
}
