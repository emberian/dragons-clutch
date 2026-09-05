//! Finalized planning for one wallet-authorized Claims Position close.
//!
//! Claims remains the semantic owner of Position/admission state and close
//! conservation. This host-only builder reauthenticates one finalized snapshot
//! and constructs the existing Trading outer; it performs no RPC, signing, or
//! submission. The request carries the immutable admission-time lamport
//! baselines. The predicted receipt carries authenticated live balances, so a
//! third-party donation cannot veto close or disappear during reclamation.

use dclutch_claims::position_admission::{
    USER_POSITION_CLOSE_ACCOUNT_COUNT_V1, UserPositionAdmissionRequestV1, UserPositionCloseFrameV1,
};
use dclutch_claims::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, PROTOCOL_POSITION_CLOSE_RECEIPT_BYTES_V2,
        PROTOCOL_POSITION_CLOSE_RESOURCE_DOMAIN_V2, ProtocolPositionActionV2,
        ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2,
        ProtocolPositionCloseEvidenceV2, ProtocolPositionCloseReceiptV2,
        ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_market::capability_manifest::funding::{
    funded_rent_minimum_v2, funded_rent_persists_v1, funded_rent_rate_from_minimum_v1,
};
use dclutch_market::rent::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_registry::svm::ProgramV3View;
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1,
};
use dclutch_source::relay::SOLANA_DEVNET_GENESIS_HASH_V1;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program};

use crate::{
    Finality, Observation, ObservedAccount, observation::decode_rent,
    user_position_admission_v1::authenticate_role_deployment,
};

/// Complete finalized snapshot required for one User Position close.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserPositionCloseSnapshotV1 {
    /// Cluster genesis hash reported with this snapshot.
    pub genesis_hash: [u8; 32],
    /// Existing Claims-owned LiabilityBasisV2 aggregate.
    pub claims_market: ObservedAccount,
    /// Existing canonical zero-vector Claims Position.
    pub position: ObservedAccount,
    /// Existing canonical Claims admission record.
    pub admission: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Registry-owned activated execution-release-set cache.
    pub activation_cache: ObservedAccount,
    /// Market-selected executable Registry program.
    pub registry_program: ObservedAccount,
    /// Activated executable Trading program.
    pub trading_program: ObservedAccount,
    /// Current Trading ProgramData account and complete ELF tail.
    pub trading_programdata: ObservedAccount,
    /// Activated executable Claims program.
    pub claims_program: ObservedAccount,
    /// Current Claims ProgramData account and complete ELF tail.
    pub claims_programdata: ObservedAccount,
    /// Wallet identity that must authorize close.
    pub owner: ObservedAccount,
    /// Existing lifecycle-scoped RentCredit selected at admission.
    pub rent_credit: ObservedAccount,
    /// Executable program owning the RentCredit.
    pub rent_program: ObservedAccount,
}

/// Exact unsigned close plan and independently predicted Claims receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserPositionClosePlanV1 {
    /// Sole Trading instruction; no funding is required for close.
    pub instruction: Instruction,
    /// Sole required transaction signer.
    pub required_signer: Pubkey,
    /// Finalized observation selecting every input.
    pub observation: Observation,
    /// Canonical Claims close request embedded in the Trading outer.
    pub claims_request: ProtocolPositionRequestV2,
    /// SHA-256 of the exact child request bytes.
    pub claims_request_digest: [u8; 32],
    /// Request-bound Trading caller-authority PDA supplied to Claims.
    pub caller_authority: Pubkey,
    /// Canonical Claims Position PDA.
    pub position: Pubkey,
    /// Canonical Claims admission-record PDA.
    pub admission: Pubkey,
    /// Authenticated live Position balance reclaimed by close.
    pub position_lamports: u64,
    /// Authenticated live admission-record balance reclaimed by close.
    pub admission_lamports: u64,
    /// Exact sum of both reclaimed balances.
    pub total_credit_lamports: u64,
    /// RentCredit balance before close.
    pub rent_credit_before_lamports: u64,
    /// Predicted RentCredit balance after close.
    pub rent_credit_after_lamports: u64,
    /// Exact program expected to produce immediate return data.
    pub expected_receipt_producer: Pubkey,
    /// Exact Claims receipt body predicted from authenticated live inputs.
    pub expected_receipt_body: [u8; PROTOCOL_POSITION_CLOSE_RECEIPT_BYTES_V2],
}

/// Stable refusal from hostile, stale, non-devnet, or nonterminal observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserPositionClosePlanErrorV1 {
    /// The supplied cluster identity was not Solana devnet.
    DevnetOnly,
    /// Accounts were not all from one nonzero finalized observation.
    InvalidObservation,
    /// A canonical sysvar, System program, executable shell, or wallet refused.
    InvalidInfrastructure,
    /// Activated release cache or current role deployment refused.
    InvalidRelease,
    /// Claims aggregate bytes, PDA, revision, or immutable links refused.
    InvalidClaimsMarket,
    /// Position/admission identity, contents, or terminal zero vector refused.
    InvalidPosition,
    /// RentCredit identity, ownership, or lifecycle binding refused.
    InvalidRentCredit,
    /// Live balances, rent principals, or credit arithmetic refused.
    InvalidRent,
    /// Canonical child/outer request, authority, or receipt construction refused.
    InvalidRequest,
    /// `dclutch_operator` refused; the cause is its own.
    Observation(crate::observation::ObservationError),
    /// `dclutch_operator` refused; the cause is its own.
    UserPositionAdmissionPlan(crate::user_position_admission_v1::UserPositionAdmissionPlanErrorV1),
    /// `dclutch_registry` refused; the cause is its own.
    Registry(dclutch_registry::Error),
    /// `dclutch_claims` refused; the cause is its own.
    LiabilityBasisState(dclutch_claims::liability_basis_state_v2::LiabilityBasisStateErrorV2),
    /// `dclutch_claims` refused; the cause is its own.
    ProtocolPosition(dclutch_claims::protocol_position_v2::ProtocolPositionErrorV2),
    /// `dclutch_market::capability_manifest` refused; the cause is its own.
    Capability(dclutch_market::capability_manifest::Error),
    /// `dclutch_market::rent` refused; the cause is its own.
    LifecycleRent(dclutch_market::rent::lifecycle_v2::LifecycleRentErrorV2),
    /// `dclutch_claims::position_admission` refused; the cause is its own.
    UserPositionAdmission(dclutch_claims::position_admission::UserPositionAdmissionErrorV1),
    /// `dclutch_registry::release_set` refused; the cause is its own.
    ReleaseSet(dclutch_registry::release_set::Error),
}

/// Reauthenticate one exact finalized snapshot and build its unsigned close.
pub fn plan_user_position_close_v1(
    snapshot: &UserPositionCloseSnapshotV1,
) -> Result<UserPositionClosePlanV1, UserPositionClosePlanErrorV1> {
    if snapshot.genesis_hash != SOLANA_DEVNET_GENESIS_HASH_V1 {
        return Err(UserPositionClosePlanErrorV1::DevnetOnly);
    }
    let observation = same_finalized_observation(snapshot)?;
    // The Rent sysvar is still AUTHENTICATED here -- key, owner, executable bit,
    // exact width, canonical body -- even though nothing prices a floor against
    // it any more. Dropping the decode with the floor would silently stop
    // checking the coordinate, which is the debt `a4b2cbb17` named at
    // `authenticate_execution_strategy_v2` and this does not repeat.
    decode_rent(&snapshot.rent_sysvar).map_err(UserPositionClosePlanErrorV1::Observation)?;
    authenticate_infrastructure(snapshot)?;
    let activated = authenticate_release_cache(snapshot)?;
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Trading,
            &snapshot.trading_program,
            &snapshot.trading_programdata,
        ),
        (
            ExecutionRoleV1::Claims,
            &snapshot.claims_program,
            &snapshot.claims_programdata,
        ),
    ] {
        authenticate_role_deployment(activated, role, program, programdata)
            .map_err(UserPositionClosePlanErrorV1::UserPositionAdmissionPlan)?;
    }
    let market = authenticate_claims_market(snapshot, activated)?;
    let admission = authenticate_position(snapshot, market)?;
    authenticate_rent_credit(snapshot, market)?;
    assemble_plan(snapshot, observation, market, admission)
}

fn same_finalized_observation(
    snapshot: &UserPositionCloseSnapshotV1,
) -> Result<Observation, UserPositionClosePlanErrorV1> {
    let accounts = [
        &snapshot.claims_market,
        &snapshot.position,
        &snapshot.admission,
        &snapshot.rent_sysvar,
        &snapshot.system_program,
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.trading_program,
        &snapshot.trading_programdata,
        &snapshot.claims_program,
        &snapshot.claims_programdata,
        &snapshot.owner,
        &snapshot.rent_credit,
        &snapshot.rent_program,
    ];
    let observation = accounts[0].observation;
    if observation.slot == 0
        || observation.finality != Finality::Finalized
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(UserPositionClosePlanErrorV1::InvalidObservation);
    }
    Ok(observation)
}

fn authenticate_infrastructure(
    snapshot: &UserPositionCloseSnapshotV1,
) -> Result<(), UserPositionClosePlanErrorV1> {
    if snapshot.system_program.key != system_program::ID
        || snapshot.system_program.owner != native_loader::ID
        || !snapshot.system_program.executable
        || snapshot.owner.owner != system_program::ID
        || snapshot.owner.executable
        || !snapshot.owner.data.is_empty()
    {
        return Err(UserPositionClosePlanErrorV1::InvalidInfrastructure);
    }
    for program in [&snapshot.registry_program, &snapshot.rent_program] {
        if program.owner != bpf_loader_upgradeable::ID
            || !program.executable
            || ProgramV3View::parse(&program.data).is_err()
        {
            return Err(UserPositionClosePlanErrorV1::InvalidInfrastructure);
        }
    }
    if snapshot.rent_credit.owner != snapshot.rent_program.key
        || snapshot.rent_credit.executable
        || !funded_rent_persists_v1(snapshot.rent_credit.lamports)
    {
        return Err(UserPositionClosePlanErrorV1::InvalidRentCredit);
    }
    Ok(())
}

fn authenticate_release_cache<'a>(
    snapshot: &'a UserPositionCloseSnapshotV1,
) -> Result<ActivatedExecutionReleaseSetViewV1<'a>, UserPositionClosePlanErrorV1> {
    let cache = &snapshot.activation_cache;
    if cache.owner != snapshot.registry_program.key
        || cache.executable
        || cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || !funded_rent_persists_v1(cache.lamports)
    {
        return Err(UserPositionClosePlanErrorV1::InvalidRelease);
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache.data)
        .map_err(UserPositionClosePlanErrorV1::Registry)?;
    let release_set = activated
        .execution_release_set_id()
        .map_err(UserPositionClosePlanErrorV1::Registry)?;
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_bytes()],
        &snapshot.registry_program.key,
    )
    .0;
    if cache.key != expected {
        return Err(UserPositionClosePlanErrorV1::InvalidRelease);
    }
    Ok(activated)
}

fn authenticate_claims_market(
    snapshot: &UserPositionCloseSnapshotV1,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> Result<LiabilityBasisMarketViewV2, UserPositionClosePlanErrorV1> {
    let account = &snapshot.claims_market;
    if account.owner != snapshot.claims_program.key
        || account.executable
        || !funded_rent_persists_v1(account.lamports)
    {
        return Err(UserPositionClosePlanErrorV1::InvalidClaimsMarket);
    }
    let market = LiabilityBasisMarketViewV2::decode(&account.data)
        .map_err(UserPositionClosePlanErrorV1::LiabilityBasisState)?;
    let expected = Pubkey::find_program_address(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            market.logical_market.as_slice(),
        ],
        &snapshot.claims_program.key,
    )
    .0;
    let release_set = activated
        .execution_release_set_id()
        .map_err(UserPositionClosePlanErrorV1::Registry)?;
    if account.key != expected
        || market.registry_program != snapshot.registry_program.key.to_bytes()
        || market.release_set != release_set.to_bytes()
    {
        return Err(UserPositionClosePlanErrorV1::InvalidClaimsMarket);
    }
    Ok(market)
}

fn authenticate_position(
    snapshot: &UserPositionCloseSnapshotV1,
    market: LiabilityBasisMarketViewV2,
) -> Result<ProtocolPositionAdmissionV2, UserPositionClosePlanErrorV1> {
    let position_seeds = ProtocolPositionSeedsV2::new(
        snapshot.claims_market.key.to_bytes(),
        snapshot.owner.key.to_bytes(),
    )
    .map_err(UserPositionClosePlanErrorV1::ProtocolPosition)?;
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(
        snapshot.claims_market.key.to_bytes(),
        snapshot.owner.key.to_bytes(),
    )
    .map_err(UserPositionClosePlanErrorV1::ProtocolPosition)?;
    let expected_position =
        Pubkey::find_program_address(&position_seeds.as_slices(), &snapshot.claims_program.key).0;
    let expected_admission =
        Pubkey::find_program_address(&admission_seeds.as_slices(), &snapshot.claims_program.key).0;
    if snapshot.position.key != expected_position
        || snapshot.admission.key != expected_admission
        || snapshot.position.owner != snapshot.claims_program.key
        || snapshot.admission.owner != snapshot.claims_program.key
        || snapshot.position.executable
        || snapshot.admission.executable
        || snapshot.admission.data.len() != PROTOCOL_POSITION_ADMISSION_BYTES_V2
        || !funded_rent_persists_v1(snapshot.position.lamports)
        || !funded_rent_persists_v1(snapshot.admission.lamports)
    {
        return Err(UserPositionClosePlanErrorV1::InvalidPosition);
    }
    let position = LiabilityBasisPositionViewV2::decode(&snapshot.position.data)
        .map_err(UserPositionClosePlanErrorV1::LiabilityBasisState)?;
    if position.market_account != snapshot.claims_market.key.to_bytes()
        || position.owner != snapshot.owner.key.to_bytes()
        || position.basis_id != market.basis_id
        || position.claim_count != market.claim_count
    {
        return Err(UserPositionClosePlanErrorV1::InvalidPosition);
    }
    for claim in 0..position.claim_count {
        if position
            .balance(&snapshot.position.data, claim)
            .map_err(UserPositionClosePlanErrorV1::LiabilityBasisState)?
            != 0
        {
            return Err(UserPositionClosePlanErrorV1::InvalidPosition);
        }
    }
    let admission = ProtocolPositionAdmissionV2::decode(&snapshot.admission.data)
        .map_err(UserPositionClosePlanErrorV1::ProtocolPosition)?;
    if admission.owner_kind() != ProtocolPositionOwnerKindV2::User
        || admission.release_set() != market.release_set
        || admission.market() != market.logical_market
        || admission.position_owner() != snapshot.owner.key.to_bytes()
        || admission.rent_credit() != snapshot.rent_credit.key.to_bytes()
        || admission.rent_program() != snapshot.rent_program.key.to_bytes()
        || admission.claims_program() != snapshot.claims_program.key.to_bytes()
        || admission.trading_program() != snapshot.trading_program.key.to_bytes()
        || admission.semantic_basis_id() != market.basis_id
        || admission.outcome_count() != market.claim_count
        || admission.generation() != market.generation
        || snapshot.position.lamports < admission.position_lamports()
        || snapshot.admission.lamports < admission.admission_lamports()
    {
        return Err(UserPositionClosePlanErrorV1::InvalidPosition);
    }
    // The two rent principals this admission recorded were written by ONE
    // transaction, at ONE cluster rate, over two accounts nothing has moved
    // since. Requiring them to equal `Rent::minimum_balance` of the moment
    // therefore refuses a live position whenever the rate moves -- in EITHER
    // direction, because an exactness has no slack: devnet's fall from 6,333 to
    // 5,080 at epoch 1141 is what stranded cohort-15, and a rise breaks the
    // same check by the same arithmetic with the sign flipped.
    //
    // Recover the rate the admission was funded at from one principal, and
    // require it to price the other. `minimum_balance` is affine in the length,
    // so one rate prices both widths exactly or the record is garbled -- which
    // is a stronger statement than the sysvar comparison ever made, and it is
    // true at every rate the cluster later adopts.
    let funded_rate = funded_rent_rate_from_minimum_v1(
        admission.admission_rent_principal(),
        PROTOCOL_POSITION_ADMISSION_BYTES_V2,
    )
    .map_err(UserPositionClosePlanErrorV1::Capability)?;
    if funded_rent_minimum_v2(funded_rate, snapshot.position.data.len())
        .map_err(UserPositionClosePlanErrorV1::Capability)?
        != admission.position_rent_principal()
    {
        return Err(UserPositionClosePlanErrorV1::InvalidRent);
    }
    Ok(admission)
}

fn authenticate_rent_credit(
    snapshot: &UserPositionCloseSnapshotV1,
    market: LiabilityBasisMarketViewV2,
) -> Result<(), UserPositionClosePlanErrorV1> {
    if snapshot.rent_credit.owner != snapshot.rent_program.key
        || snapshot.rent_credit.executable
        || !funded_rent_persists_v1(snapshot.rent_credit.lamports)
    {
        return Err(UserPositionClosePlanErrorV1::InvalidRentCredit);
    }
    let credit = LifecycleRentCreditV2::decode(&snapshot.rent_credit.data)
        .map_err(UserPositionClosePlanErrorV1::LifecycleRent)?;
    if credit.market().to_bytes() != market.logical_market
        || credit.release_set().to_bytes() != market.release_set
        || credit.generation() != market.generation
    {
        return Err(UserPositionClosePlanErrorV1::InvalidRentCredit);
    }
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let market_seed = seeds.market().to_bytes();
    let generation = seeds.generation();
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market_seed.as_slice(),
            generation.as_slice(),
            &bump,
        ],
        &snapshot.rent_program.key,
    )
    .map_err(|_| UserPositionClosePlanErrorV1::InvalidRentCredit)?;
    if snapshot.rent_credit.key != expected {
        return Err(UserPositionClosePlanErrorV1::InvalidRentCredit);
    }
    Ok(())
}

fn assemble_plan(
    snapshot: &UserPositionCloseSnapshotV1,
    observation: Observation,
    market: LiabilityBasisMarketViewV2,
    admission: ProtocolPositionAdmissionV2,
) -> Result<UserPositionClosePlanV1, UserPositionClosePlanErrorV1> {
    let claims_request = ProtocolPositionRequestV2::new(ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Close,
        owner_kind: ProtocolPositionOwnerKindV2::User,
        presence: ProtocolPositionPresenceV2::Existing,
        release_set: market.release_set,
        market: market.logical_market,
        position_owner: snapshot.owner.key.to_bytes(),
        parent_request_digest: admission.parent_request_digest(),
        rent_credit: snapshot.rent_credit.key.to_bytes(),
        rent_program: snapshot.rent_program.key.to_bytes(),
        generation: market.generation,
        expected_market_revision: market.revision,
        expected_position_revision: LiabilityBasisPositionViewV2::decode(&snapshot.position.data)
            .map_err(UserPositionClosePlanErrorV1::LiabilityBasisState)?
            .revision,
        observed_position_lamports: admission.position_lamports(),
        observed_admission_lamports: admission.admission_lamports(),
        position_rent_principal: admission.position_rent_principal(),
        admission_rent_principal: admission.admission_rent_principal(),
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    })
    .map_err(UserPositionClosePlanErrorV1::ProtocolPosition)?;
    let outer = UserPositionAdmissionRequestV1::new(claims_request)
        .map_err(UserPositionClosePlanErrorV1::UserPositionAdmission)?;
    let child = outer
        .claims_request_bytes()
        .map_err(UserPositionClosePlanErrorV1::UserPositionAdmission)?;
    let claims_request_digest = hash(&child).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        market.release_set,
        market.logical_market,
        ExecutionRoleV1::Trading,
        snapshot.owner.key.to_bytes(),
        claims_request_digest,
    )
    .map_err(UserPositionClosePlanErrorV1::ReleaseSet)?;
    let caller_authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), &snapshot.trading_program.key).0;
    let frame_keys = [
        snapshot.claims_program.key,
        caller_authority,
        snapshot.claims_market.key,
        snapshot.position.key,
        snapshot.admission.key,
        snapshot.rent_sysvar.key,
        snapshot.system_program.key,
        snapshot.activation_cache.key,
        snapshot.registry_program.key,
        snapshot.trading_program.key,
        snapshot.trading_programdata.key,
        snapshot.claims_program.key,
        snapshot.claims_programdata.key,
        snapshot.owner.key,
        snapshot.rent_credit.key,
        snapshot.rent_program.key,
    ];
    if frame_keys.len() != USER_POSITION_CLOSE_ACCOUNT_COUNT_V1 {
        return Err(UserPositionClosePlanErrorV1::InvalidRequest);
    }
    let frame = UserPositionCloseFrameV1;
    let mut accounts = Vec::with_capacity(USER_POSITION_CLOSE_ACCOUNT_COUNT_V1);
    for (index, key) in frame_keys.into_iter().enumerate() {
        let privileges = frame
            .privileges(index)
            .map_err(UserPositionClosePlanErrorV1::UserPositionAdmission)?;
        accounts.push(if privileges.writable() {
            AccountMeta::new(key, privileges.signer())
        } else {
            AccountMeta::new_readonly(key, privileges.signer())
        });
    }
    let outer_data = outer
        .to_bytes()
        .map_err(UserPositionClosePlanErrorV1::UserPositionAdmission)?;
    let instruction = Instruction {
        program_id: snapshot.trading_program.key,
        accounts,
        data: outer_data.to_vec(),
    };

    let position_lamports = snapshot.position.lamports;
    let admission_lamports = snapshot.admission.lamports;
    let total_credit_lamports = position_lamports
        .checked_add(admission_lamports)
        .ok_or(UserPositionClosePlanErrorV1::InvalidRent)?;
    let rent_credit_before_lamports = snapshot.rent_credit.lamports;
    let rent_credit_after_lamports = rent_credit_before_lamports
        .checked_add(total_credit_lamports)
        .ok_or(UserPositionClosePlanErrorV1::InvalidRent)?;
    let rent_after_bytes = rent_credit_after_lamports.to_le_bytes();
    let rent_data_digest = hash(&snapshot.rent_credit.data).to_bytes();
    let post_resource_digest = hashv(&[
        PROTOCOL_POSITION_CLOSE_RESOURCE_DOMAIN_V2,
        snapshot.position.key.as_ref(),
        snapshot.admission.key.as_ref(),
        snapshot.rent_credit.key.as_ref(),
        &rent_after_bytes,
        &rent_data_digest,
    ])
    .to_bytes();
    let expected_receipt_body = ProtocolPositionCloseReceiptV2::new(
        claims_request,
        ProtocolPositionCloseEvidenceV2 {
            request_digest: claims_request_digest,
            admission_digest: hash(&snapshot.admission.data).to_bytes(),
            claims_program: snapshot.claims_program.key.to_bytes(),
            post_resource_digest,
            position_lamports,
            admission_lamports,
            rent_credit_before: rent_credit_before_lamports,
            rent_credit_after: rent_credit_after_lamports,
        },
    )
    .and_then(ProtocolPositionCloseReceiptV2::to_bytes)
    .map_err(UserPositionClosePlanErrorV1::ProtocolPosition)?;

    Ok(UserPositionClosePlanV1 {
        instruction,
        required_signer: snapshot.owner.key,
        observation,
        claims_request,
        claims_request_digest,
        caller_authority,
        position: snapshot.position.key,
        admission: snapshot.admission.key,
        position_lamports,
        admission_lamports,
        total_credit_lamports,
        rent_credit_before_lamports,
        rent_credit_after_lamports,
        expected_receipt_producer: snapshot.claims_program.key,
        expected_receipt_body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::position_admission::{
        USER_POSITION_CLOSE_CLAIMS_CALLEE_ACCOUNT_V1, USER_POSITION_CLOSE_OWNER_ACCOUNT_V1,
    };
    use dclutch_claims::{
        liability_basis_state_v2::{
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisPositionInputV2,
            encode_liability_basis_position_into_v2, liability_basis_vector_width_v2,
        },
        protocol_position_v2::ProtocolPositionAdmissionEvidenceV2,
    };

    const OBSERVATION: Observation = Observation {
        slot: 91,
        unix_timestamp: 1_788_000_000,
        finality: Finality::Finalized,
    };

    fn key(tag: u8) -> Pubkey {
        Pubkey::new_from_array([tag; 32])
    }

    fn observed(key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) -> ObservedAccount {
        ObservedAccount {
            observation: OBSERVATION,
            key,
            owner,
            lamports,
            executable: false,
            data,
        }
    }

    /// One coherent close snapshot, parameterised by the two rent principals
    /// the admission recorded -- which is the only thing the funded-rate check
    /// reads, and the only thing these two tests need to differ on.
    fn fixture(
        position_rent_principal: u64,
        admission_rent_principal: u64,
    ) -> (
        UserPositionCloseSnapshotV1,
        LiabilityBasisMarketViewV2,
        ProtocolPositionAdmissionV2,
    ) {
        let claims_market = key(2);
        let logical_market = key(3);
        let release_set = key(4);
        let registry_program = key(5);
        let claims_program = key(6);
        let trading_program = key(7);
        let owner = key(8);
        let rent_program = key(9);
        let rent_credit = key(10);
        let basis = key(11).to_bytes();
        let market = LiabilityBasisMarketViewV2 {
            claim_count: 2,
            revision: 17,
            logical_market: logical_market.to_bytes(),
            release_set: release_set.to_bytes(),
            registry_program: registry_program.to_bytes(),
            product_instance_id: [12; 32],
            basis_id: basis,
            realm_id: [13; 32],
            custody_context: [14; 32],
            generation: 15,
        };
        let position_seeds =
            ProtocolPositionSeedsV2::new(claims_market.to_bytes(), owner.to_bytes())
                .expect("Position seeds");
        let admission_seeds =
            ProtocolPositionAdmissionSeedsV2::new(claims_market.to_bytes(), owner.to_bytes())
                .expect("admission seeds");
        let position = Pubkey::find_program_address(&position_seeds.as_slices(), &claims_program).0;
        let admission_key =
            Pubkey::find_program_address(&admission_seeds.as_slices(), &claims_program).0;
        let position_width = liability_basis_vector_width_v2(
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            market.claim_count,
        )
        .expect("Position width");
        let mut position_data = vec![0; position_width];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: 9,
                market_account: claims_market.to_bytes(),
                owner: owner.to_bytes(),
                basis_id: basis,
            },
            &[0, 0],
            &mut position_data,
        )
        .expect("Position");
        let admitted_request = ProtocolPositionRequestV2::new(ProtocolPositionRequestV2 {
            action: ProtocolPositionActionV2::Admit,
            owner_kind: ProtocolPositionOwnerKindV2::User,
            presence: ProtocolPositionPresenceV2::Vacant,
            release_set: release_set.to_bytes(),
            market: logical_market.to_bytes(),
            position_owner: owner.to_bytes(),
            parent_request_digest: [16; 32],
            rent_credit: rent_credit.to_bytes(),
            rent_program: rent_program.to_bytes(),
            generation: market.generation,
            expected_market_revision: market.revision,
            expected_position_revision: 0,
            observed_position_lamports: position_rent_principal + 2,
            observed_admission_lamports: admission_rent_principal + 2,
            position_rent_principal,
            admission_rent_principal,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        })
        .expect("admission request");
        let admission = ProtocolPositionAdmissionV2::new(
            admitted_request,
            ProtocolPositionAdmissionEvidenceV2 {
                product_record_digest: [17; 32],
                semantic_basis_id: basis,
                linked_basis_record_digest: [18; 32],
                request_digest: [19; 32],
                claims_program: claims_program.to_bytes(),
                trading_program: trading_program.to_bytes(),
                capability_descriptor: [0; 32],
                capability_outcome: 0,
                outcome_count: market.claim_count,
            },
        )
        .expect("admission");
        let snapshot = UserPositionCloseSnapshotV1 {
            genesis_hash: SOLANA_DEVNET_GENESIS_HASH_V1,
            claims_market: observed(claims_market, claims_program, 100, vec![1]),
            position: observed(
                position,
                claims_program,
                position_rent_principal + 7,
                position_data,
            ),
            admission: observed(
                admission_key,
                claims_program,
                admission_rent_principal + 8,
                admission
                    .to_state_bytes()
                    .expect("admission bytes")
                    .to_vec(),
            ),
            rent_sysvar: observed(key(20), key(21), 1, vec![1]),
            system_program: observed(system_program::ID, key(22), 1, vec![1]),
            activation_cache: observed(key(23), registry_program, 1, vec![1]),
            registry_program: observed(registry_program, key(24), 1, vec![1]),
            trading_program: observed(trading_program, key(24), 1, vec![1]),
            trading_programdata: observed(key(25), key(24), 1, vec![1]),
            claims_program: observed(claims_program, key(24), 1, vec![1]),
            claims_programdata: observed(key(26), key(24), 1, vec![1]),
            owner: observed(owner, system_program::ID, 1_000, Vec::new()),
            rent_credit: observed(rent_credit, rent_program, 50, vec![2]),
            rent_program: observed(rent_program, key(24), 1, vec![1]),
        };
        (snapshot, market, admission)
    }

    #[test]
    fn close_plan_uses_admission_baselines_but_conserves_donated_live_balances() {
        let (snapshot, market, admission) = fixture(10, 11);
        let plan = assemble_plan(&snapshot, OBSERVATION, market, admission).expect("close plan");
        assert_eq!(
            plan.instruction.accounts.len(),
            USER_POSITION_CLOSE_ACCOUNT_COUNT_V1
        );
        assert_eq!(
            plan.instruction.accounts[USER_POSITION_CLOSE_CLAIMS_CALLEE_ACCOUNT_V1].pubkey,
            key(6)
        );
        assert!(plan.instruction.accounts[USER_POSITION_CLOSE_OWNER_ACCOUNT_V1].is_signer);
        assert_eq!(plan.claims_request.action, ProtocolPositionActionV2::Close);
        assert_eq!(plan.claims_request.observed_position_lamports, 12);
        assert_eq!(plan.claims_request.observed_admission_lamports, 13);
        assert_eq!(plan.position_lamports, 17);
        assert_eq!(plan.admission_lamports, 19);
        assert_eq!(plan.total_credit_lamports, 36);
        assert_eq!(plan.rent_credit_after_lamports, 86);
        let receipt = ProtocolPositionCloseReceiptV2::decode(&plan.expected_receipt_body)
            .expect("close receipt");
        assert_eq!(receipt.position_lamports(), 17);
        assert_eq!(receipt.admission_lamports(), 19);
        assert_eq!(receipt.total_credit(), 36);
    }

    /// A POSITION ADMITTED AT ONE RATE STILL CLOSES AFTER THE CLUSTER MOVES.
    ///
    /// The two principals this admission recorded were written by one
    /// transaction at one cluster rate over two accounts nothing has touched
    /// since. Requiring them to equal `Rent::minimum_balance` of the moment
    /// refused the close whenever the rate moved -- in either direction,
    /// because an exactness has no slack. Devnet's fall from 6,333 to 5,080 at
    /// epoch 1141 is the measured instance; a rise breaks the same check by the
    /// same arithmetic with the sign flipped, and a position that cannot close
    /// is a position whose rent is stranded forever.
    ///
    /// The replacement never reads a sysvar at all: recover the rate from one
    /// principal and require it to price the other, at a different width.
    #[test]
    fn a_position_admitted_at_a_rate_the_cluster_has_left_still_closes() {
        let width = liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, 2)
            .expect("Position width");
        // Cohort-15's rate, read off devnet at finalized slot 493,000,156, and
        // two more the cluster has quoted since. None of them is consulted.
        for rate in [6_333_u32, 5_080, 6_960, 1] {
            let position_principal = funded_rent_minimum_v2(rate, width).expect("position");
            let admission_principal =
                funded_rent_minimum_v2(rate, PROTOCOL_POSITION_ADMISSION_BYTES_V2)
                    .expect("admission");
            assert_ne!(
                position_principal, admission_principal,
                "the two widths must differ, or one rate pricing both says nothing"
            );
            let (snapshot, market, _) = fixture(position_principal, admission_principal);
            let admitted = authenticate_position(&snapshot, market)
                .expect("one rate prices both recorded principals, whatever the cluster charges");
            assert_eq!(admitted.position_rent_principal(), position_principal);
            assert_eq!(admitted.admission_rent_principal(), admission_principal);
        }

        // THE HOSTILE: two principals no single rate prices. That is a garbled
        // record, and it is refused by a code that names the term to look at.
        let width_principal = funded_rent_minimum_v2(6_333, width).expect("position");
        let admission_principal =
            funded_rent_minimum_v2(6_333, PROTOCOL_POSITION_ADMISSION_BYTES_V2).expect("admission");
        let (mixed, market, _) = fixture(
            funded_rent_minimum_v2(5_080, width).expect("position at another rate"),
            admission_principal,
        );
        assert_eq!(
            authenticate_position(&mixed, market),
            Err(UserPositionClosePlanErrorV1::InvalidRent),
            "two rates in one admission is a record nothing wrote"
        );
        let (donated, market, _) = fixture(width_principal, admission_principal + 1);
        assert_eq!(
            authenticate_position(&donated, market),
            Err(UserPositionClosePlanErrorV1::Capability(
                dclutch_market::capability_manifest::Error::UnrepresentableRentRate
            )),
            "a principal one lamport off the affine line is no rate's minimum"
        );
        // A zero principal never reaches this check: `ProtocolPositionRequestV2`
        // refuses to build one, so an admission recording nothing cannot exist
        // on chain to be read back.
        assert!(
            ProtocolPositionRequestV2::new(ProtocolPositionRequestV2 {
                admission_rent_principal: 0,
                ..fixture(width_principal, admission_principal).2.request()
            })
            .is_err()
        );
    }
}
