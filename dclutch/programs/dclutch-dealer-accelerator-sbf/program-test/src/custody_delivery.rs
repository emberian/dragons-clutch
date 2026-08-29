//! Real collateral staging for the Dealer Custody delivery leg.
//!
//! The accepted-transition campaign locks value at reservation and commits the
//! Claims delta; delivery is the later permissionless effect that moves the
//! locked collateral out of its escrow and closes it. Custody's `activate_batch`
//! will not move a token until it has re-authenticated the whole graph beneath
//! the request — Market, Realm, replay cursor, adapter profile, Mint and vault
//! addresses — so this module derives that graph once, from the supported
//! contract encoders, and hands the campaign exact keys and exact bodies.
//!
//! Two facts here are load-bearing and were each invisible until something
//! reached Custody's body:
//!
//! * a Realm is content-addressed by its own bytes. `require_realm_authority`
//!   checks `hash(realm body) == request.realm`, so a campaign cannot *choose* a
//!   realm identity — it must build the record first and let the digest be the
//!   identity everything else (the Core Market's own `CoreState` included)
//!   restates.
//! * `collateral_adapter_release_id` must hash-match an entry of
//!   `PRODUCTION_ADAPTER_RELEASES`, or Custody refuses `Realm`. A placeholder is
//!   a Realm no live route can ever accept.
//!
//! The legacy adapter is selected deliberately: `solana-program-test` genesis
//! already carries the real token program at `LEGACY_TOKEN_PROGRAM_ID`, so the
//! transfer and the escrow close are executed by a real ELF without the campaign
//! acquiring an external artifact or an environment variable.

use std::vec::Vec;

use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CallerRoleV1, CompartmentV1, ContextV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_dealer_codec::scenario_custody_reservation_v1::{
    DEALER_SCENARIO_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
    DEALER_SCENARIO_RESERVATION_STATE_PDA_DOMAIN_V1,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1,
    RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use solana_program::hash::hash;
use solana_program::pubkey::Pubkey;
use solana_program_option::COption;
use solana_program_pack::Pack;
use spl_token_interface::state::{Account as SplAccount, AccountState, Mint as SplMint};

/// The collateral Mint every Dealer delivery in this campaign settles in.
pub const DELIVERY_MINT: Pubkey = Pubkey::new_from_array([0xc4; 32]);

/// One immutable collateral Realm, at the addresses its own digest derives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerDeliveryRealmV1 {
    /// Exact canonical record body.
    pub bytes: Vec<u8>,
    /// Content identity every layer of the scenario restates.
    pub digest: [u8; 32],
    /// Registry-owned finalized raw record account.
    pub raw: Pubkey,
    /// Vacant staging cursor beside it.
    pub staging: Pubkey,
}

/// Build the collateral Realm this campaign settles under.
///
/// The digest is the Realm identity: it is what the Core Market's `CoreState`
/// must carry as its `realm_id`, what every Custody request must name, and what
/// derives both account addresses. Nothing may choose it independently.
pub fn dealer_delivery_realm_v1(registry_program: Pubkey) -> DealerDeliveryRealmV1 {
    let realm = RealmV1::new(RealmV1Input {
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        collateral_mint: DELIVERY_MINT.to_bytes(),
        collateral_adapter_release_id: hash(
            &PRODUCTION_ADAPTER_RELEASES
                .first()
                .expect("the production adapter catalog is never empty")
                .to_bytes(),
        )
        .to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("canonical collateral Realm")
    .to_bytes();
    let bytes = realm.to_vec();
    debug_assert_eq!(bytes.len(), REALM_BYTES);
    let digest = hash(&bytes).to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &REALM_SCHEMA_RELEASE_ID_V1, &digest],
        &registry_program,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &digest,
        ],
        &registry_program,
    )
    .0;
    DealerDeliveryRealmV1 {
        bytes,
        digest,
        raw,
        staging,
    }
}

/// Everything the delivery leg needs that the accepted transition already fixed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerDeliveryInputV1 {
    /// Release-selected Custody program.
    pub custody_program: Pubkey,
    /// Release-selected Trading program: the caller of record.
    pub trading_program: Pubkey,
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Core Market the scenario transitions.
    pub market: Pubkey,
    /// Realm content identity, from `dealer_delivery_realm_v1`.
    pub realm: [u8; 32],
    /// Immutable Custody replay namespace for this Market.
    pub context: [u8; 32],
    /// Market generation every layer restates.
    pub generation: u64,
    /// Reserved and committed Trading checkpoint.
    pub checkpoint: Pubkey,
    /// Exact immutable request digest the checkpoint is derived from.
    pub request_digest: [u8; 32],
    /// External token account the delivery credits.
    pub destination: Pubkey,
    /// External owner of the token account the delivery credits.
    pub destination_owner: Pubkey,
    /// Immutable lamport refund beneficiary the replay cursor was opened with.
    pub replay_rent_refund: Pubkey,
    /// Exact collateral atoms the reservation locked.
    pub amount: u64,
    /// Source-vault balance after the reservation debited it.
    pub source_after: u64,
    /// Destination balance before delivery credits it.
    pub destination_before: u64,
    /// Revision the replay cursor stands at when delivery is submitted.
    pub replay_revision: u64,
}

/// The staged collateral graph one Dealer delivery executes against.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerDeliveryV1 {
    /// Real token program: the one the Realm's adapter release names.
    pub token_program: Pubkey,
    /// Realm-selected collateral Mint.
    pub mint: Pubkey,
    /// Exact canonical Mint body.
    pub mint_bytes: Vec<u8>,
    /// Custody transfer authority for this Market and release set.
    pub custody_authority: Pubkey,
    /// Custody-owned TradingPrincipal vault the reservation debited.
    pub source: Pubkey,
    /// Exact source-vault body, already debited.
    pub source_bytes: Vec<u8>,
    /// Custody RecoveryReserve escrow holding the locked collateral.
    pub escrow: Pubkey,
    /// Exact escrow body, holding exactly the reserved amount.
    pub escrow_bytes: Vec<u8>,
    /// External destination token account delivery credits.
    pub destination: Pubkey,
    /// Exact destination body before delivery.
    pub destination_bytes: Vec<u8>,
    /// Standard Custody replay cursor delivery advances.
    pub replay: Pubkey,
    /// Exact replay cursor body before delivery.
    pub replay_bytes: Vec<u8>,
    /// Custody-owned reservation state for the delivered effect.
    pub reservation_state: Pubkey,
    /// Vacant activation receipt delivery creates.
    pub activation_receipt: Pubkey,
    /// The canonical Custody request the effect body carries.
    pub request: CustodyRequestV1,
}

impl DealerDeliveryV1 {
    /// Digest of the escrow exactly as the reservation left it.
    ///
    /// Activation re-reads it as the reservation's `effect_poststate_digest`, so
    /// the reservation cannot claim an escrow state the chain does not hold.
    pub fn escrow_digest(&self) -> [u8; 32] {
        hash(&self.escrow_bytes).to_bytes()
    }

    /// Digest of the destination before delivery credits it.
    pub fn destination_digest(&self) -> [u8; 32] {
        hash(&self.destination_bytes).to_bytes()
    }

    /// Digest of the replay cursor the reservation batch pinned.
    pub fn replay_digest(&self) -> [u8; 32] {
        hash(&self.replay_bytes).to_bytes()
    }
}

/// Derive the whole collateral graph for one single-effect Dealer delivery.
///
/// Every address here is derived from the request the effect will carry, in the
/// same direction Custody derives it, so no coordinate is chosen twice.
pub fn stage_dealer_delivery_v1(input: DealerDeliveryInputV1) -> DealerDeliveryV1 {
    let token_program = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);
    let custody_authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &input.market.to_bytes(),
            &input.release_set,
        ],
        &input.custody_program,
    )
    .0;
    // The reservation state is the escrow's vault context, so the escrow address
    // is reachable only through the reservation it belongs to.
    let reservation_state = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_STATE_PDA_DOMAIN_V1,
            input.checkpoint.as_ref(),
            &[0],
        ],
        &input.custody_program,
    )
    .0;
    let escrow = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            input.market.to_bytes(),
            input.release_set,
            reservation_state.to_bytes(),
            CompartmentV1::RecoveryReserve,
        )
        .as_slices(),
        &input.custody_program,
    )
    .0;
    // A staged rollback has to be able to put the value back, which an external
    // delegated allowance cannot promise, so Custody refuses any effect whose
    // source is External. The reserved side is a Custody-owned Dealer trading
    // principal vault.
    let source = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            input.market.to_bytes(),
            input.release_set,
            input.context,
            CompartmentV1::TradingPrincipal,
        )
        .as_slices(),
        &input.custody_program,
    )
    .0;
    let destination = input.destination;
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            input.market.to_bytes(),
            input.release_set,
            CallerRoleV1::Trading,
            input.context,
        )
        .as_slices(),
        &input.custody_program,
    )
    .0;
    let activation_receipt = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            input.checkpoint.as_ref(),
            &input.request_digest,
        ],
        &input.custody_program,
    )
    .0;
    let request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::TradingPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set: input.release_set,
        market: input.market.to_bytes(),
        realm: input.realm,
        context: input.context,
        caller_program: input.trading_program.to_bytes(),
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: input.destination_owner.to_bytes(),
            order: [0; 32],
            parent_request_digest: input.request_digest,
            order_nonce: 0,
            generation: input.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: source.to_bytes(),
        destination: destination.to_bytes(),
        source_vault_context: input.context,
        destination_vault_context: [0; 32],
        mint: DELIVERY_MINT.to_bytes(),
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: input.replay_revision,
        resulting_revision: input.replay_revision.saturating_add(1),
        amount: input.amount,
        rent_lamports: 0,
    };
    let supply = input
        .amount
        .saturating_add(input.source_after)
        .saturating_add(input.destination_before);
    DealerDeliveryV1 {
        token_program,
        mint: DELIVERY_MINT,
        mint_bytes: collateral_mint_bytes(supply),
        custody_authority,
        source,
        source_bytes: token_account_bytes(DELIVERY_MINT, custody_authority, input.source_after),
        escrow,
        escrow_bytes: token_account_bytes(DELIVERY_MINT, custody_authority, input.amount),
        destination,
        destination_bytes: token_account_bytes(
            DELIVERY_MINT,
            input.destination_owner,
            input.destination_before,
        ),
        replay,
        replay_bytes: replay_bytes(&input),
        reservation_state,
        activation_receipt,
        request,
    }
}

/// Read the raw collateral balance out of a staged or observed token account.
pub fn token_account_amount(bytes: &[u8]) -> u64 {
    SplAccount::unpack(bytes).expect("canonical token account").amount
}

/// Read the owner out of a staged or observed token account.
pub fn token_account_owner(bytes: &[u8]) -> Pubkey {
    SplAccount::unpack(bytes).expect("canonical token account").owner
}

/// The exact canonical body of a collateral Mint with no live authority.
///
/// The Realm's policies are `RequireAbsent` on both, which is what makes the
/// collateral atom unmintable and unfreezable for the whole Market lifecycle.
fn collateral_mint_bytes(supply: u64) -> Vec<u8> {
    let mut output = vec![0_u8; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: COption::None,
            supply,
            decimals: 0,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut output,
    )
    .expect("canonical mint packs");
    output
}

/// The exact canonical body of one initialized token account.
fn token_account_bytes(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut output = vec![0_u8; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint,
            owner,
            amount,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut output,
    )
    .expect("canonical token account packs");
    output
}

/// The replay cursor as an earlier Custody transaction under this Market left it.
///
/// `advance` joins every identity field against each effect's own request, and
/// refuses unless `expected_revision` is exactly the revision standing here — so
/// the cursor is the reason a delivery can be submitted at most once.
fn replay_bytes(input: &DealerDeliveryInputV1) -> Vec<u8> {
    CustodyReplayV1 {
        caller_role: CallerRoleV1::Trading,
        release_set: input.release_set,
        market: input.market.to_bytes(),
        realm: input.realm,
        context: input.context,
        caller_program: input.trading_program.to_bytes(),
        rent_refund: input.replay_rent_refund.to_bytes(),
        // The source vault this scenario debits is open under this cursor.
        open_vault_count: 1,
        next_revision: input.replay_revision,
        generation: input.generation,
        // A cursor that has never accepted anything is not the normal case: this
        // Market's collateral reached its vaults through earlier Custody
        // transactions, and the cursor carries what it last accepted.
        last_request_digest: [0xa7; 32],
        last_poststate_commitment: [0xa8; 32],
    }
    .to_bytes()
    .expect("canonical replay cursor encodes")
    .to_vec()
}
