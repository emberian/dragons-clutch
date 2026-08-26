//! Chain-derived construction for registered Direct execution routes.
//!
//! These builders accept finalized hostile account snapshots, decode the
//! codec-owned registered state, rederive every controller-owned address, and
//! emit unsigned instructions. They never read RPC, sign, or submit.

use crate::{Finality, Observation, ObservedAccount, versioned::PACKET_DATA_BYTES};
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1,
};
use dclutch_core_contract::{MARKET_ROOT_BYTES, MarketRoot, Phase};
use dclutch_direct_codec::{
    CompactIntentV1, REGISTERED_INTENT_STATE_BYTES, RegisteredCreateInstructionV1,
    RegisteredFillInstructionV1, RegisteredIntentStateV1, RegisteredRetireInstructionV1,
    RegisteredTerminalAction, RegisteredTerminalInstructionV1,
};
use dclutch_direct_contract::{
    DIRECT_CAPABILITY_KIND_ID_V2, FEE_BASIS_POINTS_DENOMINATOR, PRICE_SCALE,
    VENUE_FEE_POLICY_BYTES_V3, VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3, VenueFeePolicyV3,
};
use dclutch_market_contract::market::{MARKET_ROOT_OFFSET, decode_market_outcome_count};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_PDA_DOMAIN, RealmV1,
};
use dclutch_record_contract::RAW_RECORD_PDA_SEED_V1;
use dclutch_token_svm::{
    AccountState, COption, ExactTransferProfileV1, LEGACY_TOKEN_PROGRAM_ID,
    PRODUCTION_ADAPTER_RELEASES, TokenAccount,
};
use solana_hash::Hash;
use solana_message::{VersionedMessage, v0};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use spl_token_2022_interface::instruction::approve;

use crate::compiled_direct::{
    CLAIM_PROGRAM_ID, CONTROLLER_SEED, CUSTODY_PROGRAM_ID, MARKET_SEED, POSITION_SEED, REPLAY_SEED,
};

/// Registered-intent PDA domain committed by the controller release.
pub const REGISTERED_SEED: &[u8] = b"dclutch/direct-registered/v1";
/// Exact account count for one signed registered-intent creation.
pub const REGISTERED_CREATE_ACCOUNT_COUNT: usize = 15;
/// Exact instruction count for one prepaid buyer registration transaction.
pub const REGISTERED_CREATE_INSTRUCTION_COUNT: usize = 2;
/// Exact account count for permissionless terminal seller retirement.
pub const REGISTERED_SELLER_RETIRE_ACCOUNT_COUNT: usize = 4;
/// Exact account count for maker-authorized terminal buyer retirement.
pub const REGISTERED_BUYER_RETIRE_ACCOUNT_COUNT: usize = 6;
/// Exact account count for a registered residual fill.
pub const REGISTERED_FILL_ACCOUNT_COUNT: usize = 17;
/// Exact account count for a maker-authorized registered cancellation.
pub const REGISTERED_CANCEL_ACCOUNT_COUNT: usize = 4;
/// Exact account count for a permissionless registered expiry.
pub const REGISTERED_EXPIRY_ACCOUNT_COUNT: usize = 3;

const REPLAY_STATE_BYTES: usize = 48;
const REPLAY_STATE_MAGIC: &[u8; 8] = b"DCRP\x01\0\0\0";

#[derive(Clone, Copy)]
struct AuthorityFacts {
    generation: u64,
    outcome_count: u8,
    fee_basis_points: u16,
}

#[derive(Clone, Copy)]
struct AuthorityAccounts<'a> {
    market: &'a ObservedAccount,
    realm: &'a ObservedAccount,
    fee_policy: &'a ObservedAccount,
    capability_manifest: &'a ObservedAccount,
    mint: &'a ObservedAccount,
    fee_destination: &'a ObservedAccount,
    token_program: &'a ObservedAccount,
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

/// Same-finalized chain state required to create one prepaid buyer intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectCreationState {
    /// Executable controller program selected for this experimental route.
    pub controller_program: ObservedAccount,
    /// Global controller PDA observation.
    pub controller: ObservedAccount,
    /// Maker account which must sign both the approval and registration.
    pub maker: ObservedAccount,
    /// System-owned sponsor paying the exact missing PDA rent.
    pub payer: ObservedAccount,
    /// Maker/Market/generation global replay-root observation.
    pub replay: ObservedAccount,
    /// Vacant exact maker/Market/generation/nonce registration PDA.
    pub registration: ObservedAccount,
    /// Pinned executable claim child.
    pub claim_program: ObservedAccount,
    /// Pinned executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical active Market selected by the intent.
    pub market: ObservedAccount,
    /// Immutable Realm selected by the Market identity.
    pub realm: ObservedAccount,
    /// Finalized venue fee policy selected by Direct.
    pub fee_policy: ObservedAccount,
    /// Finalized capability manifest selected by the Market identity.
    pub capability_manifest: ObservedAccount,
    /// Realm-selected collateral mint.
    pub mint: ObservedAccount,
    /// Maker-owned buyer collateral source approved atomically.
    pub collateral: ObservedAccount,
    /// Policy-selected fee destination.
    pub fee_destination: ObservedAccount,
    /// Realm-selected executable legacy SPL Token program.
    pub token_program: ObservedAccount,
    /// Canonical finalized Rent sysvar used for exact PDA top-up sizing.
    pub rent_sysvar: ObservedAccount,
}

/// Same-finalized state for permissionless terminal seller retirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectSellerRetirementState {
    /// Executable controller program selected for this route.
    pub controller_program: ObservedAccount,
    /// Global controller PDA observation.
    pub controller: ObservedAccount,
    /// Terminal seller registration whose rent will be returned.
    pub registration: ObservedAccount,
    /// Persisted maker account receiving the entire registration balance.
    pub maker: ObservedAccount,
    /// Pinned executable claim child.
    pub claim_program: ObservedAccount,
}

/// Same-finalized state for maker-authorized terminal buyer retirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectBuyerRetirementState {
    /// Executable controller program selected for this route.
    pub controller_program: ObservedAccount,
    /// Global controller PDA observation.
    pub controller: ObservedAccount,
    /// Terminal buyer registration whose rent will be returned.
    pub registration: ObservedAccount,
    /// Persisted maker signer and sole rent-refund recipient.
    pub maker: ObservedAccount,
    /// Pinned executable claim child.
    pub claim_program: ObservedAccount,
    /// Persisted buyer collateral account, writable for controller CPI revoke.
    pub collateral: ObservedAccount,
    /// Pinned executable legacy SPL Token program.
    pub token_program: ObservedAccount,
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

/// Chain-derived atomic approval and registered-intent creation route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectCreationReport {
    /// Exact legacy SPL Token approval followed by the controller instruction.
    pub instructions: [Instruction; REGISTERED_CREATE_INSTRUCTION_COUNT],
    /// Shared finalized observation selecting every hostile input account.
    pub observation: Observation,
    /// Derived global controller PDA.
    pub controller: Pubkey,
    /// Derived maker/Market/generation replay-root PDA.
    pub replay: Pubkey,
    /// Derived maker/Market/generation/nonce registration PDA.
    pub registration: Pubkey,
    /// Exact chain-derived nonce embedded in the request.
    pub nonce: u64,
    /// Exact allowance installed by the approval instruction.
    pub delegated_amount: u64,
    /// Exact missing replay-plus-registration rent paid by the sponsor.
    pub rent_debit_lamports: u64,
}

/// Exact unsigned v0 message and its fully signed packet geometry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectCreationPacket {
    /// Unsigned v0 message containing the atomic creation pair.
    pub message: VersionedMessage,
    /// Exact number of transaction signature slots selected by account aliases.
    pub required_signatures: u8,
    /// Serialized transaction bytes after all signature slots are populated.
    pub wire_bytes: usize,
}

/// Token-delegation boundary selected by terminal retirement state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisteredDirectRetirementBoundary {
    /// Seller registrations never grant a token delegation.
    SellerNoDelegation,
    /// Buyer token state is already delegate-free and needs no CPI revoke.
    BuyerAlreadyClear,
    /// The controller must revoke and verify clearance before closing.
    BuyerControllerRevokeRequired,
}

/// Chain-derived terminal registration-retirement instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectRetirementReport {
    /// Exact unsigned 4- or 6-account controller instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting every input account.
    pub observation: Observation,
    /// Persisted maker and sole registration-rent recipient.
    pub maker: Pubkey,
    /// Whether retirement requires the persisted maker's transaction signature.
    pub maker_signature_required: bool,
    /// Exact registration lamports returned to the persisted maker.
    pub rent_refund_lamports: u64,
    /// Explicit controller-side delegation boundary before claim-account close.
    pub boundary: RegisteredDirectRetirementBoundary,
}

/// Exact unsigned retirement v0 message, signer keys, and packet geometry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredDirectRetirementPacket {
    /// Unsigned v0 message containing the retirement instruction.
    pub message: VersionedMessage,
    /// Exact signer keys in message signature-slot order.
    pub required_signers: Vec<Pubkey>,
    /// Fully signed serialized transaction bytes.
    pub wire_bytes: usize,
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
    /// Replay-root bytes or their next nonce were invalid.
    InvalidReplay,
    /// The requested registration was not the exact funded buyer profile.
    InvalidCreationIntent,
    /// Mint, collateral, fee account, or delegation authority was invalid.
    InvalidToken,
    /// The exact creation transaction exceeded the Solana packet limit.
    PacketTooLarge,
    /// The finalized Rent sysvar was malformed.
    InvalidRent,
    /// The payer could not cover the exact missing account rent.
    InsufficientPayer,
    /// Registration state was not terminal and internally coherent.
    InvalidRetirement,
    /// Registration rent could not be credited to the persisted maker.
    InvalidRefund,
    /// A codec-owned request could not be encoded.
    Encoding,
}

/// Build, but never sign or submit, one exact prepaid buyer registration.
///
/// The first instruction is the official legacy SPL Token `Approve`, granting
/// the derived registration PDA exactly the worst-case cumulative buyer debit
/// at the signed limit and fee rate. The second is the codec-owned 15-account
/// controller request. Both must remain in one
/// atomic transaction: creation authenticates the delegation produced by the
/// immediately preceding approval before installing reusable intent state.
pub fn build_registered_direct_creation(
    controller_program: Pubkey,
    state: &RegisteredDirectCreationState,
    intent: CompactIntentV1,
) -> Result<RegisteredDirectCreationReport, Error> {
    let observation = creation_observation(state)?;
    validate_creation_accounts(controller_program, state)?;
    let authority = authenticate_creation_authority(state)?;
    let rent =
        crate::foundation::decode_rent(&state.rent_sysvar).map_err(|_| Error::InvalidRent)?;
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &controller_program);
    if state.controller.key != controller {
        return Err(Error::PdaMismatch);
    }
    let nonce = replay_nonce(&state.replay, controller)?;
    if intent.side != 1
        || intent.lifecycle != 2
        || intent.market != state.market.key.to_bytes()
        || intent.generation != authority.generation
        || intent.nonce != nonce
        || intent.valid_from > intent.valid_through
        || observation.slot > intent.valid_through
        || intent.maximum_fill == 0
        || intent.limit_price > PRICE_SCALE
        || u64::from(intent.fee_basis_points) > FEE_BASIS_POINTS_DENOMINATOR
        || intent.outcome >= authority.outcome_count
        || intent.fee_basis_points != authority.fee_basis_points
        || intent.collateral_account != state.collateral.key.to_bytes()
    {
        return Err(Error::InvalidCreationIntent);
    }
    let delegated_amount = registered_buyer_reserve(intent)?;
    let replay_rent_debit = if state.replay.owner == system_program::ID {
        rent.minimum_balance(REPLAY_STATE_BYTES)
            .saturating_sub(state.replay.lamports)
    } else {
        0
    };
    let registration_rent_debit = rent
        .minimum_balance(REGISTERED_INTENT_STATE_BYTES)
        .saturating_sub(state.registration.lamports);
    let rent_debit_lamports = replay_rent_debit
        .checked_add(registration_rent_debit)
        .ok_or(Error::InvalidRent)?;
    if state.payer.lamports < rent_debit_lamports {
        return Err(Error::InsufficientPayer);
    }

    let generation = authority.generation.to_le_bytes();
    let nonce_seed = nonce.to_le_bytes();
    let (replay, replay_bump) = Pubkey::find_program_address(
        &[
            REPLAY_SEED,
            state.market.key.as_ref(),
            &generation,
            state.maker.key.as_ref(),
        ],
        &controller_program,
    );
    let (registration, registration_bump) = Pubkey::find_program_address(
        &[
            REGISTERED_SEED,
            state.market.key.as_ref(),
            &generation,
            state.maker.key.as_ref(),
            &nonce_seed,
        ],
        &controller_program,
    );
    if state.replay.key != replay || state.registration.key != registration {
        return Err(Error::PdaMismatch);
    }
    validate_creation_tokens(state, delegated_amount, registration)?;

    let approval = approve(
        &state.token_program.key,
        &state.collateral.key,
        &registration,
        &state.maker.key,
        &[],
        delegated_amount,
    )
    .map_err(|_| Error::Encoding)?;
    let data = RegisteredCreateInstructionV1 {
        controller_bump,
        replay_bump,
        registration_bump,
        intent,
    }
    .encode()
    .map_err(|_| Error::Encoding)?;
    let accounts = vec![
        AccountMeta::new_readonly(controller, false),
        AccountMeta::new_readonly(state.maker.key, true),
        AccountMeta::new(state.payer.key, true),
        AccountMeta::new(replay, false),
        AccountMeta::new(registration, false),
        AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(state.market.key, false),
        AccountMeta::new_readonly(state.realm.key, false),
        AccountMeta::new_readonly(state.fee_policy.key, false),
        AccountMeta::new_readonly(state.capability_manifest.key, false),
        AccountMeta::new_readonly(state.mint.key, false),
        AccountMeta::new_readonly(state.collateral.key, false),
        AccountMeta::new_readonly(state.fee_destination.key, false),
        AccountMeta::new_readonly(state.token_program.key, false),
    ];
    debug_assert_eq!(accounts.len(), REGISTERED_CREATE_ACCOUNT_COUNT);
    Ok(RegisteredDirectCreationReport {
        instructions: [
            approval,
            Instruction {
                program_id: controller_program,
                accounts,
                data: data.to_vec(),
            },
        ],
        observation,
        controller,
        replay,
        registration,
        nonce,
        delegated_amount,
        rent_debit_lamports,
    })
}

/// Compile the exact creation pair into a v0 message and measure its
/// fully signed wire size against Solana's 1,232-byte packet limit.
pub fn compile_registered_direct_creation_packet(
    report: &RegisteredDirectCreationReport,
    recent_blockhash: Hash,
) -> Result<RegisteredDirectCreationPacket, Error> {
    let payer = report.instructions[1]
        .accounts
        .get(2)
        .map(|meta| meta.pubkey)
        .ok_or(Error::Encoding)?;
    let message = v0::Message::try_compile(&payer, &report.instructions, &[], recent_blockhash)
        .map_err(|_| Error::Encoding)?;
    let required_signatures = message.header.num_required_signatures;
    let message = VersionedMessage::V0(message);
    let signature_count = usize::from(required_signatures);
    let wire_bytes = short_vec_prefix_bytes(signature_count)
        .checked_add(signature_count.checked_mul(64).ok_or(Error::Encoding)?)
        .and_then(|value| value.checked_add(message.serialize().len()))
        .ok_or(Error::Encoding)?;
    if wire_bytes > PACKET_DATA_BYTES {
        return Err(Error::PacketTooLarge);
    }
    Ok(RegisteredDirectCreationPacket {
        message,
        required_signatures,
        wire_bytes,
    })
}

/// Build one permissionless terminal seller retirement.
///
/// The maker is recovered from persisted registration state, is writable only
/// as the rent recipient, and is deliberately not a required signer.
pub fn build_registered_direct_seller_retirement(
    controller_program: Pubkey,
    state: &RegisteredDirectSellerRetirementState,
) -> Result<RegisteredDirectRetirementReport, Error> {
    let observation = same_observation(&[
        &state.controller_program,
        &state.controller,
        &state.registration,
        &state.maker,
        &state.claim_program,
    ])?;
    let (registration, controller_bump, registration_bump) = authenticate_retirement_core(
        controller_program,
        &state.controller_program,
        &state.controller,
        &state.registration,
        &state.maker,
        &state.claim_program,
        0,
    )?;
    let data = RegisteredRetireInstructionV1 {
        controller_bump,
        registration_bump,
    }
    .encode()
    .map_err(|_| Error::Encoding)?;
    let maker = Pubkey::new_from_array(registration.maker);
    let accounts = vec![
        AccountMeta::new_readonly(state.controller.key, false),
        AccountMeta::new(state.registration.key, false),
        AccountMeta::new(maker, false),
        AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
    ];
    debug_assert_eq!(accounts.len(), REGISTERED_SELLER_RETIRE_ACCOUNT_COUNT);
    Ok(RegisteredDirectRetirementReport {
        instruction: Instruction {
            program_id: controller_program,
            accounts,
            data: data.to_vec(),
        },
        observation,
        maker,
        maker_signature_required: false,
        rent_refund_lamports: state.registration.lamports,
        boundary: RegisteredDirectRetirementBoundary::SellerNoDelegation,
    })
}

/// Build one maker-authorized terminal buyer retirement.
///
/// If the finalized token account still delegates to the exact registration,
/// the returned boundary records that the controller must complete and verify
/// its official SPL Token `Revoke` CPI before invoking the claim-account close.
pub fn build_registered_direct_buyer_retirement(
    controller_program: Pubkey,
    state: &RegisteredDirectBuyerRetirementState,
) -> Result<RegisteredDirectRetirementReport, Error> {
    let observation = same_observation(&[
        &state.controller_program,
        &state.controller,
        &state.registration,
        &state.maker,
        &state.claim_program,
        &state.collateral,
        &state.token_program,
    ])?;
    let (registration, controller_bump, registration_bump) = authenticate_retirement_core(
        controller_program,
        &state.controller_program,
        &state.controller,
        &state.registration,
        &state.maker,
        &state.claim_program,
        1,
    )?;
    require_distinct(&[
        state.controller_program.key,
        state.controller.key,
        state.registration.key,
        state.maker.key,
        state.claim_program.key,
        state.collateral.key,
        state.token_program.key,
    ])?;
    if state.token_program.key.to_bytes() != LEGACY_TOKEN_PROGRAM_ID
        || !state.token_program.executable
        || state.collateral.owner != state.token_program.key
        || state.collateral.executable
        || state.collateral.key.to_bytes() != registration.intent.collateral_account
    {
        return Err(Error::InvalidAccount);
    }
    let token = TokenAccount::parse(&state.collateral.data).map_err(|_| Error::InvalidToken)?;
    if token.owner != registration.maker
        || token.native_reserve != COption::None
        || token.state == AccountState::Uninitialized
    {
        return Err(Error::InvalidToken);
    }
    let boundary = match token.delegate {
        COption::None if token.delegated_amount == 0 => {
            RegisteredDirectRetirementBoundary::BuyerAlreadyClear
        }
        COption::Some(delegate) if delegate == state.registration.key.to_bytes() => {
            RegisteredDirectRetirementBoundary::BuyerControllerRevokeRequired
        }
        COption::None | COption::Some(_) => return Err(Error::InvalidToken),
    };
    let data = RegisteredRetireInstructionV1 {
        controller_bump,
        registration_bump,
    }
    .encode()
    .map_err(|_| Error::Encoding)?;
    let maker = Pubkey::new_from_array(registration.maker);
    let accounts = vec![
        AccountMeta::new_readonly(state.controller.key, false),
        AccountMeta::new(state.registration.key, false),
        AccountMeta::new(maker, true),
        AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
        AccountMeta::new(state.collateral.key, false),
        AccountMeta::new_readonly(state.token_program.key, false),
    ];
    debug_assert_eq!(accounts.len(), REGISTERED_BUYER_RETIRE_ACCOUNT_COUNT);
    Ok(RegisteredDirectRetirementReport {
        instruction: Instruction {
            program_id: controller_program,
            accounts,
            data: data.to_vec(),
        },
        observation,
        maker,
        maker_signature_required: true,
        rent_refund_lamports: state.registration.lamports,
        boundary,
    })
}

/// Compile one exact retirement request into an unsigned packet-safe v0
/// message and expose every required signature key in slot order.
pub fn compile_registered_direct_retirement_packet(
    report: &RegisteredDirectRetirementReport,
    payer: Pubkey,
    recent_blockhash: Hash,
) -> Result<RegisteredDirectRetirementPacket, Error> {
    if payer == Pubkey::default() {
        return Err(Error::InvalidAccount);
    }
    let message = v0::Message::try_compile(
        &payer,
        core::slice::from_ref(&report.instruction),
        &[],
        recent_blockhash,
    )
    .map_err(|_| Error::Encoding)?;
    let signature_count = usize::from(message.header.num_required_signatures);
    let required_signers = message
        .account_keys
        .get(..signature_count)
        .ok_or(Error::Encoding)?
        .to_vec();
    let message = VersionedMessage::V0(message);
    let wire_bytes = short_vec_prefix_bytes(signature_count)
        .checked_add(signature_count.checked_mul(64).ok_or(Error::Encoding)?)
        .and_then(|value| value.checked_add(message.serialize().len()))
        .ok_or(Error::Encoding)?;
    if wire_bytes > PACKET_DATA_BYTES {
        return Err(Error::PacketTooLarge);
    }
    Ok(RegisteredDirectRetirementPacket {
        message,
        required_signers,
        wire_bytes,
    })
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
    let authority = authenticate_authority(AuthorityAccounts {
        market: &state.market,
        realm: &state.realm,
        fee_policy: &state.fee_policy,
        capability_manifest: &state.capability_manifest,
        mint: &state.mint,
        fee_destination: &state.fee_destination,
        token_program: &state.token_program,
    })?;
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

#[allow(clippy::too_many_arguments)]
fn authenticate_retirement_core(
    controller_program: Pubkey,
    controller_program_account: &ObservedAccount,
    controller_account: &ObservedAccount,
    registration_account: &ObservedAccount,
    maker_account: &ObservedAccount,
    claim_program: &ObservedAccount,
    expected_side: u8,
) -> Result<(RegisteredIntentStateV1, u8, u8), Error> {
    require_distinct(&[
        controller_program_account.key,
        controller_account.key,
        registration_account.key,
        maker_account.key,
        claim_program.key,
    ])?;
    if controller_program_account.key != controller_program
        || !controller_program_account.executable
        || controller_account.executable
        || registration_account.owner != CLAIM_PROGRAM_ID
        || registration_account.executable
        || registration_account.lamports == 0
        || maker_account.executable
        || claim_program.key != CLAIM_PROGRAM_ID
        || !claim_program.executable
    {
        return Err(Error::InvalidAccount);
    }
    maker_account
        .lamports
        .checked_add(registration_account.lamports)
        .ok_or(Error::InvalidRefund)?;
    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &controller_program);
    if controller_account.key != controller {
        return Err(Error::PdaMismatch);
    }
    let registration = decode_registration(registration_account, controller)?;
    if registration.maker != maker_account.key.to_bytes() {
        return Err(Error::RegistrationBinding);
    }
    if registration.intent.side != expected_side
        || registration.intent.lifecycle > 2
        || registration.intent.valid_from > registration.intent.valid_through
        || registration.intent.maximum_fill == 0
        || registration.remaining > registration.intent.maximum_fill
        || registration.phase == 0
        || registration.phase > 3
        || (registration.phase == 1 && registration.remaining != 0)
    {
        return Err(Error::InvalidRetirement);
    }
    let (registration_key, registration_bump) =
        registration_address(controller_program, registration)?;
    if registration_key != registration_account.key {
        return Err(Error::PdaMismatch);
    }
    Ok((registration, controller_bump, registration_bump))
}

fn creation_observation(state: &RegisteredDirectCreationState) -> Result<Observation, Error> {
    same_observation(&[
        &state.controller,
        &state.controller_program,
        &state.maker,
        &state.payer,
        &state.replay,
        &state.registration,
        &state.claim_program,
        &state.system_program,
        &state.market,
        &state.realm,
        &state.fee_policy,
        &state.capability_manifest,
        &state.mint,
        &state.collateral,
        &state.fee_destination,
        &state.token_program,
        &state.rent_sysvar,
    ])
}

fn validate_creation_accounts(
    controller_program: Pubkey,
    state: &RegisteredDirectCreationState,
) -> Result<(), Error> {
    require_distinct(&[
        state.controller_program.key,
        state.controller.key,
        state.replay.key,
        state.registration.key,
        state.claim_program.key,
        state.system_program.key,
        state.market.key,
        state.realm.key,
        state.fee_policy.key,
        state.capability_manifest.key,
        state.mint.key,
        state.collateral.key,
        state.fee_destination.key,
        state.token_program.key,
    ])?;
    let fixed = [
        state.controller_program.key,
        state.controller.key,
        state.replay.key,
        state.registration.key,
        state.claim_program.key,
        state.system_program.key,
        state.market.key,
        state.realm.key,
        state.fee_policy.key,
        state.capability_manifest.key,
        state.mint.key,
        state.collateral.key,
        state.fee_destination.key,
        state.token_program.key,
    ];
    if state.controller_program.key != controller_program
        || !state.controller_program.executable
        || state.maker.key == Pubkey::default()
        || state.payer.key == Pubkey::default()
        || fixed
            .iter()
            .any(|key| *key == state.maker.key || *key == state.payer.key)
        || (state.maker.key == state.payer.key && state.maker != state.payer)
        || state.controller.executable
        || state.maker.executable
        || state.payer.owner != system_program::ID
        || state.payer.executable
        || !state.payer.data.is_empty()
        || state.replay.executable
        || state.registration.owner != system_program::ID
        || state.registration.executable
        || !state.registration.data.is_empty()
        || state.claim_program.key != CLAIM_PROGRAM_ID
        || !state.claim_program.executable
        || state.system_program.key != system_program::ID
        || !state.system_program.executable
        || state.market.executable
        || state.realm.owner != state.market.owner
        || state.realm.executable
        || state.fee_policy.owner != state.market.owner
        || state.fee_policy.executable
        || state.capability_manifest.owner != state.market.owner
        || state.capability_manifest.executable
        || state.token_program.key.to_bytes() != LEGACY_TOKEN_PROGRAM_ID
        || !state.token_program.executable
        || state.mint.owner != state.token_program.key
        || state.mint.executable
        || state.collateral.owner != state.token_program.key
        || state.collateral.executable
        || state.fee_destination.owner != state.token_program.key
        || state.fee_destination.executable
    {
        return Err(Error::InvalidAccount);
    }
    let replay_shape = (state.replay.owner == system_program::ID && state.replay.data.is_empty())
        || (state.replay.owner == CLAIM_PROGRAM_ID
            && state.replay.data.len() == REPLAY_STATE_BYTES);
    if !replay_shape {
        return Err(Error::InvalidReplay);
    }
    Ok(())
}

fn authenticate_creation_authority(
    state: &RegisteredDirectCreationState,
) -> Result<AuthorityFacts, Error> {
    let authority = authenticate_authority(AuthorityAccounts {
        market: &state.market,
        realm: &state.realm,
        fee_policy: &state.fee_policy,
        capability_manifest: &state.capability_manifest,
        mint: &state.mint,
        fee_destination: &state.fee_destination,
        token_program: &state.token_program,
    })?;
    let realm = RealmV1::decode(&state.realm.data).map_err(|_| Error::InvalidAuthority)?;
    let release = PRODUCTION_ADAPTER_RELEASES
        .into_iter()
        .find(|candidate| {
            hash(&candidate.to_bytes()).to_bytes() == *realm.collateral_adapter_release_id()
        })
        .ok_or(Error::InvalidAuthority)?;
    if release.token_program() != LEGACY_TOKEN_PROGRAM_ID
        || release.profile() != ExactTransferProfileV1::LegacyExactTransferV1
    {
        return Err(Error::InvalidAuthority);
    }
    let mint = release
        .profile()
        .check_mint(LEGACY_TOKEN_PROGRAM_ID, &state.mint.data)
        .map_err(|_| Error::InvalidToken)?;
    if (realm.mint_authority_policy() == MintAuthorityPolicy::RequireAbsent
        && mint.mint_authority != COption::None)
        || (realm.freeze_authority_policy() == FreezeAuthorityPolicy::RequireAbsent
            && mint.freeze_authority != COption::None)
    {
        return Err(Error::InvalidToken);
    }
    Ok(authority)
}

fn replay_nonce(replay: &ObservedAccount, controller: Pubkey) -> Result<u64, Error> {
    if replay.owner == system_program::ID && replay.data.is_empty() {
        return Ok(0);
    }
    if replay.owner != CLAIM_PROGRAM_ID
        || replay.data.len() != REPLAY_STATE_BYTES
        || replay.data.get(..8) != Some(REPLAY_STATE_MAGIC.as_slice())
        || replay.data.get(8..40) != Some(controller.as_ref())
    {
        return Err(Error::InvalidReplay);
    }
    let nonce = u64::from_le_bytes(
        replay
            .data
            .get(40..48)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(Error::InvalidReplay)?,
    );
    nonce.checked_add(1).ok_or(Error::InvalidReplay)?;
    Ok(nonce)
}

fn registered_buyer_reserve(intent: CompactIntentV1) -> Result<u64, Error> {
    let gross = u64::try_from(
        u128::from(intent.maximum_fill)
            .checked_mul(u128::from(intent.limit_price))
            .ok_or(Error::InvalidCreationIntent)?
            / u128::from(PRICE_SCALE),
    )
    .map_err(|_| Error::InvalidCreationIntent)?;
    let fee = u64::try_from(
        u128::from(gross)
            .checked_mul(u128::from(intent.fee_basis_points))
            .ok_or(Error::InvalidCreationIntent)?
            / u128::from(FEE_BASIS_POINTS_DENOMINATOR),
    )
    .map_err(|_| Error::InvalidCreationIntent)?;
    let reserve = gross.checked_add(fee).ok_or(Error::InvalidCreationIntent)?;
    if reserve == 0 {
        return Err(Error::InvalidCreationIntent);
    }
    Ok(reserve)
}

fn validate_creation_tokens(
    state: &RegisteredDirectCreationState,
    delegated_amount: u64,
    registration: Pubkey,
) -> Result<(), Error> {
    let exact = ExactTransferProfileV1::LegacyExactTransferV1;
    exact
        .check_mint(LEGACY_TOKEN_PROGRAM_ID, &state.mint.data)
        .map_err(|_| Error::InvalidToken)?;
    let collateral = exact
        .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &state.collateral.data)
        .map_err(|_| Error::InvalidToken)?;
    let venue = exact
        .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &state.fee_destination.data)
        .map_err(|_| Error::InvalidToken)?;
    if collateral.mint != state.mint.key.to_bytes()
        || venue.mint != state.mint.key.to_bytes()
        || collateral.owner != state.maker.key.to_bytes()
        || collateral.amount < delegated_amount
        || (collateral.delegate == COption::None && collateral.delegated_amount != 0)
        || registration == state.maker.key
    {
        return Err(Error::InvalidToken);
    }
    Ok(())
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

fn authenticate_authority(state: AuthorityAccounts<'_>) -> Result<AuthorityFacts, Error> {
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

fn short_vec_prefix_bytes(mut value: usize) -> usize {
    let mut bytes = 1_usize;
    while value >= 0x80 {
        value >>= 7;
        bytes = bytes.saturating_add(1);
    }
    bytes
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

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
    use dclutch_token_svm::{
        ACCOUNT_BYTES, LEGACY_TOKEN_PROGRAM_ID, MINT_BYTES, PRODUCTION_ADAPTER_RELEASES,
    };
    use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::SysvarSerialize};
    use solana_sdk_ids::{native_loader, system_program};
    use spl_token_2022_interface::instruction::TokenInstruction;

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

    fn mint_bytes() -> Vec<u8> {
        let mut bytes = vec![0_u8; MINT_BYTES];
        bytes[36..44].copy_from_slice(&10_000_u64.to_le_bytes());
        bytes[44] = 6;
        bytes[45] = 1;
        bytes
    }

    fn token_account_bytes(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
        let mut bytes = vec![0_u8; ACCOUNT_BYTES];
        bytes[..32].copy_from_slice(mint.as_ref());
        bytes[32..64].copy_from_slice(owner.as_ref());
        bytes[64..72].copy_from_slice(&amount.to_le_bytes());
        bytes[108] = 1;
        bytes
    }

    fn set_token_delegate(data: &mut [u8], delegate: Option<(Pubkey, u64)>) {
        match delegate {
            Some((delegate, amount)) => {
                data[72..76].copy_from_slice(&1_u32.to_le_bytes());
                data[76..108].copy_from_slice(delegate.as_ref());
                data[121..129].copy_from_slice(&amount.to_le_bytes());
            }
            None => {
                data[72..76].copy_from_slice(&0_u32.to_le_bytes());
                data[76..108].fill(0);
                data[121..129].copy_from_slice(&0_u64.to_le_bytes());
            }
        }
    }

    fn replay_bytes(controller: Pubkey, nonce: u64) -> Vec<u8> {
        let mut bytes = vec![0_u8; REPLAY_STATE_BYTES];
        bytes[..8].copy_from_slice(REPLAY_STATE_MAGIC);
        bytes[8..40].copy_from_slice(controller.as_ref());
        bytes[40..48].copy_from_slice(&nonce.to_le_bytes());
        bytes
    }

    fn rent_account(observation: Observation) -> ObservedAccount {
        let rent = Rent::default();
        let mut data = vec![0_u8; Rent::size_of()];
        let mut lamports = 1_u64;
        let mut info = AccountInfo::new(
            &solana_sdk_ids::sysvar::rent::ID,
            false,
            false,
            &mut lamports,
            &mut data,
            &solana_sdk_ids::sysvar::ID,
            false,
        );
        rent.to_account_info(&mut info).expect("serialize Rent");
        drop(info);
        observed(
            observation,
            solana_sdk_ids::sysvar::rent::ID,
            solana_sdk_ids::sysvar::ID,
            false,
            data,
        )
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

    fn creation_fixture() -> (Pubkey, RegisteredDirectCreationState, CompactIntentV1) {
        let (controller_program, fill) = fixture();
        let snapshot = fill.market.observation;
        let maker = key(2);
        let payer = key(77);
        let generation = GENERATION.to_le_bytes();
        let nonce = 0_u64.to_le_bytes();
        let (replay, _) = Pubkey::find_program_address(
            &[
                REPLAY_SEED,
                fill.market.key.as_ref(),
                &generation,
                maker.as_ref(),
            ],
            &controller_program,
        );
        let (registration, _) = Pubkey::find_program_address(
            &[
                REGISTERED_SEED,
                fill.market.key.as_ref(),
                &generation,
                maker.as_ref(),
                &nonce,
            ],
            &controller_program,
        );
        let mut mint = fill.mint.clone();
        mint.data = mint_bytes();
        let mut collateral = fill.buyer_source.clone();
        collateral.data = token_account_bytes(mint.key, maker, 5_000);
        let mut fee_destination = fill.fee_destination.clone();
        fee_destination.data = token_account_bytes(mint.key, key(78), 10);
        let intent = direct_intent(fill.market.key, collateral.key, 1, 0);
        let mut payer_account = observed(snapshot, payer, system_program::ID, false, Vec::new());
        payer_account.lamports = 10_000_000;
        (
            controller_program,
            RegisteredDirectCreationState {
                controller_program: observed(
                    snapshot,
                    controller_program,
                    native_loader::ID,
                    true,
                    Vec::new(),
                ),
                controller: fill.controller,
                maker: observed(snapshot, maker, system_program::ID, false, Vec::new()),
                payer: payer_account,
                replay: observed(snapshot, replay, system_program::ID, false, Vec::new()),
                registration: observed(
                    snapshot,
                    registration,
                    system_program::ID,
                    false,
                    Vec::new(),
                ),
                claim_program: fill.claim_program,
                system_program: observed(
                    snapshot,
                    system_program::ID,
                    native_loader::ID,
                    true,
                    Vec::new(),
                ),
                market: fill.market,
                realm: fill.realm,
                fee_policy: fill.fee_policy,
                capability_manifest: fill.capability_manifest,
                mint,
                collateral,
                fee_destination,
                token_program: fill.token_program,
                rent_sysvar: rent_account(snapshot),
            },
            intent,
        )
    }

    fn set_creation_nonce(
        controller_program: Pubkey,
        state: &mut RegisteredDirectCreationState,
        intent: &mut CompactIntentV1,
        nonce: u64,
    ) {
        state.replay.owner = CLAIM_PROGRAM_ID;
        state.replay.data = replay_bytes(state.controller.key, nonce);
        let nonce_seed = nonce.to_le_bytes();
        state.registration.key = Pubkey::find_program_address(
            &[
                REGISTERED_SEED,
                state.market.key.as_ref(),
                &GENERATION.to_le_bytes(),
                state.maker.key.as_ref(),
                &nonce_seed,
            ],
            &controller_program,
        )
        .0;
        intent.nonce = nonce;
    }

    fn terminalize_registration(account: &mut ObservedAccount, phase: u8, remaining: u64) {
        let mut registration =
            RegisteredIntentStateV1::decode(&account.data).expect("registration state");
        registration.phase = phase;
        registration.remaining = remaining;
        account.data = registration
            .encode()
            .expect("terminal registration")
            .to_vec();
    }

    fn retirement_fixtures() -> (
        Pubkey,
        RegisteredDirectSellerRetirementState,
        RegisteredDirectBuyerRetirementState,
    ) {
        let (program, fill) = fixture();
        let snapshot = fill.controller.observation;
        let controller_program = observed(snapshot, program, native_loader::ID, true, Vec::new());
        let mut seller_registration = fill.seller_registration.clone();
        terminalize_registration(&mut seller_registration, 2, 1_500);
        seller_registration.lamports = 2_000_000;
        let mut buyer_registration = fill.buyer_registration.clone();
        terminalize_registration(&mut buyer_registration, 3, 1_500);
        buyer_registration.lamports = 2_100_000;
        let seller_maker = observed(snapshot, key(1), system_program::ID, false, Vec::new());
        let buyer_maker = observed(snapshot, key(2), system_program::ID, false, Vec::new());
        let mut collateral = fill.buyer_source.clone();
        collateral.data = token_account_bytes(fill.mint.key, key(2), 5_000);
        set_token_delegate(&mut collateral.data, Some((buyer_registration.key, 1_203)));
        (
            program,
            RegisteredDirectSellerRetirementState {
                controller_program: controller_program.clone(),
                controller: fill.controller.clone(),
                registration: seller_registration,
                maker: seller_maker,
                claim_program: fill.claim_program.clone(),
            },
            RegisteredDirectBuyerRetirementState {
                controller_program,
                controller: fill.controller,
                registration: buyer_registration,
                maker: buyer_maker,
                claim_program: fill.claim_program,
                collateral,
                token_program: fill.token_program,
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
    fn terminal_seller_retirement_is_permissionless_and_refunds_only_persisted_maker() {
        let (program, seller, _) = retirement_fixtures();
        let report =
            build_registered_direct_seller_retirement(program, &seller).expect("seller retirement");
        assert_eq!(report.observation, observation(55));
        assert_eq!(report.maker, key(1));
        assert!(!report.maker_signature_required);
        assert_eq!(report.rent_refund_lamports, 2_000_000);
        assert_eq!(
            report.boundary,
            RegisteredDirectRetirementBoundary::SellerNoDelegation
        );
        assert_eq!(
            report.instruction.accounts,
            vec![
                AccountMeta::new_readonly(seller.controller.key, false),
                AccountMeta::new(seller.registration.key, false),
                AccountMeta::new(seller.maker.key, false),
                AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
            ]
        );
        let request = RegisteredRetireInstructionV1::decode(&report.instruction.data)
            .expect("codec retirement request");
        assert_eq!(
            Pubkey::create_program_address(
                &[CONTROLLER_SEED, &[request.controller_bump]],
                &program,
            ),
            Ok(seller.controller.key)
        );
        let payer = key(88);
        let packet = compile_registered_direct_retirement_packet(
            &report,
            payer,
            Hash::new_from_array([91; 32]),
        )
        .expect("seller retirement packet");
        assert_eq!(packet.required_signers, vec![payer]);
        assert_eq!(packet.wire_bytes, 319);
        assert!(packet.wire_bytes <= PACKET_DATA_BYTES);
        assert_eq!(
            compile_registered_direct_retirement_packet(
                &report,
                Pubkey::default(),
                Hash::new_from_array([91; 32]),
            ),
            Err(Error::InvalidAccount)
        );
    }

    #[test]
    fn terminal_buyer_retirement_requires_maker_and_records_controller_revoke_boundary() {
        let (program, _, buyer) = retirement_fixtures();
        let report =
            build_registered_direct_buyer_retirement(program, &buyer).expect("buyer retirement");
        assert_eq!(report.maker, key(2));
        assert!(report.maker_signature_required);
        assert_eq!(report.rent_refund_lamports, 2_100_000);
        assert_eq!(
            report.boundary,
            RegisteredDirectRetirementBoundary::BuyerControllerRevokeRequired
        );
        assert_eq!(
            report.instruction.accounts,
            vec![
                AccountMeta::new_readonly(buyer.controller.key, false),
                AccountMeta::new(buyer.registration.key, false),
                AccountMeta::new(buyer.maker.key, true),
                AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
                AccountMeta::new(buyer.collateral.key, false),
                AccountMeta::new_readonly(buyer.token_program.key, false),
            ]
        );
        let request = RegisteredRetireInstructionV1::decode(&report.instruction.data)
            .expect("codec retirement request");
        let registration =
            RegisteredIntentStateV1::decode(&buyer.registration.data).expect("buyer registration");
        assert_eq!(
            Pubkey::create_program_address(
                &[
                    REGISTERED_SEED,
                    &registration.intent.market,
                    &registration.intent.generation.to_le_bytes(),
                    buyer.maker.key.as_ref(),
                    &registration.intent.nonce.to_le_bytes(),
                    &[request.registration_bump],
                ],
                &program,
            ),
            Ok(buyer.registration.key)
        );
        let payer = key(88);
        let packet = compile_registered_direct_retirement_packet(
            &report,
            payer,
            Hash::new_from_array([91; 32]),
        )
        .expect("buyer retirement packet");
        assert_eq!(packet.required_signers, vec![payer, buyer.maker.key]);
        assert_eq!(packet.wire_bytes, 449);
        assert!(packet.wire_bytes <= PACKET_DATA_BYTES);
        let maker_paid = compile_registered_direct_retirement_packet(
            &report,
            buyer.maker.key,
            Hash::new_from_array([91; 32]),
        )
        .expect("maker-paid buyer retirement packet");
        assert_eq!(maker_paid.required_signers, vec![buyer.maker.key]);
        assert!(maker_paid.wire_bytes < packet.wire_bytes);

        let mut already_clear = buyer.clone();
        set_token_delegate(&mut already_clear.collateral.data, None);
        let clear = build_registered_direct_buyer_retirement(program, &already_clear)
            .expect("already clear buyer retirement");
        assert_eq!(
            clear.boundary,
            RegisteredDirectRetirementBoundary::BuyerAlreadyClear
        );
    }

    #[test]
    fn terminal_retirement_refuses_open_incoherent_substituted_or_unfunded_state() {
        let (program, seller, buyer) = retirement_fixtures();
        let mut mixed = seller.clone();
        mixed.maker.observation.slot += 1;
        assert_eq!(
            build_registered_direct_seller_retirement(program, &mixed),
            Err(Error::ObservationMismatch)
        );

        let mut open = seller.clone();
        terminalize_registration(&mut open.registration, 0, 1_500);
        assert_eq!(
            build_registered_direct_seller_retirement(program, &open),
            Err(Error::InvalidRetirement)
        );

        let mut impossible_fill = seller.clone();
        terminalize_registration(&mut impossible_fill.registration, 1, 1);
        assert_eq!(
            build_registered_direct_seller_retirement(program, &impossible_fill),
            Err(Error::InvalidRetirement)
        );

        let mut wrong_owner = seller.clone();
        wrong_owner.registration.owner = system_program::ID;
        assert_eq!(
            build_registered_direct_seller_retirement(program, &wrong_owner),
            Err(Error::InvalidAccount)
        );

        let mut wrong_pda = seller.clone();
        wrong_pda.registration.key = key(96);
        assert_eq!(
            build_registered_direct_seller_retirement(program, &wrong_pda),
            Err(Error::PdaMismatch)
        );

        let mut wrong_refund = seller.clone();
        wrong_refund.maker.key = key(95);
        assert_eq!(
            build_registered_direct_seller_retirement(program, &wrong_refund),
            Err(Error::RegistrationBinding)
        );

        let mut alias = seller.clone();
        alias.maker.key = alias.claim_program.key;
        assert_eq!(
            build_registered_direct_seller_retirement(program, &alias),
            Err(Error::InvalidAccount)
        );

        let mut drained = seller.clone();
        drained.registration.lamports = 0;
        assert_eq!(
            build_registered_direct_seller_retirement(program, &drained),
            Err(Error::InvalidAccount)
        );

        let mut overflow = seller.clone();
        overflow.maker.lamports = u64::MAX;
        assert_eq!(
            build_registered_direct_seller_retirement(program, &overflow),
            Err(Error::InvalidRefund)
        );

        assert_eq!(
            build_registered_direct_seller_retirement(
                program,
                &RegisteredDirectSellerRetirementState {
                    controller_program: buyer.controller_program,
                    controller: buyer.controller,
                    registration: buyer.registration,
                    maker: buyer.maker,
                    claim_program: buyer.claim_program,
                },
            ),
            Err(Error::InvalidRetirement)
        );
    }

    #[test]
    fn terminal_buyer_retirement_refuses_hostile_token_and_delegate_state() {
        let (program, _, buyer) = retirement_fixtures();
        let mut wrong_collateral = buyer.clone();
        wrong_collateral.collateral.key = key(94);
        assert_eq!(
            build_registered_direct_buyer_retirement(program, &wrong_collateral),
            Err(Error::InvalidAccount)
        );

        let mut wrong_physical_owner = buyer.clone();
        wrong_physical_owner.collateral.owner = system_program::ID;
        assert_eq!(
            build_registered_direct_buyer_retirement(program, &wrong_physical_owner),
            Err(Error::InvalidAccount)
        );

        let mut malformed = buyer.clone();
        malformed.collateral.data.pop();
        assert_eq!(
            build_registered_direct_buyer_retirement(program, &malformed),
            Err(Error::InvalidToken)
        );

        let mut wrong_token_owner = buyer.clone();
        wrong_token_owner.collateral.data[32..64].copy_from_slice(key(93).as_ref());
        assert_eq!(
            build_registered_direct_buyer_retirement(program, &wrong_token_owner),
            Err(Error::InvalidToken)
        );

        let mut foreign_delegate = buyer.clone();
        set_token_delegate(&mut foreign_delegate.collateral.data, Some((key(92), 1)));
        assert_eq!(
            build_registered_direct_buyer_retirement(program, &foreign_delegate),
            Err(Error::InvalidToken)
        );

        let mut inconsistent_clear = buyer.clone();
        set_token_delegate(&mut inconsistent_clear.collateral.data, None);
        inconsistent_clear.collateral.data[121..129].copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            build_registered_direct_buyer_retirement(program, &inconsistent_clear),
            Err(Error::InvalidToken)
        );

        let mut native = buyer.clone();
        native.collateral.data[109..113].copy_from_slice(&1_u32.to_le_bytes());
        native.collateral.data[113..121].copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            build_registered_direct_buyer_retirement(program, &native),
            Err(Error::InvalidToken)
        );

        let mut uninitialized = buyer.clone();
        uninitialized.collateral.data[108] = 0;
        assert_eq!(
            build_registered_direct_buyer_retirement(program, &uninitialized),
            Err(Error::InvalidToken)
        );

        let mut frozen = buyer.clone();
        frozen.collateral.data[108] = 2;
        assert_eq!(
            build_registered_direct_buyer_retirement(program, &frozen)
                .expect("controller-admitted frozen revoke")
                .boundary,
            RegisteredDirectRetirementBoundary::BuyerControllerRevokeRequired
        );
    }

    #[test]
    fn registered_creation_derives_approval_frame_nonce_and_packet() {
        let (program, state, intent) = creation_fixture();
        let report = build_registered_direct_creation(program, &state, intent)
            .expect("registered creation route");
        assert_eq!(report.observation, observation(55));
        assert_eq!(report.replay, state.replay.key);
        assert_eq!(report.registration, state.registration.key);
        assert_eq!((report.nonce, report.delegated_amount), (0, 1_203));
        let expected_rent = Rent::default()
            .minimum_balance(REPLAY_STATE_BYTES)
            .saturating_sub(state.replay.lamports)
            .checked_add(
                Rent::default()
                    .minimum_balance(REGISTERED_INTENT_STATE_BYTES)
                    .saturating_sub(state.registration.lamports),
            )
            .expect("rent debit");
        assert_eq!(report.rent_debit_lamports, expected_rent);

        let [approval, controller] = &report.instructions;
        assert_eq!(approval.program_id, state.token_program.key);
        assert_eq!(
            approval.accounts,
            vec![
                AccountMeta::new(state.collateral.key, false),
                AccountMeta::new_readonly(state.registration.key, false),
                AccountMeta::new_readonly(state.maker.key, true),
            ]
        );
        assert_eq!(
            TokenInstruction::unpack(&approval.data),
            Ok(TokenInstruction::Approve { amount: 1_203 })
        );
        assert_eq!(controller.program_id, program);
        assert_eq!(controller.accounts.len(), REGISTERED_CREATE_ACCOUNT_COUNT);
        assert_eq!(
            controller.accounts,
            vec![
                AccountMeta::new_readonly(state.controller.key, false),
                AccountMeta::new_readonly(state.maker.key, true),
                AccountMeta::new(state.payer.key, true),
                AccountMeta::new(state.replay.key, false),
                AccountMeta::new(state.registration.key, false),
                AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(state.market.key, false),
                AccountMeta::new_readonly(state.realm.key, false),
                AccountMeta::new_readonly(state.fee_policy.key, false),
                AccountMeta::new_readonly(state.capability_manifest.key, false),
                AccountMeta::new_readonly(state.mint.key, false),
                AccountMeta::new_readonly(state.collateral.key, false),
                AccountMeta::new_readonly(state.fee_destination.key, false),
                AccountMeta::new_readonly(state.token_program.key, false),
            ]
        );
        let request = RegisteredCreateInstructionV1::decode(&controller.data)
            .expect("codec-owned creation request");
        assert_eq!(request.intent, intent);
        assert_eq!(
            Pubkey::create_program_address(
                &[CONTROLLER_SEED, &[request.controller_bump]],
                &program,
            ),
            Ok(state.controller.key)
        );

        let packet =
            compile_registered_direct_creation_packet(&report, Hash::new_from_array([91; 32]))
                .expect("packet-safe creation");
        assert_eq!(packet.required_signatures, 2);
        assert_eq!(packet.wire_bytes, 866);
        assert!(packet.wire_bytes <= PACKET_DATA_BYTES);
        assert!(matches!(
            &packet.message,
            VersionedMessage::V0(message) if message.instructions.len() == 2
        ));
    }

    #[test]
    fn registered_creation_reuses_exact_claim_nonce_and_refuses_replay_substitution() {
        let (program, mut state, mut intent) = creation_fixture();
        set_creation_nonce(program, &mut state, &mut intent, 4);
        let report = build_registered_direct_creation(program, &state, intent)
            .expect("existing replay root");
        assert_eq!(report.nonce, 4);
        let request = RegisteredCreateInstructionV1::decode(&report.instructions[1].data)
            .expect("creation request");
        assert_eq!(request.intent.nonce, 4);

        let mut wrong_controller = state.clone();
        wrong_controller.replay.data[8] ^= 1;
        assert_eq!(
            build_registered_direct_creation(program, &wrong_controller, intent),
            Err(Error::InvalidReplay)
        );

        let mut overflow = state.clone();
        overflow.replay.data = replay_bytes(overflow.controller.key, u64::MAX);
        assert_eq!(
            build_registered_direct_creation(program, &overflow, intent),
            Err(Error::InvalidReplay)
        );

        let mut wrong_pda = state.clone();
        wrong_pda.registration.key = key(97);
        assert_eq!(
            build_registered_direct_creation(program, &wrong_pda, intent),
            Err(Error::PdaMismatch)
        );
    }

    #[test]
    fn registered_creation_refuses_hostile_authority_accounts_and_terms() {
        let (program, state, intent) = creation_fixture();
        let mut mixed = state.clone();
        mixed.realm.observation.slot += 1;
        assert_eq!(
            build_registered_direct_creation(program, &mixed, intent),
            Err(Error::ObservationMismatch)
        );

        let mut occupied = state.clone();
        occupied.registration.data.push(0);
        assert_eq!(
            build_registered_direct_creation(program, &occupied, intent),
            Err(Error::InvalidAccount)
        );

        let mut poor = state.clone();
        poor.payer.lamports = 0;
        assert_eq!(
            build_registered_direct_creation(program, &poor, intent),
            Err(Error::InsufficientPayer)
        );

        let mut hostile_rent = state.clone();
        hostile_rent.rent_sysvar.data.pop();
        assert_eq!(
            build_registered_direct_creation(program, &hostile_rent, intent),
            Err(Error::InvalidRent)
        );

        let mut hostile_manifest = state.clone();
        hostile_manifest.capability_manifest.data[0] ^= 1;
        assert_eq!(
            build_registered_direct_creation(program, &hostile_manifest, intent),
            Err(Error::InvalidAuthority)
        );

        let mut hostile_market = state.clone();
        hostile_market.market.data.pop();
        assert_eq!(
            build_registered_direct_creation(program, &hostile_market, intent),
            Err(Error::InvalidAuthority)
        );

        let mut hostile_realm = state.clone();
        hostile_realm.realm.data.pop();
        assert_eq!(
            build_registered_direct_creation(program, &hostile_realm, intent),
            Err(Error::InvalidAuthority)
        );

        let mut hostile_policy = state.clone();
        hostile_policy.fee_policy.data[0] ^= 1;
        assert_eq!(
            build_registered_direct_creation(program, &hostile_policy, intent),
            Err(Error::InvalidAuthority)
        );

        let mut wrong_fee = state.clone();
        wrong_fee.fee_destination.key = key(79);
        assert_eq!(
            build_registered_direct_creation(program, &wrong_fee, intent),
            Err(Error::InvalidAuthority)
        );

        let mut wrong_nonce = intent;
        wrong_nonce.nonce = 1;
        assert_eq!(
            build_registered_direct_creation(program, &state, wrong_nonce),
            Err(Error::InvalidCreationIntent)
        );

        let mut seller = intent;
        seller.side = 0;
        assert_eq!(
            build_registered_direct_creation(program, &state, seller),
            Err(Error::InvalidCreationIntent)
        );

        let mut expired = intent;
        expired.valid_through = 54;
        assert_eq!(
            build_registered_direct_creation(program, &state, expired),
            Err(Error::InvalidCreationIntent)
        );
    }

    #[test]
    fn registered_creation_refuses_hostile_mint_collateral_and_fee_token_state() {
        let (program, state, intent) = creation_fixture();
        let mut malformed_mint = state.clone();
        malformed_mint.mint.data.pop();
        assert_eq!(
            build_registered_direct_creation(program, &malformed_mint, intent),
            Err(Error::InvalidToken)
        );

        let mut wrong_owner = state.clone();
        wrong_owner.collateral.data[32..64].copy_from_slice(key(80).as_ref());
        assert_eq!(
            build_registered_direct_creation(program, &wrong_owner, intent),
            Err(Error::InvalidToken)
        );

        let mut underfunded = state.clone();
        underfunded.collateral.data[64..72].copy_from_slice(&1_202_u64.to_le_bytes());
        assert_eq!(
            build_registered_direct_creation(program, &underfunded, intent),
            Err(Error::InvalidToken)
        );

        let mut frozen_fee = state.clone();
        frozen_fee.fee_destination.data[108] = 2;
        assert_eq!(
            build_registered_direct_creation(program, &frozen_fee, intent),
            Err(Error::InvalidToken)
        );

        let mut inconsistent_delegate = state.clone();
        inconsistent_delegate.collateral.data[121..129].copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            build_registered_direct_creation(program, &inconsistent_delegate, intent),
            Err(Error::InvalidToken)
        );
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
