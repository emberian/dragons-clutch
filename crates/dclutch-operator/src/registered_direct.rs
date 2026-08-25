//! Chain-derived construction for registered Direct execution routes.
//!
//! These builders accept finalized hostile account snapshots, decode the
//! codec-owned registered state, rederive every controller-owned address, and
//! emit unsigned instructions. They never read RPC, sign, or submit.

use crate::{Finality, Observation, ObservedAccount};
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1,
};
use dclutch_core_contract::{MARKET_ROOT_BYTES, MarketRoot, Phase};
use dclutch_direct_codec::{
    RegisteredFillInstructionV1, RegisteredIntentStateV1, RegisteredTerminalAction,
    RegisteredTerminalInstructionV1,
};
use dclutch_direct_contract::{
    DIRECT_CAPABILITY_KIND_ID_V2, VENUE_FEE_POLICY_BYTES_V3, VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
    VenueFeePolicyV3,
};
use dclutch_market_contract::market::{MARKET_ROOT_OFFSET, decode_market_outcome_count};
use dclutch_realm_contract::{REALM_PDA_DOMAIN, RealmV1};
use dclutch_record_contract::RAW_RECORD_PDA_SEED_V1;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::compiled_direct::{
    CLAIM_PROGRAM_ID, CONTROLLER_SEED, CUSTODY_PROGRAM_ID, MARKET_SEED, POSITION_SEED,
};

/// Registered-intent PDA domain committed by the controller release.
pub const REGISTERED_SEED: &[u8] = b"dclutch/direct-registered/v1";
/// Exact account count for a registered residual fill.
pub const REGISTERED_FILL_ACCOUNT_COUNT: usize = 17;
/// Exact account count for a maker-authorized registered cancellation.
pub const REGISTERED_CANCEL_ACCOUNT_COUNT: usize = 4;
/// Exact account count for a permissionless registered expiry.
pub const REGISTERED_EXPIRY_ACCOUNT_COUNT: usize = 3;

#[derive(Clone, Copy)]
struct AuthorityFacts {
    generation: u64,
    outcome_count: u8,
    fee_basis_points: u16,
}

/// Same-finalized chain state required for one registered residual fill.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectFillState {
    /// Global controller PDA observation.
    pub controller: ObservedAccount,
    /// Seller registered-intent state.
    pub seller_registration: ObservedAccount,
    /// Buyer registered-intent state.
    pub buyer_registration: ObservedAccount,
    /// Controller-owned transaction journal.
    pub journal: ObservedAccount,
    /// Seller maker/outcome Position.
    pub seller_position: ObservedAccount,
    /// Buyer maker/outcome Position.
    pub buyer_position: ObservedAccount,
    /// Pinned executable claim child.
    pub claim_program: ObservedAccount,
    /// Pinned executable custody child.
    pub custody_program: ObservedAccount,
    /// Canonical active Market selected by both registrations.
    pub market: ObservedAccount,
    /// Immutable Realm selected by the Market identity.
    pub realm: ObservedAccount,
    /// Finalized venue fee policy selected by the Direct capability.
    pub fee_policy: ObservedAccount,
    /// Finalized capability manifest selected by the Market identity.
    pub capability_manifest: ObservedAccount,
    /// Realm-selected collateral mint.
    pub mint: ObservedAccount,
    /// Buyer collateral source selected by the buyer registration.
    pub buyer_source: ObservedAccount,
    /// Seller collateral destination selected by the seller registration.
    pub seller_destination: ObservedAccount,
    /// Policy-selected fee destination.
    pub fee_destination: ObservedAccount,
    /// Realm-selected executable token program.
    pub token_program: ObservedAccount,
}

/// Same-finalized state required for one registered terminal route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectTerminalState {
    /// Global controller PDA observation.
    pub controller: ObservedAccount,
    /// Registered-intent state to cancel or expire.
    pub registration: ObservedAccount,
    /// Pinned executable claim child.
    pub claim_program: ObservedAccount,
}

/// Chain-derived registered residual-fill instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectFillReport {
    /// Exact unsigned 17-account controller instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting all input state.
    pub observation: Observation,
    /// Seller registration PDA rederived from persisted authority.
    pub seller_registration: Pubkey,
    /// Buyer registration PDA rederived from persisted authority.
    pub buyer_registration: Pubkey,
    /// Seller Position PDA rederived from persisted authority.
    pub seller_position: Pubkey,
    /// Buyer Position PDA rederived from persisted authority.
    pub buyer_position: Pubkey,
}

/// Chain-derived registered cancel or expiry instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectTerminalReport {
    /// Exact unsigned terminal controller instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting all input state.
    pub observation: Observation,
    /// Persisted maker signer for cancellation, absent for expiry.
    pub maker: Option<Pubkey>,
    /// Exact registration-local sequence pinned into the request.
    pub expected_sequence: u64,
}

/// Refusal from hostile registered Direct state or frame derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// One observed account was not finalized.
    ObservationNotFinalized,
    /// Observed accounts did not share one exact snapshot.
    ObservationMismatch,
    /// Market, Realm, manifest, or fee-policy authority was invalid.
    InvalidAuthority,
    /// A role owner, executable bit, or alias was invalid.
    InvalidAccount,
    /// Registered state was malformed or not canonically open.
    InvalidRegistration,
    /// Persisted registration semantics disagreed with Market authority.
    RegistrationBinding,
    /// A controller, registration, or Position PDA differed.
    PdaMismatch,
    /// Fill coordinates were impossible for the observed registrations.
    InvalidCoordinates,
    /// The finalized snapshot was outside a signed fill validity window.
    FillWindowClosed,
    /// Permissionless expiry was requested before the signed window elapsed.
    ExpiryTooEarly,
    /// A codec-owned request could not be encoded.
    Encoding,
}

/// Build one exact registered residual-fill controller instruction.
///
/// `fill` and `execution_price` are matcher coordinates only. All maker,
/// registration, Market, collateral, fee, and replay authority is recovered
/// from the finalized snapshot and reauthenticated against its canonical PDA.
pub fn build_registered_direct_fill(
    controller_program: Pubkey,
    state: &RegisteredDirectFillState,
    fill: u64,
    execution_price: u64,
) -> Result<RegisteredDirectFillReport, Error> {
    let observation = fill_observation(state)?;
    validate_fill_accounts(controller_program, state)?;
    let authority = authenticate_authority(state)?;
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &controller_program);
    if state.controller.key != controller {
        return Err(Error::PdaMismatch);
    }
    let seller = decode_registration(&state.seller_registration, controller)?;
    let buyer = decode_registration(&state.buyer_registration, controller)?;
    validate_fill_bindings(
        state,
        authority,
        seller,
        buyer,
        observation,
        fill,
        execution_price,
    )?;

    let (seller_registration, seller_registration_bump) =
        registration_address(controller_program, seller)?;
    let (buyer_registration, buyer_registration_bump) =
        registration_address(controller_program, buyer)?;
    let (seller_position, seller_position_bump) =
        position_address(controller_program, state.market.key, seller)?;
    let (buyer_position, buyer_position_bump) =
        position_address(controller_program, state.market.key, buyer)?;
    if state.seller_registration.key != seller_registration
        || state.buyer_registration.key != buyer_registration
        || state.seller_position.key != seller_position
        || state.buyer_position.key != buyer_position
    {
        return Err(Error::PdaMismatch);
    }

    let data = RegisteredFillInstructionV1 {
        controller_bump,
        seller_registration_bump,
        buyer_registration_bump,
        seller_position_bump,
        buyer_position_bump,
        fill,
        execution_price,
    }
    .encode()
    .map_err(|_| Error::Encoding)?;
    let accounts = vec![
        AccountMeta::new_readonly(controller, false),
        AccountMeta::new(seller_registration, false),
        AccountMeta::new(buyer_registration, false),
        AccountMeta::new(state.journal.key, false),
        AccountMeta::new(seller_position, false),
        AccountMeta::new(buyer_position, false),
        AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(state.market.key, false),
        AccountMeta::new_readonly(state.realm.key, false),
        AccountMeta::new_readonly(state.fee_policy.key, false),
        AccountMeta::new_readonly(state.capability_manifest.key, false),
        AccountMeta::new_readonly(state.mint.key, false),
        AccountMeta::new(state.buyer_source.key, false),
        AccountMeta::new(state.seller_destination.key, false),
        AccountMeta::new(state.fee_destination.key, false),
        AccountMeta::new_readonly(state.token_program.key, false),
    ];
    debug_assert_eq!(accounts.len(), REGISTERED_FILL_ACCOUNT_COUNT);
    Ok(RegisteredDirectFillReport {
        instruction: Instruction {
            program_id: controller_program,
            accounts,
            data: data.to_vec(),
        },
        observation,
        seller_registration,
        buyer_registration,
        seller_position,
        buyer_position,
    })
}

/// Build one exact maker-authorized registered cancellation.
///
/// The signer is the maker embedded in the registered state; callers cannot
/// substitute a cancellation authority or local replay sequence.
pub fn build_registered_direct_maker_cancel(
    controller_program: Pubkey,
    state: &RegisteredDirectTerminalState,
) -> Result<RegisteredDirectTerminalReport, Error> {
    build_terminal(controller_program, state, RegisteredTerminalAction::Cancel)
}

/// Build one exact permissionless registered expiry.
///
/// Expiry is emitted only from a finalized snapshot strictly after the signed
/// `valid_through` slot. The transaction contains no maker signer role.
pub fn build_registered_direct_permissionless_expiry(
    controller_program: Pubkey,
    state: &RegisteredDirectTerminalState,
) -> Result<RegisteredDirectTerminalReport, Error> {
    build_terminal(controller_program, state, RegisteredTerminalAction::Expire)
}

fn build_terminal(
    controller_program: Pubkey,
    state: &RegisteredDirectTerminalState,
    action: RegisteredTerminalAction,
) -> Result<RegisteredDirectTerminalReport, Error> {
    let observation =
        same_observation(&[&state.controller, &state.registration, &state.claim_program])?;
    require_distinct(&[
        state.controller.key,
        state.registration.key,
        state.claim_program.key,
    ])?;
    if state.controller.executable
        || state.registration.owner != CLAIM_PROGRAM_ID
        || state.registration.executable
        || state.claim_program.key != CLAIM_PROGRAM_ID
        || !state.claim_program.executable
    {
        return Err(Error::InvalidAccount);
    }
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &controller_program);
    if state.controller.key != controller {
        return Err(Error::PdaMismatch);
    }
    let registration = decode_registration(&state.registration, controller)?;
    let (registration_key, registration_bump) =
        registration_address(controller_program, registration)?;
    if registration_key != state.registration.key {
        return Err(Error::PdaMismatch);
    }
    let maker = Pubkey::new_from_array(registration.maker);
    if maker == Pubkey::default()
        || registration.phase != 0
        || registration.remaining == 0
        || registration.remaining > registration.intent.maximum_fill
        || registration.intent.side > 1
        || registration.intent.lifecycle > 2
    {
        return Err(Error::InvalidRegistration);
    }
    if action == RegisteredTerminalAction::Expire
        && observation.slot <= registration.intent.valid_through
    {
        return Err(Error::ExpiryTooEarly);
    }
    let expected_sequence = registration.sequence;
    let data = RegisteredTerminalInstructionV1 {
        action,
        controller_bump,
        registration_bump,
        expected_sequence,
    }
    .encode()
    .map_err(|_| Error::Encoding)?;
    let (maker, accounts) = match action {
        RegisteredTerminalAction::Cancel => (
            Some(maker),
            vec![
                AccountMeta::new_readonly(controller, false),
                AccountMeta::new(registration_key, false),
                AccountMeta::new_readonly(maker, true),
                AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
            ],
        ),
        RegisteredTerminalAction::Expire => (
            None,
            vec![
                AccountMeta::new_readonly(controller, false),
                AccountMeta::new(registration_key, false),
                AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
            ],
        ),
    };
    debug_assert_eq!(
        accounts.len(),
        match action {
            RegisteredTerminalAction::Cancel => REGISTERED_CANCEL_ACCOUNT_COUNT,
            RegisteredTerminalAction::Expire => REGISTERED_EXPIRY_ACCOUNT_COUNT,
        }
    );
    Ok(RegisteredDirectTerminalReport {
        instruction: Instruction {
            program_id: controller_program,
            accounts,
            data: data.to_vec(),
        },
        observation,
        maker,
        expected_sequence,
    })
}

fn fill_observation(state: &RegisteredDirectFillState) -> Result<Observation, Error> {
    same_observation(&[
        &state.controller,
        &state.seller_registration,
        &state.buyer_registration,
        &state.journal,
        &state.seller_position,
        &state.buyer_position,
        &state.claim_program,
        &state.custody_program,
        &state.market,
        &state.realm,
        &state.fee_policy,
        &state.capability_manifest,
        &state.mint,
        &state.buyer_source,
        &state.seller_destination,
        &state.fee_destination,
        &state.token_program,
    ])
}

fn same_observation(accounts: &[&ObservedAccount]) -> Result<Observation, Error> {
    let observation = accounts
        .first()
        .map(|account| account.observation)
        .ok_or(Error::ObservationNotFinalized)?;
    if accounts
        .iter()
        .any(|account| account.observation.finality != Finality::Finalized)
    {
        return Err(Error::ObservationNotFinalized);
    }
    if accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(observation)
}

fn validate_fill_accounts(
    controller_program: Pubkey,
    state: &RegisteredDirectFillState,
) -> Result<(), Error> {
    require_distinct(&[
        state.controller.key,
        state.seller_registration.key,
        state.buyer_registration.key,
        state.journal.key,
        state.seller_position.key,
        state.buyer_position.key,
        state.claim_program.key,
        state.custody_program.key,
        state.market.key,
        state.realm.key,
        state.fee_policy.key,
        state.capability_manifest.key,
        state.mint.key,
        state.buyer_source.key,
        state.seller_destination.key,
        state.fee_destination.key,
        state.token_program.key,
    ])?;
    let protocol_program = state.market.owner;
    if state.controller.executable
        || state.seller_registration.owner != CLAIM_PROGRAM_ID
        || state.seller_registration.executable
        || state.buyer_registration.owner != CLAIM_PROGRAM_ID
        || state.buyer_registration.executable
        || state.journal.owner != controller_program
        || state.journal.executable
        || state.seller_position.owner != CLAIM_PROGRAM_ID
        || state.seller_position.executable
        || state.buyer_position.owner != CLAIM_PROGRAM_ID
        || state.buyer_position.executable
        || state.claim_program.key != CLAIM_PROGRAM_ID
        || !state.claim_program.executable
        || state.custody_program.key != CUSTODY_PROGRAM_ID
        || !state.custody_program.executable
        || state.market.executable
        || state.realm.owner != protocol_program
        || state.realm.executable
        || state.fee_policy.owner != protocol_program
        || state.fee_policy.executable
        || state.capability_manifest.owner != protocol_program
        || state.capability_manifest.executable
        || !state.token_program.executable
        || state.mint.owner != state.token_program.key
        || state.mint.executable
        || state.buyer_source.owner != state.token_program.key
        || state.buyer_source.executable
        || state.seller_destination.owner != state.token_program.key
        || state.seller_destination.executable
        || state.fee_destination.owner != state.token_program.key
        || state.fee_destination.executable
    {
        return Err(Error::InvalidAccount);
    }
    Ok(())
}

fn require_distinct(keys: &[Pubkey]) -> Result<(), Error> {
    for (index, key) in keys.iter().enumerate() {
        if keys.iter().skip(index + 1).any(|other| other == key) {
            return Err(Error::InvalidAccount);
        }
    }
    Ok(())
}

fn decode_registration(
    account: &ObservedAccount,
    controller: Pubkey,
) -> Result<RegisteredIntentStateV1, Error> {
    let registration =
        RegisteredIntentStateV1::decode(&account.data).map_err(|_| Error::InvalidRegistration)?;
    if registration.controller != controller.to_bytes() {
        return Err(Error::RegistrationBinding);
    }
    Ok(registration)
}

fn validate_fill_bindings(
    state: &RegisteredDirectFillState,
    authority: AuthorityFacts,
    seller: RegisteredIntentStateV1,
    buyer: RegisteredIntentStateV1,
    observation: Observation,
    fill: u64,
    execution_price: u64,
) -> Result<(), Error> {
    let seller_maker = Pubkey::new_from_array(seller.maker);
    let buyer_maker = Pubkey::new_from_array(buyer.maker);
    if seller.phase != 0
        || buyer.phase != 0
        || seller.remaining == 0
        || buyer.remaining == 0
        || seller.remaining > seller.intent.maximum_fill
        || buyer.remaining > buyer.intent.maximum_fill
        || seller.intent.side != 0
        || buyer.intent.side != 1
        || seller.intent.lifecycle > 2
        || buyer.intent.lifecycle > 2
        || seller_maker == Pubkey::default()
        || buyer_maker == Pubkey::default()
        || seller_maker == buyer_maker
    {
        return Err(Error::InvalidRegistration);
    }
    if seller.intent.market != state.market.key.to_bytes()
        || buyer.intent.market != state.market.key.to_bytes()
        || seller.intent.generation != authority.generation
        || buyer.intent.generation != authority.generation
        || seller.intent.outcome >= authority.outcome_count
        || buyer.intent.outcome >= authority.outcome_count
        || seller.intent.collateral_account != state.seller_destination.key.to_bytes()
        || buyer.intent.collateral_account != state.buyer_source.key.to_bytes()
        || seller.intent.fee_basis_points != authority.fee_basis_points
        || buyer.intent.fee_basis_points != authority.fee_basis_points
    {
        return Err(Error::RegistrationBinding);
    }
    if observation.slot < seller.intent.valid_from
        || observation.slot > seller.intent.valid_through
        || observation.slot < buyer.intent.valid_from
        || observation.slot > buyer.intent.valid_through
    {
        return Err(Error::FillWindowClosed);
    }
    if fill == 0
        || fill > seller.remaining
        || fill > buyer.remaining
        || execution_price < seller.intent.limit_price
        || execution_price > buyer.intent.limit_price
    {
        return Err(Error::InvalidCoordinates);
    }
    Ok(())
}

fn registration_address(
    controller_program: Pubkey,
    state: RegisteredIntentStateV1,
) -> Result<(Pubkey, u8), Error> {
    let maker = Pubkey::new_from_array(state.maker);
    if maker == Pubkey::default() {
        return Err(Error::InvalidRegistration);
    }
    let generation = state.intent.generation.to_le_bytes();
    let nonce = state.intent.nonce.to_le_bytes();
    Ok(Pubkey::find_program_address(
        &[
            REGISTERED_SEED,
            &state.intent.market,
            &generation,
            maker.as_ref(),
            &nonce,
        ],
        &controller_program,
    ))
}

fn position_address(
    controller_program: Pubkey,
    market: Pubkey,
    state: RegisteredIntentStateV1,
) -> Result<(Pubkey, u8), Error> {
    let maker = Pubkey::new_from_array(state.maker);
    if maker == Pubkey::default() {
        return Err(Error::InvalidRegistration);
    }
    let outcome = [state.intent.outcome];
    Ok(Pubkey::find_program_address(
        &[POSITION_SEED, market.as_ref(), maker.as_ref(), &outcome],
        &controller_program,
    ))
}

fn authenticate_authority(state: &RegisteredDirectFillState) -> Result<AuthorityFacts, Error> {
    let protocol_program = state.market.owner;
    let outcome_count =
        decode_market_outcome_count(&state.market.data).map_err(|_| Error::InvalidAuthority)?;
    let root_end = MARKET_ROOT_OFFSET
        .checked_add(MARKET_ROOT_BYTES)
        .ok_or(Error::InvalidAuthority)?;
    let root = MarketRoot::decode(
        state
            .market
            .data
            .get(MARKET_ROOT_OFFSET..root_end)
            .ok_or(Error::InvalidAuthority)?,
    )
    .map_err(|_| Error::InvalidAuthority)?;
    if root.phase() != Phase::Open {
        return Err(Error::InvalidAuthority);
    }
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let expected_market =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], &protocol_program).0;
    if state.market.key != expected_market {
        return Err(Error::PdaMismatch);
    }

    let realm = RealmV1::decode(&state.realm.data).map_err(|_| Error::InvalidAuthority)?;
    if realm.to_bytes().as_slice() != state.realm.data.as_slice() {
        return Err(Error::InvalidAuthority);
    }
    let realm_digest = hash(&state.realm.data).to_bytes();
    let expected_realm =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], &protocol_program).0;
    if state.realm.key != expected_realm
        || root.identity().realm_id().to_bytes() != realm_digest
        || realm.token_program() != state.token_program.key.as_ref()
        || realm.collateral_mint() != state.mint.key.as_ref()
    {
        return Err(Error::InvalidAuthority);
    }

    let manifest = CapabilityManifestV1::decode(&state.capability_manifest.data)
        .map_err(|_| Error::InvalidAuthority)?;
    if manifest.as_bytes() != state.capability_manifest.data.as_slice() {
        return Err(Error::InvalidAuthority);
    }
    let manifest_digest = hash(manifest.as_bytes()).to_bytes();
    if state.capability_manifest.key
        != raw_record_address(
            protocol_program,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            manifest_digest,
        )
        || root.identity().capability_manifest_id().to_bytes() != manifest_digest
    {
        return Err(Error::InvalidAuthority);
    }

    let policy =
        VenueFeePolicyV3::decode(&state.fee_policy.data).map_err(|_| Error::InvalidAuthority)?;
    let mut canonical_policy = [0_u8; VENUE_FEE_POLICY_BYTES_V3];
    policy
        .encode(&mut canonical_policy)
        .map_err(|_| Error::InvalidAuthority)?;
    if canonical_policy.as_slice() != state.fee_policy.data.as_slice() {
        return Err(Error::InvalidAuthority);
    }
    let policy_digest = hash(&state.fee_policy.data).to_bytes();
    if state.fee_policy.key
        != raw_record_address(
            protocol_program,
            VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
            policy_digest,
        )
        || policy.recipient() != state.fee_destination.key.as_ref()
    {
        return Err(Error::InvalidAuthority);
    }

    let mut selected = None;
    let mut index = 0_u16;
    while index < manifest.entry_count() {
        let entry = manifest.entry(index).map_err(|_| Error::InvalidAuthority)?;
        if entry.kind_id().to_bytes() == DIRECT_CAPABILITY_KIND_ID_V2 {
            if selected.is_some() {
                return Err(Error::InvalidAuthority);
            }
            selected = Some(entry);
        }
        index = index.checked_add(1).ok_or(Error::InvalidAuthority)?;
    }
    let entry = selected.ok_or(Error::InvalidAuthority)?;
    let funding = entry.funding_quote();
    if entry.release_id().to_bytes() != dclutch_direct_codec::COMPILED_DIRECT_RELEASE_ID_V1
        || entry.config_id().to_bytes() != policy_digest
        || entry.capacity_profile_id().to_bytes()
            != dclutch_direct_codec::COMPILED_DIRECT_CAPACITY_ID_V1
        || entry.child_schema_id().to_bytes()
            != dclutch_direct_codec::COMPILED_DIRECT_CHILD_SCHEMA_ID_V1
        || entry.child_derivation_id().to_bytes()
            != dclutch_direct_codec::COMPILED_DIRECT_DERIVATION_ID_V1
        || entry.activation_policy() != ActivationPolicy::RequiredAtFounding
        || entry.activation_deadline_slot() != 0
        || entry.dependency_count() != 0
        || funding.native_lamports_total() != 0
        || funding.realm_collateral_total() != 0
        || funding.realm_collateral().is_some()
    {
        return Err(Error::InvalidAuthority);
    }
    Ok(AuthorityFacts {
        generation: root.identity().generation(),
        outcome_count,
        fee_basis_points: policy.fee_basis_points(),
    })
}

fn raw_record_address(
    protocol_program: Pubkey,
    schema_release_id: [u8; 32],
    digest: [u8; 32],
) -> Pubkey {
    Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema_release_id, &digest],
        &protocol_program,
    )
    .0
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_contract::{
        CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1, ContentId,
        FundingAmountsV1, FundingQuoteV1, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity};
    use dclutch_direct_codec::{
        COMPILED_DIRECT_CAPACITY_ID_V1, COMPILED_DIRECT_CHILD_SCHEMA_ID_V1,
        COMPILED_DIRECT_DERIVATION_ID_V1, COMPILED_DIRECT_RELEASE_ID_V1, CompactIntentV1,
    };
    use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
    use dclutch_realm_contract::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1Input};
    use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
    use solana_sdk_ids::{native_loader, system_program};

    const GENERATION: u64 = 3;
    const SELLER_NONCE: u64 = 4;
    const BUYER_NONCE: u64 = 5;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn observation(slot: u64) -> Observation {
        Observation {
            slot,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn observed(
        observation: Observation,
        key: Pubkey,
        owner: Pubkey,
        executable: bool,
        data: Vec<u8>,
    ) -> ObservedAccount {
        ObservedAccount {
            observation,
            key,
            owner,
            lamports: 1_000_000,
            executable,
            data,
        }
    }

    fn capability_id(bytes: [u8; 32]) -> ContentId {
        ContentId::new(bytes).expect("nonzero capability ID")
    }

    fn core_id(bytes: [u8; 32]) -> CoreContentId {
        CoreContentId::new(bytes).expect("nonzero core ID")
    }

    fn zero_quote() -> FundingQuoteV1 {
        let none = CompartmentFundingV1::not_applicable();
        FundingQuoteV1::new(
            FundingAmountsV1::new(none, none, none, none, none, none, none).expect("zero funding"),
            None,
        )
        .expect("zero quote")
    }

    fn direct_intent(market: Pubkey, collateral: Pubkey, side: u8, nonce: u64) -> CompactIntentV1 {
        CompactIntentV1 {
            side,
            outcome: 1,
            lifecycle: 2,
            market: market.to_bytes(),
            generation: GENERATION,
            nonce,
            valid_from: 40,
            valid_through: 60,
            maximum_fill: 2_000,
            limit_price: if side == 0 { 400_000 } else { 600_000 },
            fee_basis_points: 25,
            collateral_account: collateral.to_bytes(),
        }
    }

    fn registration(
        controller: Pubkey,
        maker: Pubkey,
        intent: CompactIntentV1,
        sequence: u64,
    ) -> Vec<u8> {
        RegisteredIntentStateV1 {
            phase: 0,
            controller: controller.to_bytes(),
            maker: maker.to_bytes(),
            intent,
            remaining: 1_500,
            sequence,
        }
        .encode()
        .expect("registered state")
        .to_vec()
    }

    fn fixture() -> (Pubkey, RegisteredDirectFillState) {
        let snapshot = observation(55);
        let controller_program = key(67);
        let protocol_program = key(68);
        let seller_maker = key(1);
        let buyer_maker = key(2);
        let seller_destination = key(5);
        let buyer_source = key(6);
        let mint = key(8);
        let fee_destination = key(9);
        let token_program = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);

        let realm_value = RealmV1::new(RealmV1Input {
            token_program: token_program.to_bytes(),
            collateral_mint: mint.to_bytes(),
            collateral_adapter_release_id: hash(&PRODUCTION_ADAPTER_RELEASES[0].to_bytes())
                .to_bytes(),
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("Realm");
        let realm_data = realm_value.to_bytes().to_vec();
        let realm_digest = hash(&realm_data).to_bytes();
        let realm =
            Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], &protocol_program).0;

        let policy = VenueFeePolicyV3::new(fee_destination.to_bytes(), 25).expect("fee policy");
        let mut policy_data = vec![0_u8; VENUE_FEE_POLICY_BYTES_V3];
        policy.encode(&mut policy_data).expect("policy bytes");
        let policy_digest = hash(&policy_data).to_bytes();
        let policy_key = raw_record_address(
            protocol_program,
            VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
            policy_digest,
        );

        let entry = CapabilityEntryV1::new(
            capability_id(DIRECT_CAPABILITY_KIND_ID_V2),
            capability_id(COMPILED_DIRECT_RELEASE_ID_V1),
            capability_id(policy_digest),
            capability_id(COMPILED_DIRECT_CAPACITY_ID_V1),
            capability_id(COMPILED_DIRECT_CHILD_SCHEMA_ID_V1),
            capability_id(COMPILED_DIRECT_DERIVATION_ID_V1),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            zero_quote(),
        )
        .expect("Direct capability");
        let mut manifest_data = vec![0_u8; 16 + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest_data).expect("manifest");
        let manifest_digest = hash(&manifest_data).to_bytes();
        let manifest_key = raw_record_address(
            protocol_program,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            manifest_digest,
        );

        let identity = MarketIdentity::new(
            core_id(realm_digest),
            core_id([21; 32]),
            core_id([22; 32]),
            core_id([23; 32]),
            core_id(manifest_digest),
            GENERATION,
        );
        let mut root = MarketRoot::founding(identity, [24; 32]).expect("Market root");
        root.transition_phase(GENERATION, Phase::Open)
            .expect("open Market");
        let market_value =
            CategoricalMarketV1::<2>::new(root, 0, [0; 2], CategoricalSettlementSummaryV1::empty())
                .expect("Market");
        let mut market_data =
            vec![0_u8; CategoricalMarketV1::<2>::encoded_len().expect("Market width")];
        market_value.encode(&mut market_data).expect("Market bytes");
        let market = Pubkey::find_program_address(
            &[MARKET_SEED, &hash(&identity.to_bytes()).to_bytes()],
            &protocol_program,
        )
        .0;

        let (controller, _) = Pubkey::find_program_address(&[CONTROLLER_SEED], &controller_program);
        let seller_intent = direct_intent(market, seller_destination, 0, SELLER_NONCE);
        let buyer_intent = direct_intent(market, buyer_source, 1, BUYER_NONCE);
        let seller_state = RegisteredIntentStateV1 {
            phase: 0,
            controller: controller.to_bytes(),
            maker: seller_maker.to_bytes(),
            intent: seller_intent,
            remaining: 1_500,
            sequence: 7,
        };
        let buyer_state = RegisteredIntentStateV1 {
            phase: 0,
            controller: controller.to_bytes(),
            maker: buyer_maker.to_bytes(),
            intent: buyer_intent,
            remaining: 1_500,
            sequence: 8,
        };
        let (seller_registration, _) =
            registration_address(controller_program, seller_state).expect("seller registration");
        let (buyer_registration, _) =
            registration_address(controller_program, buyer_state).expect("buyer registration");
        let (seller_position, _) =
            position_address(controller_program, market, seller_state).expect("seller Position");
        let (buyer_position, _) =
            position_address(controller_program, market, buyer_state).expect("buyer Position");

        (
            controller_program,
            RegisteredDirectFillState {
                controller: observed(snapshot, controller, system_program::ID, false, Vec::new()),
                seller_registration: observed(
                    snapshot,
                    seller_registration,
                    CLAIM_PROGRAM_ID,
                    false,
                    registration(controller, seller_maker, seller_intent, 7),
                ),
                buyer_registration: observed(
                    snapshot,
                    buyer_registration,
                    CLAIM_PROGRAM_ID,
                    false,
                    registration(controller, buyer_maker, buyer_intent, 8),
                ),
                journal: observed(snapshot, key(30), controller_program, false, Vec::new()),
                seller_position: observed(
                    snapshot,
                    seller_position,
                    CLAIM_PROGRAM_ID,
                    false,
                    Vec::new(),
                ),
                buyer_position: observed(
                    snapshot,
                    buyer_position,
                    CLAIM_PROGRAM_ID,
                    false,
                    Vec::new(),
                ),
                claim_program: observed(
                    snapshot,
                    CLAIM_PROGRAM_ID,
                    native_loader::ID,
                    true,
                    Vec::new(),
                ),
                custody_program: observed(
                    snapshot,
                    CUSTODY_PROGRAM_ID,
                    native_loader::ID,
                    true,
                    Vec::new(),
                ),
                market: observed(snapshot, market, protocol_program, false, market_data),
                realm: observed(snapshot, realm, protocol_program, false, realm_data),
                fee_policy: observed(snapshot, policy_key, protocol_program, false, policy_data),
                capability_manifest: observed(
                    snapshot,
                    manifest_key,
                    protocol_program,
                    false,
                    manifest_data,
                ),
                mint: observed(snapshot, mint, token_program, false, Vec::new()),
                buyer_source: observed(snapshot, buyer_source, token_program, false, Vec::new()),
                seller_destination: observed(
                    snapshot,
                    seller_destination,
                    token_program,
                    false,
                    Vec::new(),
                ),
                fee_destination: observed(
                    snapshot,
                    fee_destination,
                    token_program,
                    false,
                    Vec::new(),
                ),
                token_program: observed(
                    snapshot,
                    token_program,
                    native_loader::ID,
                    true,
                    Vec::new(),
                ),
            },
        )
    }

    fn terminal_state(fill: &RegisteredDirectFillState) -> RegisteredDirectTerminalState {
        RegisteredDirectTerminalState {
            controller: fill.controller.clone(),
            registration: fill.seller_registration.clone(),
            claim_program: fill.claim_program.clone(),
        }
    }

    fn set_terminal_slot(state: &mut RegisteredDirectTerminalState, slot: u64) {
        let snapshot = observation(slot);
        state.controller.observation = snapshot;
        state.registration.observation = snapshot;
        state.claim_program.observation = snapshot;
    }

    #[test]
    fn registered_fill_is_exactly_chain_derived_and_codec_owned() {
        let (program, state) = fixture();
        let report =
            build_registered_direct_fill(program, &state, 1_000, 500_000).expect("registered fill");
        assert_eq!(
            report.instruction.accounts.len(),
            REGISTERED_FILL_ACCOUNT_COUNT
        );
        assert_eq!(report.observation, observation(55));
        assert_eq!(report.seller_registration, state.seller_registration.key);
        assert_eq!(report.buyer_registration, state.buyer_registration.key);
        assert_eq!(report.seller_position, state.seller_position.key);
        assert_eq!(report.buyer_position, state.buyer_position.key);
        let request = RegisteredFillInstructionV1::decode(&report.instruction.data)
            .expect("codec-owned fill request");
        assert_eq!((request.fill, request.execution_price), (1_000, 500_000));
        assert_eq!(
            report.instruction.accounts.first(),
            Some(&AccountMeta::new_readonly(state.controller.key, false))
        );
        assert_eq!(
            report.instruction.accounts.get(16),
            Some(&AccountMeta::new_readonly(state.token_program.key, false))
        );
    }

    #[test]
    fn registered_fill_refuses_hostile_snapshots_bindings_and_coordinates() {
        let (program, state) = fixture();
        let mut mixed = state.clone();
        mixed.buyer_registration.observation.slot += 1;
        assert_eq!(
            build_registered_direct_fill(program, &mixed, 1_000, 500_000),
            Err(Error::ObservationMismatch)
        );

        let mut malformed = state.clone();
        malformed.seller_registration.data.pop();
        assert_eq!(
            build_registered_direct_fill(program, &malformed, 1_000, 500_000),
            Err(Error::InvalidRegistration)
        );

        let mut wrong_pda = state.clone();
        wrong_pda.seller_registration.key = key(99);
        assert_eq!(
            build_registered_direct_fill(program, &wrong_pda, 1_000, 500_000),
            Err(Error::PdaMismatch)
        );

        let mut aliased = state.clone();
        aliased.fee_destination.key = aliased.seller_destination.key;
        assert_eq!(
            build_registered_direct_fill(program, &aliased, 1_000, 500_000),
            Err(Error::InvalidAccount)
        );
        assert_eq!(
            build_registered_direct_fill(program, &state, 1_501, 500_000),
            Err(Error::InvalidCoordinates)
        );
    }

    #[test]
    fn maker_cancel_and_permissionless_expiry_use_persisted_sequence() {
        let (program, fill) = fixture();
        let cancel_state = terminal_state(&fill);
        let cancel = build_registered_direct_maker_cancel(program, &cancel_state)
            .expect("maker cancellation");
        assert_eq!(
            cancel.instruction.accounts.len(),
            REGISTERED_CANCEL_ACCOUNT_COUNT
        );
        assert_eq!(cancel.maker, Some(key(1)));
        assert_eq!(cancel.expected_sequence, 7);
        assert!(
            cancel.instruction.accounts.get(2).is_some_and(|meta| {
                meta.pubkey == key(1) && meta.is_signer && !meta.is_writable
            })
        );
        let cancel_request = RegisteredTerminalInstructionV1::decode(&cancel.instruction.data)
            .expect("codec cancel");
        assert_eq!(cancel_request.action, RegisteredTerminalAction::Cancel);
        assert_eq!(cancel_request.expected_sequence, 7);

        let mut expiry_state = terminal_state(&fill);
        assert_eq!(
            build_registered_direct_permissionless_expiry(program, &expiry_state),
            Err(Error::ExpiryTooEarly)
        );
        set_terminal_slot(&mut expiry_state, 61);
        let expiry = build_registered_direct_permissionless_expiry(program, &expiry_state)
            .expect("permissionless expiry");
        assert_eq!(
            expiry.instruction.accounts.len(),
            REGISTERED_EXPIRY_ACCOUNT_COUNT
        );
        assert_eq!(expiry.maker, None);
        assert!(
            expiry
                .instruction
                .accounts
                .iter()
                .all(|meta| !meta.is_signer)
        );
        let expiry_request = RegisteredTerminalInstructionV1::decode(&expiry.instruction.data)
            .expect("codec expiry");
        assert_eq!(expiry_request.action, RegisteredTerminalAction::Expire);
        assert_eq!(expiry_request.expected_sequence, 7);
    }

    #[test]
    fn terminal_routes_refuse_hostile_state_and_pda_substitution() {
        let (program, fill) = fixture();
        let mut state = terminal_state(&fill);
        state.registration.owner = system_program::ID;
        assert_eq!(
            build_registered_direct_maker_cancel(program, &state),
            Err(Error::InvalidAccount)
        );

        let mut state = terminal_state(&fill);
        state.registration.data.pop();
        assert_eq!(
            build_registered_direct_maker_cancel(program, &state),
            Err(Error::InvalidRegistration)
        );

        let mut state = terminal_state(&fill);
        state.registration.key = key(98);
        assert_eq!(
            build_registered_direct_maker_cancel(program, &state),
            Err(Error::PdaMismatch)
        );

        let mut state = terminal_state(&fill);
        state.claim_program.observation.finality = Finality::Confirmed;
        assert_eq!(
            build_registered_direct_maker_cancel(program, &state),
            Err(Error::ObservationNotFinalized)
        );
    }
}
