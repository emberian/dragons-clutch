//! Core-authorized market-wide aggregate-empty closure.
//!
//! This adapter is the sole physical producer of
//! [`ClaimsMarketClosureReceiptV1`]. It authenticates the selected Core caller,
//! proves every runtime-width aggregate supply is zero, credits all aggregate
//! lamports to the immutable RentCredit, and emits the receipt only after the
//! aggregate is closed.

use dclutch_claims::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_SEED_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
    },
    market_closure_v1::{
        CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1,
        CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1, ClaimsMarketClosureReceiptInputV1,
        ClaimsMarketClosureReceiptV1, ClaimsMarketClosureRequestV1,
    },
    retirement_checkpoint_handoff_v1::{
        CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_POST_DIGEST_DOMAIN_V1,
        CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_BYTES_V1,
        ClaimsRetirementCheckpointHandoffReceiptV1, ClaimsRetirementCheckpointHandoffRequestV1,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_market::rent::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_market::{
    AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1, CoreState, MarketCoreStateSeedsV2, STATE_BYTES,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_registry::svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
    RegistryContinuationRequestV1,
};
use dclutch_registry::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    program::set_return_data,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use dclutch_claims::protocol_position_v2::{
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionSeedsV2,
};
use dclutch_product::payoff::runtime_v3::{ProductBasisV3, semantic_basis_id_v3};

use super::{ClaimsSbfError, FailureEscrowIdentityV1, authenticate_activated_role};
use crate::liability_basis_v2::read_vector;
use crate::market_admission_v1::CLAIMS_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1;
use crate::signed_delta_v3::{ClosureBurnRefusalV3, burn_failure_coordinate_v1};

/// Core caller PDA signer.
pub const AUTHORITY_ACCOUNT_V1: usize = 0;
/// Writable canonical LiabilityBasisV2 aggregate.
pub const AGGREGATE_ACCOUNT_V1: usize = 1;
/// Writable immutable RentCredit beneficiary.
pub const RENT_CREDIT_ACCOUNT_V1: usize = 2;
/// Registry activation cache.
pub const ACTIVATION_CACHE_ACCOUNT_V1: usize = 3;
/// Immutable Market-selected Registry program.
pub const REGISTRY_PROGRAM_ACCOUNT_V1: usize = 4;
/// Current Claims program.
pub const CLAIMS_PROGRAM_ACCOUNT_V1: usize = 5;
/// Current Claims ProgramData.
pub const CLAIMS_PROGRAMDATA_ACCOUNT_V1: usize = 6;
/// Current Core program.
pub const CORE_PROGRAM_ACCOUNT_V1: usize = 7;
/// Current Core ProgramData.
pub const CORE_PROGRAMDATA_ACCOUNT_V1: usize = 8;
/// Canonical Retiring Core Market.
pub const CORE_MARKET_ACCOUNT_V1: usize = 9;
/// Infrastructure-selected Rent program owning RentCredit.
pub const RENT_PROGRAM_ACCOUNT_V1: usize = 10;
/// Exact Claims market-closure frame width.
pub const MARKET_CLOSURE_ACCOUNT_COUNT_V1: usize = 11;
/// Exact continuation-authorized Claims market-closure frame width.
pub const MARKET_CLOSURE_CONTINUATION_ACCOUNT_COUNT_V1: usize = 12;
/// Accounts a refunding Market's closure burn adds after the fixed frame.
///
/// TRAILING, and that is the whole of why a categorical Market's closure keeps
/// its exact current eleven (or twelve) accounts and its exact current
/// behaviour: a frame that does not carry these is byte-for-byte the frame that
/// shipped, and the burn is unreachable from it.
pub const MARKET_CLOSURE_ESCROW_ACCOUNT_COUNT_V1: usize = 3;
/// Exact Claims market-closure frame width with the failure-escrow burn.
pub const MARKET_CLOSURE_BURN_ACCOUNT_COUNT_V1: usize =
    MARKET_CLOSURE_ACCOUNT_COUNT_V1 + MARKET_CLOSURE_ESCROW_ACCOUNT_COUNT_V1;
/// Exact continuation-authorized Claims market-closure frame width with the burn.
pub const MARKET_CLOSURE_BURN_CONTINUATION_ACCOUNT_COUNT_V1: usize =
    MARKET_CLOSURE_CONTINUATION_ACCOUNT_COUNT_V1 + MARKET_CLOSURE_ESCROW_ACCOUNT_COUNT_V1;

/// Stable physical closure refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimsMarketClosureSbfErrorV1 {
    /// The fixed account frame or privileges refused.
    Accounts = 0x5500,
    /// Caller PDA or current Registry releases refused.
    Authority = 0x5501,
    /// Core/aggregate/RentCredit identities or revisions refused.
    Identity = 0x5502,
    /// A nonzero aggregate supply prevented closure.
    Liability = 0x5503,
    /// Checked refund accounting or commit-last closure refused.
    Commit = 0x5504,
    /// Typed receipt construction refused.
    Receipt = 0x5505,
    /// The named linked basis record was not this Market's, or its Market does
    /// not refund on failure.
    ///
    /// Split from [`Self::Identity`] because it sends its reader somewhere
    /// else entirely. `Identity` is about the Core Market, the aggregate and
    /// the RentCredit -- accounts a retirement operator derives. This one is
    /// about the Product graph's linked basis record, whose identity is
    /// content-addressed by the aggregate's own `basis_id`, and it is what a
    /// substituted record raises. A categorical Market that presents the
    /// escrow tail at all raises it too: the burn is licensed by the RECORD
    /// saying this column is owed nothing under every certificate, and a
    /// record that does not say so licenses nothing.
    Basis = 0x5506,
}

dclutch_refusal_registry::pin_refusal_band!(
    ClaimsMarketClosureSbfErrorV1,
    dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x500,
    [
        Accounts, Authority, Identity, Liability, Commit, Receipt, Basis
    ]
);

#[derive(Clone, Copy)]
struct ClosureAccounts<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
    registry_admission: Option<&'accounts AccountInfo<'info>>,
    escrow: Option<ClosureEscrowV1<'accounts, 'info>>,
}

/// The three accounts a refunding Market's closure burn adds, and nothing else.
///
/// Decision 0025's fourth addendum fixes them at three: the escrow's Position,
/// its protocol-Position admission, and the linked basis record. No Hoard --
/// outstanding collateral is a Custody token account, so "this column releases
/// no collateral" cannot be read off the aggregate and is proved instead by the
/// record's `refunds_on_failure` here plus retirement's own `CloseVault`.
#[derive(Clone, Copy)]
struct ClosureEscrowV1<'accounts, 'info> {
    /// Writable `LiabilityBasisV2` Position holding the failure column.
    position: &'accounts AccountInfo<'info>,
    /// Writable protocol-Position admission under the same derived owner.
    admission: &'accounts AccountInfo<'info>,
    /// Immutable `ProductBasisV3` record this Market was founded on.
    basis_record: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> ClosureAccounts<'accounts, 'info> {
    fn parse(
        accounts: &'accounts [AccountInfo<'info>],
        continuation: bool,
    ) -> Result<Self, ProgramError> {
        let base = accounts
            .get(..MARKET_CLOSURE_ACCOUNT_COUNT_V1)
            .ok_or(ClaimsMarketClosureSbfErrorV1::Accounts)?;
        let tail = accounts
            .get(MARKET_CLOSURE_ACCOUNT_COUNT_V1..)
            .ok_or(ClaimsMarketClosureSbfErrorV1::Accounts)?;
        let (registry_admission, tail) = if continuation {
            let (admission, rest) = tail
                .split_first()
                .ok_or(ClaimsMarketClosureSbfErrorV1::Accounts)?;
            (Some(admission), rest)
        } else {
            (None, tail)
        };
        let escrow = match tail {
            [] => None,
            [position, admission, basis_record] => Some(ClosureEscrowV1 {
                position,
                admission,
                basis_record,
            }),
            _ => return Err(ClaimsMarketClosureSbfErrorV1::Accounts.into()),
        };
        let [
            authority,
            aggregate,
            rent_credit,
            cache,
            registry,
            claims_program,
            claims_programdata,
            core_program,
            core_programdata,
            core_market,
            rent_program,
        ] = base
        else {
            return Err(ClaimsMarketClosureSbfErrorV1::Accounts.into());
        };
        Ok(Self {
            authority,
            aggregate,
            rent_credit,
            cache,
            registry,
            claims_program,
            claims_programdata,
            core_program,
            core_programdata,
            core_market,
            rent_program,
            registry_admission,
            escrow,
        })
    }

    /// Every account this frame carries, in frame order, for the alias sweep.
    fn frame(
        self,
    ) -> (
        [&'accounts AccountInfo<'info>; MARKET_CLOSURE_BURN_CONTINUATION_ACCOUNT_COUNT_V1],
        usize,
    ) {
        let mut keys = [self.authority; MARKET_CLOSURE_BURN_CONTINUATION_ACCOUNT_COUNT_V1];
        let base = [
            self.authority,
            self.aggregate,
            self.rent_credit,
            self.cache,
            self.registry,
            self.claims_program,
            self.claims_programdata,
            self.core_program,
            self.core_programdata,
            self.core_market,
            self.rent_program,
        ];
        let mut count = 0;
        for account in base {
            if let Some(slot) = keys.get_mut(count) {
                *slot = account;
            }
            count = count.saturating_add(1);
        }
        for account in self.registry_admission.into_iter().chain(
            self.escrow
                .into_iter()
                .flat_map(|escrow| [escrow.position, escrow.admission, escrow.basis_record]),
        ) {
            if let Some(slot) = keys.get_mut(count) {
                *slot = account;
            }
            count = count.saturating_add(1);
        }
        (keys, count)
    }
}

fn split_continuation(
    instruction_data: &[u8],
) -> Result<(&[u8], Option<RegistryContinuationRequestV1>), ProgramError> {
    if instruction_data.len()
        == dclutch_claims::market_closure_v1::CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1
    {
        return Ok((instruction_data, None));
    }
    let expected = dclutch_claims::market_closure_v1::CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1
        .checked_add(REGISTRY_CONTINUATION_REQUEST_BYTES_V1)
        .ok_or(ClaimsMarketClosureSbfErrorV1::Accounts)?;
    if instruction_data.len() != expected {
        return Err(ClaimsMarketClosureSbfErrorV1::Accounts.into());
    }
    let (request, continuation) = instruction_data
        .split_at(dclutch_claims::market_closure_v1::CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1);
    let continuation = RegistryContinuationRequestV1::decode(continuation)
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Authority)?;
    Ok((request, Some(continuation)))
}

/// Close one exact empty Claims aggregate and return its typed receipt.
#[inline(never)]
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let (request_bytes, continuation) = split_continuation(instruction_data)?;
    let request = ClaimsMarketClosureRequestV1::decode(request_bytes)
        .map_err(|_| ClaimsSbfError::Instruction)?;
    let request_input = request.input();
    let request_digest = hash(request_bytes).to_bytes();
    let accounts = ClosureAccounts::parse(accounts, continuation.is_some())?;
    authenticate_privileges(program_id, accounts)?;
    authenticate_authority(accounts, request_input, request_digest)?;
    authenticate_releases(accounts, request_input.release_set, continuation)?;
    let core = authenticate_core(accounts, request_input)?;
    authenticate_rent_credit(accounts, core)?;
    let (pre_digest, market) = authenticate_aggregate_identity(accounts, request_input)?;
    burn_failure_escrow_column_v1(program_id, accounts, market)?;
    require_empty_aggregate(accounts, market)?;
    let refund_lamports = aggregate_refund_lamports(accounts)?;
    let rent_after = accounts
        .rent_credit
        .lamports()
        .checked_add(refund_lamports)
        .ok_or(ClaimsMarketClosureSbfErrorV1::Commit)?;
    close_aggregate(accounts, rent_after)?;
    let post_digest = hashv(&[
        CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1.as_slice(),
        accounts.aggregate.key.as_ref(),
        accounts.rent_credit.key.as_ref(),
        request_input.resulting_revision.to_le_bytes().as_slice(),
        refund_lamports.to_le_bytes().as_slice(),
        rent_after.to_le_bytes().as_slice(),
    ])
    .to_bytes();
    let receipt = ClaimsMarketClosureReceiptV1::new(ClaimsMarketClosureReceiptInputV1 {
        producer: program_id.to_bytes(),
        release_set: request_input.release_set,
        market: request_input.market,
        aggregate: request_input.aggregate,
        rent_credit: request_input.rent_credit,
        request_digest,
        pre_resource_digest: pre_digest,
        post_resource_digest: post_digest,
        generation: request_input.generation,
        pre_revision: request_input.expected_revision,
        post_revision: request_input.resulting_revision,
        liability_units: 0,
        refund_lamports,
        claim_count: request_input.claim_count,
    })
    .map_err(|_| ClaimsMarketClosureSbfErrorV1::Receipt)?;
    set_return_data(&receipt.to_bytes());
    Ok(())
}

/// Prove zero liabilities, retain every aggregate lamport, and hand the exact
/// aggregate PDA to Core as its durable retirement checkpoint.
#[inline(never)]
pub fn process_checkpoint_handoff(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let (request_bytes, continuation) = split_continuation(instruction_data)?;
    if request_bytes.len() != CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_BYTES_V1 {
        return Err(ClaimsSbfError::Instruction.into());
    }
    let request = ClaimsRetirementCheckpointHandoffRequestV1::decode(request_bytes)
        .map_err(|_| ClaimsSbfError::Instruction)?;
    let request_input = request.input();
    let request_digest = hash(request_bytes).to_bytes();
    let accounts = ClosureAccounts::parse(accounts, continuation.is_some())?;
    authenticate_privileges(program_id, accounts)?;
    authenticate_authority(accounts, request_input, request_digest)?;
    authenticate_releases(accounts, request_input.release_set, continuation)?;
    let core = authenticate_core(accounts, request_input)?;
    authenticate_rent_credit(accounts, core)?;
    let (pre_digest, market) = authenticate_aggregate_identity(accounts, request_input)?;
    burn_failure_escrow_column_v1(program_id, accounts, market)?;
    require_empty_aggregate(accounts, market)?;
    let refund_lamports = aggregate_refund_lamports(accounts)?;
    let rent_before = accounts.rent_credit.lamports();
    handoff_aggregate_to_core(accounts)?;
    let checkpoint_width = AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1.to_le_bytes();
    let refund = refund_lamports.to_le_bytes();
    let rent = rent_before.to_le_bytes();
    let post_digest = hashv(&[
        CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_POST_DIGEST_DOMAIN_V1,
        accounts.aggregate.key.as_ref(),
        accounts.core_program.key.as_ref(),
        checkpoint_width.as_slice(),
        refund.as_slice(),
        rent.as_slice(),
    ])
    .to_bytes();
    let receipt =
        ClaimsRetirementCheckpointHandoffReceiptV1::new(ClaimsMarketClosureReceiptInputV1 {
            producer: program_id.to_bytes(),
            release_set: request_input.release_set,
            market: request_input.market,
            aggregate: request_input.aggregate,
            rent_credit: request_input.rent_credit,
            request_digest,
            pre_resource_digest: pre_digest,
            post_resource_digest: post_digest,
            generation: request_input.generation,
            pre_revision: request_input.expected_revision,
            post_revision: request_input.resulting_revision,
            liability_units: 0,
            refund_lamports,
            claim_count: request_input.claim_count,
        })
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Receipt)?;
    set_return_data(&receipt.to_bytes());
    Ok(())
}

#[inline(never)]
fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: ClosureAccounts<'_, '_>,
) -> ProgramResult {
    if accounts.authority.is_writable
        || !accounts.authority.is_signer
        || accounts.authority.executable
        || !accounts.aggregate.is_writable
        || accounts.aggregate.is_signer
        || accounts.aggregate.executable
        || !accounts.rent_credit.is_writable
        || accounts.rent_credit.is_signer
        || accounts.rent_credit.executable
        || accounts.cache.is_writable
        || accounts.cache.is_signer
        || accounts.cache.executable
        || accounts.registry.is_writable
        || accounts.registry.is_signer
        || !accounts.registry.executable
        || accounts.claims_program.key != program_id
        || accounts.claims_program.is_writable
        || accounts.claims_program.is_signer
        || !accounts.claims_program.executable
        || accounts.claims_programdata.is_writable
        || accounts.claims_programdata.is_signer
        || accounts.claims_programdata.executable
        || accounts.core_program.is_writable
        || accounts.core_program.is_signer
        || !accounts.core_program.executable
        || accounts.core_programdata.is_writable
        || accounts.core_programdata.is_signer
        || accounts.core_programdata.executable
        || accounts.core_market.is_writable
        || accounts.core_market.is_signer
        || accounts.core_market.executable
        || accounts.rent_program.is_writable
        || accounts.rent_program.is_signer
        || !accounts.rent_program.executable
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Accounts.into());
    }
    if let Some(escrow) = accounts.escrow
        && (!escrow.position.is_writable
            || escrow.position.is_signer
            || escrow.position.executable
            || !escrow.admission.is_writable
            || escrow.admission.is_signer
            || escrow.admission.executable
            || escrow.basis_record.is_writable
            || escrow.basis_record.is_signer
            || escrow.basis_record.executable)
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Accounts.into());
    }
    if let Some(admission) = accounts.registry_admission
        && (!admission.is_signer
            || admission.is_writable
            || admission.executable
            || admission.owner != &system_program::ID
            || !admission.data_is_empty()
            || admission.lamports() != 0)
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Accounts.into());
    }
    // One sweep over the WHOLE frame, whatever its width. The eleven-account
    // spelling that used to sit here was duplicated once for the continuation
    // admission, and a third copy for the escrow tail would have been the
    // moment an account escaped the alias rule by being added to one list.
    let (frame, count) = accounts.frame();
    require_distinct(
        frame
            .get(..count)
            .ok_or(ClaimsMarketClosureSbfErrorV1::Accounts)?,
    )
}

#[inline(never)]
fn authenticate_authority(
    accounts: ClosureAccounts<'_, '_>,
    request: dclutch_claims::market_closure_v1::ClaimsMarketClosureRequestInputV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Core,
        request.parent_request_digest,
        request_digest,
    )
    .map_err(|_| ClaimsMarketClosureSbfErrorV1::Authority)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), accounts.core_program.key).0;
    if expected != *accounts.authority.key {
        return Err(ClaimsMarketClosureSbfErrorV1::Authority.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_releases(
    accounts: ClosureAccounts<'_, '_>,
    release_set: [u8; 32],
    continuation: Option<RegistryContinuationRequestV1>,
) -> ProgramResult {
    if let Some(continuation) = continuation {
        return authenticate_continuation_releases(accounts, release_set, continuation);
    }
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Claims,
            accounts.claims_program,
            accounts.claims_programdata,
        ),
        (
            ExecutionRoleV1::Core,
            accounts.core_program,
            accounts.core_programdata,
        ),
    ] {
        let receipt = authenticate_activated_role(
            accounts.registry,
            accounts.cache,
            role,
            program,
            programdata,
            &release_set,
        )?;
        if receipt.execution_release_set_id().as_bytes() != &release_set {
            return Err(ClaimsMarketClosureSbfErrorV1::Authority.into());
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_continuation_releases(
    accounts: ClosureAccounts<'_, '_>,
    release_set: [u8; 32],
    continuation: RegistryContinuationRequestV1,
) -> ProgramResult {
    let admission = accounts
        .registry_admission
        .ok_or(ClaimsMarketClosureSbfErrorV1::Authority)?;
    let expected_roles = [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ];
    if continuation.release_set_id().to_bytes() != release_set
        || continuation.continuation_role() != ExecutionRoleV1::Core
        || usize::from(continuation.role_count()) != expected_roles.len()
        || expected_roles
            .iter()
            .enumerate()
            .any(|(index, role)| continuation.role(index) != Some(*role))
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Authority.into());
    }
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_slice()],
        accounts.registry.key,
    )
    .0;
    if expected_cache != *accounts.cache.key || accounts.cache.owner != accounts.registry.key {
        return Err(ClaimsMarketClosureSbfErrorV1::Authority.into());
    }
    let cache_bytes = accounts
        .cache
        .try_borrow_data()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
    if hash(&cache_bytes).to_bytes() != continuation.activation_cache_digest().to_bytes() {
        return Err(ClaimsMarketClosureSbfErrorV1::Authority.into());
    }
    let cache = ActivatedExecutionReleaseSetViewV1::decode(&cache_bytes)
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Authority)?;
    if cache
        .execution_release_set_id()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Authority)?
        .to_bytes()
        != release_set
        || !selected_program_matches(
            cache,
            ExecutionRoleV1::Claims,
            accounts.claims_program,
            accounts.claims_programdata,
        )?
        || !selected_program_matches(
            cache,
            ExecutionRoleV1::Core,
            accounts.core_program,
            accounts.core_programdata,
        )?
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Authority.into());
    }
    drop(cache_bytes);
    let batch = continuation
        .role_batch_request()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Authority)?;
    let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes())
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Authority)?;
    let seeds = RegistryContinuationAdmissionSeedsV1::new(
        continuation,
        accounts.cache.key.to_bytes(),
        batch_digest,
    )
    .map_err(|_| ClaimsMarketClosureSbfErrorV1::Authority)?;
    let release = seeds.release_set();
    let cache_key = seeds.activation_cache();
    let request_digest = seeds.batch_request_digest();
    let mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let continuation_digest = seeds.continuation_digest();
    let expected = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache_key.as_slice(),
            request_digest.as_slice(),
            mask.as_slice(),
            role.as_slice(),
            continuation_digest.as_slice(),
        ],
        accounts.registry.key,
    )
    .0;
    if expected != *admission.key {
        return Err(ClaimsMarketClosureSbfErrorV1::Authority.into());
    }
    Ok(())
}

fn selected_program_matches(
    cache: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
) -> Result<bool, ProgramError> {
    let release = cache
        .role(role)
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Authority)?
        .release();
    Ok(release.program().to_bytes() == program.key.to_bytes()
        && release.programdata() == programdata.key.to_bytes())
}

#[inline(never)]
fn authenticate_core(
    accounts: ClosureAccounts<'_, '_>,
    request: dclutch_claims::market_closure_v1::ClaimsMarketClosureRequestInputV1,
) -> Result<CoreState, ProgramError> {
    if request.core_program != accounts.core_program.key.to_bytes()
        || accounts.core_market.owner != accounts.core_program.key
        || accounts.core_market.key.to_bytes() != request.market
        || accounts.core_market.data_len() != STATE_BYTES
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    let core_bytes = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
    let core =
        CoreState::decode(&core_bytes).map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        accounts.core_program.key,
    )
    .0;
    if expected != *accounts.core_market.key
        || !CLAIMS_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(core.phase)
        || core.identity.market_id.to_bytes() != request.market
        || core.identity.selected_release_set.to_bytes() != request.release_set
        || core.identity.registry_program.to_bytes() != accounts.registry.key.to_bytes()
        || core.identity.generation != request.generation
        || core.outstanding_capabilities != 0
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    Ok(core)
}

#[inline(never)]
fn authenticate_rent_credit(accounts: ClosureAccounts<'_, '_>, core: CoreState) -> ProgramResult {
    if accounts.rent_credit.owner != accounts.rent_program.key {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    let data = accounts
        .rent_credit
        .try_borrow_data()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
    let credit = LifecycleRentCreditV2::decode(&data)
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?;
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market.as_slice(),
            generation.as_slice(),
            &bump,
        ],
        accounts.rent_program.key,
    )
    .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?;
    if expected != *accounts.rent_credit.key
        || accounts.rent_credit.key.to_bytes() != core.rent_beneficiary.to_bytes()
        || credit.market().to_bytes() != core.identity.market_id.to_bytes()
        || credit.release_set().to_bytes() != core.identity.selected_release_set.to_bytes()
        || credit.generation() != core.identity.generation
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_aggregate_identity(
    accounts: ClosureAccounts<'_, '_>,
    request: dclutch_claims::market_closure_v1::ClaimsMarketClosureRequestInputV1,
) -> Result<([u8; 32], LiabilityBasisMarketViewV2), ProgramError> {
    if accounts.aggregate.owner != accounts.claims_program.key
        || accounts.aggregate.key.to_bytes() != request.aggregate
        || accounts.rent_credit.key.to_bytes() != request.rent_credit
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    let expected = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, request.market.as_slice()],
        accounts.claims_program.key,
    )
    .0;
    if expected != *accounts.aggregate.key {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    let bytes = accounts
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
    let market = LiabilityBasisMarketViewV2::decode(&bytes)
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?;
    if market.logical_market != request.market
        || market.release_set != request.release_set
        || market.registry_program != accounts.registry.key.to_bytes()
        || market.generation != request.generation
        || market.claim_count != request.claim_count
        || market.revision != request.expected_revision
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    // The PRESTATE digest, and it is taken here rather than after the burn on
    // purpose: it is what a retirement operator projected off the aggregate it
    // OBSERVED, before this instruction moved anything.
    let pre_digest = hashv(&[
        CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1.as_slice(),
        accounts.aggregate.key.as_ref(),
        bytes.as_ref(),
    ])
    .to_bytes();
    Ok((pre_digest, market))
}

/// Prove every runtime-width aggregate supply is zero.
///
/// Runs AFTER the burn, so on a refunding Market it is the burn's own
/// postcondition -- `the_closure_burn_admits_the_retirement_it_foreclosed`, by
/// the same predicate with nothing relaxed in it. On every other Market it is
/// the conjunct that has always been here, reaching the same
/// [`ClaimsMarketClosureSbfErrorV1::Liability`] for the same reason.
#[inline(never)]
fn require_empty_aggregate(
    accounts: ClosureAccounts<'_, '_>,
    market: LiabilityBasisMarketViewV2,
) -> ProgramResult {
    let bytes = accounts
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
    let mut claim_index = 0;
    while claim_index < market.claim_count {
        if market
            .supply(&bytes, claim_index)
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?
            != 0
        {
            return Err(ClaimsMarketClosureSbfErrorV1::Liability.into());
        }
        claim_index = claim_index
            .checked_add(1)
            .ok_or(ClaimsMarketClosureSbfErrorV1::Liability)?;
    }
    Ok(())
}

/// Everything the closure refunds, read after the escrow pair has surrendered
/// its rent to the aggregate.
///
/// One number, and deliberately so: whichever disposition follows -- credit to
/// the RentCredit ([`close_aggregate`]) or carry into Core's checkpoint
/// ([`handoff_aggregate_to_core`]) -- moves the escrow's rent along the exact
/// path the aggregate's own rent already took, and decision 0021's refund
/// source is reached without this route learning a second one.
fn aggregate_refund_lamports(accounts: ClosureAccounts<'_, '_>) -> Result<u64, ProgramError> {
    let refund_lamports = accounts.aggregate.lamports();
    if refund_lamports == 0 {
        return Err(ClaimsMarketClosureSbfErrorV1::Identity.into());
    }
    Ok(refund_lamports)
}

/// Whether the Market this aggregate belongs to refunds on failure, proved
/// against the RECORD and against the aggregate's own `basis_id`.
///
/// One account, no Product-graph walk. `semantic_basis_id_v3` is the content
/// address a founding committed to when it wrote `basis_id`, and the preimage
/// it hashes carries the kind, the width and the payout scale -- exactly the
/// triple `categorical_refunds_on_failure_v3` reads. So a record that
/// reproduces this Market's `basis_id` cannot disagree with the Market about
/// whether it refunds, and a substituted record cannot reproduce it.
fn refunds_on_failure_v1(
    basis_record: &AccountInfo<'_>,
    market: LiabilityBasisMarketViewV2,
) -> Result<bool, ProgramError> {
    let bytes = basis_record
        .try_borrow_data()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
    let semantic =
        semantic_basis_id_v3(&bytes).map_err(|_| ClaimsMarketClosureSbfErrorV1::Basis)?;
    let basis = ProductBasisV3::decode(&bytes).map_err(|_| ClaimsMarketClosureSbfErrorV1::Basis)?;
    if semantic != market.basis_id || basis.basis_width() != market.claim_count {
        return Err(ClaimsMarketClosureSbfErrorV1::Basis.into());
    }
    Ok(basis.refunds_on_failure())
}

/// Discharge a refunding Market's failure column and close the escrow that
/// held it, in the same act that closes the aggregate.
///
/// # Why here and nowhere else
///
/// The column is unpayable under every certificate and its holder is a
/// program-derived address with no key, so no party can retire it and a Market
/// carrying it can never close (decision 0025's third addendum; the on-chain
/// witness is `a_seated_failure_column_refuses_the_claims_handoff_by_name`).
/// The burn's authority is the Core caller PDA this route already
/// authenticates -- shape A buys the outcome by adding no keyless-signer
/// exemption at all.
///
/// And the ORDER is forced rather than preferred: `protocol_position_v2`'s
/// close refuses unless the aggregate is still Claims-owned and decodes as
/// `LiabilityBasisV2`, and retirement's first checkpoint reassigns the
/// aggregate to Core in the same instruction this runs inside. There is no
/// later transaction in which the escrow could be closed, so it is closed
/// here.
///
/// # What it proves before it writes
///
/// The record refunds on failure; the escrow is the derived PDA at
/// `(market, failure selector)` and its admission is the derived admission
/// under the same owner; the escrow holds the failure column and NOTHING else;
/// and the aggregate's supply at that coordinate is exactly what the escrow
/// holds. That last one is the residue rule as an equality rather than as a
/// licence to ignore an index: a Market whose failure column is not WHOLLY in
/// the escrow refuses [`ClaimsMarketClosureSbfErrorV1::Liability`], because
/// part of it is in hands that can be paid.
#[inline(never)]
fn burn_failure_escrow_column_v1(
    program_id: &Pubkey,
    accounts: ClosureAccounts<'_, '_>,
    market: LiabilityBasisMarketViewV2,
) -> ProgramResult {
    let Some(escrow) = accounts.escrow else {
        return Ok(());
    };
    if !refunds_on_failure_v1(escrow.basis_record, market)? {
        return Err(ClaimsMarketClosureSbfErrorV1::Basis.into());
    }
    let derived =
        FailureEscrowIdentityV1::derive(program_id, market.logical_market, market.claim_count)
            .map_err(|_| ClaimsSbfError::FailureEscrow)?;
    let aggregate = accounts.aggregate.key.to_bytes();
    let position_seeds = ProtocolPositionSeedsV2::new(aggregate, derived.owner)
        .map_err(|_| ClaimsSbfError::FailureEscrow)?;
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(aggregate, derived.owner)
        .map_err(|_| ClaimsSbfError::FailureEscrow)?;
    if Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0
        != *escrow.position.key
        || Pubkey::find_program_address(&admission_seeds.as_slices(), program_id).0
            != *escrow.admission.key
        || escrow.position.owner != program_id
        || escrow.admission.owner != program_id
        || escrow.admission.data_is_empty()
    {
        return Err(ClaimsSbfError::FailureEscrow.into());
    }
    let residue = {
        let position_bytes = escrow
            .position
            .try_borrow_data()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
        let position = LiabilityBasisPositionViewV2::decode(&position_bytes)
            .map_err(|_| ClaimsSbfError::FailureEscrow)?;
        if position.market_account != aggregate
            || position.owner != derived.owner
            || position.basis_id != market.basis_id
            || position.claim_count != market.claim_count
        {
            return Err(ClaimsSbfError::FailureEscrow.into());
        }
        let balances = read_vector(
            &position_bytes,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            position.claim_count,
        )
        .map_err(|_| ClaimsSbfError::FailureEscrow)?;
        let mut residue = 0;
        for (index, balance) in balances.iter().enumerate() {
            let ordinary = u32::try_from(index)
                .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?
                != derived.failure_selector;
            if ordinary && *balance != 0 {
                // The escrow is holding a TRADEABLE claim. Its holder can be
                // paid, so this is an outstanding liability and not a residue,
                // and it is the same accusation a stranger holding part of the
                // failure column gets.
                return Err(ClaimsMarketClosureSbfErrorV1::Liability.into());
            }
            if !ordinary {
                residue = *balance;
            }
        }
        residue
    };
    let supply = {
        let aggregate_bytes = accounts
            .aggregate
            .try_borrow_data()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Accounts)?;
        market
            .supply(&aggregate_bytes, derived.failure_selector)
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)?
    };
    if supply == 0 {
        // Nothing is seated. `FailureEscrowUnseated` is what the complete-set
        // gate raises for exactly this shape and it means the same thing here:
        // the right account was named and the MARKET is the wrong shape.
        return Err(ClaimsSbfError::FailureEscrowUnseated.into());
    }
    if supply != residue {
        return Err(ClaimsMarketClosureSbfErrorV1::Liability.into());
    }
    {
        let mut aggregate_bytes = accounts
            .aggregate
            .try_borrow_mut_data()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        let mut position_bytes = escrow
            .position
            .try_borrow_mut_data()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        // The one live writer of a claim on a live Market. Every arm of its
        // refusal is unreachable from here -- the width seats a failure
        // coordinate because `derive` returned, the coordinate IS that one, the
        // quantity is nonzero and both resources hold exactly it -- and a
        // refusal reverts the whole transaction anyway, so no half-burned pair
        // can reach the chain either way.
        burn_failure_coordinate_v1(
            &mut aggregate_bytes,
            &mut position_bytes,
            market.claim_count,
            derived.failure_selector,
            residue,
        )
        .map_err(|refusal| match refusal {
            ClosureBurnRefusalV3::Width | ClosureBurnRefusalV3::Coordinate => {
                ClaimsSbfError::FailureEscrow.into()
            }
            ClosureBurnRefusalV3::Quantity | ClosureBurnRefusalV3::Debit => {
                ProgramError::from(ClaimsMarketClosureSbfErrorV1::Commit)
            }
        })?;
    }
    close_escrow_pair_into_aggregate_v1(accounts, escrow)
}

/// Close the emptied escrow pair and hand its rent to the aggregate.
///
/// The aggregate is the ONE account this route already disposes of, and both
/// dispositions carry the escrow's rent to decision 0021's refund source
/// without a fourth account in the frame: `close_aggregate` credits it to the
/// immutable RentCredit, and `handoff_aggregate_to_core` carries it into the
/// retirement checkpoint that pays the refund wallet at `Finish`.
#[inline(never)]
fn close_escrow_pair_into_aggregate_v1(
    accounts: ClosureAccounts<'_, '_>,
    escrow: ClosureEscrowV1<'_, '_>,
) -> ProgramResult {
    let position_lamports = escrow.position.lamports();
    let admission_lamports = escrow.admission.lamports();
    let aggregate_after = accounts
        .aggregate
        .lamports()
        .checked_add(position_lamports)
        .and_then(|total| total.checked_add(admission_lamports))
        .ok_or(ClaimsMarketClosureSbfErrorV1::Commit)?;
    {
        let mut position = escrow
            .position
            .try_borrow_mut_data()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        let mut admission = escrow
            .admission
            .try_borrow_mut_data()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        position.fill(0);
        admission.fill(0);
    }
    {
        let mut position = escrow
            .position
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        let mut admission = escrow
            .admission
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        let mut aggregate = accounts
            .aggregate
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        **position = 0;
        **admission = 0;
        **aggregate = aggregate_after;
    }
    for closed in [escrow.position, escrow.admission] {
        closed
            .resize(0)
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        closed.assign(&system_program::ID);
        if closed.owner != &system_program::ID || !closed.data_is_empty() || closed.lamports() != 0
        {
            return Err(ClaimsMarketClosureSbfErrorV1::Commit.into());
        }
    }
    if accounts.aggregate.lamports() != aggregate_after {
        return Err(ClaimsMarketClosureSbfErrorV1::Commit.into());
    }
    Ok(())
}

#[inline(never)]
fn close_aggregate(accounts: ClosureAccounts<'_, '_>, rent_after: u64) -> ProgramResult {
    {
        let mut data = accounts
            .aggregate
            .try_borrow_mut_data()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        data.fill(0);
    }
    {
        let mut aggregate_lamports = accounts
            .aggregate
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        let mut credit_lamports = accounts
            .rent_credit
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        **aggregate_lamports = 0;
        **credit_lamports = rent_after;
    }
    accounts
        .aggregate
        .resize(0)
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
    accounts.aggregate.assign(&system_program::ID);
    if accounts.aggregate.owner != &system_program::ID
        || !accounts.aggregate.data_is_empty()
        || accounts.aggregate.lamports() != 0
        || accounts.rent_credit.lamports() != rent_after
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Commit.into());
    }
    Ok(())
}

#[inline(never)]
fn handoff_aggregate_to_core(accounts: ClosureAccounts<'_, '_>) -> ProgramResult {
    let aggregate_lamports = accounts.aggregate.lamports();
    let rent_lamports = accounts.rent_credit.lamports();
    {
        let mut data = accounts
            .aggregate
            .try_borrow_mut_data()
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
        data.fill(0);
    }
    accounts
        .aggregate
        .resize(AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1)
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?;
    accounts.aggregate.assign(accounts.core_program.key);
    let zeroed = accounts
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsMarketClosureSbfErrorV1::Commit)?
        .iter()
        .all(|value| *value == 0);
    if accounts.aggregate.owner != accounts.core_program.key
        || accounts.aggregate.data_len() != AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1
        || !zeroed
        || accounts.aggregate.lamports() != aggregate_lamports
        || accounts.rent_credit.lamports() != rent_lamports
    {
        return Err(ClaimsMarketClosureSbfErrorV1::Commit.into());
    }
    Ok(())
}

fn require_distinct(accounts: &[&AccountInfo<'_>]) -> ProgramResult {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .ok_or(ClaimsMarketClosureSbfErrorV1::Accounts)?
            .iter()
            .any(|other| other.key == account.key)
        {
            return Err(ClaimsMarketClosureSbfErrorV1::Accounts.into());
        }
    }
    Ok(())
}
