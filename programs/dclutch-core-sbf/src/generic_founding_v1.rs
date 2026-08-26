//! Shared physical finalization for every Claims FoundingV5 Market.
//!
//! Family wrappers authenticate how a founding context was selected. This
//! module alone authenticates the resulting one-shot Core permit, Claims
//! resources, canonical Custody replay/Hoard, and commit-last Market opening.

use alloc::boxed::Box;

use dclutch_claims_svm::founding_v5::{
    CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5, ClaimsFoundingReceiptV5,
    ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5,
};
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CustodyReplayV1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
};
use dclutch_market_core_codec::{
    Action, Admission, ChildEffectObservation, CoreState, FoundingIntentV5, Request,
    SERIES_FOUNDING_PERMIT_BYTES_V1, SeriesFoundingPermitV1, SeriesOpenObservation,
    open_series_market,
};
use dclutch_rent_contract::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_token_svm::{AccountState, TokenAccount, TokenProgram};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program::invoke_signed,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign};

use crate::CoreSbfError;

/// Fully authenticated inputs to the family-neutral Claims founding compiler.
pub(crate) struct GenericFoundingPermitInputV1 {
    pub(crate) bump: u8,
    pub(crate) release_set: [u8; 32],
    pub(crate) market: [u8; 32],
    pub(crate) product_record: [u8; 32],
    pub(crate) product_id: [u8; 32],
    pub(crate) linked_basis_record: [u8; 32],
    pub(crate) semantic_basis: [u8; 32],
    pub(crate) source: [u8; 32],
    pub(crate) founder: [u8; 32],
    pub(crate) context: [u8; 32],
    pub(crate) capability_root: [u8; 32],
    pub(crate) projected_replay: [u8; 32],
    pub(crate) funding_source: [u8; 32],
    pub(crate) hoard: [u8; 32],
    pub(crate) projected_request_digest: [u8; 32],
    pub(crate) projected_receipt_digest: [u8; 32],
    pub(crate) custody_lock_request_digest: [u8; 32],
    pub(crate) custody_lock_receipt_digest: [u8; 32],
    pub(crate) trading_program: [u8; 32],
    pub(crate) claims_program: [u8; 32],
    pub(crate) rent_credit: [u8; 32],
    pub(crate) rent_program: [u8; 32],
    pub(crate) aggregate: [u8; 32],
    pub(crate) position: [u8; 32],
    pub(crate) admission: [u8; 32],
    pub(crate) generation: u64,
    pub(crate) claim_count: u32,
    pub(crate) quantity: u64,
    pub(crate) basis_scale: u64,
    pub(crate) expiry_slot: u64,
    pub(crate) projected_resulting_revision: u64,
    pub(crate) normal_replay_revision: u64,
    pub(crate) source_amount: u64,
    pub(crate) hoard_amount: u64,
    pub(crate) aggregate_rent: u64,
    pub(crate) position_rent: u64,
    pub(crate) admission_rent: u64,
    pub(crate) aggregate_lamports: u64,
    pub(crate) position_lamports: u64,
    pub(crate) admission_lamports: u64,
}

/// One exact Claims request and matching Core-owned one-shot permit.
pub(crate) struct GenericFoundingPermitPlanV1 {
    pub(crate) permit: Box<SeriesFoundingPermitV1>,
}

/// Compile one Claims FoundingV5 request and permit from authenticated facts.
#[inline(never)]
pub(crate) fn build_permit_plan(
    input: GenericFoundingPermitInputV1,
) -> Result<GenericFoundingPermitPlanV1, CoreSbfError> {
    let intent = FoundingIntentV5::new(
        input.bump,
        identity(input.release_set)?,
        identity(input.market)?,
        identity(input.product_record)?,
        identity(input.source)?,
        identity(input.founder)?,
        identity(input.context)?,
        identity(input.capability_root)?,
        identity(input.projected_replay)?,
        identity(input.funding_source)?,
        identity(input.hoard)?,
        identity(input.projected_request_digest)?,
        identity(input.projected_receipt_digest)?,
        identity(input.trading_program)?,
        identity(input.claims_program)?,
        identity(input.rent_credit)?,
        input.generation,
        input.quantity,
        input.basis_scale,
        input.expiry_slot,
        input.projected_resulting_revision,
        input.normal_replay_revision,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let intent_digest = hash(&intent.encode().map_err(|_| CoreSbfError::Reference)?).to_bytes();
    let claims = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
        release_set: input.release_set,
        market: input.market,
        product_record_digest: input.product_record,
        product_instance_id: input.product_id,
        linked_basis_record_digest: input.linked_basis_record,
        semantic_basis_id: input.semantic_basis,
        founder: input.founder,
        founding_intent_digest: intent_digest,
        aggregate: input.aggregate,
        position: input.position,
        admission: input.admission,
        hoard: input.hoard,
        rent_credit: input.rent_credit,
        rent_program: input.rent_program,
        claims_program: input.claims_program,
        trading_program: input.trading_program,
        funding_source: input.funding_source,
        custody_replay: input.projected_replay,
        custody_request_digest: input.custody_lock_request_digest,
        custody_receipt_digest: input.custody_lock_receipt_digest,
        generation: input.generation,
        claim_count: input.claim_count,
        quantity: input.quantity,
        basis_scale: input.basis_scale,
        pre_source_amount: input.source_amount,
        post_source_amount: 0,
        pre_hoard_amount: 0,
        post_hoard_amount: input.hoard_amount,
        pre_custody_revision: 0,
        post_custody_revision: input.normal_replay_revision,
        aggregate_rent_principal: input.aggregate_rent,
        position_rent_principal: input.position_rent,
        admission_rent_principal: input.admission_rent,
        observed_aggregate_lamports: input.aggregate_lamports,
        observed_position_lamports: input.position_lamports,
        observed_admission_lamports: input.admission_lamports,
        pre_aggregate_revision: 0,
        post_aggregate_revision: 1,
        pre_position_revision: 0,
        post_position_revision: 1,
    })
    .map_err(|_| CoreSbfError::Reference)?;
    let claims_digest = hash(&claims.to_bytes()).to_bytes();
    Ok(GenericFoundingPermitPlanV1 {
        permit: Box::new(
            SeriesFoundingPermitV1::new(intent, identity(intent_digest)?, identity(claims_digest)?)
                .map_err(|_| CoreSbfError::Reference)?,
        ),
    })
}

/// Allocate, assign, and commit the exact Core-owned one-shot permit.
#[inline(never)]
pub(crate) fn create_permit<'info>(
    program_id: &Pubkey,
    permit_account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    permit: &SeriesFoundingPermitV1,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    if permit_account.owner != &system_program::ID
        || !permit_account.data_is_empty()
        || permit_account.lamports() < rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)
    {
        return Err(CoreSbfError::Creation);
    }
    let seeds = permit.seeds();
    let base = seeds.as_slices();
    let bump = [permit.intent().bump()];
    let signer = [base[0], base[1], base[2], base[3], bump.as_slice()];
    for instruction in [
        allocate(
            permit_account.key,
            u64::try_from(SERIES_FOUNDING_PERMIT_BYTES_V1).map_err(|_| CoreSbfError::Arithmetic)?,
        ),
        assign(permit_account.key, program_id),
    ] {
        invoke_signed(
            &instruction,
            &[permit_account.clone(), system.clone()],
            &[&signer],
        )
        .map_err(|_| CoreSbfError::Creation)?;
    }
    let encoded = permit.encode().map_err(|_| CoreSbfError::Commit)?;
    permit_account
        .try_borrow_mut_data()
        .map_err(|_| CoreSbfError::Commit)?
        .copy_from_slice(&encoded);
    let data = permit_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Commit)?;
    if permit_account.owner != program_id || SeriesFoundingPermitV1::decode(&data) != Ok(*permit) {
        return Err(CoreSbfError::Commit);
    }
    Ok(())
}

/// Accounts whose semantics are identical for Series and generic founding.
pub(crate) struct GenericFoundingOpenAccounts<'accounts, 'info> {
    pub(crate) market: &'accounts AccountInfo<'info>,
    pub(crate) permit: &'accounts AccountInfo<'info>,
    pub(crate) rent_credit: &'accounts AccountInfo<'info>,
    pub(crate) rent_program: &'accounts AccountInfo<'info>,
    pub(crate) trading_program: &'accounts AccountInfo<'info>,
    pub(crate) claims_program: &'accounts AccountInfo<'info>,
    pub(crate) custody_program: &'accounts AccountInfo<'info>,
    pub(crate) capability_root: &'accounts AccountInfo<'info>,
    pub(crate) custody_replay: &'accounts AccountInfo<'info>,
    pub(crate) hoard: &'accounts AccountInfo<'info>,
    pub(crate) funding_source: &'accounts AccountInfo<'info>,
    pub(crate) aggregate: &'accounts AccountInfo<'info>,
    pub(crate) position: &'accounts AccountInfo<'info>,
    pub(crate) admission: &'accounts AccountInfo<'info>,
}

/// Authenticate the sole Core permit for one already family-admitted context.
#[inline(never)]
pub(crate) fn authenticate_permit(
    program_id: &Pubkey,
    frame: &GenericFoundingOpenAccounts<'_, '_>,
    context: [u8; 32],
    founder: [u8; 32],
    rent: &Rent,
    current_slot: u64,
    state: CoreState,
) -> Result<Box<SeriesFoundingPermitV1>, CoreSbfError> {
    if frame.permit.owner != program_id
        || frame.permit.data_len() != SERIES_FOUNDING_PERMIT_BYTES_V1
        || !rent.is_exempt(frame.permit.lamports(), SERIES_FOUNDING_PERMIT_BYTES_V1)
    {
        return Err(CoreSbfError::Reference);
    }
    let permit_data = frame
        .permit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let permit =
        SeriesFoundingPermitV1::decode(&permit_data).map_err(|_| CoreSbfError::Reference)?;
    drop(permit_data);
    let intent = permit.intent();
    let (expected_permit, bump) =
        Pubkey::find_program_address(&permit.seeds().as_slices(), program_id);
    if expected_permit != *frame.permit.key
        || bump != intent.bump()
        || intent.market().to_bytes() != frame.market.key.to_bytes()
        || intent.release_set().to_bytes() != state.identity.selected_release_set.to_bytes()
        || intent.ticket_context().to_bytes() != context
        || intent.founder().to_bytes() != founder
        || intent.product_record().to_bytes() != state.identity.product_record.to_bytes()
        || intent.source().to_bytes() != state.identity.resolution_policy.to_bytes()
        || intent.parent_root().to_bytes() != frame.capability_root.key.to_bytes()
        || intent.projected_replay().to_bytes() != frame.custody_replay.key.to_bytes()
        || intent.funding_source().to_bytes() != frame.funding_source.key.to_bytes()
        || intent.hoard().to_bytes() != frame.hoard.key.to_bytes()
        || intent.trading_program().to_bytes() != frame.trading_program.key.to_bytes()
        || intent.claims_program().to_bytes() != frame.claims_program.key.to_bytes()
        || intent.rent_credit().to_bytes() != frame.rent_credit.key.to_bytes()
        || intent.generation() != state.identity.generation
        || current_slot > intent.expiry_slot()
    {
        return Err(CoreSbfError::Reference);
    }
    Ok(Box::new(permit))
}

/// Decode the exact Claims receipt retained by the atomic Trading outer.
#[inline(never)]
pub(crate) fn decode_claims_receipt(
    claims_receipt_bytes: &[u8],
) -> Result<Box<ClaimsFoundingReceiptV5>, CoreSbfError> {
    ClaimsFoundingReceiptV5::decode(claims_receipt_bytes)
        .map(Box::new)
        .map_err(|_| CoreSbfError::ChildAck)
}

/// Authenticate Claims and Custody poststate against the one-shot permit.
#[inline(never)]
pub(crate) fn authenticate_claims_and_custody(
    frame: &GenericFoundingOpenAccounts<'_, '_>,
    permit: &SeriesFoundingPermitV1,
    receipt: &ClaimsFoundingReceiptV5,
    state: CoreState,
) -> Result<(), CoreSbfError> {
    let intent = permit.intent();
    let request = receipt.request();
    receipt
        .verify_for(&request, permit.claims_request_digest().to_bytes())
        .map_err(|_| CoreSbfError::ChildAck)?;
    let intent_digest = hash(&intent.encode().map_err(|_| CoreSbfError::ChildAck)?).to_bytes();
    permit
        .verify_for_intent_and_request(
            intent,
            identity(intent_digest)?,
            identity(receipt.request_digest())?,
        )
        .map_err(|_| CoreSbfError::ChildAck)?;
    if request.release_set() != state.identity.selected_release_set.to_bytes()
        || request.market() != frame.market.key.to_bytes()
        || request.product_record_digest() != state.identity.product_record.to_bytes()
        || request.product_instance_id() != state.identity.product_id.to_bytes()
        || request.founder() != intent.founder().to_bytes()
        || request.founding_intent_digest() != intent_digest
        || request.aggregate() != frame.aggregate.key.to_bytes()
        || request.position() != frame.position.key.to_bytes()
        || request.admission() != frame.admission.key.to_bytes()
        || request.funding_source() != frame.funding_source.key.to_bytes()
        || request.hoard() != frame.hoard.key.to_bytes()
        || request.custody_replay() != frame.custody_replay.key.to_bytes()
        || request.rent_credit() != frame.rent_credit.key.to_bytes()
        || request.rent_program() != frame.rent_program.key.to_bytes()
        || request.claims_program() != frame.claims_program.key.to_bytes()
        || request.trading_program() != frame.trading_program.key.to_bytes()
        || request.generation() != state.identity.generation
        || request.quantity() != intent.quantity()
        || request.basis_scale() != intent.basis_scale()
        || request.post_custody_revision() != intent.normal_replay_revision()
        || request.post_source_amount() != 0
        || request.pre_source_amount() != request.collateral_transferred()
        || request.post_hoard_amount() != request.collateral_transferred()
    {
        return Err(CoreSbfError::ChildAck);
    }
    authenticate_claims_poststate(frame, receipt)?;
    authenticate_custody_poststate(frame, request, intent)
}

/// Apply the sole checked Market Open transition after physical poststate.
#[inline(never)]
pub(crate) fn apply_open(
    state: &mut CoreState,
    receipt: &ClaimsFoundingReceiptV5,
    claims_admission: Admission,
    custody_admission: Admission,
) -> Result<(), CoreSbfError> {
    let request = receipt.request();
    open_series_market(
        Request::administrative(
            Action::OpenMarket,
            state.identity.generation,
            state.identity.market_id,
        ),
        state,
        SeriesOpenObservation {
            claims_admission,
            custody_admission,
            quantity: request.quantity(),
            basis_scale: request.basis_scale(),
            source_debit: request.pre_source_amount(),
            hoard_credit: request.post_hoard_amount(),
            hoard_funding_authenticated: true,
            found_state_bound_by_custody: true,
            claims_custody_join_authenticated: true,
            ticket_prepared_authenticated: true,
            ticket_consumed_candidate_authenticated: true,
            claims_effect: complete_child_effect(),
            custody_effect: complete_child_effect(),
        },
    )
    .map_err(|_| CoreSbfError::Transition)
}

/// Authenticate the permanent LifecycleRentCreditV2 before permit closure.
pub(crate) fn authenticate_rent_credit(
    frame: &GenericFoundingOpenAccounts<'_, '_>,
    beneficiary: [u8; 32],
    release_set: [u8; 32],
    generation: u64,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    if frame.rent_credit.owner != frame.rent_program.key
        || frame.rent_credit.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !rent.is_exempt(frame.rent_credit.lamports(), LIFECYCLE_RENT_CREDIT_BYTES_V2)
    {
        return Err(CoreSbfError::RentCredit);
    }
    let data = frame
        .rent_credit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::RentCredit)?;
    let credit = LifecycleRentCreditV2::decode(&data).map_err(|_| CoreSbfError::RentCredit)?;
    if credit.refund_wallet().to_bytes() != beneficiary
        || credit.market().to_bytes() != frame.market.key.to_bytes()
        || credit.release_set().to_bytes() != release_set
        || credit.generation() != generation
    {
        return Err(CoreSbfError::RentCredit);
    }
    let seeds = credit.pda_seeds();
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[seeds.domain(), &market, &generation, bump.as_slice()],
        frame.rent_program.key,
    )
    .map_err(|_| CoreSbfError::RentCredit)?;
    if expected != *frame.rent_credit.key {
        return Err(CoreSbfError::RentCredit);
    }
    Ok(())
}

/// Commit the authenticated candidate Market state.
pub(crate) fn commit_market(
    market: &AccountInfo<'_>,
    state: CoreState,
    program_id: &Pubkey,
) -> Result<(), CoreSbfError> {
    let encoded = state.encode().map_err(|_| CoreSbfError::Commit)?;
    market
        .try_borrow_mut_data()
        .map_err(|_| CoreSbfError::Commit)?
        .copy_from_slice(&encoded);
    let data = market.try_borrow_data().map_err(|_| CoreSbfError::Commit)?;
    if market.owner != program_id || CoreState::decode(&data) != Ok(state) {
        return Err(CoreSbfError::Commit);
    }
    Ok(())
}

/// Close the consumed one-shot permit only to the immutable RentCredit.
pub(crate) fn close_permit(
    permit: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    program_id: &Pubkey,
) -> Result<(), CoreSbfError> {
    if permit.owner != program_id || permit.key == rent_credit.key {
        return Err(CoreSbfError::Commit);
    }
    let destination = rent_credit
        .lamports()
        .checked_add(permit.lamports())
        .ok_or(CoreSbfError::Arithmetic)?;
    permit
        .try_borrow_mut_data()
        .map_err(|_| CoreSbfError::Commit)?
        .fill(0);
    **permit
        .try_borrow_mut_lamports()
        .map_err(|_| CoreSbfError::Commit)? = 0;
    **rent_credit
        .try_borrow_mut_lamports()
        .map_err(|_| CoreSbfError::Commit)? = destination;
    permit.resize(0).map_err(|_| CoreSbfError::Commit)?;
    permit.assign(&system_program::ID);
    Ok(())
}

fn authenticate_claims_poststate(
    frame: &GenericFoundingOpenAccounts<'_, '_>,
    receipt: &ClaimsFoundingReceiptV5,
) -> Result<(), CoreSbfError> {
    let request = receipt.request();
    for (account, expected, digest) in [
        (
            frame.aggregate,
            request.aggregate(),
            receipt.aggregate_digest(),
        ),
        (
            frame.position,
            request.position(),
            receipt.position_digest(),
        ),
        (
            frame.admission,
            request.admission(),
            receipt.admission_digest(),
        ),
    ] {
        if account.owner != frame.claims_program.key || account.key.to_bytes() != expected {
            return Err(CoreSbfError::ChildAck);
        }
        let data = account
            .try_borrow_data()
            .map_err(|_| CoreSbfError::ChildAck)?;
        if hash(&data).to_bytes() != digest {
            return Err(CoreSbfError::ChildAck);
        }
    }
    let aggregate = frame
        .aggregate
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let position = frame
        .position
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let admission = frame
        .admission
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    if hashv(&[
        CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
        &aggregate,
        &position,
        &admission,
    ])
    .to_bytes()
        != receipt.post_resource_digest()
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn authenticate_custody_poststate(
    frame: &GenericFoundingOpenAccounts<'_, '_>,
    request: dclutch_claims_svm::founding_v5::ClaimsFoundingRequestV5,
    intent: dclutch_market_core_codec::FoundingIntentV5,
) -> Result<(), CoreSbfError> {
    if frame.funding_source.owner != &system_program::ID
        || frame.funding_source.lamports() != 0
        || !frame.funding_source.data_is_empty()
        || frame.custody_replay.owner != frame.custody_program.key
        || frame.custody_replay.data_len() != CUSTODY_REPLAY_BYTES_V1
        || TokenProgram::parse(frame.hoard.owner.to_bytes())
            .map_err(|_| CoreSbfError::ChildAck)?
            .program_id()
            != frame.hoard.owner.to_bytes()
    {
        return Err(CoreSbfError::ChildAck);
    }
    let hoard_data = frame
        .hoard
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let hoard = TokenAccount::parse(&hoard_data).map_err(|_| CoreSbfError::ChildAck)?;
    if hoard.amount != request.post_hoard_amount()
        || hoard.state != AccountState::Initialized
        || !hoard.delegate.is_none()
        || hoard.delegated_amount != 0
        || !hoard.native_reserve.is_none()
        || !hoard.close_authority.is_none()
    {
        return Err(CoreSbfError::ChildAck);
    }
    let replay_data = frame
        .custody_replay
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let replay = CustodyReplayV1::decode(&replay_data).map_err(|_| CoreSbfError::ChildAck)?;
    let context = intent.ticket_context().to_bytes();
    let projected_context =
        hashv(&[PROJECTED_HOARD_CONTEXT_DOMAIN_V1, context.as_slice()]).to_bytes();
    let replay_expected = Pubkey::find_program_address(
        &[
            dclutch_custody_contract::CUSTODY_REPLAY_PDA_DOMAIN_V1,
            &request.market(),
            &request.release_set(),
            &projected_context,
        ],
        frame.custody_program.key,
    )
    .0;
    if replay_expected != *frame.custody_replay.key
        || replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != request.release_set()
        || replay.market != request.market()
        || replay.context != projected_context
        || replay.caller_program != request.trading_program()
        || replay.rent_refund != request.rent_credit()
        || replay.open_vault_count != 1
        || replay.next_revision != intent.normal_replay_revision()
        || replay.last_request_digest != intent.projected_request_digest().to_bytes()
        || replay.last_poststate_commitment != intent.projected_receipt_digest().to_bytes()
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn complete_child_effect() -> ChildEffectObservation {
    ChildEffectObservation {
        exact_request_authenticated: true,
        exact_receipt_authenticated: true,
        post_resource_authenticated: true,
    }
}

fn identity(bytes: [u8; 32]) -> Result<dclutch_market_core_codec::Identity, CoreSbfError> {
    dclutch_market_core_codec::Identity::new(bytes).map_err(|_| CoreSbfError::ChildAck)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn input() -> GenericFoundingPermitInputV1 {
        GenericFoundingPermitInputV1 {
            bump: 7,
            release_set: id(1),
            market: id(2),
            product_record: id(3),
            product_id: id(4),
            linked_basis_record: id(5),
            semantic_basis: id(6),
            source: id(7),
            founder: id(8),
            context: id(9),
            capability_root: id(10),
            projected_replay: id(11),
            funding_source: id(12),
            hoard: id(13),
            projected_request_digest: id(14),
            projected_receipt_digest: id(15),
            custody_lock_request_digest: id(16),
            custody_lock_receipt_digest: id(17),
            trading_program: id(18),
            claims_program: id(19),
            rent_credit: id(20),
            rent_program: id(21),
            aggregate: id(22),
            position: id(23),
            admission: id(24),
            generation: 1,
            claim_count: 3,
            quantity: 2,
            basis_scale: 5,
            expiry_slot: 100,
            projected_resulting_revision: 3,
            normal_replay_revision: 1,
            source_amount: 10,
            hoard_amount: 10,
            aggregate_rent: 100,
            position_rent: 100,
            admission_rent: 100,
            aggregate_lamports: 100,
            position_lamports: 100,
            admission_lamports: 100,
        }
    }

    #[test]
    fn permit_compiler_binds_exact_claims_request_and_refuses_conservation_drift() {
        let valid = input();
        let plan = build_permit_plan(valid).expect("permit");
        assert_eq!(plan.permit.intent().market().to_bytes(), id(2));
        assert_eq!(plan.permit.intent().quantity(), 2);

        let mut hostile = input();
        hostile.hoard_amount = 9;
        assert_eq!(
            build_permit_plan(hostile).err(),
            Some(CoreSbfError::Reference)
        );
    }
}
