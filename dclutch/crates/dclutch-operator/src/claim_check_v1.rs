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

use dclutch_claims::claim_check_compaction_request_v1::CompactPositionToClaimCheckRequestV1;
use dclutch_claims::claim_check_conservation_v1::{
    ClaimCheckAccountObservationV1, ClaimCheckCompactionObservationV1, ClaimCheckCompactionPlanV1,
    ClaimCheckCompactionPostV1,
};
use dclutch_claims::claim_check_request_v1::{
    CloseClaimCheckEscrowRequestV1, RedeemClaimCheckRequestV1,
};
use dclutch_claims::claim_check_v1::{
    CLAIM_CHECK_BYTES_V1, CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1,
    COMPACTION_CRANK_REWARD_LAMPORTS_V1, COMPACTION_DEADLINE_SLOTS_V1, ClaimCheckEscrowSeedsV1,
    ClaimCheckEscrowV1, ClaimCheckRedemptionRoleV1, ClaimCheckSeedsV1, ClaimCheckV1,
    ClaimCheckVaultSeedsV1,
};
use dclutch_claims::fractional_claim_check_conservation_v1::{
    FractionalClaimCheckRedemptionObservationV1, FractionalClaimCheckRedemptionPlanV1,
};
use dclutch_claims::fractional_claim_check_v1::{
    FRACTIONAL_CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1, FractionalClaimCheckRedemptionRoleV1,
    FractionalClaimCheckSeedsV1, FractionalClaimCheckV1, FractionalRedeemClaimCheckRequestV1,
};
use dclutch_claims::protocol_position_v2::{
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2,
};
use dclutch_claims::terminal_settlement_v3::TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3;
use dclutch_custody::token_svm::TokenAccount;
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{native_loader, system_program};

use crate::{
    Finality, Observation, ObservedAccount,
    observation::decode_rent,
    versioned::{VersionedMessagePlanV0, compile_v0_message},
    wallet_terminal_payout_v3::{
        WalletTerminalPayoutReportV3, project_wallet_terminal_payout_postcondition_v3,
        wallet_terminal_payout_account_frame_v3,
    },
};

/// Stable refusal from claim-check operator construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimCheckOperatorErrorV1 {
    /// A coordinate was zero, or two that must differ aliased.
    Coordinate,
    /// The record does not live at the address these coordinates derive.
    Address,
    /// The record, escrow, and vault did not describe one market.
    Binding,
    /// The requested burn cannot conserve the observed shards or collateral.
    Conservation,
    /// `dclutch_claims` refused; the cause is its own.
    ClaimCheck(dclutch_claims::claim_check_v1::ClaimCheckErrorV1),
    /// `dclutch_claims` refused; the cause is its own.
    FractionalClaimCheckConservation(dclutch_claims::fractional_claim_check_conservation_v1::FractionalClaimCheckConservationErrorV1),
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
        .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
    let escrow_seeds = ClaimCheckEscrowSeedsV1::new(aggregate.to_bytes())
        .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
    let vault_seeds = ClaimCheckVaultSeedsV1::new(aggregate.to_bytes())
        .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
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
        ClaimCheckV1::decode(record_bytes).map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
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
    .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
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
                .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?
                .to_vec(),
        },
        statement,
        expected_token_credit: statement.entitlement_atoms,
        expected_lamport_credit: statement.recoverable_lamports,
        expected_vacant: [coordinates.record],
    })
}

/// Exact account width of native claim-check compaction.
///
/// The first 36 accounts are the terminal-payout frame owned by
/// `wallet_terminal_payout_v3`; the six-account suffix is escrow, claim-check,
/// admission, RentCredit, opener, and System Program.
pub const CLAIM_CHECK_COMPACTION_ACCOUNT_COUNT_V1: usize = TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 + 6;
/// RentCredit suffix coordinate in the compaction frame.
pub const CLAIM_CHECK_COMPACTION_RENT_CREDIT_ACCOUNT_V1: usize =
    TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 + 3;
/// Escrow-opener suffix coordinate in the compaction frame.
pub const CLAIM_CHECK_COMPACTION_OPENER_ACCOUNT_V1: usize =
    TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 + 4;

/// Finalized public state required to turn one sleeping wallet Position into a
/// durable claim-check.
///
/// `payout` must already have been built for the escrow and its vault. The
/// compaction planner calls that exact payout projection and wraps its account
/// frame; it never evaluates the Product or authors collateral arithmetic a
/// second time.
#[derive(Clone, Copy, Debug)]
pub struct ClaimCheckCompactionSnapshotV1<'a> {
    /// Authoritative terminal payout into the escrow vault.
    pub payout: &'a WalletTerminalPayoutReportV3,
    /// Claims aggregate named by the payout.
    pub aggregate: &'a ObservedAccount,
    /// Sleeping wallet Position, including its live lamports.
    pub position: &'a ObservedAccount,
    /// Existing per-market claim-check escrow.
    pub escrow: &'a ObservedAccount,
    /// Escrow-owned collateral token account.
    pub vault: &'a ObservedAccount,
    /// Custody Hoard debited by terminal payout.
    pub hoard: &'a ObservedAccount,
    /// Vacant claim-check address; harmless prefunded lamports are admitted.
    pub claim_check: &'a ObservedAccount,
    /// Persisted admission proving that the Position belongs to a wallet.
    pub admission: &'a ObservedAccount,
    /// Market lifecycle RentCredit receiving residual closed-account lamports.
    pub rent_credit: &'a ObservedAccount,
    /// Permissionless signer turning this crank.
    pub cranker: &'a ObservedAccount,
    /// Wallet that opened the escrow and is owed its recorded outlay.
    pub opener: &'a ObservedAccount,
    /// Canonical Rent sysvar used for the record's exact rent floor.
    pub rent_sysvar: &'a ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: &'a ObservedAccount,
}

/// Stable refusal from native claim-check compaction planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimCheckCompactionOperatorErrorV1 {
    /// Inputs were not one nonzero finalized observation.
    Observation,
    /// The terminal payout report was not its own exact canonical frame.
    Payout,
    /// A program, PDA, owner, or immutable cross-record join disagreed.
    Binding,
    /// The release-fixed holder grace period has not elapsed.
    Deadline,
    /// The Position owner cannot sign a later claim-check redemption.
    Scope,
    /// The finalized lookup table was stale, reordered, or not canonical.
    LookupTable,
    /// Versioned-message compilation refused.
    Routing(crate::versioned::Error),
    /// Exact projected poststate could not be produced.
    Postcondition,
    /// `dclutch_claims` refused; the cause is its own.
    ClaimCheck(dclutch_claims::claim_check_v1::ClaimCheckErrorV1),
    /// `dclutch_operator` refused; the cause is its own.
    ObservationError(crate::observation::ObservationError),
    /// `dclutch_operator` refused; the cause is its own.
    ClaimCheckOperator(crate::claim_check_v1::ClaimCheckOperatorErrorV1),
    /// `dclutch_custody::token_svm` refused; the cause is its own.
    Token(dclutch_custody::token_svm::Error),
    /// `dclutch_claims` refused; the cause is its own.
    ClaimCheckConservation(
        dclutch_claims::claim_check_conservation_v1::ClaimCheckConservationErrorV1,
    ),
    /// `dclutch_claims` refused; the cause is its own.
    ProtocolPosition(dclutch_claims::protocol_position_v2::ProtocolPositionErrorV2),
}

/// Slot-independent fields of the claim-check that an accepted crank will
/// persist.
///
/// The accepted transaction slot is runtime evidence, not a value a caller may
/// predict. [`ClaimCheckCompactionExpectedRecordV1::at_slot`] binds that final
/// field after the signature is finalized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckCompactionExpectedRecordV1 {
    /// Claims aggregate coordinate.
    pub aggregate: [u8; 32],
    /// Sleeping Position's wallet owner.
    pub owner: [u8; 32],
    /// Logical Core Market retained after retirement.
    pub market: [u8; 32],
    /// Market-selected execution release.
    pub release_set: [u8; 32],
    /// Shared claim-check collateral vault.
    pub vault: [u8; 32],
    /// Realm-selected collateral Mint.
    pub collateral_mint: [u8; 32],
    /// Digest of the exact post-payout Position bytes before they close.
    pub position_atoms_digest: [u8; 32],
    /// Observed vault credit promised to this holder.
    pub entitlement_atoms: u64,
    /// Immutable Market generation.
    pub generation: u64,
    /// Canonical claim-check PDA bump.
    pub bump: u8,
}

impl ClaimCheckCompactionExpectedRecordV1 {
    /// Materialize the exact record for the slot reported by the accepted
    /// transaction.
    pub fn at_slot(
        self,
        compacted_slot: u64,
    ) -> Result<ClaimCheckV1, ClaimCheckCompactionOperatorErrorV1> {
        ClaimCheckV1 {
            aggregate: self.aggregate,
            owner: self.owner,
            market: self.market,
            release_set: self.release_set,
            vault: self.vault,
            collateral_mint: self.collateral_mint,
            position_atoms_digest: self.position_atoms_digest,
            entitlement_atoms: self.entitlement_atoms,
            compacted_slot,
            generation: self.generation,
            bump: self.bump,
        }
        .new()
        .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheck)
    }
}

/// Unsigned permissionless compaction and its exact conservation commitments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimCheckCompactionReportV1 {
    /// Exact 42-account Claims instruction.
    pub instruction: Instruction,
    /// Finalized observation selecting every prestate.
    pub observation: Observation,
    /// Sole signer required by the crank instruction.
    pub required_signer: Pubkey,
    /// Authoritative terminal payout embedded unchanged by this crank.
    pub payout: WalletTerminalPayoutReportV3,
    /// Offline-derivable record, escrow, and vault coordinates.
    pub coordinates: ClaimCheckCoordinatesV1,
    /// Exact payout request wrapped without reinterpretation.
    pub payout_request_digest: [u8; 32],
    /// Pure conservation plan admitted by the on-chain semantic owner.
    pub conservation: ClaimCheckCompactionPlanV1,
    /// Exact post-compaction escrow bytes.
    pub expected_escrow_bytes: Vec<u8>,
    /// Expected record fields, absent for a zero-payout Position.
    pub expected_record: Option<ClaimCheckCompactionExpectedRecordV1>,
    /// Position and admission accounts that must be absent afterwards.
    pub expected_vacant: [Pubkey; 2],
}

/// Build one permissionless compaction by wrapping the authoritative terminal
/// payout planner and applying the authoritative claim-check conservation plan.
pub fn build_claim_check_compaction_v1(
    snapshot: ClaimCheckCompactionSnapshotV1<'_>,
) -> Result<ClaimCheckCompactionReportV1, ClaimCheckCompactionOperatorErrorV1> {
    let observation = same_compaction_observation(&snapshot)?;
    let rent = decode_rent(snapshot.rent_sysvar)
        .map_err(ClaimCheckCompactionOperatorErrorV1::ObservationError)?;
    authenticate_compaction_infrastructure(&snapshot)?;
    authenticate_payout_report(snapshot.payout, &snapshot)?;

    let request = CompactPositionToClaimCheckRequestV1::new(snapshot.payout.request)
        .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheck)?;
    let input = request.input();
    let escrow = ClaimCheckEscrowV1::decode(&snapshot.escrow.data)
        .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheck)?;
    let coordinates = project_claim_check_coordinates_v1(
        &snapshot.payout.route.claims_program,
        &snapshot.aggregate.key,
        &snapshot.payout.owner,
    )
    .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheckOperator)?;
    authenticate_compaction_records(&snapshot, escrow, coordinates)?;

    let deadline = escrow
        .opened_slot
        .checked_add(COMPACTION_DEADLINE_SLOTS_V1)
        .ok_or(ClaimCheckCompactionOperatorErrorV1::Deadline)?;
    if observation.slot < deadline {
        return Err(ClaimCheckCompactionOperatorErrorV1::Deadline);
    }

    let expected_payout = project_wallet_terminal_payout_postcondition_v3(snapshot.payout)
        .map_err(|_| ClaimCheckCompactionOperatorErrorV1::Postcondition)?;
    let hoard_before = TokenAccount::parse(&snapshot.hoard.data)
        .map_err(ClaimCheckCompactionOperatorErrorV1::Token)?;
    let hoard_after = TokenAccount::parse(&expected_payout.hoard_token_bytes)
        .map_err(ClaimCheckCompactionOperatorErrorV1::Token)?;
    let vault_before = TokenAccount::parse(&snapshot.vault.data)
        .map_err(ClaimCheckCompactionOperatorErrorV1::Token)?;
    let vault_after = TokenAccount::parse(&expected_payout.recipient_token_bytes)
        .map_err(ClaimCheckCompactionOperatorErrorV1::Token)?;
    let mints_record = vault_after.amount > vault_before.amount;
    let claim_check_rent = if mints_record {
        rent.minimum_balance(CLAIM_CHECK_BYTES_V1)
    } else {
        0
    };
    let conservation = ClaimCheckCompactionPlanV1::new(ClaimCheckCompactionObservationV1 {
        payout_atoms: snapshot.payout.payout,
        hoard_before: hoard_before.amount,
        hoard_after: hoard_after.amount,
        vault_before: vault_before.amount,
        vault_after: vault_after.amount,
        position: compaction_account(snapshot.position),
        admission: compaction_account(snapshot.admission),
        claim_check: compaction_account(snapshot.claim_check),
        cranker: compaction_account(snapshot.cranker),
        opener: compaction_account(snapshot.opener),
        rent_credit: compaction_account(snapshot.rent_credit),
        claim_check_rent,
        opener_debt: escrow.opener_outlay,
        crank_reward_cap: COMPACTION_CRANK_REWARD_LAMPORTS_V1,
    })
    .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheckConservation)?;

    let mut expected_escrow = ClaimCheckEscrowV1 {
        opener_outlay: conservation.opener_debt_after(),
        ..escrow
    };
    if conservation.mints_claim_check() {
        expected_escrow = expected_escrow
            .admit_claim_check()
            .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheck)?;
    }
    let expected_record = if conservation.mints_claim_check() {
        let seeds = ClaimCheckSeedsV1::new(snapshot.aggregate.key.to_bytes(), input.owner)
            .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheck)?;
        let (_, bump) =
            Pubkey::find_program_address(&seeds.as_slices(), &snapshot.payout.route.claims_program);
        Some(ClaimCheckCompactionExpectedRecordV1 {
            aggregate: snapshot.aggregate.key.to_bytes(),
            owner: input.owner,
            market: escrow.market,
            release_set: escrow.release_set,
            vault: escrow.vault,
            collateral_mint: escrow.collateral_mint,
            position_atoms_digest: hash(&expected_payout.position_bytes).to_bytes(),
            entitlement_atoms: conservation.entitlement_atoms(),
            generation: escrow.generation,
            bump,
        })
    } else {
        None
    };

    let mut accounts = wallet_terminal_payout_account_frame_v3(snapshot.payout);
    accounts[0] = AccountMeta::new(snapshot.cranker.key, true);
    accounts.extend([
        AccountMeta::new(snapshot.escrow.key, false),
        AccountMeta::new(snapshot.claim_check.key, false),
        AccountMeta::new(snapshot.admission.key, false),
        AccountMeta::new(snapshot.rent_credit.key, false),
        AccountMeta::new(snapshot.opener.key, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ]);
    if accounts.len() != CLAIM_CHECK_COMPACTION_ACCOUNT_COUNT_V1 {
        return Err(ClaimCheckCompactionOperatorErrorV1::Binding);
    }
    Ok(ClaimCheckCompactionReportV1 {
        instruction: Instruction {
            program_id: snapshot.payout.route.claims_program,
            accounts,
            data: request
                .to_bytes()
                .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheck)?
                .to_vec(),
        },
        observation,
        required_signer: snapshot.cranker.key,
        payout: snapshot.payout.clone(),
        coordinates,
        payout_request_digest: snapshot.payout.request_digest,
        conservation,
        expected_escrow_bytes: expected_escrow
            .to_bytes()
            .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheck)?
            .to_vec(),
        expected_record,
        expected_vacant: [snapshot.position.key, snapshot.admission.key],
    })
}

/// Solana's transaction compute ceiling declared by native compaction.
///
/// The wrapped payout has market-dependent Product width, and compaction then
/// allocates a record and closes two accounts. There is no priority fee here;
/// a tighter guessed bound would only manufacture a liveness failure.
pub const CLAIM_CHECK_COMPACTION_COMPUTE_UNITS_V1: u32 = 1_400_000;

/// Canonical address-table sequence for one compaction.
pub fn canonical_claim_check_compaction_lookup_addresses_v1(
    report: &ClaimCheckCompactionReportV1,
    payer: Pubkey,
) -> Result<Vec<Pubkey>, ClaimCheckCompactionOperatorErrorV1> {
    if payer == Pubkey::default()
        || report.required_signer == Pubkey::default()
        || payer == report.required_signer
    {
        return Err(ClaimCheckCompactionOperatorErrorV1::Binding);
    }
    let mut addresses = Vec::new();
    for address in core::iter::once(report.instruction.program_id)
        .chain(report.instruction.accounts.iter().map(|meta| meta.pubkey))
    {
        if address != payer && address != report.required_signer && !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    if addresses.is_empty() || addresses.len() > 256 {
        return Err(ClaimCheckCompactionOperatorErrorV1::LookupTable);
    }
    Ok(addresses)
}

/// Packet-safe unsigned compaction transaction and its exact signer set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimCheckCompactionTransactionPlanV1 {
    /// Exact v0 message compiled through one canonical finalized table.
    pub message: VersionedMessagePlanV0,
    /// Fee payer followed by the distinct permissionless cranker.
    pub required_signers: [Pubkey; 2],
    /// Exact compaction report carried by the message.
    pub compaction: ClaimCheckCompactionReportV1,
}

/// Compile one compaction through exactly one finalized canonical lookup table.
///
/// The payer must be distinct from the cranker, opener, and RentCredit. Fees
/// are charged before instruction entry; keeping the payer outside those three
/// sinks makes the conservation report's absolute post-balances exact rather
/// than silently off by the transaction fee.
pub fn compile_claim_check_compaction_v0(
    report: ClaimCheckCompactionReportV1,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_table: &ObservedAccount,
) -> Result<ClaimCheckCompactionTransactionPlanV1, ClaimCheckCompactionOperatorErrorV1> {
    if report.observation.slot == 0 || report.observation.finality != Finality::Finalized {
        return Err(ClaimCheckCompactionOperatorErrorV1::Observation);
    }
    if report.instruction.accounts.len() != CLAIM_CHECK_COMPACTION_ACCOUNT_COUNT_V1 {
        return Err(ClaimCheckCompactionOperatorErrorV1::Binding);
    }
    let opener = report.instruction.accounts[CLAIM_CHECK_COMPACTION_OPENER_ACCOUNT_V1].pubkey;
    let rent_credit =
        report.instruction.accounts[CLAIM_CHECK_COMPACTION_RENT_CREDIT_ACCOUNT_V1].pubkey;
    if payer == opener || payer == rent_credit || payer == report.required_signer {
        return Err(ClaimCheckCompactionOperatorErrorV1::Binding);
    }
    if lookup_table.observation != report.observation
        || lookup_table.owner != lookup_table_program::id()
        || lookup_table.executable
    {
        return Err(ClaimCheckCompactionOperatorErrorV1::LookupTable);
    }
    let expected = canonical_claim_check_compaction_lookup_addresses_v1(&report, payer)?;
    let table = AddressLookupTable::deserialize(&lookup_table.data)
        .map_err(|_| ClaimCheckCompactionOperatorErrorV1::LookupTable)?;
    if table.addresses.as_ref() != expected.as_slice() {
        return Err(ClaimCheckCompactionOperatorErrorV1::LookupTable);
    }
    let instructions = [
        ComputeBudgetInstruction::set_compute_unit_limit(CLAIM_CHECK_COMPACTION_COMPUTE_UNITS_V1),
        report.instruction.clone(),
    ];
    let message = compile_v0_message(
        payer,
        &instructions,
        recent_blockhash,
        report.observation,
        core::slice::from_ref(lookup_table),
    )
    .map_err(ClaimCheckCompactionOperatorErrorV1::Routing)?;
    if message.required_signatures != 2 {
        return Err(ClaimCheckCompactionOperatorErrorV1::Binding);
    }
    Ok(ClaimCheckCompactionTransactionPlanV1 {
        message,
        required_signers: [payer, report.required_signer],
        compaction: report,
    })
}

/// Exact accepted state required to authenticate one compaction.
#[derive(Clone, Copy, Debug)]
pub struct ClaimCheckCompactionPoststateV1<'a> {
    /// Slot reported by the finalized accepted transaction.
    pub accepted_slot: u64,
    /// Claims terminal-payout return data.
    pub terminal_receipt_bytes: &'a [u8],
    /// Claims aggregate after the burn.
    pub aggregate_bytes: &'a [u8],
    /// Claims-role Custody replay after payout.
    pub custody_replay_bytes: &'a [u8],
    /// Hoard token account after payout.
    pub hoard_token_bytes: &'a [u8],
    /// Claim-check vault after payout.
    pub vault_token_bytes: &'a [u8],
    /// Escrow record after debt/count mutation.
    pub escrow: &'a ObservedAccount,
    /// Closed Position, represented as a vacant observation.
    pub position: &'a ObservedAccount,
    /// Closed admission record, represented as a vacant observation.
    pub admission: &'a ObservedAccount,
    /// Minted record or still-vacant zero-payout address.
    pub claim_check: &'a ObservedAccount,
    /// Cranker balance after its instruction credit.
    pub cranker: &'a ObservedAccount,
    /// Opener balance after debt repayment.
    pub opener: &'a ObservedAccount,
    /// RentCredit after residual credit.
    pub rent_credit: &'a ObservedAccount,
}

/// Authenticate every persisted economic effect of an accepted compaction.
pub fn verify_claim_check_compaction_postcondition_v1(
    report: &ClaimCheckCompactionReportV1,
    post: ClaimCheckCompactionPoststateV1<'_>,
) -> Result<(), ClaimCheckCompactionOperatorErrorV1> {
    if report.instruction.accounts.len() != CLAIM_CHECK_COMPACTION_ACCOUNT_COUNT_V1 {
        return Err(ClaimCheckCompactionOperatorErrorV1::Postcondition);
    }
    let expected_payout = project_wallet_terminal_payout_postcondition_v3(&report.payout)
        .map_err(|_| ClaimCheckCompactionOperatorErrorV1::Postcondition)?;
    let accounts = [
        post.escrow,
        post.position,
        post.admission,
        post.claim_check,
        post.cranker,
        post.opener,
        post.rent_credit,
    ];
    let observation = accounts[0].observation;
    if post.accepted_slot == 0
        || post.accepted_slot > observation.slot
        || observation.finality != Finality::Finalized
        || accounts
            .iter()
            .any(|account| account.observation != observation)
        || post.terminal_receipt_bytes != expected_payout.receipt_bytes
        || post.aggregate_bytes != expected_payout.aggregate_bytes
        || post.custody_replay_bytes != expected_payout.custody_replay_bytes
        || post.hoard_token_bytes != expected_payout.hoard_token_bytes
        || post.vault_token_bytes != expected_payout.recipient_token_bytes
        || post.escrow.key != report.coordinates.escrow
        || post.escrow.owner != report.payout.route.claims_program
        || post.escrow.executable
        || post.escrow.data != report.expected_escrow_bytes
        || post.position.key != report.expected_vacant[0]
        || post.position.owner != system_program::ID
        || post.position.lamports != 0
        || post.position.executable
        || !post.position.data.is_empty()
        || post.admission.key != report.expected_vacant[1]
        || post.admission.owner != system_program::ID
        || post.admission.lamports != 0
        || post.admission.executable
        || !post.admission.data.is_empty()
        || post.claim_check.key != report.coordinates.record
        || post.cranker.key != report.required_signer
        || post.opener.key
            != report.instruction.accounts[CLAIM_CHECK_COMPACTION_OPENER_ACCOUNT_V1].pubkey
        || post.rent_credit.key
            != report.instruction.accounts[CLAIM_CHECK_COMPACTION_RENT_CREDIT_ACCOUNT_V1].pubkey
    {
        return Err(ClaimCheckCompactionOperatorErrorV1::Postcondition);
    }
    match report.expected_record {
        Some(expected) => {
            let bytes = expected
                .at_slot(post.accepted_slot)?
                .to_bytes()
                .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheck)?;
            if post.claim_check.owner != report.payout.route.claims_program
                || post.claim_check.executable
                || post.claim_check.data != bytes
            {
                return Err(ClaimCheckCompactionOperatorErrorV1::Postcondition);
            }
        }
        None => {
            if post.claim_check.owner != system_program::ID
                || post.claim_check.executable
                || !post.claim_check.data.is_empty()
            {
                return Err(ClaimCheckCompactionOperatorErrorV1::Postcondition);
            }
        }
    }
    let hoard = TokenAccount::parse(post.hoard_token_bytes)
        .map_err(ClaimCheckCompactionOperatorErrorV1::Token)?;
    let vault = TokenAccount::parse(post.vault_token_bytes)
        .map_err(ClaimCheckCompactionOperatorErrorV1::Token)?;
    report
        .conservation
        .validate_post(ClaimCheckCompactionPostV1 {
            position_lamports: post.position.lamports,
            admission_lamports: post.admission.lamports,
            claim_check_lamports: post.claim_check.lamports,
            cranker_lamports: post.cranker.lamports,
            opener_lamports: post.opener.lamports,
            rent_credit_lamports: post.rent_credit.lamports,
            hoard_lamports_of_collateral: hoard.amount,
            vault_lamports_of_collateral: vault.amount,
        })
        .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheckConservation)
}

fn same_compaction_observation(
    snapshot: &ClaimCheckCompactionSnapshotV1<'_>,
) -> Result<Observation, ClaimCheckCompactionOperatorErrorV1> {
    let observation = snapshot.payout.observation;
    let accounts = [
        snapshot.aggregate,
        snapshot.position,
        snapshot.escrow,
        snapshot.vault,
        snapshot.hoard,
        snapshot.claim_check,
        snapshot.admission,
        snapshot.rent_credit,
        snapshot.cranker,
        snapshot.opener,
        snapshot.rent_sysvar,
        snapshot.system_program,
    ];
    if observation.slot == 0
        || observation.finality != Finality::Finalized
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(ClaimCheckCompactionOperatorErrorV1::Observation);
    }
    Ok(observation)
}

fn authenticate_compaction_infrastructure(
    snapshot: &ClaimCheckCompactionSnapshotV1<'_>,
) -> Result<(), ClaimCheckCompactionOperatorErrorV1> {
    if snapshot.system_program.key != system_program::ID
        || snapshot.system_program.owner != native_loader::ID
        || !snapshot.system_program.executable
        || !snapshot.system_program.data.is_empty()
        || snapshot.cranker.key == Pubkey::default()
        || snapshot.cranker.owner != system_program::ID
        || snapshot.cranker.executable
        || !snapshot.cranker.data.is_empty()
        || snapshot.opener.key == Pubkey::default()
        || snapshot.opener.owner != system_program::ID
        || snapshot.opener.executable
        || !snapshot.opener.data.is_empty()
    {
        return Err(ClaimCheckCompactionOperatorErrorV1::Binding);
    }
    for (left, right) in [
        (snapshot.cranker, snapshot.opener),
        (snapshot.cranker, snapshot.rent_credit),
        (snapshot.opener, snapshot.rent_credit),
    ] {
        if left.key == right.key
            && (left.lamports != right.lamports
                || left.owner != right.owner
                || left.data != right.data)
        {
            return Err(ClaimCheckCompactionOperatorErrorV1::Observation);
        }
    }
    Ok(())
}

fn authenticate_payout_report(
    report: &WalletTerminalPayoutReportV3,
    snapshot: &ClaimCheckCompactionSnapshotV1<'_>,
) -> Result<(), ClaimCheckCompactionOperatorErrorV1> {
    let request = report.request.input();
    let frame = wallet_terminal_payout_account_frame_v3(report);
    let request_bytes = report.request.to_bytes();
    if report.instruction.program_id != report.route.claims_program
        || report.instruction.accounts != frame
        || report.instruction.data != request_bytes
        || report.request_digest != hash(&request_bytes).to_bytes()
        || report.owner.to_bytes() != request.owner
        || report.route.claims_program.to_bytes() != request.claims_program
        || report.route.custody_program.to_bytes() != request.custody_program
        || report.route.position.to_bytes() != request.position
        || report.route.recipient.to_bytes() != request.recipient_token_account
        || report.route.collateral_mint.to_bytes() != request.collateral_mint
        || report.route.token_program.to_bytes() != request.token_program
        || report.route.aggregate != snapshot.aggregate.key
        || report.route.position != snapshot.position.key
        || report.route.hoard != snapshot.hoard.key
        || report.route.recipient != snapshot.vault.key
        || report.pre_aggregate_bytes != snapshot.aggregate.data
        || report.pre_position_bytes != snapshot.position.data
        || report.pre_hoard_token_bytes != snapshot.hoard.data
        || report.pre_recipient_token_bytes != snapshot.vault.data
    {
        return Err(ClaimCheckCompactionOperatorErrorV1::Payout);
    }
    Ok(())
}

fn authenticate_compaction_records(
    snapshot: &ClaimCheckCompactionSnapshotV1<'_>,
    escrow: ClaimCheckEscrowV1,
    coordinates: ClaimCheckCoordinatesV1,
) -> Result<(), ClaimCheckCompactionOperatorErrorV1> {
    let report = snapshot.payout;
    let request = report.request.input();
    let admission = ProtocolPositionAdmissionV2::decode(&snapshot.admission.data)
        .map_err(ClaimCheckCompactionOperatorErrorV1::ProtocolPosition)?;
    let admission_seeds =
        ProtocolPositionAdmissionSeedsV2::new(snapshot.aggregate.key.to_bytes(), request.owner)
            .map_err(ClaimCheckCompactionOperatorErrorV1::ProtocolPosition)?;
    let expected_admission =
        Pubkey::find_program_address(&admission_seeds.as_slices(), &report.route.claims_program).0;
    let escrow_bump = Pubkey::find_program_address(
        &ClaimCheckEscrowSeedsV1::new(snapshot.aggregate.key.to_bytes())
            .map_err(ClaimCheckCompactionOperatorErrorV1::ClaimCheck)?
            .as_slices(),
        &report.route.claims_program,
    );
    let vault = TokenAccount::parse(&snapshot.vault.data)
        .map_err(ClaimCheckCompactionOperatorErrorV1::Token)?;
    let hoard = TokenAccount::parse(&snapshot.hoard.data)
        .map_err(ClaimCheckCompactionOperatorErrorV1::Token)?;
    if snapshot.aggregate.owner != report.route.claims_program
        || snapshot.aggregate.executable
        || snapshot.aggregate.data.is_empty()
        || snapshot.position.owner != report.route.claims_program
        || snapshot.position.executable
        || snapshot.escrow.key != coordinates.escrow
        || snapshot.escrow.owner != report.route.claims_program
        || snapshot.escrow.executable
        || snapshot.vault.key != coordinates.vault
        || snapshot.vault.owner != report.route.token_program
        || snapshot.vault.executable
        || snapshot.hoard.owner != report.route.token_program
        || snapshot.hoard.executable
        || snapshot.claim_check.key != coordinates.record
        || snapshot.claim_check.owner != system_program::ID
        || snapshot.claim_check.executable
        || !snapshot.claim_check.data.is_empty()
        || snapshot.admission.key != expected_admission
        || snapshot.admission.owner != report.route.claims_program
        || snapshot.admission.executable
        || snapshot.rent_credit.key.to_bytes() != admission.rent_credit()
        || snapshot.rent_credit.owner.to_bytes() != admission.rent_program()
        || snapshot.rent_credit.executable
        || snapshot.opener.key.to_bytes() != escrow.opener
        || escrow.aggregate != snapshot.aggregate.key.to_bytes()
        || escrow.market != request.market
        || escrow.release_set != request.release_set
        || escrow.vault != snapshot.vault.key.to_bytes()
        || escrow.collateral_mint != request.collateral_mint
        || escrow.generation != request.generation
        || escrow.bump != escrow_bump.1
        || admission.owner_kind() != ProtocolPositionOwnerKindV2::User
        || admission.position_owner() != request.owner
        || admission.market() != request.market
        || admission.release_set() != request.release_set
        || admission.generation() != request.generation
        || admission.claims_program() != request.claims_program
        || vault.owner != snapshot.escrow.key.to_bytes()
        || vault.mint != request.collateral_mint
        || hoard.mint != request.collateral_mint
    {
        return if admission.owner_kind() == ProtocolPositionOwnerKindV2::User {
            Err(ClaimCheckCompactionOperatorErrorV1::Binding)
        } else {
            Err(ClaimCheckCompactionOperatorErrorV1::Scope)
        };
    }
    Ok(())
}

fn compaction_account(account: &ObservedAccount) -> ClaimCheckAccountObservationV1 {
    ClaimCheckAccountObservationV1 {
        identity: account.key.to_bytes(),
        lamports: account.lamports,
    }
}

/// Where one compacted Fractional coordinate survives market retirement.
///
/// Unlike a native claim-check, the record is addressed by its shard Mint, not
/// by a wallet. Any present or future shard holder therefore derives the same
/// durable collateral record without an index and proves entitlement by
/// signing for the shard account they actually present.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckCoordinatesV1 {
    /// Claims aggregate the Fractional coordinate was admitted against.
    pub aggregate: Pubkey,
    /// Token-2022 shard Mint whose live supply is the outstanding claim.
    pub shard_mint: Pubkey,
    /// Durable per-Mint claim-check record.
    pub record: Pubkey,
    /// Per-market claim-check escrow and PermissionedBurn authority.
    pub escrow: Pubkey,
    /// Escrow-owned collateral token account.
    pub vault: Pubkey,
}

/// Derive the complete post-retirement Fractional address set offline.
pub fn project_fractional_claim_check_coordinates_v1(
    claims_program: &Pubkey,
    aggregate: &Pubkey,
    shard_mint: &Pubkey,
) -> Result<FractionalClaimCheckCoordinatesV1, ClaimCheckOperatorErrorV1> {
    let record_seeds =
        FractionalClaimCheckSeedsV1::new(aggregate.to_bytes(), shard_mint.to_bytes())
            .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
    let escrow_seeds = ClaimCheckEscrowSeedsV1::new(aggregate.to_bytes())
        .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
    let vault_seeds = ClaimCheckVaultSeedsV1::new(aggregate.to_bytes())
        .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
    Ok(FractionalClaimCheckCoordinatesV1 {
        aggregate: *aggregate,
        shard_mint: *shard_mint,
        record: Pubkey::find_program_address(&record_seeds.as_slices(), claims_program).0,
        escrow: Pubkey::find_program_address(&escrow_seeds.as_slices(), claims_program).0,
        vault: Pubkey::find_program_address(&vault_seeds.as_slices(), claims_program).0,
    })
}

/// Durable Fractional entitlement facts a holder can inspect before signing.
///
/// Construction is intentionally private to
/// [`read_fractional_claim_check_statement_v1`]. A caller cannot manufacture a
/// statement that omits the record/escrow join and then ask the builder to
/// present it as chain-derived.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckStatementV1 {
    /// Offline-derivable record, escrow, and vault coordinates.
    pub coordinates: FractionalClaimCheckCoordinatesV1,
    /// Logical market identity retained by the durable record.
    pub market: Pubkey,
    /// Collateral Mint the vault pays.
    pub collateral_mint: Pubkey,
    /// Shard atoms that form one whole Claims coordinate.
    pub denominator: u64,
    /// Collateral atoms paid per whole Claims coordinate.
    pub payout_per_claim: u64,
    /// Collateral atoms still promised by this record.
    pub remaining_escrowed_atoms: u64,
    /// Shard supply observed at compaction, for audit rather than authorization.
    pub compacted_shard_supply: u64,
    /// Slot at which compaction resolved the coordinate.
    pub compacted_slot: u64,
    /// Live record lamports returned only by the settling redemption.
    pub recoverable_lamports: u64,
    /// Live records the shared escrow still serves, including this one.
    pub outstanding_claim_checks: u64,
    record: FractionalClaimCheckV1,
}

/// Decode and join one Fractional record with its shared escrow.
///
/// The record and escrow addresses are re-derived, and aggregate, market,
/// release, vault, collateral Mint, and generation must agree. The resulting
/// statement needs no live Market, Trading, capability-root, Registry, or
/// indexer account.
pub fn read_fractional_claim_check_statement_v1(
    claims_program: &Pubkey,
    record_account: &Pubkey,
    record_bytes: &[u8],
    record_lamports: u64,
    escrow_account: &Pubkey,
    escrow_bytes: &[u8],
) -> Result<FractionalClaimCheckStatementV1, ClaimCheckOperatorErrorV1> {
    let record = FractionalClaimCheckV1::decode(record_bytes)
        .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
    let escrow =
        ClaimCheckEscrowV1::decode(escrow_bytes).map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
    let coordinates = project_fractional_claim_check_coordinates_v1(
        claims_program,
        &Pubkey::new_from_array(record.aggregate),
        &Pubkey::new_from_array(record.shard_mint),
    )?;
    if coordinates.record != *record_account || coordinates.escrow != *escrow_account {
        return Err(ClaimCheckOperatorErrorV1::Address);
    }
    if coordinates.vault.to_bytes() != record.vault
        || record.aggregate != escrow.aggregate
        || record.market != escrow.market
        || record.release_set != escrow.release_set
        || record.vault != escrow.vault
        || record.collateral_mint != escrow.collateral_mint
        || record.generation != escrow.generation
        || escrow.outstanding_claim_checks == 0
    {
        return Err(ClaimCheckOperatorErrorV1::Binding);
    }
    Ok(FractionalClaimCheckStatementV1 {
        coordinates,
        market: Pubkey::new_from_array(record.market),
        collateral_mint: Pubkey::new_from_array(record.collateral_mint),
        denominator: record.denominator,
        payout_per_claim: record.payout_per_claim,
        remaining_escrowed_atoms: record.escrowed_atoms,
        compacted_shard_supply: record.compacted_shard_supply,
        compacted_slot: record.compacted_slot,
        recoverable_lamports: record_lamports,
        outstanding_claim_checks: escrow.outstanding_claim_checks,
        record,
    })
}

/// Live balances the operator reads immediately before constructing a burn.
///
/// They remain an untrusted projection: the program re-reads every one. Their
/// purpose here is to give the holder an exact consequence report and to refuse
/// an obviously stale or impossible request before wallet approval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckBalancesV1 {
    /// Shard atoms in the holder-owned Token-2022 account.
    pub holder_shard_atoms: u64,
    /// Current shard Mint supply.
    pub shard_mint_supply: u64,
    /// Collateral atoms in the escrow vault.
    pub vault_collateral_atoms: u64,
    /// Collateral atoms already in the holder-owned account.
    pub holder_collateral_atoms: u64,
    /// Current lamports at the holder wallet.
    pub holder_lamports: u64,
}

/// Unsigned Fractional burn/pay construction and its exact consequences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckRedemptionReportV1 {
    /// Exact 9-role unsigned instruction; only the holder signs.
    pub instruction: Instruction,
    /// Chain-derived durable facts shown to the holder.
    pub statement: FractionalClaimCheckStatementV1,
    /// Exact conservation plan also executed by Claims.
    pub plan: FractionalClaimCheckRedemptionPlanV1,
    /// Shard atoms the request presents at its one floor-division boundary.
    pub requested_shard_atoms: u64,
    /// Exact shard atoms PermissionedBurn consumes.
    pub expected_shard_burn: u64,
    /// Exact vault debit, before any collateral-Mint transfer fee.
    pub expected_vault_debit: u64,
    /// Most collateral atoms the holder account can gain.
    pub holder_credit_ceiling: u64,
    /// Whether this burn closes the record and decrements escrow outstanding.
    pub settles_record: bool,
    /// Record address expected to be absent, only when settling.
    pub expected_vacant_record: Option<Pubkey>,
    /// Lamports credited to the holder, zero on a partial redemption.
    pub expected_lamport_credit: u64,
    /// Shared-escrow outstanding count after a successful transaction.
    pub expected_escrow_outstanding: u64,
}

/// Build one holder-signed Fractional claim-check burn/pay instruction.
///
/// The frame comes from [`FractionalClaimCheckRedemptionRoleV1`], the request
/// carries only aggregate, shard Mint, and requested atoms, and all economic
/// consequences come from [`FractionalClaimCheckRedemptionPlanV1`]. This
/// function neither reconstructs a payoff curve nor invents a second rounding
/// rule.
#[allow(clippy::too_many_arguments)]
pub fn build_fractional_claim_check_redemption_v1(
    claims_program: &Pubkey,
    token_program: &Pubkey,
    holder: &Pubkey,
    holder_collateral_account: &Pubkey,
    holder_shard_account: &Pubkey,
    requested_shard_atoms: u64,
    balances: FractionalClaimCheckBalancesV1,
    statement: FractionalClaimCheckStatementV1,
) -> Result<FractionalClaimCheckRedemptionReportV1, ClaimCheckOperatorErrorV1> {
    if token_program.to_bytes() != dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID {
        return Err(ClaimCheckOperatorErrorV1::Binding);
    }
    let coordinates = statement.coordinates;
    let plan =
        FractionalClaimCheckRedemptionPlanV1::new(FractionalClaimCheckRedemptionObservationV1 {
            record: statement.record,
            shard_atoms: requested_shard_atoms,
            holder_shards_before: balances.holder_shard_atoms,
            shard_supply_before: balances.shard_mint_supply,
            vault_before: balances.vault_collateral_atoms,
            holder_collateral_before: balances.holder_collateral_atoms,
            record_lamports: statement.recoverable_lamports,
            holder_lamports_before: balances.holder_lamports,
        })
        .map_err(ClaimCheckOperatorErrorV1::FractionalClaimCheckConservation)?;
    let request = FractionalRedeemClaimCheckRequestV1 {
        aggregate: coordinates.aggregate.to_bytes(),
        shard_mint: coordinates.shard_mint.to_bytes(),
        requested_shard_atoms,
    }
    .new()
    .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
    let addresses = [
        *holder,
        coordinates.record,
        coordinates.escrow,
        coordinates.vault,
        *holder_collateral_account,
        statement.collateral_mint,
        coordinates.shard_mint,
        *holder_shard_account,
        *token_program,
    ];
    if addresses
        .iter()
        .any(|address| *address == Pubkey::default())
        || addresses.iter().enumerate().any(|(index, left)| {
            addresses
                .iter()
                .skip(index.saturating_add(1))
                .any(|right| right == left)
        })
    {
        return Err(ClaimCheckOperatorErrorV1::Coordinate);
    }
    let accounts = FractionalClaimCheckRedemptionRoleV1::frame()
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
    if accounts.len() != FRACTIONAL_CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1 {
        return Err(ClaimCheckOperatorErrorV1::Coordinate);
    }
    let expected_lamport_credit = if plan.settles() {
        statement.recoverable_lamports
    } else {
        0
    };
    let expected_escrow_outstanding = statement
        .outstanding_claim_checks
        .checked_sub(u64::from(plan.settles()))
        .ok_or(ClaimCheckOperatorErrorV1::Conservation)?;
    Ok(FractionalClaimCheckRedemptionReportV1 {
        instruction: Instruction {
            program_id: *claims_program,
            accounts,
            data: request
                .to_bytes()
                .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?
                .to_vec(),
        },
        statement,
        plan,
        requested_shard_atoms,
        expected_shard_burn: plan.consumed_shards(),
        expected_vault_debit: plan.collateral_atoms(),
        holder_credit_ceiling: balances
            .holder_collateral_atoms
            .checked_add(plan.collateral_atoms())
            .ok_or(ClaimCheckOperatorErrorV1::Conservation)?,
        settles_record: plan.settles(),
        expected_vacant_record: plan.settles().then_some(coordinates.record),
        expected_lamport_credit,
        expected_escrow_outstanding,
    })
}

/// Whether a market's escrow can be closed, and by whom.
///
/// Anyone may close a settled escrow and keep the rent. An escrow still owing
/// somebody cannot be closed at all, and that is the ruling working as
/// intended: the claim survives, so the collateral has to be somewhere.
pub fn escrow_is_closeable_v1(escrow_bytes: &[u8]) -> Result<bool, ClaimCheckOperatorErrorV1> {
    Ok(ClaimCheckEscrowV1::decode(escrow_bytes)
        .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?
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
    .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?;
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
            .map_err(ClaimCheckOperatorErrorV1::ClaimCheck)?
            .to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use dclutch_claims::protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionAdmissionEvidenceV2, ProtocolPositionPresenceV2,
        ProtocolPositionRequestV2,
    };
    use solana_address_lookup_table_interface::state::LookupTableMeta;
    use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::SysvarSerialize};
    use solana_sdk_ids::sysvar;

    use crate::wallet_terminal_payout_v3::tests::{test_report, test_report_for_recipient};

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

    struct CompactionFixture {
        report: WalletTerminalPayoutReportV3,
        aggregate: ObservedAccount,
        position: ObservedAccount,
        escrow: ObservedAccount,
        vault: ObservedAccount,
        hoard: ObservedAccount,
        claim_check: ObservedAccount,
        admission: ObservedAccount,
        rent_credit: ObservedAccount,
        cranker: ObservedAccount,
        opener: ObservedAccount,
        rent_sysvar: ObservedAccount,
        system_program: ObservedAccount,
    }

    impl CompactionFixture {
        fn snapshot(&self) -> ClaimCheckCompactionSnapshotV1<'_> {
            ClaimCheckCompactionSnapshotV1 {
                payout: &self.report,
                aggregate: &self.aggregate,
                position: &self.position,
                escrow: &self.escrow,
                vault: &self.vault,
                hoard: &self.hoard,
                claim_check: &self.claim_check,
                admission: &self.admission,
                rent_credit: &self.rent_credit,
                cranker: &self.cranker,
                opener: &self.opener,
                rent_sysvar: &self.rent_sysvar,
                system_program: &self.system_program,
            }
        }
    }

    fn observed(
        observation: Observation,
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
        data: Vec<u8>,
    ) -> ObservedAccount {
        ObservedAccount {
            observation,
            key,
            owner,
            lamports,
            executable,
            data,
        }
    }

    fn rent_account(observation: Observation, rent: &Rent) -> ObservedAccount {
        let mut lamports = 1;
        let mut data = vec![0; Rent::size_of()];
        let key = sysvar::rent::ID;
        let owner = sysvar::ID;
        let mut info =
            AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        assert_eq!(rent.clone().to_account_info(&mut info), Some(()));
        observed(observation, key, owner, lamports, false, data)
    }

    fn compaction_fixture(
        owner_kind: ProtocolPositionOwnerKindV2,
        deadline_elapsed: bool,
    ) -> CompactionFixture {
        let seed_report = test_report(1);
        let coordinates = project_claim_check_coordinates_v1(
            &seed_report.route.claims_program,
            &seed_report.route.aggregate,
            &seed_report.owner,
        )
        .expect("coordinates");
        let mut report = test_report_for_recipient(1, coordinates.vault, coordinates.escrow);
        report.observation.slot = if deadline_elapsed {
            COMPACTION_DEADLINE_SLOTS_V1 + 200
        } else {
            COMPACTION_DEADLINE_SLOTS_V1
        };
        let observation = report.observation;
        let input = report.request.input();
        let opener = Pubkey::new_from_array([201; 32]);
        let cranker = Pubkey::new_from_array([202; 32]);
        let rent_credit = Pubkey::new_from_array([203; 32]);
        let rent_program = Pubkey::new_from_array([204; 32]);
        let trading_program = Pubkey::new_from_array([205; 32]);
        let escrow_bump = Pubkey::find_program_address(
            &ClaimCheckEscrowSeedsV1::new(report.route.aggregate.to_bytes())
                .expect("escrow seeds")
                .as_slices(),
            &report.route.claims_program,
        )
        .1;
        let escrow = ClaimCheckEscrowV1 {
            aggregate: report.route.aggregate.to_bytes(),
            market: input.market,
            release_set: input.release_set,
            vault: coordinates.vault.to_bytes(),
            collateral_mint: input.collateral_mint,
            opener: opener.to_bytes(),
            opened_slot: 100,
            opener_outlay: 4_700_000,
            outstanding_claim_checks: 0,
            generation: input.generation,
            bump: escrow_bump,
        }
        .new()
        .expect("escrow");
        let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(
            report.route.aggregate.to_bytes(),
            report.owner.to_bytes(),
        )
        .expect("admission seeds");
        let admission_key = Pubkey::find_program_address(
            &admission_seeds.as_slices(),
            &report.route.claims_program,
        )
        .0;
        let admission_request = ProtocolPositionRequestV2::new(ProtocolPositionRequestV2 {
            action: ProtocolPositionActionV2::Admit,
            owner_kind,
            presence: ProtocolPositionPresenceV2::Vacant,
            release_set: input.release_set,
            market: input.market,
            position_owner: input.owner,
            parent_request_digest: [206; 32],
            rent_credit: rent_credit.to_bytes(),
            rent_program: rent_program.to_bytes(),
            generation: input.generation,
            expected_market_revision: input.expected_market_revision,
            expected_position_revision: 0,
            observed_position_lamports: 4_000_000,
            observed_admission_lamports: 4_000_000,
            position_rent_principal: 3_000_000,
            admission_rent_principal: 3_000_000,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        })
        .expect("admission request");
        let admission = ProtocolPositionAdmissionV2::new(
            admission_request,
            ProtocolPositionAdmissionEvidenceV2 {
                product_record_digest: [207; 32],
                semantic_basis_id: input.semantic_basis_id,
                linked_basis_record_digest: input.linked_basis_record_digest,
                request_digest: [208; 32],
                claims_program: input.claims_program,
                trading_program: trading_program.to_bytes(),
                capability_descriptor: [0; 32],
                capability_outcome: 0,
                outcome_count: 3,
            },
        )
        .expect("admission");
        let rent = Rent::default();
        CompactionFixture {
            aggregate: observed(
                observation,
                report.route.aggregate,
                report.route.claims_program,
                5_000_000,
                false,
                report.pre_aggregate_bytes.clone(),
            ),
            position: observed(
                observation,
                report.route.position,
                report.route.claims_program,
                4_000_000,
                false,
                report.pre_position_bytes.clone(),
            ),
            escrow: observed(
                observation,
                coordinates.escrow,
                report.route.claims_program,
                3_000_000,
                false,
                escrow.to_bytes().expect("escrow bytes").to_vec(),
            ),
            vault: observed(
                observation,
                coordinates.vault,
                report.route.token_program,
                2_000_000,
                false,
                report.pre_recipient_token_bytes.clone(),
            ),
            hoard: observed(
                observation,
                report.route.hoard,
                report.route.token_program,
                2_000_000,
                false,
                report.pre_hoard_token_bytes.clone(),
            ),
            claim_check: observed(
                observation,
                coordinates.record,
                system_program::ID,
                1,
                false,
                Vec::new(),
            ),
            admission: observed(
                observation,
                admission_key,
                report.route.claims_program,
                4_000_000,
                false,
                admission
                    .to_state_bytes()
                    .expect("admission bytes")
                    .to_vec(),
            ),
            rent_credit: observed(
                observation,
                rent_credit,
                rent_program,
                900_000,
                false,
                vec![1],
            ),
            cranker: observed(
                observation,
                cranker,
                system_program::ID,
                10_000_000,
                false,
                Vec::new(),
            ),
            opener: observed(
                observation,
                opener,
                system_program::ID,
                20_000_000,
                false,
                Vec::new(),
            ),
            rent_sysvar: rent_account(observation, &rent),
            system_program: observed(
                observation,
                system_program::ID,
                native_loader::ID,
                1,
                true,
                Vec::new(),
            ),
            report,
        }
    }

    fn compaction_lookup(report: &ClaimCheckCompactionReportV1, payer: Pubkey) -> ObservedAccount {
        let addresses = canonical_claim_check_compaction_lookup_addresses_v1(report, payer)
            .expect("canonical compaction lookup");
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(Pubkey::new_from_array([231; 32])),
                last_extended_slot: report.observation.slot - 1,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        observed(
            report.observation,
            Pubkey::new_from_array([232; 32]),
            lookup_table_program::id(),
            4_000_000,
            false,
            table.serialize_for_tests().expect("lookup bytes"),
        )
    }

    fn fractional_statement(
        escrowed_atoms: u64,
        outstanding: u64,
    ) -> FractionalClaimCheckStatementV1 {
        let aggregate = Pubkey::new_from_array([1; 32]);
        let shard_mint = Pubkey::new_from_array([10; 32]);
        let coordinates =
            project_fractional_claim_check_coordinates_v1(&claims(), &aggregate, &shard_mint)
                .expect("fractional coordinates");
        let record_bump = Pubkey::find_program_address(
            &FractionalClaimCheckSeedsV1::new(aggregate.to_bytes(), shard_mint.to_bytes())
                .expect("record seeds")
                .as_slices(),
            &claims(),
        )
        .1;
        let escrow_bump = Pubkey::find_program_address(
            &ClaimCheckEscrowSeedsV1::new(aggregate.to_bytes())
                .expect("escrow seeds")
                .as_slices(),
            &claims(),
        )
        .1;
        let record = FractionalClaimCheckV1 {
            aggregate: aggregate.to_bytes(),
            shard_mint: shard_mint.to_bytes(),
            market: [3; 32],
            release_set: [4; 32],
            vault: coordinates.vault.to_bytes(),
            collateral_mint: [6; 32],
            position_atoms_digest: [7; 32],
            escrowed_atoms,
            denominator: 10,
            payout_per_claim: 4,
            compacted_shard_supply: 70,
            compacted_slot: 12_345,
            generation: 9,
            representation_coordinate: 1,
            bump: record_bump,
        }
        .new()
        .expect("fractional record");
        let escrow = ClaimCheckEscrowV1 {
            aggregate: aggregate.to_bytes(),
            market: [3; 32],
            release_set: [4; 32],
            vault: coordinates.vault.to_bytes(),
            collateral_mint: [6; 32],
            opener: [8; 32],
            opened_slot: 12_000,
            opener_outlay: 2_672_640,
            outstanding_claim_checks: outstanding,
            generation: 9,
            bump: escrow_bump,
        }
        .new()
        .expect("escrow");
        read_fractional_claim_check_statement_v1(
            &claims(),
            &coordinates.record,
            &record.to_bytes().expect("record bytes"),
            3_118_080,
            &coordinates.escrow,
            &escrow.to_bytes().expect("escrow bytes"),
        )
        .expect("fractional statement")
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
    fn permissionless_compaction_wraps_the_exact_payout_and_conservation_owners() {
        let fixture = compaction_fixture(ProtocolPositionOwnerKindV2::User, true);
        let report = build_claim_check_compaction_v1(fixture.snapshot()).expect("compaction");
        assert_eq!(
            report.instruction.accounts.len(),
            CLAIM_CHECK_COMPACTION_ACCOUNT_COUNT_V1
        );
        let signers = report
            .instruction
            .accounts
            .iter()
            .filter(|account| account.is_signer)
            .collect::<Vec<_>>();
        assert_eq!(signers.len(), 1);
        assert_eq!(signers[0].pubkey, fixture.cranker.key);
        let wrapped = CompactPositionToClaimCheckRequestV1::decode(&report.instruction.data)
            .expect("wrapped request");
        assert_eq!(wrapped.settlement(), fixture.report.request);
        assert_eq!(
            report.instruction.accounts[..TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3],
            {
                let mut frame = wallet_terminal_payout_account_frame_v3(&fixture.report);
                frame[0] = AccountMeta::new(fixture.cranker.key, true);
                frame
            }
        );
        assert_eq!(
            report.conservation.entitlement_atoms(),
            fixture.report.payout
        );
        assert_eq!(
            report.conservation.crank_reward(),
            COMPACTION_CRANK_REWARD_LAMPORTS_V1
        );
        let record = report.expected_record.expect("positive record");
        assert_eq!(record.entitlement_atoms, fixture.report.payout);
        assert_eq!(record.owner, fixture.report.owner.to_bytes());
        assert_eq!(
            record
                .at_slot(fixture.report.observation.slot)
                .expect("accepted record")
                .compacted_slot,
            fixture.report.observation.slot
        );
        let escrow =
            ClaimCheckEscrowV1::decode(&report.expected_escrow_bytes).expect("post escrow");
        assert_eq!(escrow.outstanding_claim_checks, 1);
        assert_eq!(
            escrow.opener_outlay,
            report.conservation.opener_debt_after()
        );
        assert_eq!(
            report.expected_vacant,
            [fixture.position.key, fixture.admission.key]
        );
    }

    #[test]
    fn compaction_v0_uses_one_exact_table_and_separates_fee_from_conservation() {
        let fixture = compaction_fixture(ProtocolPositionOwnerKindV2::User, true);
        let report = build_claim_check_compaction_v1(fixture.snapshot()).expect("compaction");
        let payer = Pubkey::new_from_array([230; 32]);
        let table = compaction_lookup(&report, payer);
        let transaction = compile_claim_check_compaction_v0(
            report.clone(),
            payer,
            Hash::new_from_array([233; 32]),
            &table,
        )
        .expect("packet-safe compaction");
        assert_eq!(transaction.required_signers, [payer, fixture.cranker.key]);
        assert_eq!(transaction.message.required_signatures, 2);
        assert!(transaction.message.loaded_addresses > 0);
        assert!(transaction.message.wire_bytes <= crate::versioned::PACKET_DATA_BYTES);

        assert_eq!(
            compile_claim_check_compaction_v0(
                report.clone(),
                fixture.cranker.key,
                Hash::new_from_array([233; 32]),
                &table,
            ),
            Err(ClaimCheckCompactionOperatorErrorV1::Binding)
        );
        let mut reordered = table;
        let decoded = AddressLookupTable::deserialize(&reordered.data).expect("table");
        let mut addresses = decoded.addresses.into_owned();
        addresses.swap(0, 1);
        reordered.data = AddressLookupTable {
            meta: decoded.meta,
            addresses: Cow::Owned(addresses),
        }
        .serialize_for_tests()
        .expect("reordered table");
        assert_eq!(
            compile_claim_check_compaction_v0(
                report,
                payer,
                Hash::new_from_array([233; 32]),
                &reordered,
            ),
            Err(ClaimCheckCompactionOperatorErrorV1::LookupTable)
        );
    }

    #[test]
    fn compaction_poststate_joins_terminal_effects_record_closes_and_conservation() {
        let fixture = compaction_fixture(ProtocolPositionOwnerKindV2::User, true);
        let report = build_claim_check_compaction_v1(fixture.snapshot()).expect("compaction");
        let payout = project_wallet_terminal_payout_postcondition_v3(&fixture.report)
            .expect("payout poststate");
        let observation = Observation {
            slot: fixture.report.observation.slot + 1,
            ..fixture.report.observation
        };
        let accepted_slot = fixture.report.observation.slot;
        let record = report.expected_record.expect("positive record");
        let claim_check = observed(
            observation,
            report.coordinates.record,
            fixture.report.route.claims_program,
            fixture
                .claim_check
                .lamports
                .checked_add(report.conservation.claim_check_top_up())
                .expect("claim-check lamports"),
            false,
            record
                .at_slot(accepted_slot)
                .expect("record")
                .to_bytes()
                .expect("record bytes")
                .to_vec(),
        );
        let position = observed(
            observation,
            fixture.position.key,
            system_program::ID,
            0,
            false,
            Vec::new(),
        );
        let admission = observed(
            observation,
            fixture.admission.key,
            system_program::ID,
            0,
            false,
            Vec::new(),
        );
        let escrow = observed(
            observation,
            fixture.escrow.key,
            fixture.escrow.owner,
            fixture.escrow.lamports,
            false,
            report.expected_escrow_bytes.clone(),
        );
        let cranker = observed(
            observation,
            fixture.cranker.key,
            fixture.cranker.owner,
            fixture.cranker.lamports + report.conservation.crank_reward(),
            false,
            Vec::new(),
        );
        let opener = observed(
            observation,
            fixture.opener.key,
            fixture.opener.owner,
            fixture.opener.lamports + report.conservation.opener_repayment(),
            false,
            Vec::new(),
        );
        let rent_credit = observed(
            observation,
            fixture.rent_credit.key,
            fixture.rent_credit.owner,
            fixture.rent_credit.lamports + report.conservation.rent_credit_residue(),
            false,
            fixture.rent_credit.data.clone(),
        );
        let post = ClaimCheckCompactionPoststateV1 {
            accepted_slot,
            terminal_receipt_bytes: &payout.receipt_bytes,
            aggregate_bytes: &payout.aggregate_bytes,
            custody_replay_bytes: &payout.custody_replay_bytes,
            hoard_token_bytes: &payout.hoard_token_bytes,
            vault_token_bytes: &payout.recipient_token_bytes,
            escrow: &escrow,
            position: &position,
            admission: &admission,
            claim_check: &claim_check,
            cranker: &cranker,
            opener: &opener,
            rent_credit: &rent_credit,
        };
        assert_eq!(
            verify_claim_check_compaction_postcondition_v1(&report, post),
            Ok(())
        );

        let mut changed = payout.recipient_token_bytes.clone();
        changed[64] ^= 1;
        assert_eq!(
            verify_claim_check_compaction_postcondition_v1(
                &report,
                ClaimCheckCompactionPoststateV1 {
                    vault_token_bytes: &changed,
                    ..post
                },
            ),
            Err(ClaimCheckCompactionOperatorErrorV1::Postcondition)
        );
    }

    #[test]
    fn fixed_deadline_and_unsignable_position_kind_refuse_before_instruction_output() {
        let early = compaction_fixture(ProtocolPositionOwnerKindV2::User, false);
        assert_eq!(
            build_claim_check_compaction_v1(early.snapshot()),
            Err(ClaimCheckCompactionOperatorErrorV1::Deadline)
        );
        let unsignable = compaction_fixture(ProtocolPositionOwnerKindV2::TradingRecord, true);
        assert_eq!(
            build_claim_check_compaction_v1(unsignable.snapshot()),
            Err(ClaimCheckCompactionOperatorErrorV1::Scope)
        );
    }

    #[test]
    fn substituted_payout_or_occupied_claim_check_refuses() {
        let mut substituted = compaction_fixture(ProtocolPositionOwnerKindV2::User, true);
        substituted.report.instruction.accounts[33].pubkey = substituted.cranker.key;
        assert_eq!(
            build_claim_check_compaction_v1(substituted.snapshot()),
            Err(ClaimCheckCompactionOperatorErrorV1::Payout)
        );

        let mut occupied = compaction_fixture(ProtocolPositionOwnerKindV2::User, true);
        occupied.claim_check.owner = occupied.report.route.claims_program;
        occupied.claim_check.data = vec![1];
        assert_eq!(
            build_claim_check_compaction_v1(occupied.snapshot()),
            Err(ClaimCheckCompactionOperatorErrorV1::Binding)
        );
    }

    #[test]
    fn mixed_finality_and_inconsistent_aliased_sinks_refuse() {
        let mut mixed = compaction_fixture(ProtocolPositionOwnerKindV2::User, true);
        mixed.admission.observation.slot += 1;
        assert_eq!(
            build_claim_check_compaction_v1(mixed.snapshot()),
            Err(ClaimCheckCompactionOperatorErrorV1::Observation)
        );

        let mut aliased = compaction_fixture(ProtocolPositionOwnerKindV2::User, true);
        aliased.cranker.key = aliased.opener.key;
        assert_ne!(aliased.cranker.lamports, aliased.opener.lamports);
        assert_eq!(
            build_claim_check_compaction_v1(aliased.snapshot()),
            Err(ClaimCheckCompactionOperatorErrorV1::Observation)
        );
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

    #[test]
    fn every_fractional_holder_finds_the_same_mint_addressed_record() {
        let aggregate = Pubkey::new_from_array([1; 32]);
        let mint = Pubkey::new_from_array([10; 32]);
        let coordinates =
            project_fractional_claim_check_coordinates_v1(&claims(), &aggregate, &mint)
                .expect("coordinates");
        let again = project_fractional_claim_check_coordinates_v1(&claims(), &aggregate, &mint)
            .expect("coordinates");
        assert_eq!(coordinates, again);
        assert_ne!(coordinates.record, coordinates.escrow);
        assert_ne!(coordinates.escrow, coordinates.vault);
    }

    #[test]
    fn fractional_partial_report_uses_the_kernel_plan_and_the_route_frame() {
        let statement = fractional_statement(28, 1);
        let holder = Pubkey::new_from_array([11; 32]);
        let holder_collateral = Pubkey::new_from_array([12; 32]);
        let holder_shards = Pubkey::new_from_array([13; 32]);
        let token_program = Pubkey::new_from_array(dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID);
        let report = build_fractional_claim_check_redemption_v1(
            &claims(),
            &token_program,
            &holder,
            &holder_collateral,
            &holder_shards,
            20,
            FractionalClaimCheckBalancesV1 {
                holder_shard_atoms: 70,
                shard_mint_supply: 70,
                vault_collateral_atoms: 28,
                holder_collateral_atoms: 0,
                holder_lamports: 1_000_000_000,
            },
            statement,
        )
        .expect("partial report");
        assert_eq!(
            report.instruction.accounts.len(),
            FRACTIONAL_CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1
        );
        let signers = report
            .instruction
            .accounts
            .iter()
            .filter(|account| account.is_signer)
            .collect::<Vec<_>>();
        assert_eq!(signers.len(), 1);
        assert_eq!(signers[0].pubkey, holder);
        assert_eq!(report.expected_shard_burn, 20);
        assert_eq!(report.expected_vault_debit, 8);
        assert_eq!(report.holder_credit_ceiling, 8);
        assert!(!report.settles_record);
        assert_eq!(report.expected_vacant_record, None);
        assert_eq!(report.expected_lamport_credit, 0);
        assert_eq!(report.expected_escrow_outstanding, 1);
        assert_eq!(report.plan.vault_after(), 20);
        assert_eq!(report.plan.escrowed_after(), 20);
        assert_eq!(
            FractionalRedeemClaimCheckRequestV1::decode(&report.instruction.data)
                .expect("request")
                .requested_shard_atoms,
            20
        );
    }

    #[test]
    fn fractional_settlement_reports_record_and_rent_closure() {
        let statement = fractional_statement(20, 1);
        let holder = Pubkey::new_from_array([11; 32]);
        let report = build_fractional_claim_check_redemption_v1(
            &claims(),
            &Pubkey::new_from_array(dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID),
            &holder,
            &Pubkey::new_from_array([12; 32]),
            &Pubkey::new_from_array([13; 32]),
            50,
            FractionalClaimCheckBalancesV1 {
                holder_shard_atoms: 50,
                shard_mint_supply: 50,
                vault_collateral_atoms: 20,
                holder_collateral_atoms: 8,
                holder_lamports: 1_000_000_000,
            },
            statement,
        )
        .expect("settling report");
        assert_eq!(report.expected_shard_burn, 50);
        assert_eq!(report.expected_vault_debit, 20);
        assert_eq!(report.holder_credit_ceiling, 28);
        assert!(report.settles_record);
        assert_eq!(
            report.expected_vacant_record,
            Some(statement.coordinates.record)
        );
        assert_eq!(report.expected_lamport_credit, 3_118_080);
        assert_eq!(report.expected_escrow_outstanding, 0);
        assert_eq!(report.plan.record_lamports_after(), 0);
        assert_eq!(report.plan.holder_lamports_after(), 1_003_118_080);
    }

    #[test]
    fn fractional_dust_is_refused_before_a_wallet_is_asked() {
        let statement = fractional_statement(28, 1);
        assert_eq!(
            build_fractional_claim_check_redemption_v1(
                &claims(),
                &Pubkey::new_from_array(dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID),
                &Pubkey::new_from_array([11; 32]),
                &Pubkey::new_from_array([12; 32]),
                &Pubkey::new_from_array([13; 32]),
                9,
                FractionalClaimCheckBalancesV1 {
                    holder_shard_atoms: 70,
                    shard_mint_supply: 70,
                    vault_collateral_atoms: 28,
                    holder_collateral_atoms: 0,
                    holder_lamports: 1_000_000_000,
                },
                statement,
            ),
            Err(ClaimCheckOperatorErrorV1::FractionalClaimCheckConservation(dclutch_claims::fractional_claim_check_conservation_v1::FractionalClaimCheckConservationErrorV1::NoWholeClaim))
        );
    }

    #[test]
    fn fractional_builder_refuses_a_token_program_or_aliased_role() {
        let statement = fractional_statement(28, 1);
        let holder = Pubkey::new_from_array([11; 32]);
        let balances = FractionalClaimCheckBalancesV1 {
            holder_shard_atoms: 70,
            shard_mint_supply: 70,
            vault_collateral_atoms: 28,
            holder_collateral_atoms: 0,
            holder_lamports: 1_000_000_000,
        };
        assert_eq!(
            build_fractional_claim_check_redemption_v1(
                &claims(),
                &Pubkey::new_from_array([14; 32]),
                &holder,
                &Pubkey::new_from_array([12; 32]),
                &Pubkey::new_from_array([13; 32]),
                20,
                balances,
                statement,
            ),
            Err(ClaimCheckOperatorErrorV1::Binding)
        );
        assert_eq!(
            build_fractional_claim_check_redemption_v1(
                &claims(),
                &Pubkey::new_from_array(dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID),
                &holder,
                &holder,
                &Pubkey::new_from_array([13; 32]),
                20,
                balances,
                statement,
            ),
            Err(ClaimCheckOperatorErrorV1::Coordinate)
        );
    }
}
