//! Chain-derived construction for the compiled Direct two-instruction batch.

use crate::{
    Finality, Observation, ObservedAccount,
    versioned::{VersionedMessagePlanV0, compile_v0_message},
};
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1,
};
use dclutch_core_contract::{MARKET_ROOT_BYTES, MarketRoot, Phase};
use dclutch_direct_codec::{
    COMPACT_INTENT_BYTES, COMPILED_DIRECT_CAPACITY_ID_V1, COMPILED_DIRECT_CHILD_SCHEMA_ID_V1,
    COMPILED_DIRECT_DERIVATION_ID_V1, COMPILED_DIRECT_RELEASE_ID_V1, CompactIntentV1,
    ControllerInstructionV1,
};
use dclutch_direct_contract::{
    DIRECT_CAPABILITY_KIND_ID_V2, VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3, VenueFeePolicyV3,
};
use dclutch_market_contract::market::{MARKET_ROOT_OFFSET, decode_market_outcome_count};
use dclutch_realm_contract::{REALM_PDA_DOMAIN, RealmV1};
use dclutch_record_contract::RAW_RECORD_PDA_SEED_V1;
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{ed25519_program, sysvar};

/// Global compiled-Direct controller authority seed.
pub const CONTROLLER_SEED: &[u8] = b"dclutch-controller-v1";
/// Compiled-Direct replay-root seed.
pub const REPLAY_SEED: &[u8] = b"dclutch/direct-replay/v3";
/// Compiled-Direct maker/outcome Position seed.
pub const POSITION_SEED: &[u8] = b"dclutch/position/v1";
/// Canonical protocol Market PDA domain.
pub const MARKET_SEED: &[u8] = b"dclutch/market-root/v1";
/// Pinned experimental claim-child identity.
pub const CLAIM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);
/// Pinned real custody-child identity.
pub const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([75_u8; 32]);

const ED_DESCRIPTOR_BYTES: usize = 14;
const ED_PAYLOAD_OFFSET: usize = 2 + 2 * ED_DESCRIPTOR_BYTES;
const SELLER_MESSAGE_OFFSET: usize = 32;
const BUYER_MESSAGE_OFFSET: usize = 168;

#[derive(Clone, Copy)]
struct AuthorityFacts {
    generation: u64,
    fee_basis_points: u16,
}

/// Untrusted signature material paired with the exact intent it purports to sign.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedCompactIntentV1 {
    /// Native Ed25519 public key that owns maker identity.
    pub maker: Pubkey,
    /// Detached Ed25519 signature over the exact encoded compact intent.
    pub signature: [u8; 64],
    /// Exact reusable limit intent.
    pub intent: CompactIntentV1,
}

/// Same-finalized chain state required to construct one compiled Direct frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledDirectState {
    /// Controller PDA account.
    pub controller: ObservedAccount,
    /// Seller canonical replay root.
    pub seller_replay: ObservedAccount,
    /// Buyer canonical replay root.
    pub buyer_replay: ObservedAccount,
    /// Controller-owned transaction journal.
    pub journal: ObservedAccount,
    /// Seller canonical maker/outcome Position.
    pub seller_position: ObservedAccount,
    /// Buyer canonical maker/outcome Position.
    pub buyer_position: ObservedAccount,
    /// Pinned executable claim child.
    pub claim_program: ObservedAccount,
    /// Pinned executable custody child.
    pub custody_program: ObservedAccount,
    /// Canonical active Market selected by both intents.
    pub market: ObservedAccount,
    /// Immutable Realm selected by the Market identity.
    pub realm: ObservedAccount,
    /// Finalized venue fee policy selected by the Direct manifest entry.
    pub fee_policy: ObservedAccount,
    /// Finalized capability manifest selected by the Market identity.
    pub capability_manifest: ObservedAccount,
    /// Realm-selected collateral mint.
    pub mint: ObservedAccount,
    /// Buyer collateral source selected by the buyer intent.
    pub buyer_source: ObservedAccount,
    /// Seller collateral destination selected by the seller intent.
    pub seller_destination: ObservedAccount,
    /// Policy-selected fee destination.
    pub fee_destination: ObservedAccount,
    /// Realm-selected executable token program.
    pub token_program: ObservedAccount,
}

/// Matcher coordinates; admission remains solely onchain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatchCoordinatesV1 {
    /// Proposed fill.
    pub fill: u64,
    /// Proposed execution price at the profile scale.
    pub execution_price: u64,
}

/// Exact native-Ed25519 plus controller transaction material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledDirectReport {
    /// Native Ed25519 verification followed by compiled controller execution.
    pub instructions: [Instruction; 2],
    /// Same finalized observation that selected all accounts.
    pub observation: Observation,
    /// Derived global controller PDA.
    pub controller: Pubkey,
    /// Derived seller replay root.
    pub seller_replay: Pubkey,
    /// Derived buyer replay root.
    pub buyer_replay: Pubkey,
    /// Derived seller Position.
    pub seller_position: Pubkey,
    /// Derived buyer Position.
    pub buyer_position: Pubkey,
    /// Reusable Market-scoped routing addresses suitable for one lookup table.
    ///
    /// These keys compress transaction packets but never become protocol
    /// authority; the controller still authenticates every loaded account.
    pub market_lookup_addresses: [Pubkey; 12],
}

/// Refusal from stale, inconsistent, or noncanonical chain state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// One input was not finalized.
    ObservationNotFinalized,
    /// Inputs did not share one exact observation.
    ObservationMismatch,
    /// Market, Realm, capability manifest, or fee-policy bytes were invalid.
    InvalidAuthority,
    /// A program/state owner or executable bit was incompatible.
    InvalidAccount,
    /// Signed intent/Market bindings differed.
    IntentBinding,
    /// Maker keys or detached signatures were invalid at the structural layer.
    SignatureMaterial,
    /// An observed canonical PDA differed from its derivation.
    PdaMismatch,
    /// Fixed instruction encoding failed.
    Encoding,
}

/// Build, but never sign or submit, the exact compiled Direct instruction pair.
///
/// Detached signatures remain untrusted until the native Ed25519 instruction
/// executes. This builder validates structural material and derives every PDA;
/// economic admission remains exclusively in the compiled onchain transition.
pub fn build_compiled_direct(
    controller_program: Pubkey,
    state: &CompiledDirectState,
    seller: SignedCompactIntentV1,
    buyer: SignedCompactIntentV1,
    coordinates: MatchCoordinatesV1,
) -> Result<CompiledDirectReport, Error> {
    let observation = same_finalized_observation(state)?;
    let authority = authenticate_authority(state)?;
    validate_program_accounts(controller_program, state)?;
    if seller.maker == Pubkey::default()
        || buyer.maker == Pubkey::default()
        || seller.maker == buyer.maker
        || seller.signature.iter().all(|byte| *byte == 0)
        || buyer.signature.iter().all(|byte| *byte == 0)
    {
        return Err(Error::SignatureMaterial);
    }
    let market_key = state.market.key.to_bytes();
    if seller.intent.market != market_key
        || buyer.intent.market != market_key
        || seller.intent.generation != authority.generation
        || buyer.intent.generation != authority.generation
        || seller.intent.fee_basis_points != authority.fee_basis_points
        || buyer.intent.fee_basis_points != authority.fee_basis_points
        || seller.intent.collateral_account != state.seller_destination.key.to_bytes()
        || buyer.intent.collateral_account != state.buyer_source.key.to_bytes()
    {
        return Err(Error::IntentBinding);
    }

    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &controller_program);
    let generation = authority.generation.to_le_bytes();
    let (seller_replay, seller_replay_bump) = Pubkey::find_program_address(
        &[
            REPLAY_SEED,
            state.market.key.as_ref(),
            &generation,
            seller.maker.as_ref(),
        ],
        &controller_program,
    );
    let (buyer_replay, buyer_replay_bump) = Pubkey::find_program_address(
        &[
            REPLAY_SEED,
            state.market.key.as_ref(),
            &generation,
            buyer.maker.as_ref(),
        ],
        &controller_program,
    );
    let seller_outcome = [seller.intent.outcome];
    let buyer_outcome = [buyer.intent.outcome];
    let (seller_position, seller_position_bump) = Pubkey::find_program_address(
        &[
            POSITION_SEED,
            state.market.key.as_ref(),
            seller.maker.as_ref(),
            &seller_outcome,
        ],
        &controller_program,
    );
    let (buyer_position, buyer_position_bump) = Pubkey::find_program_address(
        &[
            POSITION_SEED,
            state.market.key.as_ref(),
            buyer.maker.as_ref(),
            &buyer_outcome,
        ],
        &controller_program,
    );
    if state.controller.key != controller
        || state.seller_replay.key != seller_replay
        || state.buyer_replay.key != buyer_replay
        || state.seller_position.key != seller_position
        || state.buyer_position.key != buyer_position
    {
        return Err(Error::PdaMismatch);
    }

    let controller_data = ControllerInstructionV1 {
        controller_bump,
        seller_replay_bump,
        buyer_replay_bump,
        seller_position_bump,
        buyer_position_bump,
        fill: coordinates.fill,
        execution_price: coordinates.execution_price,
        seller: seller.intent,
        buyer: buyer.intent,
    }
    .encode()
    .map_err(|_| Error::Encoding)?;
    let controller_instruction = Instruction {
        program_id: controller_program,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new(seller_replay, false),
            AccountMeta::new(buyer_replay, false),
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
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
        ],
        data: controller_data.to_vec(),
    };
    let signature_instruction = ed25519_batch(seller, buyer, &controller_data)?;

    Ok(CompiledDirectReport {
        instructions: [signature_instruction, controller_instruction],
        observation,
        controller,
        seller_replay,
        buyer_replay,
        seller_position,
        buyer_position,
        market_lookup_addresses: [
            controller,
            state.journal.key,
            CLAIM_PROGRAM_ID,
            CUSTODY_PROGRAM_ID,
            state.market.key,
            state.realm.key,
            state.fee_policy.key,
            state.capability_manifest.key,
            state.mint.key,
            state.fee_destination.key,
            state.token_program.key,
            sysvar::instructions::ID,
        ],
    })
}

/// Compile the exact Direct instruction pair into a packet-safe unsigned v0
/// message using finalized, already-active lookup-table observations.
///
/// This function does not sign or submit. Lookup tables are only a routing
/// projection: the controller remains responsible for all semantic authority.
pub fn compile_compiled_direct_v0(
    report: &CompiledDirectReport,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
) -> Result<VersionedMessagePlanV0, crate::versioned::Error> {
    compile_v0_message(
        payer,
        &report.instructions,
        recent_blockhash,
        report.observation,
        lookup_tables,
    )
}

fn same_finalized_observation(state: &CompiledDirectState) -> Result<Observation, Error> {
    let accounts = [
        &state.controller,
        &state.seller_replay,
        &state.buyer_replay,
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
    ];
    let observation = state.market.observation;
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

fn authenticate_authority(state: &CompiledDirectState) -> Result<AuthorityFacts, Error> {
    let protocol_program = state.market.owner;
    if state.realm.owner != protocol_program
        || state.fee_policy.owner != protocol_program
        || state.capability_manifest.owner != protocol_program
    {
        return Err(Error::InvalidAuthority);
    }
    decode_market_outcome_count(&state.market.data).map_err(|_| Error::InvalidAuthority)?;
    let root_end = MARKET_ROOT_OFFSET
        .checked_add(MARKET_ROOT_BYTES)
        .ok_or(Error::Encoding)?;
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
        index = index.checked_add(1).ok_or(Error::Encoding)?;
    }
    let entry = selected.ok_or(Error::InvalidAuthority)?;
    let funding = entry.funding_quote();
    if entry.release_id().to_bytes() != COMPILED_DIRECT_RELEASE_ID_V1
        || entry.config_id().to_bytes() != policy_digest
        || entry.capacity_profile_id().to_bytes() != COMPILED_DIRECT_CAPACITY_ID_V1
        || entry.child_schema_id().to_bytes() != COMPILED_DIRECT_CHILD_SCHEMA_ID_V1
        || entry.child_derivation_id().to_bytes() != COMPILED_DIRECT_DERIVATION_ID_V1
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

fn validate_program_accounts(
    controller_program: Pubkey,
    state: &CompiledDirectState,
) -> Result<(), Error> {
    if state.controller.executable
        || state.seller_replay.owner != CLAIM_PROGRAM_ID
        || state.seller_replay.executable
        || state.buyer_replay.owner != CLAIM_PROGRAM_ID
        || state.buyer_replay.executable
        || state.seller_position.owner != CLAIM_PROGRAM_ID
        || state.seller_position.executable
        || state.buyer_position.owner != CLAIM_PROGRAM_ID
        || state.buyer_position.executable
        || state.journal.owner != controller_program
        || state.journal.executable
        || state.market.executable
        || state.realm.executable
        || state.fee_policy.executable
        || state.capability_manifest.executable
        || state.claim_program.key != CLAIM_PROGRAM_ID
        || !state.claim_program.executable
        || state.custody_program.key != CUSTODY_PROGRAM_ID
        || !state.custody_program.executable
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

fn ed25519_batch(
    seller: SignedCompactIntentV1,
    buyer: SignedCompactIntentV1,
    controller_data: &[u8],
) -> Result<Instruction, Error> {
    let mut data = vec![0_u8; ED_PAYLOAD_OFFSET + 2 * 96];
    put_u16(&mut data, 0, 2)?;
    for (index, material, message_offset) in [
        (0_usize, seller, SELLER_MESSAGE_OFFSET),
        (1_usize, buyer, BUYER_MESSAGE_OFFSET),
    ] {
        let descriptor = 2 + index * ED_DESCRIPTOR_BYTES;
        let public_key_offset = ED_PAYLOAD_OFFSET + index * 96;
        let signature_offset = public_key_offset + 32;
        put_u16(&mut data, descriptor, to_u16(signature_offset)?)?;
        put_u16(&mut data, descriptor + 2, u16::MAX)?;
        put_u16(&mut data, descriptor + 4, to_u16(public_key_offset)?)?;
        put_u16(&mut data, descriptor + 6, u16::MAX)?;
        put_u16(&mut data, descriptor + 8, to_u16(message_offset)?)?;
        put_u16(&mut data, descriptor + 10, to_u16(COMPACT_INTENT_BYTES)?)?;
        put_u16(&mut data, descriptor + 12, 1)?;
        put(&mut data, public_key_offset, material.maker.as_ref())?;
        put(&mut data, signature_offset, &material.signature)?;
        let end = message_offset
            .checked_add(COMPACT_INTENT_BYTES)
            .ok_or(Error::Encoding)?;
        if controller_data.get(message_offset..end).is_none() {
            return Err(Error::Encoding);
        }
    }
    Ok(Instruction {
        program_id: ed25519_program::ID,
        accounts: vec![],
        data,
    })
}

fn to_u16(value: usize) -> Result<u16, Error> {
    u16::try_from(value).map_err(|_| Error::Encoding)
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) -> Result<(), Error> {
    put(output, offset, &value.to_le_bytes())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = offset.checked_add(value.len()).ok_or(Error::Encoding)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Encoding)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_contract::{
        CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1, FundingAmountsV1,
        FundingQuoteV1, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::{ContentId, MarketIdentity};
    use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
    use dclutch_realm_contract::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1Input};
    use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
    use solana_address_lookup_table_interface::{
        program as lookup_table_program,
        state::{AddressLookupTable, LookupTableMeta},
    };
    use solana_sdk_ids::system_program;
    use std::borrow::Cow;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
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

    fn intent(market: Pubkey, collateral: Pubkey, side: u8) -> CompactIntentV1 {
        CompactIntentV1 {
            side,
            outcome: 1,
            lifecycle: 0,
            market: market.to_bytes(),
            generation: 3,
            nonce: 0,
            valid_from: 0,
            valid_through: u64::MAX,
            maximum_fill: 2_000,
            limit_price: if side == 0 { 400_000 } else { 600_000 },
            fee_basis_points: 25,
            collateral_account: collateral.to_bytes(),
        }
    }

    fn content(bytes: [u8; 32]) -> ContentId {
        ContentId::new(bytes).expect("nonzero content ID")
    }

    fn zero_quote() -> FundingQuoteV1 {
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("zero funding"),
            None,
        )
        .expect("zero quote")
    }

    fn fixture() -> (
        Pubkey,
        CompiledDirectState,
        SignedCompactIntentV1,
        SignedCompactIntentV1,
    ) {
        let observation = Observation {
            slot: 55,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        };
        let program = key(67);
        let protocol_program = key(68);
        let seller = key(1);
        let buyer = key(2);
        let seller_destination = key(5);
        let buyer_source = key(6);
        let token_program = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);
        let mint = key(8);
        let fee_destination = key(9);

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
        let mut policy_data = vec![0_u8; dclutch_direct_contract::VENUE_FEE_POLICY_BYTES_V3];
        policy.encode(&mut policy_data).expect("fee policy bytes");
        let policy_digest = hash(&policy_data).to_bytes();
        let policy_key = raw_record_address(
            protocol_program,
            VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
            policy_digest,
        );
        let entry = CapabilityEntryV1::new(
            content(DIRECT_CAPABILITY_KIND_ID_V2),
            content(COMPILED_DIRECT_RELEASE_ID_V1),
            content(policy_digest),
            content(COMPILED_DIRECT_CAPACITY_ID_V1),
            content(COMPILED_DIRECT_CHILD_SCHEMA_ID_V1),
            content(COMPILED_DIRECT_DERIVATION_ID_V1),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            zero_quote(),
        )
        .expect("compiled Direct entry");
        let mut manifest_data = vec![0_u8; 16 + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest_data)
            .expect("capability manifest");
        let manifest_digest = hash(&manifest_data).to_bytes();
        let manifest_key = raw_record_address(
            protocol_program,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            manifest_digest,
        );

        let identity = MarketIdentity::new(
            content(realm_digest),
            content([21; 32]),
            content([22; 32]),
            content([23; 32]),
            content(manifest_digest),
            3,
        );
        let mut root = MarketRoot::founding(identity, [24; 32]).expect("Market root");
        root.transition_phase(3, Phase::Open).expect("open Market");
        let market_value =
            CategoricalMarketV1::<2>::new(root, 0, [0; 2], CategoricalSettlementSummaryV1::empty())
                .expect("Market");
        let mut market_data =
            vec![0_u8; CategoricalMarketV1::<2>::encoded_len().expect("Market width")];
        market_value.encode(&mut market_data).expect("Market bytes");
        let identity_digest = hash(&identity.to_bytes()).to_bytes();
        let market_key =
            Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], &protocol_program).0;
        let generation = 3_u64.to_le_bytes();
        let outcome = [1_u8];
        let (controller, _) = Pubkey::find_program_address(&[CONTROLLER_SEED], &program);
        let (seller_replay, _) = Pubkey::find_program_address(
            &[
                REPLAY_SEED,
                market_key.as_ref(),
                &generation,
                seller.as_ref(),
            ],
            &program,
        );
        let (buyer_replay, _) = Pubkey::find_program_address(
            &[
                REPLAY_SEED,
                market_key.as_ref(),
                &generation,
                buyer.as_ref(),
            ],
            &program,
        );
        let (seller_position, _) = Pubkey::find_program_address(
            &[
                POSITION_SEED,
                market_key.as_ref(),
                seller.as_ref(),
                &outcome,
            ],
            &program,
        );
        let (buyer_position, _) = Pubkey::find_program_address(
            &[POSITION_SEED, market_key.as_ref(), buyer.as_ref(), &outcome],
            &program,
        );
        let state = CompiledDirectState {
            controller: observed(observation, controller, system_program::ID, false, vec![]),
            seller_replay: observed(observation, seller_replay, CLAIM_PROGRAM_ID, false, vec![]),
            buyer_replay: observed(observation, buyer_replay, CLAIM_PROGRAM_ID, false, vec![]),
            journal: observed(observation, key(10), program, false, vec![]),
            seller_position: observed(
                observation,
                seller_position,
                CLAIM_PROGRAM_ID,
                false,
                vec![],
            ),
            buyer_position: observed(observation, buyer_position, CLAIM_PROGRAM_ID, false, vec![]),
            claim_program: observed(observation, CLAIM_PROGRAM_ID, key(99), true, vec![]),
            custody_program: observed(observation, CUSTODY_PROGRAM_ID, key(99), true, vec![]),
            market: observed(
                observation,
                market_key,
                protocol_program,
                false,
                market_data,
            ),
            realm: observed(observation, realm, protocol_program, false, realm_data),
            fee_policy: observed(
                observation,
                policy_key,
                protocol_program,
                false,
                policy_data,
            ),
            capability_manifest: observed(
                observation,
                manifest_key,
                protocol_program,
                false,
                manifest_data,
            ),
            mint: observed(observation, mint, token_program, false, vec![]),
            buyer_source: observed(observation, buyer_source, token_program, false, vec![]),
            seller_destination: observed(
                observation,
                seller_destination,
                token_program,
                false,
                vec![],
            ),
            fee_destination: observed(observation, fee_destination, token_program, false, vec![]),
            token_program: observed(observation, token_program, key(99), true, vec![]),
        };
        (
            program,
            state,
            SignedCompactIntentV1 {
                maker: seller,
                signature: [11; 64],
                intent: intent(market_key, seller_destination, 0),
            },
            SignedCompactIntentV1 {
                maker: buyer,
                signature: [12; 64],
                intent: intent(market_key, buyer_source, 1),
            },
        )
    }

    #[test]
    fn derives_exact_batch_from_one_finalized_snapshot() {
        let (program, state, seller, buyer) = fixture();
        let report = build_compiled_direct(
            program,
            &state,
            seller,
            buyer,
            MatchCoordinatesV1 {
                fill: 2_000,
                execution_price: 500_000,
            },
        )
        .expect("canonical batch");
        assert_eq!(report.instructions[0].program_id, ed25519_program::ID);
        assert_eq!(report.instructions[1].program_id, program);
        assert_eq!(report.instructions[1].accounts.len(), 18);
        let decoded =
            ControllerInstructionV1::decode(&report.instructions[1].data).expect("controller data");
        assert_eq!(decoded.seller, seller.intent);
        assert_eq!(decoded.buyer, buyer.intent);
        assert_eq!(decoded.fill, 2_000);
        assert_eq!(report.instructions[0].data.len(), 222);
        assert_eq!(report.market_lookup_addresses.len(), 12);

        let table_key = key(91);
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(key(92)),
                last_extended_slot: report.observation.slot - 1,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(report.market_lookup_addresses.to_vec()),
        };
        let table_account = observed(
            report.observation,
            table_key,
            lookup_table_program::id(),
            false,
            table.serialize_for_tests().expect("lookup table bytes"),
        );
        let versioned = compile_compiled_direct_v0(
            &report,
            key(93),
            Hash::new_from_array([94; 32]),
            &[table_account],
        )
        .expect("Market-scoped v0 message");
        assert_eq!(versioned.required_signatures, 1);
        assert_eq!(versioned.loaded_addresses, 12);
        assert_eq!(versioned.wire_bytes, 990);
        eprintln!(
            "compiled Direct reusable Market-table v0 wire bytes: {}",
            versioned.wire_bytes
        );
    }

    #[test]
    fn refuses_stale_authority_and_structural_signature_material() {
        let (program, mut state, seller, buyer) = fixture();
        state.buyer_replay.key = Pubkey::new_unique();
        assert_eq!(
            build_compiled_direct(
                program,
                &state,
                seller,
                buyer,
                MatchCoordinatesV1 {
                    fill: 2_000,
                    execution_price: 500_000,
                },
            ),
            Err(Error::PdaMismatch)
        );
        let (program, mut state, seller, mut buyer) = fixture();
        state.market.observation.slot += 1;
        assert_eq!(
            build_compiled_direct(
                program,
                &state,
                seller,
                buyer,
                MatchCoordinatesV1 {
                    fill: 2_000,
                    execution_price: 500_000,
                },
            ),
            Err(Error::ObservationMismatch)
        );
        let (program, state, seller, _) = fixture();
        buyer.signature = [0; 64];
        assert_eq!(
            build_compiled_direct(
                program,
                &state,
                seller,
                buyer,
                MatchCoordinatesV1 {
                    fill: 2_000,
                    execution_price: 500_000,
                },
            ),
            Err(Error::SignatureMaterial)
        );
    }
}
