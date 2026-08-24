//! Atomic founding of one authenticated categorical-Pyth Market and Fund.

use dclutch_capability_contract::{CapabilityFundingDerivationV1, CapabilityManifestV1};
use dclutch_collateral_contract::{
    AccountPrivilege, FoundMarketAndFundV1, InstructionTag, validate_account_frame,
};
use dclutch_core_contract::{MARKET_ROOT_BYTES, MarketIdentity, MarketRoot};
use dclutch_market_contract::market::{
    CategoricalMarketV1, CategoricalSettlementSummaryV1, MARKET_BASE_BYTES,
};
use dclutch_product_contract::{
    ContentId as ProductContentId, capacity::CapacityProfileV1, claim::CategoricalUnitV1,
    product::InstanceV1,
};
use dclutch_pyth_contract::{
    funding::{
        FUNDING_BYTES, FundingStateV1, construct_required_resolution_funding,
        required_resolution_minimum_balance,
    },
    resolution_material::CategoricalPythResolutionMaterialV1,
};
use dclutch_realm_contract::RealmV1;
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{
    AdapterError,
    authenticate::MARKET_SEED,
    records::{
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1,
        CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1, PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        PYTH_RESOLUTION_MATERIAL_SCHEMA_RELEASE_ID_V1, REALM_SCHEMA_RELEASE_ID_V1,
        with_authenticated_finalized_record_v1,
    },
};

const FOUNDING_ACCOUNTS: usize = 18;
const MIN_OUTCOMES: u8 = 2;
const MAX_OUTCOMES: u8 = 16;

struct FoundingFrame<'a, 'info> {
    sponsor: &'a AccountInfo<'info>,
    market: &'a AccountInfo<'info>,
    fund: &'a AccountInfo<'info>,
    rent_credit: &'a AccountInfo<'info>,
    realm: &'a AccountInfo<'info>,
    product_instance: &'a AccountInfo<'info>,
    claim_basis: &'a AccountInfo<'info>,
    capacity_profile: &'a AccountInfo<'info>,
    resolution_material: &'a AccountInfo<'info>,
    capability_manifest: &'a AccountInfo<'info>,
    realm_staging_cursor: &'a AccountInfo<'info>,
    product_instance_staging_cursor: &'a AccountInfo<'info>,
    claim_basis_staging_cursor: &'a AccountInfo<'info>,
    capacity_profile_staging_cursor: &'a AccountInfo<'info>,
    resolution_material_staging_cursor: &'a AccountInfo<'info>,
    capability_manifest_staging_cursor: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> FoundingFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != FOUNDING_ACCOUNTS {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            sponsor: account(accounts, 0)?,
            market: account(accounts, 1)?,
            fund: account(accounts, 2)?,
            rent_credit: account(accounts, 3)?,
            realm: account(accounts, 4)?,
            product_instance: account(accounts, 5)?,
            claim_basis: account(accounts, 6)?,
            capacity_profile: account(accounts, 7)?,
            resolution_material: account(accounts, 8)?,
            capability_manifest: account(accounts, 9)?,
            realm_staging_cursor: account(accounts, 10)?,
            product_instance_staging_cursor: account(accounts, 11)?,
            claim_basis_staging_cursor: account(accounts, 12)?,
            capacity_profile_staging_cursor: account(accounts, 13)?,
            resolution_material_staging_cursor: account(accounts, 14)?,
            capability_manifest_staging_cursor: account(accounts, 15)?,
            system_program: account(accounts, 16)?,
            rent_sysvar: account(accounts, 17)?,
        };
        let privileges = [
            privilege(frame.sponsor),
            privilege(frame.market),
            privilege(frame.fund),
            privilege(frame.rent_credit),
            privilege(frame.realm),
            privilege(frame.product_instance),
            privilege(frame.claim_basis),
            privilege(frame.capacity_profile),
            privilege(frame.resolution_material),
            privilege(frame.capability_manifest),
            privilege(frame.realm_staging_cursor),
            privilege(frame.product_instance_staging_cursor),
            privilege(frame.claim_basis_staging_cursor),
            privilege(frame.capacity_profile_staging_cursor),
            privilege(frame.resolution_material_staging_cursor),
            privilege(frame.capability_manifest_staging_cursor),
            privilege(frame.system_program),
            privilege(frame.rent_sysvar),
        ];
        validate_account_frame(InstructionTag::FoundMarketAndFund, &privileges)
            .map_err(|_| AdapterError::AccountPrivilege)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FoundingPlan {
    identity: MarketIdentity,
    identity_digest: [u8; 32],
    outcome_count: u8,
    market_bump: u8,
    fund_bump: u8,
    funding_derivation: CapabilityFundingDerivationV1,
    market_rent: u64,
    fund_balance: u64,
    fund: FundingStateV1,
    rent_credit: RentCreditV1,
    rent_credit_lamports: u64,
    rent_refund: [u8; 32],
    sponsor_before: u64,
    market_lamports_before: u64,
    fund_lamports_before: u64,
}

impl FoundingPlan {
    /// Exact lamports the authenticated Market and Fund creation will debit.
    pub(crate) fn required_payer_debit(&self) -> Result<u64, ProgramError> {
        self.market_rent
            .checked_add(self.fund_balance)
            .ok_or(AdapterError::Arithmetic.into())
    }
}

/// Atomically create and persist one Market and its prepaid resolution Fund.
pub(crate) fn process_found_market_and_fund(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: FoundMarketAndFundV1,
) -> Result<(), ProgramError> {
    let sponsor = *account(accounts, 0)?.key;
    let plan = preflight_found_market_and_fund(program_id, accounts, instruction, sponsor, 0)?;
    execute_preflighted_found_market_and_fund(program_id, accounts, plan)
}

/// Authenticate one Found transition while separating its temporary payer
/// from the immutable Market rent-refund identity.
///
/// `additional_payer_lamports` is an exact amount a composing transition will
/// credit to account 0 only after this complete preflight succeeds. Ordinary
/// Found passes zero and uses account 0 for both roles. Series passes its
/// ticket principal and the ticket-bound refund authority, so a permissionless
/// actor can execute Found without acquiring protocol rent.
pub(crate) fn preflight_found_market_and_fund(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: FoundMarketAndFundV1,
    rent_refund: Pubkey,
    additional_payer_lamports: u64,
) -> Result<FoundingPlan, ProgramError> {
    let frame = FoundingFrame::parse(accounts)?;
    let current_slot = Clock::get()
        .map_err(|_| AdapterError::FoundingAuthentication)?
        .slot;
    authenticate_founding(
        program_id,
        &frame,
        instruction,
        current_slot,
        &rent_refund,
        additional_payer_lamports,
    )
}

/// Execute a previously authenticated Found plan against the same exact
/// Found18 frame. The plan is module-private to construction, preventing a
/// composing adapter from overriding authenticated funding or identity facts.
pub(crate) fn execute_preflighted_found_market_and_fund(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    plan: FoundingPlan,
) -> Result<(), ProgramError> {
    let frame = FoundingFrame::parse(accounts)?;
    if frame.sponsor.lamports() != plan.sponsor_before {
        return Err(AdapterError::FoundingPostcondition.into());
    }

    let market_bump = [plan.market_bump];
    let market_signer = [
        MARKET_SEED,
        plan.identity_digest.as_slice(),
        market_bump.as_slice(),
    ];
    fund_allocate_assign(
        program_id,
        frame.sponsor,
        frame.market,
        frame.system_program,
        (plan.market_rent, market_bytes(plan.outcome_count)?),
        &market_signer,
        AdapterError::MarketCreateCpi,
    )?;

    let sponsor_after_market = plan
        .sponsor_before
        .checked_sub(plan.market_rent)
        .ok_or(AdapterError::Arithmetic)?;
    let market_after = plan
        .market_lamports_before
        .checked_add(plan.market_rent)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() != sponsor_after_market
        || frame.market.lamports() != market_after
        || frame.market.owner != program_id
        || frame.market.data_len() != market_bytes(plan.outcome_count)?
    {
        return Err(AdapterError::FoundingPostcondition.into());
    }

    let funding_components = plan.funding_derivation.seed_components();
    let fund_bump = [plan.fund_bump];
    let fund_signer = [
        funding_components[0],
        funding_components[1],
        funding_components[2],
        funding_components[3],
        funding_components[4],
        funding_components[5],
        fund_bump.as_slice(),
    ];
    fund_allocate_assign(
        program_id,
        frame.sponsor,
        frame.fund,
        frame.system_program,
        (plan.fund_balance, FUNDING_BYTES),
        &fund_signer,
        AdapterError::FundCreateCpi,
    )?;

    persist_founding(program_id, &frame, plan)?;
    Ok(())
}

#[inline(never)]
fn authenticate_founding(
    program_id: &Pubkey,
    frame: &FoundingFrame<'_, '_>,
    instruction: FoundMarketAndFundV1,
    current_slot: u64,
    rent_refund: &Pubkey,
    additional_payer_lamports: u64,
) -> Result<FoundingPlan, ProgramError> {
    authenticate_account_identities(program_id, frame)?;
    authenticate_immutable_records(program_id, frame, instruction)?;

    let identity = instruction.identity();
    let identity_digest = hash(&identity.to_bytes()).to_bytes();
    let (expected_market, market_bump) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], program_id);
    if frame.market.key != &expected_market {
        return Err(AdapterError::AccountIdentity.into());
    }
    let root = founding_root(identity, rent_refund.to_bytes())?;
    let expected_market_bytes = validate_selected_market(instruction.outcome_count(), root)?;

    let rent = Rent::from_account_info(frame.rent_sysvar)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let market_rent = rent.minimum_balance(expected_market_bytes);
    let fund_rent = rent.minimum_balance(FUNDING_BYTES);
    let (fund, funding_derivation) =
        authenticate_fund(program_id, frame, identity, fund_rent, current_slot)?;
    let (expected_fund, fund_bump) =
        Pubkey::find_program_address(&funding_derivation.seed_components(), program_id);
    if frame.fund.key != &expected_fund {
        return Err(AdapterError::AccountIdentity.into());
    }
    let rent_credit = authenticate_rent_credit(program_id, frame.rent_credit, rent_refund)?;
    let rent_credit_lamports = frame
        .rent_credit
        .try_lamports()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let fund_balance =
        required_resolution_minimum_balance(fund).map_err(|_| AdapterError::Arithmetic)?;
    let total_debit = market_rent
        .checked_add(fund_balance)
        .ok_or(AdapterError::Arithmetic)?;
    let sponsor_before = frame
        .sponsor
        .lamports()
        .checked_add(additional_payer_lamports)
        .ok_or(AdapterError::Arithmetic)?;
    if sponsor_before < total_debit {
        return Err(AdapterError::FundUnderfunded.into());
    }

    // Preflight every mutable borrow before either System CPI. The runtime
    // transaction remains the rollback authority for any later refusal.
    preflight_mutable(frame.sponsor)?;
    preflight_mutable(frame.market)?;
    preflight_mutable(frame.fund)?;
    drop(
        frame
            .market
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::FoundingAuthentication)?,
    );
    drop(
        frame
            .fund
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::FoundingAuthentication)?,
    );

    Ok(FoundingPlan {
        identity,
        identity_digest,
        outcome_count: instruction.outcome_count(),
        market_bump,
        fund_bump,
        funding_derivation,
        market_rent,
        fund_balance,
        fund,
        rent_credit,
        rent_credit_lamports,
        rent_refund: rent_refund.to_bytes(),
        sponsor_before,
        market_lamports_before: frame.market.lamports(),
        fund_lamports_before: frame.fund.lamports(),
    })
}

fn authenticate_fund(
    program_id: &Pubkey,
    frame: &FoundingFrame<'_, '_>,
    identity: MarketIdentity,
    fund_rent: u64,
    current_slot: u64,
) -> Result<(FundingStateV1, CapabilityFundingDerivationV1), ProgramError> {
    let material_digest = record_digest(frame.resolution_material)?;
    let material = with_authenticated_finalized_record_v1(
        program_id,
        frame.resolution_material,
        frame.resolution_material_staging_cursor,
        frame.rent_sysvar,
        PYTH_RESOLUTION_MATERIAL_SCHEMA_RELEASE_ID_V1,
        material_digest,
        |record| {
            CategoricalPythResolutionMaterialV1::decode(record.exact_content())
                .map_err(|_| AdapterError::FoundingAuthentication.into())
        },
    )?;
    if hash(&material.policy().to_bytes()).to_bytes() != identity.resolution_policy_id().to_bytes()
        || hash(&material.feed_profile().to_bytes()).to_bytes()
            != *material.policy().feed_profile_id()
    {
        return Err(AdapterError::FoundingAuthentication.into());
    }
    let (funding, derivation) = with_authenticated_finalized_record_v1(
        program_id,
        frame.capability_manifest,
        frame.capability_manifest_staging_cursor,
        frame.rent_sysvar,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        identity.capability_manifest_id().to_bytes(),
        |record| {
            let manifest = CapabilityManifestV1::decode(record.exact_content())
                .map_err(|_| AdapterError::FoundingAuthentication)?;
            let selected = manifest
                .required_founding_entry_for_config(identity.resolution_policy_id())
                .map_err(|_| AdapterError::FoundingAuthentication)?;
            if selected.entry().release_id().to_bytes() != *material.policy().release_id() {
                return Err(AdapterError::FoundingAuthentication.into());
            }
            let funding = construct_required_resolution_funding(
                identity.capability_manifest_id(),
                manifest,
                selected,
                fund_rent,
                current_slot,
            )
            .map_err(|_| AdapterError::FoundingAuthentication)?;
            CapabilityFundingDerivationV1::new(
                frame.market.key.to_bytes(),
                identity.generation(),
                identity.capability_manifest_id(),
                manifest,
                funding,
            )
            .map(|derivation| (funding, derivation))
            .map_err(|_| AdapterError::FoundingAuthentication.into())
        },
    )?;
    let (expected, _) = Pubkey::find_program_address(&derivation.seed_components(), program_id);
    if frame.fund.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok((funding, derivation))
}

fn authenticate_account_identities(
    program_id: &Pubkey,
    frame: &FoundingFrame<'_, '_>,
) -> Result<(), ProgramError> {
    if frame.sponsor.owner != &system_program::ID
        || !frame.sponsor.data_is_empty()
        || frame.market.owner != &system_program::ID
        || !frame.market.data_is_empty()
        || frame.fund.owner != &system_program::ID
        || !frame.fund.data_is_empty()
        || frame.system_program.key != &system_program::ID
        || frame.system_program.owner != &native_loader::ID
        || frame.rent_sysvar.key != &sysvar::rent::ID
        || frame.rent_sysvar.owner != &sysvar::ID
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    for record in [
        frame.realm,
        frame.product_instance,
        frame.claim_basis,
        frame.capacity_profile,
        frame.resolution_material,
        frame.capability_manifest,
    ] {
        if record.owner != program_id {
            return Err(AdapterError::AccountIdentity.into());
        }
    }
    Ok(())
}

fn authenticate_immutable_records(
    program_id: &Pubkey,
    frame: &FoundingFrame<'_, '_>,
    instruction: FoundMarketAndFundV1,
) -> Result<(), ProgramError> {
    let identity = instruction.identity();

    let _realm = with_authenticated_finalized_record_v1(
        program_id,
        frame.realm,
        frame.realm_staging_cursor,
        frame.rent_sysvar,
        REALM_SCHEMA_RELEASE_ID_V1,
        identity.realm_id().to_bytes(),
        |record| {
            RealmV1::decode(record.exact_content())
                .map_err(|_| AdapterError::FoundingAuthentication.into())
        },
    )?;
    let instance = with_authenticated_finalized_record_v1(
        program_id,
        frame.product_instance,
        frame.product_instance_staging_cursor,
        frame.rent_sysvar,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        identity.product_instance_id().to_bytes(),
        |record| {
            InstanceV1::decode(record.exact_content())
                .map_err(|_| AdapterError::FoundingAuthentication.into())
        },
    )?;
    let claim = with_authenticated_finalized_record_v1(
        program_id,
        frame.claim_basis,
        frame.claim_basis_staging_cursor,
        frame.rent_sysvar,
        CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1,
        identity.claim_basis_id().to_bytes(),
        |record| {
            CategoricalUnitV1::decode(record.exact_content())
                .map_err(|_| AdapterError::FoundingAuthentication.into())
        },
    )?;
    if u32::from(instruction.outcome_count()) != claim.outcome_count()
        || u32::from(instruction.outcome_count()) != instance.partition_cell_count()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    let capacity = with_authenticated_finalized_record_v1(
        program_id,
        frame.capacity_profile,
        frame.capacity_profile_staging_cursor,
        frame.rent_sysvar,
        CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1,
        claim.capacity_profile_id().content_id().to_bytes(),
        |record| {
            CapacityProfileV1::decode(record.exact_content())
                .map_err(|_| AdapterError::FoundingAuthentication.into())
        },
    )?;
    claim
        .validate_capacity(capacity)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let product_claim_id = ProductContentId::new(identity.claim_basis_id().to_bytes())
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    instance
        .validate_claim_basis(product_claim_id, claim)
        .map_err(|_| AdapterError::ContentIdentity)?;
    Ok(())
}

fn record_digest(account: &AccountInfo<'_>) -> Result<[u8; 32], ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    Ok(hash(&data).to_bytes())
}

fn persist_founding(
    program_id: &Pubkey,
    frame: &FoundingFrame<'_, '_>,
    plan: FoundingPlan,
) -> Result<(), ProgramError> {
    let expected_sponsor = plan
        .sponsor_before
        .checked_sub(plan.market_rent)
        .and_then(|balance| balance.checked_sub(plan.fund_balance))
        .ok_or(AdapterError::Arithmetic)?;
    let expected_market = plan
        .market_lamports_before
        .checked_add(plan.market_rent)
        .ok_or(AdapterError::Arithmetic)?;
    let expected_fund = plan
        .fund_lamports_before
        .checked_add(plan.fund_balance)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() != expected_sponsor
        || frame.market.lamports() != expected_market
        || frame.market.owner != program_id
        || frame.market.data_len() != market_bytes(plan.outcome_count)?
        || frame.fund.lamports() != expected_fund
        || frame.fund.owner != program_id
        || frame.fund.data_len() != FUNDING_BYTES
        || frame.rent_credit.lamports() != plan.rent_credit_lamports
    {
        return Err(AdapterError::FoundingPostcondition.into());
    }

    let root = founding_root(plan.identity, plan.rent_refund)?;
    let mut market_data = frame
        .market
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    encode_selected_market(plan.outcome_count, &mut market_data, root)?;
    drop(market_data);

    let mut fund_data = frame
        .fund
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    fund_data.copy_from_slice(&plan.fund.to_bytes());
    if FundingStateV1::decode(&fund_data) != Ok(plan.fund) {
        return Err(AdapterError::FoundingPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, frame.rent_credit, plan.rent_credit)?;
    Ok(())
}

fn authenticate_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authority_key: &Pubkey,
) -> Result<RentCreditV1, ProgramError> {
    let authority = RefundAuthority::new(authority_key.to_bytes())
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let authority_bytes = authority.to_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
        program_id,
    );
    if account.key != &expected
        || account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| AdapterError::FoundingAuthentication)?;
    credit
        .validate_binding(authority, bump)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    if credit.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(credit)
}

fn require_unchanged_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected: RentCreditV1,
) -> Result<(), ProgramError> {
    if account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::FoundingPostcondition.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    if RentCreditV1::decode(&data) != Ok(expected) || expected.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::FoundingPostcondition.into());
    }
    Ok(())
}

fn founding_root(identity: MarketIdentity, sponsor: [u8; 32]) -> Result<MarketRoot, ProgramError> {
    let mut root =
        MarketRoot::founding(identity, sponsor).map_err(|_| AdapterError::MarketTransition)?;
    root.register_child(identity.generation(), 0)
        .map_err(|_| AdapterError::MarketTransition)?;
    Ok(root)
}

type MarketValidator = fn(MarketRoot) -> Result<usize, ProgramError>;

type MarketEncoder = fn(&mut [u8], MarketRoot) -> Result<(), ProgramError>;

const MARKET_VALIDATORS: [MarketValidator; 15] = [
    validate_market::<2>,
    validate_market::<3>,
    validate_market::<4>,
    validate_market::<5>,
    validate_market::<6>,
    validate_market::<7>,
    validate_market::<8>,
    validate_market::<9>,
    validate_market::<10>,
    validate_market::<11>,
    validate_market::<12>,
    validate_market::<13>,
    validate_market::<14>,
    validate_market::<15>,
    validate_market::<16>,
];

const MARKET_ENCODERS: [MarketEncoder; 15] = [
    encode_market::<2>,
    encode_market::<3>,
    encode_market::<4>,
    encode_market::<5>,
    encode_market::<6>,
    encode_market::<7>,
    encode_market::<8>,
    encode_market::<9>,
    encode_market::<10>,
    encode_market::<11>,
    encode_market::<12>,
    encode_market::<13>,
    encode_market::<14>,
    encode_market::<15>,
    encode_market::<16>,
];

fn validate_selected_market(outcome_count: u8, root: MarketRoot) -> Result<usize, ProgramError> {
    let validator = MARKET_VALIDATORS
        .get(outcome_index(outcome_count)?)
        .copied()
        .ok_or(AdapterError::FoundingAuthentication)?;
    validator(root)
}

fn encode_selected_market(
    outcome_count: u8,
    output: &mut [u8],
    root: MarketRoot,
) -> Result<(), ProgramError> {
    let encoder = MARKET_ENCODERS
        .get(outcome_index(outcome_count)?)
        .copied()
        .ok_or(AdapterError::FoundingPostcondition)?;
    encoder(output, root)
}

fn validate_market<const N: usize>(root: MarketRoot) -> Result<usize, ProgramError> {
    CategoricalMarketV1::<N>::new(root, 0, [0; N], CategoricalSettlementSummaryV1::empty())
        .map_err(|_| AdapterError::MarketTransition)?;
    CategoricalMarketV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic.into())
}

fn encode_market<const N: usize>(output: &mut [u8], root: MarketRoot) -> Result<(), ProgramError> {
    let state =
        CategoricalMarketV1::<N>::new(root, 0, [0; N], CategoricalSettlementSummaryV1::empty())
            .map_err(|_| AdapterError::FoundingPostcondition)?;
    state
        .encode(output)
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    if CategoricalMarketV1::<N>::decode(output) != Ok(state) {
        return Err(AdapterError::FoundingPostcondition.into());
    }
    Ok(())
}

fn market_bytes(outcome_count: u8) -> Result<usize, ProgramError> {
    usize::from(outcome_count)
        .checked_mul(8)
        .and_then(|supply| MARKET_BASE_BYTES.checked_add(supply))
        .ok_or(AdapterError::Arithmetic.into())
}

fn outcome_index(outcome_count: u8) -> Result<usize, ProgramError> {
    if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&outcome_count) {
        return Err(AdapterError::FoundingAuthentication.into());
    }
    Ok(usize::from(outcome_count.saturating_sub(MIN_OUTCOMES)))
}

fn preflight_mutable(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    drop(
        account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::FoundingAuthentication)?,
    );
    Ok(())
}

fn fund_allocate_assign<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    funding: (u64, usize),
    signer: &[&[u8]],
    cpi_error: AdapterError,
) -> Result<(), ProgramError> {
    let (amount, space) = funding;
    invoke_signed(
        &transfer(payer.key, destination.key, amount),
        &[payer.clone(), destination.clone(), system.clone()],
        &[],
    )
    .map_err(|_| cpi_error)?;
    let space_u64 = u64::try_from(space).map_err(|_| AdapterError::Arithmetic)?;
    invoke_signed(
        &allocate(destination.key, space_u64),
        &[destination.clone(), system.clone()],
        &[signer],
    )
    .map_err(|_| cpi_error)?;
    invoke_signed(
        &assign(destination.key, program_id),
        &[destination.clone(), system.clone()],
        &[signer],
    )
    .map_err(|_| cpi_error)?;
    if destination.owner != program_id || destination.data_len() != space {
        return Err(AdapterError::FoundingPostcondition.into());
    }
    Ok(())
}

fn privilege(account: &AccountInfo<'_>) -> AccountPrivilege {
    AccountPrivilege {
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        is_executable: account.executable,
    }
}

fn require_distinct(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.key == account.key)
        {
            return Err(AdapterError::AccountIdentity.into());
        }
    }
    Ok(())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(AdapterError::AccountFrameLength.into())
}

const _: () = assert!(MARKET_ROOT_BYTES == 232);

#[cfg(test)]
mod tests {
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::ContentId as CoreContentId;
    use dclutch_kernel::resolution::categorical_pyth_v1::{
        CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
    };
    use dclutch_product_contract::{
        capacity::{CapacityEnvelope, CapacityProfileId, CapacityProfileV1Input},
        claim::CategoricalUnitV1Input,
        product::InstanceV1Input,
    };
    use dclutch_pyth_contract::{
        feed_profile::PythFeedProfileV1, policy::CategoricalPythPolicyRecordV1,
        resolution_material::RESOLUTION_MATERIAL_BYTES,
    };
    use dclutch_realm_contract::{
        FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_BYTES, RealmV1Input,
    };
    use dclutch_record_contract::{ContentDigest, RecordKeyV1, SchemaReleaseId};
    use std::{boxed::Box, vec, vec::Vec};

    use super::*;

    struct Fixture {
        program_id: Pubkey,
        instruction: FoundMarketAndFundV1,
        accounts: Vec<AccountInfo<'static>>,
    }

    #[derive(Clone, Copy)]
    enum ManifestCase {
        Valid,
        Empty,
        WrongConfig,
        WrongRelease,
        WrongRent,
        ZeroBounty,
        ExtraneousPrincipal,
        Ambiguous,
    }

    impl Fixture {
        fn new(outcome_count: u8) -> Self {
            Self::new_with_manifest(outcome_count, ManifestCase::Valid)
        }

        fn new_with_manifest(outcome_count: u8, manifest_case: ManifestCase) -> Self {
            let program_id = Pubkey::new_unique();
            let realm = RealmV1::new(RealmV1Input {
                token_program: [2; 32],
                collateral_mint: [3; 32],
                collateral_adapter_release_id: [4; 32],
                mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
                freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
            })
            .expect("valid Realm");
            let realm_bytes = realm.to_bytes();
            let realm_digest = hash(&realm_bytes).to_bytes();

            let capacity = capacity();
            let capacity_bytes = capacity.to_bytes();
            let capacity_id = product_id(hash(&capacity_bytes).to_bytes());
            let claim = CategoricalUnitV1::new(
                CategoricalUnitV1Input {
                    capacity_profile_id: CapacityProfileId::new(capacity_id),
                    outcome_count: u32::from(outcome_count),
                },
                capacity,
            )
            .expect("valid categorical basis");
            let claim_bytes = claim.to_bytes();
            let claim_digest = hash(&claim_bytes).to_bytes();
            let instance = InstanceV1::new(InstanceV1Input {
                terms_id: product_id([5; 32]),
                occurrence_id: product_id([6; 32]),
                claim_basis_id: product_id(claim_digest),
                capacity_profile_id: CapacityProfileId::new(capacity_id),
                partition_cell_count: u32::from(outcome_count),
            })
            .expect("valid Product instance");
            let instance_bytes = instance.to_bytes();

            let feed_profile =
                PythFeedProfileV1::new([7; 32], [8; 32], [9; 32]).expect("valid profile");
            let policy = policy(outcome_count, hash(&feed_profile.to_bytes()).to_bytes());
            let material = CategoricalPythResolutionMaterialV1::new(policy, feed_profile)
                .expect("valid material");
            let material_bytes = material.to_bytes();
            let policy_id = core_id(hash(&policy.to_bytes()).to_bytes());
            let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);
            let manifest_bytes = manifest_bytes(manifest_case, policy_id, policy, fund_rent);

            let identity = MarketIdentity::new(
                core_id(realm_digest),
                core_id(hash(&instance_bytes).to_bytes()),
                core_id(claim_digest),
                core_id(hash(&policy.to_bytes()).to_bytes()),
                core_id(hash(&manifest_bytes).to_bytes()),
                7,
            );
            let identity_digest = hash(&identity.to_bytes()).to_bytes();
            let (market_key, _) =
                Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], &program_id);
            let sponsor_key = Pubkey::new_unique();
            let fund_key = if matches!(manifest_case, ManifestCase::Valid) {
                let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest");
                let selected = manifest
                    .required_founding_entry_for_config(identity.resolution_policy_id())
                    .expect("selected funding");
                let funding = construct_required_resolution_funding(
                    identity.capability_manifest_id(),
                    manifest,
                    selected,
                    fund_rent,
                    0,
                )
                .expect("funding");
                let derivation = CapabilityFundingDerivationV1::new(
                    market_key.to_bytes(),
                    identity.generation(),
                    identity.capability_manifest_id(),
                    manifest,
                    funding,
                )
                .expect("derivation");
                Pubkey::find_program_address(&derivation.seed_components(), &program_id).0
            } else {
                Pubkey::new_unique()
            };
            let (rent_credit_key, rent_credit_bump) = Pubkey::find_program_address(
                &[RENT_CREDIT_PDA_DOMAIN_V1, sponsor_key.as_ref()],
                &program_id,
            );
            let rent_credit = RentCreditV1::new(
                RefundAuthority::new(sponsor_key.to_bytes()).expect("authority"),
                rent_credit_bump,
            );
            let instruction =
                FoundMarketAndFundV1::new(identity, outcome_count).expect("valid instruction");

            let (realm_key, realm_cursor) =
                record_pair(&program_id, REALM_SCHEMA_RELEASE_ID_V1, realm_digest);
            let (instance_key, instance_cursor) = record_pair(
                &program_id,
                PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
                hash(&instance_bytes).to_bytes(),
            );
            let (claim_key, claim_cursor) = record_pair(
                &program_id,
                CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1,
                claim_digest,
            );
            let (capacity_key, capacity_cursor) = record_pair(
                &program_id,
                CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1,
                hash(&capacity_bytes).to_bytes(),
            );
            let (material_key, material_cursor) = record_pair(
                &program_id,
                PYTH_RESOLUTION_MATERIAL_SCHEMA_RELEASE_ID_V1,
                hash(&material_bytes).to_bytes(),
            );
            let manifest_digest = identity.capability_manifest_id().to_bytes();
            let (manifest_key, manifest_cursor) = record_pair(
                &program_id,
                CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
                manifest_digest,
            );

            let mut accounts = vec![
                leak_account(
                    sponsor_key,
                    true,
                    true,
                    100_000_000,
                    vec![],
                    system_program::ID,
                    false,
                ),
                leak_account(
                    market_key,
                    false,
                    true,
                    0,
                    vec![],
                    system_program::ID,
                    false,
                ),
                leak_account(fund_key, false, true, 0, vec![], system_program::ID, false),
                leak_account(
                    rent_credit_key,
                    false,
                    false,
                    1,
                    rent_credit.to_bytes().to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    realm_key,
                    false,
                    false,
                    Rent::default().minimum_balance(realm_bytes.len()),
                    realm_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    instance_key,
                    false,
                    false,
                    Rent::default().minimum_balance(instance_bytes.len()),
                    instance_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    claim_key,
                    false,
                    false,
                    Rent::default().minimum_balance(claim_bytes.len()),
                    claim_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    capacity_key,
                    false,
                    false,
                    Rent::default().minimum_balance(capacity_bytes.len()),
                    capacity_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    material_key,
                    false,
                    false,
                    Rent::default().minimum_balance(material_bytes.len()),
                    material_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    manifest_key,
                    false,
                    false,
                    Rent::default().minimum_balance(manifest_bytes.len()),
                    manifest_bytes,
                    program_id,
                    false,
                ),
                leak_account(
                    realm_cursor,
                    false,
                    false,
                    0,
                    vec![],
                    system_program::ID,
                    false,
                ),
                leak_account(
                    instance_cursor,
                    false,
                    false,
                    0,
                    vec![],
                    system_program::ID,
                    false,
                ),
                leak_account(
                    claim_cursor,
                    false,
                    false,
                    0,
                    vec![],
                    system_program::ID,
                    false,
                ),
                leak_account(
                    capacity_cursor,
                    false,
                    false,
                    0,
                    vec![],
                    system_program::ID,
                    false,
                ),
                leak_account(
                    material_cursor,
                    false,
                    false,
                    0,
                    vec![],
                    system_program::ID,
                    false,
                ),
                leak_account(
                    manifest_cursor,
                    false,
                    false,
                    0,
                    vec![],
                    system_program::ID,
                    false,
                ),
                leak_account(
                    system_program::ID,
                    false,
                    false,
                    1,
                    vec![],
                    native_loader::ID,
                    true,
                ),
                leak_account(
                    sysvar::rent::ID,
                    false,
                    false,
                    1,
                    vec![0; Rent::size_of()],
                    sysvar::ID,
                    false,
                ),
            ];
            let rent_account = accounts.get_mut(17).expect("rent account");
            assert_eq!(Rent::default().to_account_info(rent_account), Some(()));
            Self {
                program_id,
                instruction,
                accounts,
            }
        }

        fn authenticate(&self) -> Result<FoundingPlan, ProgramError> {
            let frame = FoundingFrame::parse(&self.accounts)?;
            authenticate_founding(
                &self.program_id,
                &frame,
                self.instruction,
                44,
                frame.sponsor.key,
                0,
            )
        }
    }

    fn record_pair(program_id: &Pubkey, schema: [u8; 32], digest: [u8; 32]) -> (Pubkey, Pubkey) {
        let key = RecordKeyV1::new(
            SchemaReleaseId::new(schema).expect("schema"),
            ContentDigest::new(digest).expect("digest"),
        );
        (
            crate::records::derive_record_pda(program_id, key, false).0,
            crate::records::derive_record_pda(program_id, key, true).0,
        )
    }

    fn manifest_bytes(
        manifest_case: ManifestCase,
        policy_id: CoreContentId,
        policy: CategoricalPythPolicyRecordV1,
        fund_rent: u64,
    ) -> Vec<u8> {
        let entries = match manifest_case {
            ManifestCase::Empty => vec![],
            ManifestCase::Valid => vec![resolution_entry(
                13,
                policy_id,
                *policy.release_id(),
                fund_rent,
                3,
                5,
                0,
            )],
            ManifestCase::WrongConfig => vec![resolution_entry(
                13,
                core_id([31; 32]),
                *policy.release_id(),
                fund_rent,
                3,
                5,
                0,
            )],
            ManifestCase::WrongRelease => vec![resolution_entry(
                13, policy_id, [32; 32], fund_rent, 3, 5, 0,
            )],
            ManifestCase::WrongRent => vec![resolution_entry(
                13,
                policy_id,
                *policy.release_id(),
                fund_rent.checked_add(1).expect("bounded rent"),
                3,
                5,
                0,
            )],
            ManifestCase::ZeroBounty => vec![resolution_entry(
                13,
                policy_id,
                *policy.release_id(),
                fund_rent,
                3,
                0,
                0,
            )],
            ManifestCase::ExtraneousPrincipal => vec![resolution_entry(
                13,
                policy_id,
                *policy.release_id(),
                fund_rent,
                3,
                5,
                1,
            )],
            ManifestCase::Ambiguous => vec![
                resolution_entry(13, policy_id, *policy.release_id(), fund_rent, 3, 5, 0),
                resolution_entry(14, policy_id, *policy.release_id(), fund_rent, 3, 5, 0),
            ],
        };
        let mut bytes = vec![
            0;
            MANIFEST_HEADER_BYTES
                .checked_add(
                    entries
                        .len()
                        .checked_mul(CAPABILITY_ENTRY_BYTES)
                        .expect("bounded manifest entries"),
                )
                .expect("bounded manifest bytes")
        ];
        CapabilityManifestV1::encode_into(&entries, &mut bytes).expect("canonical manifest");
        bytes
    }

    #[allow(clippy::too_many_arguments)]
    fn resolution_entry(
        kind: u8,
        config_id: CoreContentId,
        release_id: [u8; 32],
        rent: u64,
        provider: u64,
        bounty: u64,
        creation: u64,
    ) -> CapabilityEntryV1 {
        let native_or_not_applicable = |amount| {
            if amount == 0 {
                CompartmentFundingV1::not_applicable()
            } else {
                CompartmentFundingV1::native_lamports(amount).expect("bounded native amount")
            }
        };
        CapabilityEntryV1::new(
            core_id([kind; 32]),
            core_id(release_id),
            config_id,
            core_id([41; 32]),
            core_id([42; 32]),
            core_id([43; 32]),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(
                FundingAmountsV1::new(
                    native_or_not_applicable(rent),
                    native_or_not_applicable(creation),
                    CompartmentFundingV1::not_applicable(),
                    native_or_not_applicable(provider),
                    native_or_not_applicable(bounty),
                    CompartmentFundingV1::not_applicable(),
                    CompartmentFundingV1::not_applicable(),
                )
                .expect("representable typed quote"),
                None,
            )
            .expect("representable quote"),
        )
        .expect("canonical capability entry")
    }

    fn capacity() -> CapacityProfileV1 {
        CapacityProfileV1::new(CapacityProfileV1Input {
            envelope: CapacityEnvelope::Measured,
            verifier_release_id: product_id([10; 32]),
            envelope_basis_id: product_id([11; 32]),
            max_artifact_bytes: 128,
            page_payload_bytes: 64,
            max_pages: 2,
            max_partition_cells: 16,
        })
        .expect("valid capacity")
    }

    fn policy(outcome_count: u8, feed_profile_id: [u8; 32]) -> CategoricalPythPolicyRecordV1 {
        let price_cell_count = u16::from(outcome_count.saturating_sub(1));
        let mut upper_edges = [0; MAX_PRICE_CELLS];
        let active_edges = usize::from(price_cell_count.saturating_sub(1));
        for (index, edge) in upper_edges.iter_mut().take(active_edges).enumerate() {
            *edge = u128::try_from(index)
                .expect("small edge")
                .checked_add(1)
                .expect("small edge");
        }
        CategoricalPythPolicyRecordV1::new(CategoricalPythV1PolicyInput {
            pyth_release_id: [12; 32],
            feed_profile_id,
            target_time: 100,
            grace: 5,
            window: 10,
            max_crossing_lag: 5,
            max_age: 20,
            max_future_skew: 5,
            confidence_multiplier: 1,
            max_confidence_bps: 10_000,
            max_normalized_confidence_atoms: 100,
            normalized_decimals: 8,
            price_cell_count,
            upper_edges,
            failure_outcome_index: price_cell_count,
        })
        .expect("valid policy")
    }

    fn core_id(bytes: [u8; 32]) -> CoreContentId {
        CoreContentId::new(bytes).expect("nonzero core ID")
    }

    fn product_id(bytes: [u8; 32]) -> ProductContentId {
        ProductContentId::new(bytes).expect("nonzero Product ID")
    }

    fn leak_account(
        key: Pubkey,
        signer: bool,
        writable: bool,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn test_account<'a>(
        accounts: &'a [AccountInfo<'static>],
        index: usize,
    ) -> &'a AccountInfo<'static> {
        accounts.get(index).expect("fixture account")
    }

    fn test_account_mut<'a>(
        accounts: &'a mut [AccountInfo<'static>],
        index: usize,
    ) -> &'a mut AccountInfo<'static> {
        accounts.get_mut(index).expect("fixture account")
    }

    #[test]
    fn every_profile_width_authenticates_and_uses_exact_typed_market_bytes() {
        for outcome_count in MIN_OUTCOMES..=MAX_OUTCOMES {
            let fixture = Fixture::new(outcome_count);
            let plan = fixture.authenticate().expect("authenticated founding");
            assert_eq!(plan.outcome_count, outcome_count);
            assert_eq!(
                market_bytes(outcome_count),
                Ok(MARKET_BASE_BYTES + usize::from(outcome_count) * 8)
            );

            let frame = FoundingFrame::parse(&fixture.accounts).expect("exact frame");
            let root =
                founding_root(plan.identity, frame.sponsor.key.to_bytes()).expect("founding root");
            let mut output = vec![0; market_bytes(outcome_count).expect("market bytes")];
            encode_selected_market(outcome_count, &mut output, root)
                .expect("typed encode and decode postcondition");
            assert_eq!(output.get(10), Some(&outcome_count));
        }
    }

    #[test]
    fn hostile_frame_privilege_alias_owner_and_pda_refuse() {
        let canonical = Fixture::new(2);
        assert_eq!(
            FoundingFrame::parse(canonical.accounts.get(..10).expect("fixture prefix")).err(),
            Some(ProgramError::from(AdapterError::AccountFrameLength))
        );

        let mut missing_signer = Fixture::new(2);
        test_account_mut(&mut missing_signer.accounts, 0).is_signer = false;
        assert_eq!(
            missing_signer.authenticate().err(),
            Some(ProgramError::from(AdapterError::AccountPrivilege))
        );

        let mut aliased = Fixture::new(2);
        let sponsor = test_account(&aliased.accounts, 0).clone();
        *test_account_mut(&mut aliased.accounts, 1) = sponsor;
        assert_eq!(
            aliased.authenticate().err(),
            Some(ProgramError::from(AdapterError::AccountIdentity))
        );

        let wrong_owner = Fixture::new(2);
        test_account(&wrong_owner.accounts, 4).assign(&system_program::ID);
        assert_eq!(
            wrong_owner.authenticate().err(),
            Some(ProgramError::from(AdapterError::AccountIdentity))
        );

        let mut wrong_pda = Fixture::new(2);
        *test_account_mut(&mut wrong_pda.accounts, 1) = leak_account(
            Pubkey::new_unique(),
            false,
            true,
            0,
            vec![],
            system_program::ID,
            false,
        );
        assert_eq!(
            wrong_pda.authenticate().err(),
            Some(ProgramError::from(AdapterError::AccountIdentity))
        );
    }

    #[test]
    fn hostile_product_capacity_policy_and_manifest_bytes_refuse() {
        let bad_claim = Fixture::new(2);
        let mut claim_data = test_account(&bad_claim.accounts, 6)
            .try_borrow_mut_data()
            .expect("claim data");
        *claim_data.get_mut(12).expect("claim reserved byte") = 1;
        drop(claim_data);
        assert!(bad_claim.authenticate().is_err());

        let wrong_capacity_identity = Fixture::new(2);
        let mut capacity_data = test_account(&wrong_capacity_identity.accounts, 7)
            .try_borrow_mut_data()
            .expect("capacity data");
        *capacity_data.get_mut(16).expect("capacity identity byte") ^= 1;
        drop(capacity_data);
        assert!(wrong_capacity_identity.authenticate().is_err());

        let wrong_feed_link = Fixture::new(2);
        let mut material_data = test_account(&wrong_feed_link.accounts, 8)
            .try_borrow_mut_data()
            .expect("material data");
        *material_data
            .get_mut(16 + 48)
            .expect("feed-profile link byte") ^= 1;
        drop(material_data);
        assert!(wrong_feed_link.authenticate().is_err());

        let mut trailing_manifest = Fixture::new(2);
        let manifest_key = *test_account(&trailing_manifest.accounts, 9).key;
        let program_id = trailing_manifest.program_id;
        *test_account_mut(&mut trailing_manifest.accounts, 9) = leak_account(
            manifest_key,
            false,
            false,
            1,
            vec![0; dclutch_capability_contract::MANIFEST_HEADER_BYTES + 1],
            program_id,
            false,
        );
        assert!(trailing_manifest.authenticate().is_err());
    }

    #[test]
    fn manifest_is_the_unique_resolution_fund_capital_authority() {
        for manifest_case in [
            ManifestCase::Empty,
            ManifestCase::WrongConfig,
            ManifestCase::WrongRelease,
            ManifestCase::WrongRent,
            ManifestCase::ZeroBounty,
            ManifestCase::ExtraneousPrincipal,
            ManifestCase::Ambiguous,
        ] {
            assert_eq!(
                Fixture::new_with_manifest(2, manifest_case)
                    .authenticate()
                    .err(),
                Some(ProgramError::from(AdapterError::FoundingAuthentication))
            );
        }

        let valid = Fixture::new(2);
        let plan = valid.authenticate().expect("manifest-authorized funding");
        let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);
        assert_eq!(plan.fund.remaining().provider().amount(), 3);
        assert_eq!(plan.fund.remaining().bounty().amount(), 5);
        assert_eq!(plan.fund_balance, fund_rent + 3 + 5);
    }

    #[test]
    fn dusted_vacancies_compose_but_existing_state_and_underfunding_refuse() {
        let dusted = Fixture::new(2);
        **test_account(&dusted.accounts, 1)
            .try_borrow_mut_lamports()
            .expect("market lamports") = 1;
        **test_account(&dusted.accounts, 2)
            .try_borrow_mut_lamports()
            .expect("fund lamports") = 2;
        let dusted_plan = dusted.authenticate().expect("dusted vacant PDAs");
        assert_eq!(dusted_plan.market_lamports_before, 1);
        assert_eq!(dusted_plan.fund_lamports_before, 2);

        let existing_market = Fixture::new(2);
        test_account(&existing_market.accounts, 1).assign(&existing_market.program_id);
        assert_eq!(
            existing_market.authenticate().err(),
            Some(ProgramError::from(AdapterError::AccountIdentity))
        );

        let insufficient = Fixture::new(2);
        **test_account(&insufficient.accounts, 0)
            .try_borrow_mut_lamports()
            .expect("sponsor lamports") = 1;
        assert_eq!(
            insufficient.authenticate().err(),
            Some(ProgramError::from(AdapterError::FundUnderfunded))
        );
    }

    #[test]
    fn composing_payer_cannot_substitute_the_semantic_rent_refund() {
        let mut fixture = Fixture::new(2);
        let payer = *test_account(&fixture.accounts, 0).key;
        let semantic_refund = Pubkey::new_unique();
        assert_ne!(payer, semantic_refund);
        let frame = FoundingFrame::parse(&fixture.accounts).expect("exact Found18");
        assert_eq!(
            authenticate_founding(
                &fixture.program_id,
                &frame,
                fixture.instruction,
                44,
                &semantic_refund,
                0,
            )
            .err(),
            Some(ProgramError::from(AdapterError::AccountIdentity))
        );

        let (credit_key, credit_bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, semantic_refund.as_ref()],
            &fixture.program_id,
        );
        let credit = RentCreditV1::new(
            RefundAuthority::new(semantic_refund.to_bytes()).expect("refund authority"),
            credit_bump,
        );
        let program_id = fixture.program_id;
        *test_account_mut(&mut fixture.accounts, 3) = leak_account(
            credit_key,
            false,
            false,
            1,
            credit.to_bytes().to_vec(),
            program_id,
            false,
        );
        let frame = FoundingFrame::parse(&fixture.accounts).expect("exact Found18");
        let plan = authenticate_founding(
            &fixture.program_id,
            &frame,
            fixture.instruction,
            44,
            &semantic_refund,
            0,
        )
        .expect("separate payer and refund authenticate");
        assert_eq!(plan.rent_refund, semantic_refund.to_bytes());
        assert_eq!(
            founding_root(plan.identity, plan.rent_refund)
                .expect("founding root")
                .rent_refund(),
            semantic_refund.to_bytes()
        );
    }

    #[test]
    fn persistence_retains_one_fund_child_and_exact_refund_authority() {
        for outcome_count in [MIN_OUTCOMES, MAX_OUTCOMES] {
            let mut fixture = Fixture::new(outcome_count);
            let plan = fixture.authenticate().expect("authenticated founding");
            let market_key = *test_account(&fixture.accounts, 1).key;
            let fund_key = *test_account(&fixture.accounts, 2).key;
            let sponsor_final = plan
                .sponsor_before
                .checked_sub(plan.market_rent)
                .and_then(|value| value.checked_sub(plan.fund_balance))
                .expect("funded fixture");
            **test_account(&fixture.accounts, 0)
                .try_borrow_mut_lamports()
                .expect("sponsor lamports") = sponsor_final;
            let program_id = fixture.program_id;
            *test_account_mut(&mut fixture.accounts, 1) = leak_account(
                market_key,
                false,
                true,
                plan.market_rent,
                vec![0; market_bytes(outcome_count).expect("market width")],
                program_id,
                false,
            );
            *test_account_mut(&mut fixture.accounts, 2) = leak_account(
                fund_key,
                false,
                true,
                plan.fund_balance,
                vec![0; FUNDING_BYTES],
                program_id,
                false,
            );
            let frame = FoundingFrame::parse(&fixture.accounts).expect("created frame");
            persist_founding(&fixture.program_id, &frame, plan).expect("exact persistence");
            let fund_data = frame.fund.try_borrow_data().expect("fund data");
            let fund = FundingStateV1::decode(&fund_data).expect("canonical Fund");
            assert_eq!(
                fund.manifest_content_id(),
                plan.identity.capability_manifest_id()
            );
            assert_eq!(fund.activation_slot(), 44);
            assert_eq!(fund.remaining().provider().amount(), 3);
            assert_eq!(fund.remaining().bounty().amount(), 5);
            assert_eq!(
                frame.resolution_material.data_len(),
                RESOLUTION_MATERIAL_BYTES
            );
        }
    }

    #[test]
    fn declared_market_root_width_remains_the_contract_width() {
        assert_eq!(MARKET_ROOT_BYTES, 232);
        assert_eq!(REALM_BYTES, 112);
    }
}
