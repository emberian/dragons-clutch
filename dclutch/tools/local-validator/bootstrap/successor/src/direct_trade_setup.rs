//! Pure construction and verification for Direct's exact pre-Hot setup.
//!
//! This module owns no RPC, key, journal, or manifest behavior. It accepts
//! already-observed account bytes and coordinates, derives every request-bound
//! PDA, emits the two setup instruction families, and joins their exact
//! poststates back to those plans.

use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CUSTODY_POSTSTATE_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyFrameRoleV1, CustodyFrameSpecV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyRequestV1, INITIALIZE_REPLAY_ACCOUNT_COUNT_V1, OperationV1,
};
use dclutch_trading::{
    replay_setup_v1::{
        DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1, DIRECT_REPLAY_SETUP_REQUEST_BYTES_V1,
        DirectReplaySetupReceiptV1, DirectReplaySetupRequestV1,
        direct_replay_setup_parent_digest_v1,
    },
    successor::{DirectCoordinatesV1, MakerReplaySeedsV1},
};
use dclutch_market::{CoreState, Phase as CorePhase, StateBumpsV1};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    hash::{hash, hashv},
    rent::Rent,
};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{Error, Result};

const DIRECT_REPLAY_SETUP_ACCOUNT_COUNT_V1: usize = INITIALIZE_REPLAY_ACCOUNT_COUNT_V1 as usize + 1;

/// All explicit accounts in the top-level Trading replay-setup frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectReplaySetupCoordinatesV1 {
    pub(crate) caller_authority: Pubkey,
    pub(crate) market: Pubkey,
    pub(crate) activation_cache: Pubkey,
    pub(crate) registry_program: Pubkey,
    pub(crate) trading_program: Pubkey,
    pub(crate) trading_programdata: Pubkey,
    pub(crate) realm_raw: Pubkey,
    pub(crate) realm_staging: Pubkey,
    pub(crate) custody_replay: Pubkey,
    pub(crate) payer: Pubkey,
    pub(crate) system_program: Pubkey,
    pub(crate) rent_sysvar: Pubkey,
    pub(crate) rent_refund: Pubkey,
    pub(crate) custody_program: Pubkey,
}

/// Pure inputs for one exact Trading `DirectReplaySetup` instruction.
#[derive(Clone, Debug)]
pub(crate) struct DirectReplaySetupBuildInputV1<'a> {
    /// Exact authenticated top-level request bytes.
    pub(crate) request_bytes: &'a [u8],
    /// Exact authenticated canonical pre-setup Market bytes.
    pub(crate) market_bytes: &'a [u8],
    /// Exact finalized Realm record bytes at `coordinates.realm_raw`.
    pub(crate) realm_bytes: &'a [u8],
    /// Buyer identity already authenticated by the session producer.
    pub(crate) buyer_maker: Pubkey,
    /// Buyer Trading maker root already projected by the session producer.
    pub(crate) buyer_maker_root: Pubkey,
    pub(crate) coordinates: DirectReplaySetupCoordinatesV1,
    /// Runtime Rent parameters observed for the transaction's Rent sysvar.
    pub(crate) rent: Rent,
    /// Lamports on the vacant replay PDA immediately before setup.
    pub(crate) observed_replay_lamports: u64,
}

/// Complete pure plan for one Direct replay setup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectReplaySetupPlanV1 {
    pub(crate) instruction: Instruction,
    pub(crate) request: DirectReplaySetupRequestV1,
    pub(crate) request_bytes: [u8; DIRECT_REPLAY_SETUP_REQUEST_BYTES_V1],
    pub(crate) request_digest: [u8; 32],
    pub(crate) buyer_maker_root: Pubkey,
    pub(crate) custody_request: CustodyRequestV1,
    pub(crate) custody_request_digest: [u8; 32],
    pub(crate) exact_replay_rent: u64,
    pub(crate) observed_replay_lamports: u64,
    pub(crate) coordinates: DirectReplaySetupCoordinatesV1,
}

/// Request-bound PDA coordinates derived by the setup semantic owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DerivedDirectReplaySetupAccountsV1 {
    pub(crate) caller_authority: Pubkey,
    pub(crate) custody_replay: Pubkey,
}

/// Inputs that are sufficient to derive the two request-bound setup accounts.
#[derive(Clone, Debug)]
pub(crate) struct DirectReplaySetupDerivationInputV1<'a> {
    pub(crate) request_bytes: &'a [u8],
    pub(crate) market_bytes: &'a [u8],
    pub(crate) realm_bytes: &'a [u8],
    pub(crate) buyer_maker: Pubkey,
    pub(crate) buyer_maker_root: Pubkey,
    pub(crate) trading_program: Pubkey,
    pub(crate) custody_program: Pubkey,
    pub(crate) payer: Pubkey,
    pub(crate) rent_refund: Pubkey,
    pub(crate) rent: Rent,
}

/// One account observation supplied to a pure poststate verifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectSetupObservedAccountV1<'a> {
    pub(crate) address: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data: &'a [u8],
}

/// Fully authenticated Direct replay-setup poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedDirectReplaySetupPoststateV1 {
    pub(crate) receipt: DirectReplaySetupReceiptV1,
    pub(crate) replay: CustodyReplayV1,
}

/// Exact replay and return-data bytes committed before any signer is opened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectReplaySetupExpectedPoststateV1 {
    pub(crate) receipt: DirectReplaySetupReceiptV1,
    pub(crate) receipt_bytes: [u8; DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1],
    pub(crate) replay: CustodyReplayV1,
    pub(crate) replay_bytes: [u8; CUSTODY_REPLAY_BYTES_V1],
}

/// Derive the caller authority and replay PDA without restating the child
/// request outside this module.
pub(crate) fn derive_direct_replay_setup_accounts_v1(
    input: DirectReplaySetupDerivationInputV1<'_>,
) -> Result<DerivedDirectReplaySetupAccountsV1> {
    let request = DirectReplaySetupRequestV1::decode(input.request_bytes)
        .map_err(|error| refusal(format!("Direct replay setup request: {error:?}")))?;
    let request_bytes = request
        .to_bytes()
        .map_err(|error| refusal(format!("encode Direct replay setup request: {error:?}")))?;
    let market = CoreState::decode(input.market_bytes)
        .map_err(|error| refusal(format!("Direct setup Market: {error:?}")))?;
    if request_bytes.as_slice() != input.request_bytes
        || market
            .encode()
            .map_err(|error| refusal(format!("encode Direct setup Market: {error:?}")))?
            .as_slice()
            != input.market_bytes
        || market.phase != CorePhase::Open
        || request.expected_market_digest != hash(input.market_bytes).to_bytes()
        || request.market != market.identity.market_id.to_bytes()
        || request.maker != input.buyer_maker.to_bytes()
        || request.generation != market.identity.generation
        || market.identity.realm_id.to_bytes() != hash(input.realm_bytes).to_bytes()
        || market.rent_beneficiary.to_bytes() != input.rent_refund.to_bytes()
    {
        return Err(refusal(
            "Direct replay setup derivation did not match its canonical Market/request",
        ));
    }
    let maker_seeds = MakerReplaySeedsV1::new(
        DirectCoordinatesV1::new(request.market, request.generation)
            .map_err(|error| refusal(format!("Direct coordinates: {error:?}")))?,
        request.maker,
    )
    .map_err(|error| refusal(format!("buyer maker-root seeds: {error:?}")))?;
    let derived_maker_root =
        Pubkey::find_program_address(&maker_seeds.as_slices(), &input.trading_program).0;
    if derived_maker_root != input.buyer_maker_root {
        return Err(refusal("Direct replay setup maker root was foreign"));
    }
    let request_digest = hash(&request_bytes).to_bytes();
    let custody_request = expected_custody_request_v1(
        request,
        market,
        input.trading_program,
        input.buyer_maker_root,
        input.payer,
        input.rent_refund,
        input.rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1),
        request_digest,
    )?;
    let custody_request_bytes = custody_request
        .to_bytes()
        .map_err(|error| refusal(format!("encode Direct child Custody request: {error:?}")))?;
    let custody_request_digest = hash(&custody_request_bytes).to_bytes();
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(custody_request.release_set)
            .map_err(|error| refusal(format!("Direct setup release: {error:?}")))?,
        custody_request.market,
        ExecutionRoleV1::Trading,
        custody_request.context,
        custody_request_digest,
    )
    .map_err(|error| refusal(format!("Direct setup caller-authority seeds: {error:?}")))?;
    let caller_authority =
        Pubkey::find_program_address(&caller_seeds.as_slices(), &input.trading_program).0;
    let replay_seeds = CustodyReplaySeedsV1::from_request(custody_request);
    let custody_replay =
        Pubkey::find_program_address(&replay_seeds.as_slices(), &input.custody_program).0;
    Ok(DerivedDirectReplaySetupAccountsV1 {
        caller_authority,
        custody_replay,
    })
}

/// Build and internally re-authenticate one exact top-level setup instruction.
pub(crate) fn build_direct_replay_setup_v1(
    input: DirectReplaySetupBuildInputV1<'_>,
) -> Result<DirectReplaySetupPlanV1> {
    let request = DirectReplaySetupRequestV1::decode(input.request_bytes)
        .map_err(|error| refusal(format!("Direct replay setup request: {error:?}")))?;
    let request_bytes = request
        .to_bytes()
        .map_err(|error| refusal(format!("encode Direct replay setup request: {error:?}")))?;
    if request_bytes.as_slice() != input.request_bytes {
        return Err(refusal("Direct replay setup request was not canonical"));
    }

    let market = CoreState::decode(input.market_bytes)
        .map_err(|error| refusal(format!("Direct setup Market: {error:?}")))?;
    let canonical_market = market
        .encode()
        .map_err(|error| refusal(format!("encode Direct setup Market: {error:?}")))?;
    let coordinates = input.coordinates;
    if canonical_market.as_slice() != input.market_bytes
        || market.phase != CorePhase::Open
        || request.expected_market_digest != hash(input.market_bytes).to_bytes()
        || request.market != coordinates.market.to_bytes()
        || market.identity.market_id.to_bytes() != request.market
        || request.maker != input.buyer_maker.to_bytes()
        || market.identity.generation != request.generation
        || market.identity.registry_program.to_bytes() != coordinates.registry_program.to_bytes()
        || market.identity.realm_id.to_bytes() != hash(input.realm_bytes).to_bytes()
        || market.rent_beneficiary.to_bytes() != coordinates.rent_refund.to_bytes()
    {
        return Err(refusal(
            "Direct replay setup request did not match the authenticated Open Market/buyer frame",
        ));
    }
    require_direct_frame_coordinates_v1(coordinates)?;

    let maker_seeds = MakerReplaySeedsV1::new(
        DirectCoordinatesV1::new(request.market, request.generation)
            .map_err(|error| refusal(format!("Direct coordinates: {error:?}")))?,
        request.maker,
    )
    .map_err(|error| refusal(format!("buyer maker-root seeds: {error:?}")))?;
    let derived_maker_root =
        Pubkey::find_program_address(&maker_seeds.as_slices(), &coordinates.trading_program).0;
    if derived_maker_root != input.buyer_maker_root
        || derived_maker_root == coordinates.market
        || derived_maker_root == coordinates.payer
        || derived_maker_root == coordinates.rent_refund
    {
        return Err(refusal(
            "Direct replay setup buyer maker root was foreign or aliased",
        ));
    }

    let exact_replay_rent = input.rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);
    if exact_replay_rent == 0 {
        return Err(refusal("Direct replay setup Rent minimum was zero"));
    }
    let request_digest = hash(&request_bytes).to_bytes();
    let custody_request = expected_custody_request_v1(
        request,
        market,
        coordinates.trading_program,
        derived_maker_root,
        coordinates.payer,
        coordinates.rent_refund,
        exact_replay_rent,
        request_digest,
    )?;
    let custody_request_bytes = custody_request
        .to_bytes()
        .map_err(|error| refusal(format!("encode Direct child Custody request: {error:?}")))?;
    let custody_request_digest = hash(&custody_request_bytes).to_bytes();
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(custody_request.release_set)
            .map_err(|error| refusal(format!("Direct setup release: {error:?}")))?,
        custody_request.market,
        ExecutionRoleV1::Trading,
        custody_request.context,
        custody_request_digest,
    )
    .map_err(|error| refusal(format!("Direct setup caller-authority seeds: {error:?}")))?;
    let derived_caller =
        Pubkey::find_program_address(&caller_seeds.as_slices(), &coordinates.trading_program).0;
    let replay_seeds = CustodyReplaySeedsV1::from_request(custody_request);
    let derived_replay =
        Pubkey::find_program_address(&replay_seeds.as_slices(), &coordinates.custody_program).0;
    if coordinates.caller_authority != derived_caller
        || coordinates.custody_replay != derived_replay
    {
        return Err(refusal(
            "Direct replay setup caller authority or Custody replay was foreign",
        ));
    }

    let instruction = build_direct_replay_instruction_v1(coordinates, &request_bytes)?;
    let plan = DirectReplaySetupPlanV1 {
        instruction,
        request,
        request_bytes,
        request_digest,
        buyer_maker_root: derived_maker_root,
        custody_request,
        custody_request_digest,
        exact_replay_rent,
        observed_replay_lamports: input.observed_replay_lamports,
        coordinates,
    };
    verify_direct_replay_setup_instruction_v1(&plan, &plan.instruction)?;
    Ok(plan)
}

/// Refuse any reordered, reprivileged, or otherwise altered setup instruction.
pub(crate) fn verify_direct_replay_setup_instruction_v1(
    plan: &DirectReplaySetupPlanV1,
    instruction: &Instruction,
) -> Result<()> {
    let expected = build_direct_replay_instruction_v1(plan.coordinates, &plan.request_bytes)?;
    if instruction != &expected {
        return Err(refusal(
            "Direct replay setup instruction was reordered or reprivileged",
        ));
    }
    Ok(())
}

/// Verify the Trading return receipt and the complete initialized Custody replay.
pub(crate) fn verify_direct_replay_setup_poststate_v1(
    plan: &DirectReplaySetupPlanV1,
    return_program: Pubkey,
    return_receipt_bytes: &[u8],
    replay: DirectSetupObservedAccountV1<'_>,
) -> Result<VerifiedDirectReplaySetupPoststateV1> {
    if return_program != plan.coordinates.trading_program
        || return_receipt_bytes.len() != DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1
        || replay.address != plan.coordinates.custody_replay
        || replay.owner != plan.coordinates.custody_program
        || replay.executable
        || replay.lamports != plan.exact_replay_rent
        || replay.data.len() != CUSTODY_REPLAY_BYTES_V1
    {
        return Err(refusal(
            "Direct replay setup return producer or replay account envelope was foreign",
        ));
    }

    let expected = project_direct_replay_setup_poststate_v1(plan)?;
    if replay.data != expected.replay_bytes {
        return Err(refusal(
            "Direct replay setup CustodyReplay full bytes did not match the request projection",
        ));
    }
    let decoded_replay = CustodyReplayV1::decode(replay.data)
        .map_err(|error| refusal(format!("decode Direct Custody replay: {error:?}")))?;
    if decoded_replay != expected.replay {
        return Err(refusal(
            "Direct replay setup CustodyReplay was noncanonical",
        ));
    }

    if return_receipt_bytes != expected.receipt_bytes {
        return Err(refusal(
            "Direct replay setup receipt bytes changed after planning",
        ));
    }
    let receipt = DirectReplaySetupReceiptV1::decode(return_receipt_bytes)
        .map_err(|error| refusal(format!("Direct setup receipt: {error:?}")))?;
    if receipt != expected.receipt {
        return Err(refusal(
            "Direct replay setup receipt did not match the exact request and poststate",
        ));
    }
    Ok(VerifiedDirectReplaySetupPoststateV1 {
        receipt,
        replay: decoded_replay,
    })
}

/// Project the complete replay and typed ACK from the frozen prestate.
pub(crate) fn project_direct_replay_setup_poststate_v1(
    plan: &DirectReplaySetupPlanV1,
) -> Result<DirectReplaySetupExpectedPoststateV1> {
    let poststate_commitment = expected_initialize_replay_poststate_commitment_v1(plan);
    let replay = CustodyReplayV1::initialize(
        plan.custody_request,
        plan.custody_request_digest,
        poststate_commitment,
    )
    .map_err(|error| refusal(format!("project Direct Custody replay: {error:?}")))?;
    let replay_bytes = replay
        .to_bytes()
        .map_err(|error| refusal(format!("encode Direct Custody replay: {error:?}")))?;
    let replay_digest = hash(&replay_bytes).to_bytes();
    let payer_top_up = plan
        .exact_replay_rent
        .saturating_sub(plan.observed_replay_lamports);
    let refunded_excess = plan
        .observed_replay_lamports
        .saturating_sub(plan.exact_replay_rent);
    let receipt = DirectReplaySetupReceiptV1 {
        request_digest: plan.request_digest,
        market: plan.request.market,
        maker: plan.request.maker,
        maker_root: plan.buyer_maker_root.to_bytes(),
        custody_replay: plan.coordinates.custody_replay.to_bytes(),
        rent_refund: plan.coordinates.rent_refund.to_bytes(),
        payer: plan.coordinates.payer.to_bytes(),
        custody_request_digest: plan.custody_request_digest,
        custody_poststate: poststate_commitment,
        custody_replay_digest: replay_digest,
        observed_lamports: plan.observed_replay_lamports,
        payer_top_up,
        refunded_excess,
        exact_rent: plan.exact_replay_rent,
        post_lamports: plan.exact_replay_rent,
    }
    .new()
    .map_err(|error| refusal(format!("project Direct setup receipt: {error:?}")))?;
    let receipt_bytes = receipt
        .to_bytes()
        .map_err(|error| refusal(format!("encode Direct setup receipt: {error:?}")))?;
    Ok(DirectReplaySetupExpectedPoststateV1 {
        receipt,
        receipt_bytes,
        replay,
        replay_bytes,
    })
}

fn expected_custody_request_v1(
    top: DirectReplaySetupRequestV1,
    market: CoreState,
    trading_program: Pubkey,
    maker_root: Pubkey,
    payer: Pubkey,
    rent_refund: Pubkey,
    exact_rent: u64,
    top_request_digest: [u8; 32],
) -> Result<CustodyRequestV1> {
    let parent_request_digest = direct_replay_setup_parent_digest_v1(
        top_request_digest,
        maker_root.to_bytes(),
        rent_refund.to_bytes(),
        payer.to_bytes(),
        exact_rent,
    );
    let request = CustodyRequestV1 {
        operation: OperationV1::InitializeReplay,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set: market.identity.selected_release_set.to_bytes(),
        market: top.market,
        realm: market.identity.realm_id.to_bytes(),
        context: maker_root.to_bytes(),
        caller_program: trading_program.to_bytes(),
        semantic: ContextV1 {
            candidate: top.maker,
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: [0; 32],
            parent_request_digest,
            order_nonce: 0,
            generation: top.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer: payer.to_bytes(),
        rent_refund: rent_refund.to_bytes(),
        expected_revision: 0,
        resulting_revision: 1,
        amount: 0,
        rent_lamports: exact_rent,
    };
    request
        .validate()
        .map_err(|error| refusal(format!("Direct child Custody request: {error:?}")))?;
    Ok(request)
}

fn require_direct_frame_coordinates_v1(coordinates: DirectReplaySetupCoordinatesV1) -> Result<()> {
    let accounts = direct_frame_accounts_v1(coordinates)?;
    if coordinates.system_program != system_program::ID
        || coordinates.rent_sysvar != sysvar::rent::ID
        || coordinates.trading_program == coordinates.custody_program
    {
        return Err(refusal(
            "Direct replay setup frame contained a foreign program",
        ));
    }
    for (index, account) in accounts.iter().enumerate() {
        if accounts[index + 1..].contains(account) {
            return Err(refusal("Direct replay setup frame aliased two accounts"));
        }
    }
    Ok(())
}

fn direct_frame_accounts_v1(
    coordinates: DirectReplaySetupCoordinatesV1,
) -> Result<[Pubkey; DIRECT_REPLAY_SETUP_ACCOUNT_COUNT_V1]> {
    let frame = CustodyFrameSpecV1::new(OperationV1::InitializeReplay);
    let mut accounts = [Pubkey::default(); DIRECT_REPLAY_SETUP_ACCOUNT_COUNT_V1];
    for index in 0..INITIALIZE_REPLAY_ACCOUNT_COUNT_V1 {
        let role = frame
            .account(index)
            .map_err(|error| refusal(format!("Direct Custody frame coordinate: {error:?}")))?
            .role();
        accounts[usize::from(index)] = match role {
            CustodyFrameRoleV1::CallerAuthority => coordinates.caller_authority,
            CustodyFrameRoleV1::CoreMarket => coordinates.market,
            CustodyFrameRoleV1::ActivationCache => coordinates.activation_cache,
            CustodyFrameRoleV1::RegistryProgram => coordinates.registry_program,
            CustodyFrameRoleV1::CallerProgram => coordinates.trading_program,
            CustodyFrameRoleV1::CallerProgramData => coordinates.trading_programdata,
            CustodyFrameRoleV1::RealmRecord => coordinates.realm_raw,
            CustodyFrameRoleV1::RealmStaging => coordinates.realm_staging,
            CustodyFrameRoleV1::Replay => coordinates.custody_replay,
            CustodyFrameRoleV1::Payer => coordinates.payer,
            CustodyFrameRoleV1::SystemProgram => coordinates.system_program,
            CustodyFrameRoleV1::RentSysvar => coordinates.rent_sysvar,
            CustodyFrameRoleV1::RentRefund => coordinates.rent_refund,
            CustodyFrameRoleV1::Mint
            | CustodyFrameRoleV1::Vault
            | CustodyFrameRoleV1::CustodyAuthority
            | CustodyFrameRoleV1::TokenProgram
            | CustodyFrameRoleV1::TransferSource
            | CustodyFrameRoleV1::TransferDestination => {
                return Err(refusal(
                    "Direct InitializeReplay selected a non-setup Custody role",
                ));
            }
        };
    }
    accounts[usize::from(INITIALIZE_REPLAY_ACCOUNT_COUNT_V1)] = coordinates.custody_program;
    Ok(accounts)
}

fn build_direct_replay_instruction_v1(
    coordinates: DirectReplaySetupCoordinatesV1,
    request_bytes: &[u8; DIRECT_REPLAY_SETUP_REQUEST_BYTES_V1],
) -> Result<Instruction> {
    let accounts = direct_frame_accounts_v1(coordinates)?;
    let frame = CustodyFrameSpecV1::new(OperationV1::InitializeReplay);
    let mut metas = Vec::with_capacity(DIRECT_REPLAY_SETUP_ACCOUNT_COUNT_V1);
    for index in 0..INITIALIZE_REPLAY_ACCOUNT_COUNT_V1 {
        let account = frame
            .account(index)
            .map_err(|error| refusal(format!("Direct Custody frame coordinate: {error:?}")))?;
        let signer = account.role() == CustodyFrameRoleV1::Payer;
        metas.push(if account.privileges().writable() {
            AccountMeta::new(accounts[usize::from(index)], signer)
        } else {
            AccountMeta::new_readonly(accounts[usize::from(index)], signer)
        });
    }
    metas.push(AccountMeta::new_readonly(
        coordinates.custody_program,
        false,
    ));
    Ok(Instruction {
        program_id: coordinates.trading_program,
        accounts: metas,
        data: request_bytes.to_vec(),
    })
}

fn expected_initialize_replay_poststate_commitment_v1(plan: &DirectReplaySetupPlanV1) -> [u8; 32] {
    let zero = 0_u64.to_le_bytes();
    hashv(&[
        CUSTODY_POSTSTATE_DOMAIN_V1,
        &plan.custody_request_digest,
        plan.coordinates.custody_replay.as_ref(),
        plan.coordinates.custody_replay.as_ref(),
        &zero,
        &zero,
        &zero,
        &zero,
        &plan.exact_replay_rent.to_le_bytes(),
    ])
    .to_bytes()
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(message)
}

#[cfg(test)]
mod tests {
    use dclutch_market::{Identity, MarketIdentity, Readiness};

    use super::*;

    fn key(tag: u8) -> Pubkey {
        Pubkey::new_from_array([tag; 32])
    }

    fn identity(tag: u8) -> Identity {
        Identity::new([tag; 32]).expect("identity")
    }

    fn market_state(
        market: Pubkey,
        realm_digest: [u8; 32],
        registry: Pubkey,
        refund: Pubkey,
    ) -> CoreState {
        CoreState {
            phase: CorePhase::Open,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity: MarketIdentity {
                market_id: Identity::new(market.to_bytes()).expect("market"),
                realm_id: Identity::new(realm_digest).expect("realm"),
                product_record: identity(31),
                product_id: identity(32),
                resolution_policy: identity(33),
                capability_manifest: identity(34),
                selected_release_set: identity(35),
                registry_program: Identity::new(registry.to_bytes()).expect("registry"),
                generation: 9,
            },
            outstanding_capabilities: 1,
            principal_cap_sets: 1,
            rent_beneficiary: Identity::new(refund.to_bytes()).expect("refund"),
            terminal_receipt: None,
            bumps: StateBumpsV1::UNRECORDED,
        }
    }

    fn replay_fixture() -> (
        [u8; DIRECT_REPLAY_SETUP_REQUEST_BYTES_V1],
        [u8; dclutch_market::STATE_BYTES],
        DirectReplaySetupBuildInputV1<'static>,
    ) {
        let market = key(1);
        let realm = key(2);
        let registry = key(3);
        let trading = key(4);
        let custody = key(5);
        let payer = key(6);
        let refund = key(7);
        let buyer = key(8);
        let realm_bytes = Box::leak(vec![0x22; 64].into_boxed_slice());
        let state = market_state(market, hash(realm_bytes).to_bytes(), registry, refund);
        let market_bytes = Box::leak(Box::new(state.encode().expect("market bytes")));
        let request = DirectReplaySetupRequestV1 {
            market: market.to_bytes(),
            maker: buyer.to_bytes(),
            expected_market_digest: hash(market_bytes).to_bytes(),
            generation: state.identity.generation,
        };
        let request_bytes = Box::leak(Box::new(request.to_bytes().expect("request")));
        let maker_seeds = MakerReplaySeedsV1::new(
            DirectCoordinatesV1::new(request.market, request.generation).expect("coordinates"),
            request.maker,
        )
        .expect("maker seeds");
        let maker_root = Pubkey::find_program_address(&maker_seeds.as_slices(), &trading).0;
        let rent = Rent::default();
        let exact_rent = rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);
        let request_digest = hash(request_bytes).to_bytes();
        let custody_request = expected_custody_request_v1(
            request,
            state,
            trading,
            maker_root,
            payer,
            refund,
            exact_rent,
            request_digest,
        )
        .expect("custody request");
        let custody_request_digest =
            hash(&custody_request.to_bytes().expect("custody bytes")).to_bytes();
        let caller_seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(custody_request.release_set).expect("release"),
            custody_request.market,
            ExecutionRoleV1::Trading,
            custody_request.context,
            custody_request_digest,
        )
        .expect("caller seeds");
        let caller_authority = Pubkey::find_program_address(&caller_seeds.as_slices(), &trading).0;
        let custody_replay = Pubkey::find_program_address(
            &CustodyReplaySeedsV1::from_request(custody_request).as_slices(),
            &custody,
        )
        .0;
        let input = DirectReplaySetupBuildInputV1 {
            request_bytes,
            market_bytes,
            realm_bytes,
            buyer_maker: buyer,
            buyer_maker_root: maker_root,
            coordinates: DirectReplaySetupCoordinatesV1 {
                caller_authority,
                market,
                activation_cache: key(10),
                registry_program: registry,
                trading_program: trading,
                trading_programdata: key(11),
                realm_raw: realm,
                realm_staging: key(12),
                custody_replay,
                payer,
                system_program: system_program::ID,
                rent_sysvar: sysvar::rent::ID,
                rent_refund: refund,
                custody_program: custody,
            },
            rent,
            observed_replay_lamports: exact_rent + 7,
        };
        (*request_bytes, *market_bytes, input)
    }

    fn expected_replay_poststate(
        plan: &DirectReplaySetupPlanV1,
    ) -> (
        [u8; CUSTODY_REPLAY_BYTES_V1],
        [u8; DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1],
    ) {
        let poststate = expected_initialize_replay_poststate_commitment_v1(plan);
        let replay = CustodyReplayV1::initialize(
            plan.custody_request,
            plan.custody_request_digest,
            poststate,
        )
        .expect("replay")
        .to_bytes()
        .expect("replay bytes");
        let receipt = DirectReplaySetupReceiptV1 {
            request_digest: plan.request_digest,
            market: plan.request.market,
            maker: plan.request.maker,
            maker_root: plan.buyer_maker_root.to_bytes(),
            custody_replay: plan.coordinates.custody_replay.to_bytes(),
            rent_refund: plan.coordinates.rent_refund.to_bytes(),
            payer: plan.coordinates.payer.to_bytes(),
            custody_request_digest: plan.custody_request_digest,
            custody_poststate: poststate,
            custody_replay_digest: hash(&replay).to_bytes(),
            observed_lamports: plan.observed_replay_lamports,
            payer_top_up: 0,
            refunded_excess: plan.observed_replay_lamports - plan.exact_replay_rent,
            exact_rent: plan.exact_replay_rent,
            post_lamports: plan.exact_replay_rent,
        }
        .to_bytes()
        .expect("receipt");
        (replay, receipt)
    }

    #[test]
    fn direct_setup_owns_exact_frame_request_and_poststate() {
        let (_, _, input) = replay_fixture();
        let plan = build_direct_replay_setup_v1(input.clone()).expect("plan");
        assert_eq!(
            plan.instruction.accounts.len(),
            DIRECT_REPLAY_SETUP_ACCOUNT_COUNT_V1
        );
        assert!(!plan.instruction.accounts[0].is_signer);
        assert!(
            plan.instruction
                .accounts
                .iter()
                .any(|account| account.pubkey == plan.coordinates.payer && account.is_signer)
        );
        let (replay_bytes, receipt_bytes) = expected_replay_poststate(&plan);
        let verified = verify_direct_replay_setup_poststate_v1(
            &plan,
            plan.coordinates.trading_program,
            &receipt_bytes,
            DirectSetupObservedAccountV1 {
                address: plan.coordinates.custody_replay,
                owner: plan.coordinates.custody_program,
                lamports: plan.exact_replay_rent,
                executable: false,
                data: &replay_bytes,
            },
        )
        .expect("poststate");
        assert_eq!(verified.replay.context, plan.buyer_maker_root.to_bytes());
        assert_eq!(verified.receipt.refunded_excess, 7);
    }

    #[test]
    fn direct_setup_refuses_reordering_aliases_and_foreign_derivations() {
        let (_, _, input) = replay_fixture();
        let plan = build_direct_replay_setup_v1(input.clone()).expect("plan");
        let mut reordered = plan.instruction.clone();
        reordered.accounts.swap(1, 2);
        assert!(verify_direct_replay_setup_instruction_v1(&plan, &reordered).is_err());

        let mut aliased = input.clone();
        aliased.coordinates.activation_cache = aliased.coordinates.realm_staging;
        assert!(build_direct_replay_setup_v1(aliased).is_err());
        let mut foreign_root = input.clone();
        foreign_root.buyer_maker_root = key(90);
        assert!(build_direct_replay_setup_v1(foreign_root).is_err());
        let mut foreign_caller = input.clone();
        foreign_caller.coordinates.caller_authority = key(91);
        assert!(build_direct_replay_setup_v1(foreign_caller).is_err());
        let mut foreign_replay = input.clone();
        foreign_replay.coordinates.custody_replay = key(92);
        assert!(build_direct_replay_setup_v1(foreign_replay).is_err());
    }

    #[test]
    fn direct_setup_refuses_foreign_request_receipt_owner_and_poststate() {
        let (request_bytes, _, input) = replay_fixture();
        let mut request = DirectReplaySetupRequestV1::decode(&request_bytes).expect("request");
        request.maker = key(93).to_bytes();
        let hostile_request = request.to_bytes().expect("hostile request");
        let foreign_request = DirectReplaySetupBuildInputV1 {
            request_bytes: &hostile_request,
            ..input.clone()
        };
        assert!(build_direct_replay_setup_v1(foreign_request).is_err());

        let plan = build_direct_replay_setup_v1(input).expect("plan");
        let (replay_bytes, receipt_bytes) = expected_replay_poststate(&plan);
        let observation = DirectSetupObservedAccountV1 {
            address: plan.coordinates.custody_replay,
            owner: plan.coordinates.custody_program,
            lamports: plan.exact_replay_rent,
            executable: false,
            data: &replay_bytes,
        };
        let mut wrong_receipt =
            DirectReplaySetupReceiptV1::decode(&receipt_bytes).expect("receipt");
        wrong_receipt.maker = key(94).to_bytes();
        assert!(
            verify_direct_replay_setup_poststate_v1(
                &plan,
                plan.coordinates.trading_program,
                &wrong_receipt.to_bytes().expect("wrong receipt"),
                observation,
            )
            .is_err()
        );
        assert!(
            verify_direct_replay_setup_poststate_v1(
                &plan,
                plan.coordinates.trading_program,
                &receipt_bytes,
                DirectSetupObservedAccountV1 {
                    owner: key(95),
                    ..observation
                },
            )
            .is_err()
        );
        let mut wrong_replay = CustodyReplayV1::decode(&replay_bytes).expect("decoded replay");
        wrong_replay.next_revision += 1;
        let wrong_replay_bytes = wrong_replay.to_bytes().expect("wrong replay");
        assert!(
            verify_direct_replay_setup_poststate_v1(
                &plan,
                plan.coordinates.trading_program,
                &receipt_bytes,
                DirectSetupObservedAccountV1 {
                    data: &wrong_replay_bytes,
                    ..observation
                },
            )
            .is_err()
        );
    }

    #[test]
    fn direct_setup_refuses_wrong_rent_and_dust_normalization() {
        let (_, _, input) = replay_fixture();
        let plan = build_direct_replay_setup_v1(input).expect("plan");
        let (replay_bytes, receipt_bytes) = expected_replay_poststate(&plan);
        let observation = DirectSetupObservedAccountV1 {
            address: plan.coordinates.custody_replay,
            owner: plan.coordinates.custody_program,
            lamports: plan.exact_replay_rent,
            executable: false,
            data: &replay_bytes,
        };
        assert!(
            verify_direct_replay_setup_poststate_v1(
                &plan,
                plan.coordinates.trading_program,
                &receipt_bytes,
                DirectSetupObservedAccountV1 {
                    lamports: plan.exact_replay_rent + 1,
                    ..observation
                },
            )
            .is_err()
        );
        let mut hostile_normalization = receipt_bytes;
        let refunded_excess_offset = DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1 - 24;
        hostile_normalization[refunded_excess_offset] ^= 1;
        assert!(
            verify_direct_replay_setup_poststate_v1(
                &plan,
                plan.coordinates.trading_program,
                &hostile_normalization,
                observation,
            )
            .is_err()
        );
    }
}
