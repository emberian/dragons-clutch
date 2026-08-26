//! Authenticated family-neutral signed-delta batches over LiabilityBasisV2.
//!
//! The adapter consumes one canonical already-netted delta per touched
//! `(Position, outcome)` coordinate. It authenticates every immutable Product,
//! basis, release, Core, aggregate, and Position join, builds every candidate,
//! borrows every writable account, and commits the complete batch once.

extern crate alloc;

use alloc::vec::Vec;
use core::{
    cell::RefMut,
    convert::{TryFrom, TryInto},
};

use dclutch_claims_svm::{
    CallerRole,
    protocol_position_v2::ProtocolPositionSeedsV2,
    signed_delta_v3::{DeltaDirectionV3, SignedDeltaPlanV3, SignedDeltaReceiptV3, SignedDeltaV3},
};
use dclutch_core_contract::ContentId;
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::sysvar;

use super::{affine_batch_v2::authenticate_runtime_product_basis_core_v2, reauthenticate};
use crate::liability_basis_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, MarketViewV2, PositionViewV2,
};

/// Exact fixed account count before the runtime Position tail.
pub const SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3: usize = 20;

const AUTHORITY_ACCOUNT: usize = 0;
const MARKET_ACCOUNT: usize = 1;
const BASIS_RECORD_ACCOUNT: usize = 2;
const BASIS_STAGING_ACCOUNT: usize = 3;
const PRODUCT_RECORD_ACCOUNT: usize = 4;
const PRODUCT_STAGING_ACCOUNT: usize = 5;
const RESULT_DOMAIN_RECORD_ACCOUNT: usize = 6;
const RESULT_DOMAIN_STAGING_ACCOUNT: usize = 7;
const PORTFOLIO_RECORD_ACCOUNT: usize = 8;
const PORTFOLIO_STAGING_ACCOUNT: usize = 9;
const RENT_ACCOUNT: usize = 10;
const CORE_MARKET_ACCOUNT: usize = 11;
const ACTIVATION_CACHE_ACCOUNT: usize = 12;
const REGISTRY_PROGRAM_ACCOUNT: usize = 13;
const CALLER_PROGRAM_ACCOUNT: usize = 14;
const CALLER_PROGRAMDATA_ACCOUNT: usize = 15;
const CLAIMS_PROGRAM_ACCOUNT: usize = 16;
const CLAIMS_PROGRAMDATA_ACCOUNT: usize = 17;
const CORE_PROGRAM_ACCOUNT: usize = 18;
const CORE_PROGRAMDATA_ACCOUNT: usize = 19;

const MARKET_REVISION_OFFSET: usize = 16;
const POSITION_REVISION_OFFSET: usize = 16;
const SCALAR_BYTES: usize = 8;
const TABLE_DIGEST_DOMAIN_V3: &[u8] = b"dclutch/claims/signed-delta-table/v3";
const RESOURCE_DIGEST_DOMAIN_V3: &[u8] = b"dclutch/claims/signed-delta-post-resources/v3";

/// Stable signed-delta SBF refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SignedDeltaSbfErrorV3 {
    /// Instruction bytes did not decode as the canonical public ABI.
    Instruction = 200,
    /// Account count, order, privileges, owners, or aliases refused.
    Accounts = 201,
    /// Registry current-release authentication or caller authority refused.
    Release = 202,
    /// Product graph, linked basis, semantic identity, or Core join refused.
    ProductBasis = 203,
    /// Aggregate or Position PDA, width, identity, or revision refused.
    ClaimsState = 204,
    /// An exact signed delta overflowed or underflowed a resource.
    Candidate = 205,
    /// Complete candidate buffers could not all be borrowed and committed last.
    Commit = 206,
    /// The canonical success receipt could not be constructed.
    Receipt = 207,
}

impl From<SignedDeltaSbfErrorV3> for ProgramError {
    fn from(value: SignedDeltaSbfErrorV3) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct SignedDeltaAccountsV3<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    basis_record: &'accounts AccountInfo<'info>,
    basis_staging: &'accounts AccountInfo<'info>,
    product_record: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    result_domain_record: &'accounts AccountInfo<'info>,
    result_domain_staging: &'accounts AccountInfo<'info>,
    portfolio_record: &'accounts AccountInfo<'info>,
    portfolio_staging: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    caller_program: &'accounts AccountInfo<'info>,
    caller_programdata: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    positions: &'accounts [AccountInfo<'info>],
}

impl<'accounts, 'info> SignedDeltaAccountsV3<'accounts, 'info> {
    fn parse(
        accounts: &'accounts [AccountInfo<'info>],
        position_count: u32,
    ) -> Result<Self, ProgramError> {
        let count = usize::try_from(position_count)
            .ok()
            .and_then(|count| SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3.checked_add(count))
            .ok_or(SignedDeltaSbfErrorV3::Accounts)?;
        if accounts.len() != count {
            return Err(SignedDeltaSbfErrorV3::Accounts.into());
        }
        Ok(Self {
            authority: account(accounts, AUTHORITY_ACCOUNT)?,
            market: account(accounts, MARKET_ACCOUNT)?,
            basis_record: account(accounts, BASIS_RECORD_ACCOUNT)?,
            basis_staging: account(accounts, BASIS_STAGING_ACCOUNT)?,
            product_record: account(accounts, PRODUCT_RECORD_ACCOUNT)?,
            product_staging: account(accounts, PRODUCT_STAGING_ACCOUNT)?,
            result_domain_record: account(accounts, RESULT_DOMAIN_RECORD_ACCOUNT)?,
            result_domain_staging: account(accounts, RESULT_DOMAIN_STAGING_ACCOUNT)?,
            portfolio_record: account(accounts, PORTFOLIO_RECORD_ACCOUNT)?,
            portfolio_staging: account(accounts, PORTFOLIO_STAGING_ACCOUNT)?,
            rent: account(accounts, RENT_ACCOUNT)?,
            core_market: account(accounts, CORE_MARKET_ACCOUNT)?,
            cache: account(accounts, ACTIVATION_CACHE_ACCOUNT)?,
            registry: account(accounts, REGISTRY_PROGRAM_ACCOUNT)?,
            caller_program: account(accounts, CALLER_PROGRAM_ACCOUNT)?,
            caller_programdata: account(accounts, CALLER_PROGRAMDATA_ACCOUNT)?,
            claims_program: account(accounts, CLAIMS_PROGRAM_ACCOUNT)?,
            claims_programdata: account(accounts, CLAIMS_PROGRAMDATA_ACCOUNT)?,
            core_program: account(accounts, CORE_PROGRAM_ACCOUNT)?,
            core_programdata: account(accounts, CORE_PROGRAMDATA_ACCOUNT)?,
            positions: accounts
                .get(SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3..)
                .ok_or(SignedDeltaSbfErrorV3::Accounts)?,
        })
    }
}

/// Execute one authenticated runtime-width signed-delta batch.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let plan = SignedDeltaPlanV3::decode(instruction_data)
        .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?;
    let accounts = SignedDeltaAccountsV3::parse(account_infos, plan.position_count())?;
    authenticate_privileges(program_id, &accounts)?;
    let packet_digest = hash(instruction_data).to_bytes();
    authenticate_authority(&accounts, plan, packet_digest)?;
    authenticate_releases(&accounts, plan)?;

    let market_before = accounts
        .market
        .try_borrow_data()
        .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?;
    let market =
        MarketViewV2::decode(&market_before).map_err(|_| SignedDeltaSbfErrorV3::ClaimsState)?;
    authenticate_market(program_id, &accounts, plan, market)?;
    authenticate_product_and_basis(&accounts, plan, market)?;
    let (mut market_candidate, mut position_candidates) =
        build_candidates(program_id, &accounts, plan, market, &market_before)?;
    drop(market_before);

    apply_deltas(plan, &mut market_candidate, &mut position_candidates)?;
    let post_market_revision = plan
        .expected_market_revision()
        .checked_add(1)
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
    put_u64(
        &mut market_candidate,
        MARKET_REVISION_OFFSET,
        post_market_revision,
    )?;
    for candidate in &mut position_candidates {
        let revision = read_u64(candidate, POSITION_REVISION_OFFSET)?
            .checked_add(1)
            .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
        put_u64(candidate, POSITION_REVISION_OFFSET, revision)?;
    }

    let (positions, aggregates, deltas) = plan.table_bytes();
    let table_digest = hashv(&[TABLE_DIGEST_DOMAIN_V3, positions, aggregates, deltas]).to_bytes();
    let post_resource_digest = resource_digest(&market_candidate, &position_candidates);
    let receipt = SignedDeltaReceiptV3::new(
        plan,
        packet_digest,
        table_digest,
        program_id.to_bytes(),
        post_resource_digest,
        post_market_revision,
    )
    .map_err(|_| SignedDeltaSbfErrorV3::Receipt)?;
    commit_candidates(&accounts, &market_candidate, &position_candidates)?;
    set_return_data(&receipt.to_bytes());
    Ok(())
}

fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: &SignedDeltaAccountsV3<'_, '_>,
) -> Result<(), ProgramError> {
    if !accounts.authority.is_signer
        || accounts.authority.is_writable
        || accounts.authority.executable
        || !accounts.market.is_writable
        || accounts.market.is_signer
        || accounts.market.executable
        || accounts.claims_program.key != program_id
        || !accounts.claims_program.executable
        || accounts.claims_program.is_signer
        || accounts.claims_program.is_writable
        || !accounts.registry.executable
        || accounts.registry.is_signer
        || accounts.registry.is_writable
        || !accounts.caller_program.executable
        || accounts.caller_program.is_signer
        || accounts.caller_program.is_writable
        || !accounts.core_program.executable
        || accounts.core_program.is_signer
        || accounts.core_program.is_writable
        || accounts.rent.key != &sysvar::rent::ID
    {
        return Err(SignedDeltaSbfErrorV3::Accounts.into());
    }
    for account in [
        accounts.basis_record,
        accounts.basis_staging,
        accounts.product_record,
        accounts.product_staging,
        accounts.result_domain_record,
        accounts.result_domain_staging,
        accounts.portfolio_record,
        accounts.portfolio_staging,
        accounts.rent,
        accounts.core_market,
        accounts.cache,
        accounts.caller_programdata,
        accounts.claims_programdata,
        accounts.core_programdata,
    ] {
        if account.is_signer || account.is_writable || account.executable {
            return Err(SignedDeltaSbfErrorV3::Accounts.into());
        }
    }
    for (left, position) in accounts.positions.iter().enumerate() {
        if !position.is_writable
            || position.is_signer
            || position.executable
            || position.key == accounts.market.key
            || accounts
                .positions
                .iter()
                .skip(left.saturating_add(1))
                .any(|right| right.key == position.key)
        {
            return Err(SignedDeltaSbfErrorV3::Accounts.into());
        }
    }
    Ok(())
}

fn authenticate_authority(
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
    packet_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(plan.release_set()).map_err(|_| SignedDeltaSbfErrorV3::Release)?,
        plan.market(),
        execution_role(plan.caller_role()),
        plan.request_id(),
        packet_digest,
    )
    .map_err(|_| SignedDeltaSbfErrorV3::Release)?;
    if accounts.authority.key
        != &Pubkey::find_program_address(&seeds.as_slices(), accounts.caller_program.key).0
    {
        return Err(SignedDeltaSbfErrorV3::Release.into());
    }
    Ok(())
}

fn authenticate_releases(
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
) -> Result<(), ProgramError> {
    let caller = reauthenticate(
        accounts.registry,
        accounts.cache,
        execution_role(plan.caller_role()),
        accounts.caller_program,
        accounts.caller_programdata,
    )
    .map_err(|_| SignedDeltaSbfErrorV3::Release)?;
    let claims = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Claims,
        accounts.claims_program,
        accounts.claims_programdata,
    )
    .map_err(|_| SignedDeltaSbfErrorV3::Release)?;
    let core = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Core,
        accounts.core_program,
        accounts.core_programdata,
    )
    .map_err(|_| SignedDeltaSbfErrorV3::Release)?;
    for receipt in [caller, claims, core] {
        if receipt.execution_release_set_id().as_bytes() != &plan.release_set() {
            return Err(SignedDeltaSbfErrorV3::Release.into());
        }
    }
    Ok(())
}

fn authenticate_market(
    program_id: &Pubkey,
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    let expected_market = Pubkey::find_program_address(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            market.logical_market.as_slice(),
        ],
        program_id,
    )
    .0;
    if accounts.market.owner != program_id
        || accounts.market.key != &expected_market
        || market.logical_market != plan.market()
        || market.release_set != plan.release_set()
        || market.registry_program != accounts.registry.key.to_bytes()
        || market.product_instance_id == [0; 32]
        || market.basis_id != plan.semantic_basis_id()
        || market.claim_count != plan.claim_count()
        || market.revision != plan.expected_market_revision()
    {
        return Err(SignedDeltaSbfErrorV3::ClaimsState.into());
    }
    Ok(())
}

fn authenticate_product_and_basis(
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    Ok(authenticate_runtime_product_basis_core_v2(
        accounts.registry,
        accounts.rent,
        accounts.core_market,
        accounts.core_program,
        accounts.basis_record,
        accounts.basis_staging,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: accounts.product_record,
                staging: accounts.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: accounts.result_domain_record,
                staging: accounts.result_domain_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: accounts.portfolio_record,
                staging: accounts.portfolio_staging,
            },
        },
        market,
        plan.product_record_digest(),
        plan.linked_basis_record_digest(),
        dclutch_market_core_codec::Phase::Open,
    )
    .map_err(|_| SignedDeltaSbfErrorV3::ProductBasis)?)
}

fn build_candidates(
    program_id: &Pubkey,
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    plan: SignedDeltaPlanV3<'_>,
    market: MarketViewV2,
    market_before: &[u8],
) -> Result<(Vec<u8>, Vec<Vec<u8>>), ProgramError> {
    let mut candidates = Vec::with_capacity(accounts.positions.len());
    for (index, account) in accounts.positions.iter().enumerate() {
        let table_index = u32::try_from(index).map_err(|_| SignedDeltaSbfErrorV3::ClaimsState)?;
        let expected = plan
            .position(table_index)
            .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?;
        let seeds = ProtocolPositionSeedsV2::new(accounts.market.key.to_bytes(), expected.owner())
            .map_err(|_| SignedDeltaSbfErrorV3::ClaimsState)?;
        let expected_key = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
        let data = account
            .try_borrow_data()
            .map_err(|_| SignedDeltaSbfErrorV3::Accounts)?;
        let position =
            PositionViewV2::decode(&data).map_err(|_| SignedDeltaSbfErrorV3::ClaimsState)?;
        if account.owner != program_id
            || account.key != &expected_key
            || position.market_account != accounts.market.key.to_bytes()
            || position.owner != expected.owner()
            || position.basis_id != market.basis_id
            || position.claim_count != market.claim_count
            || position.revision != expected.expected_revision()
        {
            return Err(SignedDeltaSbfErrorV3::ClaimsState.into());
        }
        candidates.push(data.to_vec());
    }
    Ok((market_before.to_vec(), candidates))
}

fn apply_deltas(
    plan: SignedDeltaPlanV3<'_>,
    market: &mut [u8],
    positions: &mut [Vec<u8>],
) -> Result<(), ProgramError> {
    for outcome in 0..plan.claim_count() {
        apply_coordinate(
            market,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            outcome,
            plan.aggregate_delta(outcome)
                .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?,
        )?;
    }
    for index in 0..plan.position_delta_count() {
        let row = plan
            .position_delta(index)
            .map_err(|_| SignedDeltaSbfErrorV3::Instruction)?;
        let position_index =
            usize::try_from(row.position_index()).map_err(|_| SignedDeltaSbfErrorV3::Candidate)?;
        let candidate = positions
            .get_mut(position_index)
            .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
        apply_coordinate(
            candidate,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            row.outcome(),
            row.delta(),
        )?;
    }
    Ok(())
}

fn apply_coordinate(
    bytes: &mut [u8],
    header: usize,
    outcome: u32,
    delta: SignedDeltaV3,
) -> Result<(), ProgramError> {
    let offset = usize::try_from(outcome)
        .ok()
        .and_then(|outcome| outcome.checked_mul(SCALAR_BYTES))
        .and_then(|relative| header.checked_add(relative))
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
    let before = read_u64(bytes, offset)?;
    let after = match delta.direction() {
        DeltaDirectionV3::Neutral => Some(before),
        DeltaDirectionV3::Credit => before.checked_add(delta.magnitude()),
        DeltaDirectionV3::Debit => before.checked_sub(delta.magnitude()),
    }
    .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
    put_u64(bytes, offset, after)
}

fn resource_digest(market: &[u8], positions: &[Vec<u8>]) -> [u8; 32] {
    let mut resources: Vec<&[u8]> = Vec::with_capacity(positions.len().saturating_add(2));
    resources.push(RESOURCE_DIGEST_DOMAIN_V3);
    resources.push(market);
    for position in positions {
        resources.push(position);
    }
    hashv(&resources).to_bytes()
}

fn commit_candidates(
    accounts: &SignedDeltaAccountsV3<'_, '_>,
    market_candidate: &[u8],
    position_candidates: &[Vec<u8>],
) -> Result<(), ProgramError> {
    let mut market = accounts
        .market
        .try_borrow_mut_data()
        .map_err(|_| SignedDeltaSbfErrorV3::Commit)?;
    if market.len() != market_candidate.len()
        || position_candidates.len() != accounts.positions.len()
    {
        return Err(SignedDeltaSbfErrorV3::Commit.into());
    }
    let mut positions: Vec<RefMut<'_, &mut [u8]>> = Vec::with_capacity(accounts.positions.len());
    for (account, candidate) in accounts.positions.iter().zip(position_candidates) {
        let data = account
            .try_borrow_mut_data()
            .map_err(|_| SignedDeltaSbfErrorV3::Commit)?;
        if data.len() != candidate.len() {
            return Err(SignedDeltaSbfErrorV3::Commit.into());
        }
        positions.push(data);
    }
    market.copy_from_slice(market_candidate);
    for (mut position, candidate) in positions.into_iter().zip(position_candidates) {
        position.copy_from_slice(candidate);
    }
    Ok(())
}

const fn execution_role(role: CallerRole) -> ExecutionRoleV1 {
    match role {
        CallerRole::Core => ExecutionRoleV1::Core,
        CallerRole::Trading => ExecutionRoleV1::Trading,
    }
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| SignedDeltaSbfErrorV3::Accounts.into())
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ProgramError> {
    let end = offset
        .checked_add(SCALAR_BYTES)
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
    let field: [u8; SCALAR_BYTES] = bytes
        .get(offset..end)
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?
        .try_into()
        .map_err(|_| SignedDeltaSbfErrorV3::Candidate)?;
    Ok(u64::from_le_bytes(field))
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), ProgramError> {
    let end = offset
        .checked_add(SCALAR_BYTES)
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?;
    bytes
        .get_mut(offset..end)
        .ok_or(SignedDeltaSbfErrorV3::Candidate)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use dclutch_claims_svm::signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SIGNED_DELTA_PLAN_MAGIC_V3,
        SignedDeltaPlanInputV3, SignedDeltaPositionV3, SignedDeltaV3, plan_bytes,
    };

    fn delta(direction: DeltaDirectionV3, magnitude: u64) -> SignedDeltaV3 {
        SignedDeltaV3::new(direction, magnitude).expect("delta")
    }

    fn plan_fixture() -> Vec<u8> {
        let positions = [
            SignedDeltaPositionV3::new([7; 32], 4).expect("a"),
            SignedDeltaPositionV3::new([8; 32], 9).expect("b"),
        ];
        let rows = [
            PositionDeltaV3::new(
                PositionDeltaInputV3 {
                    position_index: 0,
                    outcome: 1,
                    delta: delta(DeltaDirectionV3::Debit, u64::MAX),
                },
                2,
                2,
            )
            .expect("debit"),
            PositionDeltaV3::new(
                PositionDeltaInputV3 {
                    position_index: 1,
                    outcome: 1,
                    delta: delta(DeltaDirectionV3::Credit, u64::MAX),
                },
                2,
                2,
            )
            .expect("credit"),
        ];
        let aggregates = [
            delta(DeltaDirectionV3::Neutral, 0),
            delta(DeltaDirectionV3::Neutral, 0),
        ];
        let mut bytes = vec![0; plan_bytes(2, 2, 2).expect("width")];
        SignedDeltaPlanV3::encode_into(
            SignedDeltaPlanInputV3 {
                caller_role: CallerRole::Trading,
                release_set: [1; 32],
                market: [2; 32],
                request_id: [3; 32],
                product_record_digest: [4; 32],
                semantic_basis_id: [5; 32],
                linked_basis_record_digest: [6; 32],
                expected_market_revision: 3,
                claim_count: 2,
            },
            &positions,
            &aggregates,
            &rows,
            &mut bytes,
        )
        .expect("encode");
        bytes
    }

    #[test]
    fn candidate_application_is_atomic_and_full_range() {
        let bytes = plan_fixture();
        let plan = SignedDeltaPlanV3::decode(&bytes).expect("plan");
        let mut market = vec![0; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 16];
        let mut positions = vec![
            vec![0; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 16],
            vec![0; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 16],
        ];
        put_u64(
            positions.get_mut(0).expect("a"),
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8,
            u64::MAX,
        )
        .expect("balance");
        apply_deltas(plan, &mut market, &mut positions).expect("transfer");
        assert_eq!(
            read_u64(
                positions.first().expect("a"),
                LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8
            ),
            Ok(0)
        );
        assert_eq!(
            read_u64(
                positions.get(1).expect("b"),
                LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8
            ),
            Ok(u64::MAX)
        );
        let before = positions.clone();
        assert_eq!(
            apply_deltas(plan, &mut market, &mut positions),
            Err(SignedDeltaSbfErrorV3::Candidate.into())
        );
        assert_eq!(
            positions, before,
            "refusal does not partially mutate another coordinate"
        );
    }

    #[test]
    fn dispatch_magic_is_exact() {
        let bytes = plan_fixture();
        assert_eq!(
            bytes.get(..SIGNED_DELTA_PLAN_MAGIC_V3.len()),
            Some(SIGNED_DELTA_PLAN_MAGIC_V3.as_slice())
        );
    }
}
