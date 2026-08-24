//! Atomic founding of one authenticated categorical-Pyth Market and Fund.

use dclutch_capability_contract::{CapabilityManifestV1, FundingQuoteV1};
use dclutch_collateral_contract::{
    AccountPrivilege, FoundMarketAndFundV1, InstructionTag, validate_account_frame,
};
use dclutch_core_contract::{MARKET_ROOT_BYTES, MarketIdentity, MarketRoot};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::CapacityProfileV1,
    claim::{CategoricalUnitV1, ClaimBasisProfileV1},
    product::InstanceV1,
};
use dclutch_pyth_contract::{
    feed_profile::PythFeedProfileV1,
    funding::{FUNDING_BYTES, ResolutionFundV1},
    market::{MARKET_BASE_BYTES, MarketStateV1},
    policy::CategoricalPythPolicyRecordV1,
    receipt::ResolutionReceiptV1,
    resolution_material::CategoricalPythResolutionMaterialV1,
};
use dclutch_realm_contract::{REALM_PDA_DOMAIN, RealmV1};
use solana_program::{
    account_info::AccountInfo, hash::hash, program::invoke_signed, program_error::ProgramError,
    pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

use crate::{
    AdapterError,
    authenticate::{FUND_SEED, MARKET_SEED},
};

const FOUNDING_ACCOUNTS: usize = 11;
const MIN_OUTCOMES: u8 = 2;
const MAX_OUTCOMES: u8 = 16;

struct FoundingFrame<'a, 'info> {
    sponsor: &'a AccountInfo<'info>,
    market: &'a AccountInfo<'info>,
    fund: &'a AccountInfo<'info>,
    realm: &'a AccountInfo<'info>,
    product_instance: &'a AccountInfo<'info>,
    claim_basis: &'a AccountInfo<'info>,
    capacity_profile: &'a AccountInfo<'info>,
    resolution_material: &'a AccountInfo<'info>,
    capability_manifest: &'a AccountInfo<'info>,
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
            realm: account(accounts, 3)?,
            product_instance: account(accounts, 4)?,
            claim_basis: account(accounts, 5)?,
            capacity_profile: account(accounts, 6)?,
            resolution_material: account(accounts, 7)?,
            capability_manifest: account(accounts, 8)?,
            system_program: account(accounts, 9)?,
            rent_sysvar: account(accounts, 10)?,
        };
        let privileges = [
            privilege(frame.sponsor),
            privilege(frame.market),
            privilege(frame.fund),
            privilege(frame.realm),
            privilege(frame.product_instance),
            privilege(frame.claim_basis),
            privilege(frame.capacity_profile),
            privilege(frame.resolution_material),
            privilege(frame.capability_manifest),
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
struct FoundingPlan {
    identity: MarketIdentity,
    identity_digest: [u8; 32],
    outcome_count: u8,
    market_bump: u8,
    fund_bump: u8,
    market_rent: u64,
    fund_balance: u64,
    provider_fee_reimbursement: u64,
    resolution_success_bounty: u64,
    sponsor_before: u64,
}

/// Atomically create and persist one Market and its prepaid resolution Fund.
pub(crate) fn process_found_market_and_fund(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: FoundMarketAndFundV1,
) -> Result<(), ProgramError> {
    let frame = FoundingFrame::parse(accounts)?;
    let plan = authenticate_founding(program_id, &frame, instruction)?;

    let market_space =
        u64::try_from(market_bytes(plan.outcome_count)?).map_err(|_| AdapterError::Arithmetic)?;
    let create_market = create_account(
        frame.sponsor.key,
        frame.market.key,
        plan.market_rent,
        market_space,
        program_id,
    );
    let market_bump = [plan.market_bump];
    let market_signer = [
        MARKET_SEED,
        plan.identity_digest.as_slice(),
        market_bump.as_slice(),
    ];
    invoke_signed(
        &create_market,
        &[
            frame.sponsor.clone(),
            frame.market.clone(),
            frame.system_program.clone(),
        ],
        &[&market_signer],
    )
    .map_err(|_| AdapterError::MarketCreateCpi)?;

    let sponsor_after_market = plan
        .sponsor_before
        .checked_sub(plan.market_rent)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() != sponsor_after_market
        || frame.market.lamports() != plan.market_rent
        || frame.market.owner != program_id
        || frame.market.data_len() != market_bytes(plan.outcome_count)?
    {
        return Err(AdapterError::FoundingPostcondition.into());
    }

    let fund_space = u64::try_from(FUNDING_BYTES).map_err(|_| AdapterError::Arithmetic)?;
    let create_fund = create_account(
        frame.sponsor.key,
        frame.fund.key,
        plan.fund_balance,
        fund_space,
        program_id,
    );
    let fund_bump = [plan.fund_bump];
    let fund_signer = [FUND_SEED, frame.market.key.as_ref(), fund_bump.as_slice()];
    invoke_signed(
        &create_fund,
        &[
            frame.sponsor.clone(),
            frame.fund.clone(),
            frame.system_program.clone(),
        ],
        &[&fund_signer],
    )
    .map_err(|_| AdapterError::FundCreateCpi)?;

    persist_founding(program_id, &frame, plan)?;
    Ok(())
}

#[inline(never)]
fn authenticate_founding(
    program_id: &Pubkey,
    frame: &FoundingFrame<'_, '_>,
    instruction: FoundMarketAndFundV1,
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
    let (expected_fund, fund_bump) =
        Pubkey::find_program_address(&[FUND_SEED, frame.market.key.as_ref()], program_id);
    if frame.fund.key != &expected_fund {
        return Err(AdapterError::AccountIdentity.into());
    }

    let root = founding_root(identity, frame.sponsor.key.to_bytes())?;
    let material_data = frame
        .resolution_material
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let material = CategoricalPythResolutionMaterialV1::decode(&material_data)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let expected_market_bytes = validate_selected_market(
        instruction.outcome_count(),
        root,
        *material.policy(),
        *material.feed_profile(),
    )?;
    drop(material_data);

    let rent = Rent::from_account_info(frame.rent_sysvar)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let market_rent = rent.minimum_balance(expected_market_bytes);
    let fund_rent = rent.minimum_balance(FUNDING_BYTES);
    let quote = authenticate_fund_quote(frame, identity, fund_rent)?;
    let fund = ResolutionFundV1::new(
        frame.market.key.to_bytes(),
        identity.generation(),
        frame.sponsor.key.to_bytes(),
        quote.provider_principal(),
        quote.bounty_principal(),
    )
    .map_err(|_| AdapterError::FoundingAuthentication)?;
    let fund_balance = fund
        .minimum_balance(fund_rent)
        .map_err(|_| AdapterError::Arithmetic)?;
    if fund_balance != quote.total_principal() {
        return Err(AdapterError::FoundingAuthentication.into());
    }
    let total_debit = market_rent
        .checked_add(fund_balance)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() < total_debit {
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
        market_rent,
        fund_balance,
        provider_fee_reimbursement: quote.provider_principal(),
        resolution_success_bounty: quote.bounty_principal(),
        sponsor_before: frame.sponsor.lamports(),
    })
}

fn authenticate_fund_quote(
    frame: &FoundingFrame<'_, '_>,
    identity: MarketIdentity,
    fund_rent: u64,
) -> Result<FundingQuoteV1, ProgramError> {
    let material_data = frame
        .resolution_material
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let material = CategoricalPythResolutionMaterialV1::decode(&material_data)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let manifest_data = frame
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let manifest = CapabilityManifestV1::decode(&manifest_data)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let selected = manifest
        .required_founding_entry_for_config(identity.resolution_policy_id())
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    if selected.entry().release_id().to_bytes() != *material.policy().release_id() {
        return Err(AdapterError::FoundingAuthentication.into());
    }
    selected
        .validate_one_shot_resolution_fund_quote(fund_rent)
        .map_err(|_| AdapterError::FoundingAuthentication.into())
}

fn authenticate_account_identities(
    program_id: &Pubkey,
    frame: &FoundingFrame<'_, '_>,
) -> Result<(), ProgramError> {
    if frame.sponsor.owner != &system_program::ID
        || !frame.sponsor.data_is_empty()
        || frame.market.owner != &system_program::ID
        || !frame.market.data_is_empty()
        || frame.market.lamports() != 0
        || frame.fund.owner != &system_program::ID
        || !frame.fund.data_is_empty()
        || frame.fund.lamports() != 0
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

    let realm_data = frame
        .realm
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let realm = RealmV1::decode(&realm_data).map_err(|_| AdapterError::FoundingAuthentication)?;
    let realm_digest = hash(&realm_data).to_bytes();
    if realm.to_bytes().as_slice() != &realm_data[..]
        || realm_digest != identity.realm_id().to_bytes()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    let (expected_realm, _) =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], program_id);
    if frame.realm.key != &expected_realm {
        return Err(AdapterError::AccountIdentity.into());
    }
    drop(realm_data);

    let instance_data = frame
        .product_instance
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let instance =
        InstanceV1::decode(&instance_data).map_err(|_| AdapterError::FoundingAuthentication)?;
    let instance_digest = hash(&instance_data).to_bytes();
    if instance.to_bytes().as_slice() != &instance_data[..]
        || instance_digest != identity.product_instance_id().to_bytes()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    drop(instance_data);

    let claim_data = frame
        .claim_basis
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let claim =
        CategoricalUnitV1::decode(&claim_data).map_err(|_| AdapterError::FoundingAuthentication)?;
    let claim_digest = hash(&claim_data).to_bytes();
    if claim.to_bytes().as_slice() != &claim_data[..]
        || claim_digest != identity.claim_basis_id().to_bytes()
        || u32::from(instruction.outcome_count()) != claim.outcome_count()
        || u32::from(instruction.outcome_count()) != instance.partition_cell_count()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    drop(claim_data);

    let capacity_data = frame
        .capacity_profile
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let capacity = CapacityProfileV1::decode(&capacity_data)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let capacity_digest = hash(&capacity_data).to_bytes();
    if capacity.to_bytes().as_slice() != &capacity_data[..]
        || capacity_digest != claim.capacity_profile_id().content_id().to_bytes()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    claim
        .validate_capacity(capacity)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let product_claim_id =
        ProductContentId::new(claim_digest).map_err(|_| AdapterError::FoundingAuthentication)?;
    instance
        .validate_claim_basis(
            product_claim_id,
            ClaimBasisProfileV1::CategoricalUnit(claim),
        )
        .map_err(|_| AdapterError::ContentIdentity)?;
    drop(capacity_data);

    let material_data = frame
        .resolution_material
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let material = CategoricalPythResolutionMaterialV1::decode(&material_data)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    if hash(&material.policy().to_bytes()).to_bytes() != identity.resolution_policy_id().to_bytes()
        || hash(&material.feed_profile().to_bytes()).to_bytes()
            != *material.policy().feed_profile_id()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    drop(material_data);

    let manifest_data = frame
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    let manifest = CapabilityManifestV1::decode(&manifest_data)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    if hash(manifest.as_bytes()).to_bytes() != identity.capability_manifest_id().to_bytes() {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(())
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
    if frame.sponsor.lamports() != expected_sponsor
        || frame.market.lamports() != plan.market_rent
        || frame.market.owner != program_id
        || frame.market.data_len() != market_bytes(plan.outcome_count)?
        || frame.fund.lamports() != plan.fund_balance
        || frame.fund.owner != program_id
        || frame.fund.data_len() != FUNDING_BYTES
    {
        return Err(AdapterError::FoundingPostcondition.into());
    }

    let material_data = frame
        .resolution_material
        .try_borrow_data()
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    let material = CategoricalPythResolutionMaterialV1::decode(&material_data)
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    let root = founding_root(plan.identity, frame.sponsor.key.to_bytes())?;
    let mut market_data = frame
        .market
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    encode_selected_market(
        plan.outcome_count,
        &mut market_data,
        root,
        *material.policy(),
        *material.feed_profile(),
    )?;
    drop(market_data);
    drop(material_data);

    let fund = ResolutionFundV1::new(
        frame.market.key.to_bytes(),
        plan.identity.generation(),
        frame.sponsor.key.to_bytes(),
        plan.provider_fee_reimbursement,
        plan.resolution_success_bounty,
    )
    .map_err(|_| AdapterError::FoundingPostcondition)?;
    let mut fund_data = frame
        .fund
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    fund.encode(&mut fund_data)
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    if ResolutionFundV1::decode(&fund_data) != Ok(fund) {
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

type MarketValidator =
    fn(MarketRoot, CategoricalPythPolicyRecordV1, PythFeedProfileV1) -> Result<usize, ProgramError>;

type MarketEncoder = fn(
    &mut [u8],
    MarketRoot,
    CategoricalPythPolicyRecordV1,
    PythFeedProfileV1,
) -> Result<(), ProgramError>;

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

fn validate_selected_market(
    outcome_count: u8,
    root: MarketRoot,
    policy: CategoricalPythPolicyRecordV1,
    feed_profile: PythFeedProfileV1,
) -> Result<usize, ProgramError> {
    let validator = MARKET_VALIDATORS
        .get(outcome_index(outcome_count)?)
        .copied()
        .ok_or(AdapterError::FoundingAuthentication)?;
    validator(root, policy, feed_profile)
}

fn encode_selected_market(
    outcome_count: u8,
    output: &mut [u8],
    root: MarketRoot,
    policy: CategoricalPythPolicyRecordV1,
    feed_profile: PythFeedProfileV1,
) -> Result<(), ProgramError> {
    let encoder = MARKET_ENCODERS
        .get(outcome_index(outcome_count)?)
        .copied()
        .ok_or(AdapterError::FoundingPostcondition)?;
    encoder(output, root, policy, feed_profile)
}

fn validate_market<const N: usize>(
    root: MarketRoot,
    policy: CategoricalPythPolicyRecordV1,
    feed_profile: PythFeedProfileV1,
) -> Result<usize, ProgramError> {
    let outcome_count = u8::try_from(N).map_err(|_| AdapterError::FoundingAuthentication)?;
    let receipt = ResolutionReceiptV1::empty(outcome_count)
        .map_err(|_| AdapterError::FoundingAuthentication)?;
    MarketStateV1::<N>::new(root, policy, feed_profile, 0, [0; N], receipt)
        .map_err(|_| AdapterError::MarketTransition)?;
    MarketStateV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic.into())
}

fn encode_market<const N: usize>(
    output: &mut [u8],
    root: MarketRoot,
    policy: CategoricalPythPolicyRecordV1,
    feed_profile: PythFeedProfileV1,
) -> Result<(), ProgramError> {
    let outcome_count = u8::try_from(N).map_err(|_| AdapterError::FoundingPostcondition)?;
    let receipt = ResolutionReceiptV1::empty(outcome_count)
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    let state = MarketStateV1::<N>::new(root, policy, feed_profile, 0, [0; N], receipt)
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    state
        .encode(output)
        .map_err(|_| AdapterError::FoundingPostcondition)?;
    if MarketStateV1::<N>::decode(output) != Ok(state) {
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
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::ContentId as CoreContentId;
    use dclutch_kernel::resolution::categorical_pyth_v1::{
        CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
    };
    use dclutch_product_contract::{
        capacity::{CapacityEnvelope, CapacityProfileId, CapacityProfileV1Input, ExactWordWidth},
        claim::{CATEGORICAL_UNIT_DENOMINATOR, CategoricalUnitV1Input, RedemptionRounding},
        product::InstanceV1Input,
    };
    use dclutch_pyth_contract::{
        feed_profile::PythFeedProfileV1, policy::CategoricalPythPolicyRecordV1,
        resolution_material::RESOLUTION_MATERIAL_BYTES,
    };
    use dclutch_realm_contract::{
        FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_BYTES, RealmV1Input,
    };
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
                collateral_semantic_id: [1; 32],
                token_program: [2; 32],
                collateral_mint: [3; 32],
                collateral_adapter_release_id: [4; 32],
                mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
                freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
            })
            .expect("valid Realm");
            let realm_bytes = realm.to_bytes();
            let realm_digest = hash(&realm_bytes).to_bytes();
            let (realm_key, _) =
                Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], &program_id);

            let capacity = capacity();
            let capacity_bytes = capacity.to_bytes();
            let capacity_id = product_id(hash(&capacity_bytes).to_bytes());
            let claim = CategoricalUnitV1::new(
                CategoricalUnitV1Input {
                    capacity_profile_id: CapacityProfileId::new(capacity_id),
                    outcome_count: u32::from(outcome_count),
                    payout_denominator: CATEGORICAL_UNIT_DENOMINATOR,
                    rounding: RedemptionRounding::ExactOnly,
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
            let (fund_key, _) =
                Pubkey::find_program_address(&[FUND_SEED, market_key.as_ref()], &program_id);
            let instruction =
                FoundMarketAndFundV1::new(identity, outcome_count).expect("valid instruction");

            let mut accounts = vec![
                leak_account(
                    Pubkey::new_unique(),
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
                    realm_key,
                    false,
                    false,
                    1,
                    realm_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    Pubkey::new_unique(),
                    false,
                    false,
                    1,
                    instance_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    Pubkey::new_unique(),
                    false,
                    false,
                    1,
                    claim_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    Pubkey::new_unique(),
                    false,
                    false,
                    1,
                    capacity_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    Pubkey::new_unique(),
                    false,
                    false,
                    1,
                    material_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    Pubkey::new_unique(),
                    false,
                    false,
                    1,
                    manifest_bytes,
                    program_id,
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
            let rent_account = accounts.get_mut(10).expect("rent account");
            assert_eq!(Rent::default().to_account_info(rent_account), Some(()));
            Self {
                program_id,
                instruction,
                accounts,
            }
        }

        fn authenticate(&self) -> Result<FoundingPlan, ProgramError> {
            let frame = FoundingFrame::parse(&self.accounts)?;
            authenticate_founding(&self.program_id, &frame, self.instruction)
        }
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
            FundingQuoteV1::new(rent, creation, 0, provider, bounty, 0, 0)
                .expect("representable quote"),
        )
        .expect("canonical capability entry")
    }

    fn capacity() -> CapacityProfileV1 {
        CapacityProfileV1::new(CapacityProfileV1Input {
            envelope: CapacityEnvelope::Measured,
            word_width: ExactWordWidth::Eight,
            verifier_release_id: product_id([10; 32]),
            envelope_basis_id: product_id([11; 32]),
            max_artifact_bytes: 128,
            page_payload_bytes: 64,
            max_pages: 2,
            max_partition_cells: 16,
            max_coefficient_entries: 16,
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
            let material_data = frame
                .resolution_material
                .try_borrow_data()
                .expect("material data");
            let material = CategoricalPythResolutionMaterialV1::decode(&material_data)
                .expect("canonical material");
            let root =
                founding_root(plan.identity, frame.sponsor.key.to_bytes()).expect("founding root");
            let mut output = vec![0; market_bytes(outcome_count).expect("market bytes")];
            encode_selected_market(
                outcome_count,
                &mut output,
                root,
                *material.policy(),
                *material.feed_profile(),
            )
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
        let mut claim_data = test_account(&bad_claim.accounts, 5)
            .try_borrow_mut_data()
            .expect("claim data");
        *claim_data.get_mut(12).expect("claim reserved byte") = 1;
        drop(claim_data);
        assert_eq!(
            bad_claim.authenticate().err(),
            Some(ProgramError::from(AdapterError::FoundingAuthentication))
        );

        let wrong_capacity_identity = Fixture::new(2);
        let mut capacity_data = test_account(&wrong_capacity_identity.accounts, 6)
            .try_borrow_mut_data()
            .expect("capacity data");
        *capacity_data.get_mut(16).expect("capacity identity byte") ^= 1;
        drop(capacity_data);
        assert_eq!(
            wrong_capacity_identity.authenticate().err(),
            Some(ProgramError::from(AdapterError::ContentIdentity))
        );

        let wrong_feed_link = Fixture::new(2);
        let mut material_data = test_account(&wrong_feed_link.accounts, 7)
            .try_borrow_mut_data()
            .expect("material data");
        *material_data
            .get_mut(16 + 48)
            .expect("feed-profile link byte") ^= 1;
        drop(material_data);
        assert!(wrong_feed_link.authenticate().is_err());

        let mut trailing_manifest = Fixture::new(2);
        let manifest_key = *test_account(&trailing_manifest.accounts, 8).key;
        let program_id = trailing_manifest.program_id;
        *test_account_mut(&mut trailing_manifest.accounts, 8) = leak_account(
            manifest_key,
            false,
            false,
            1,
            vec![0; dclutch_capability_contract::MANIFEST_HEADER_BYTES + 1],
            program_id,
            false,
        );
        assert_eq!(
            trailing_manifest.authenticate().err(),
            Some(ProgramError::from(AdapterError::FoundingAuthentication))
        );
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
        assert_eq!(plan.provider_fee_reimbursement, 3);
        assert_eq!(plan.resolution_success_bounty, 5);
        assert_eq!(plan.fund_balance, fund_rent + 3 + 5);
    }

    #[test]
    fn existing_state_and_insufficient_atomic_capital_refuse_preflight() {
        let existing_market = Fixture::new(2);
        **test_account(&existing_market.accounts, 1)
            .try_borrow_mut_lamports()
            .expect("market lamports") = 1;
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
            let fund = ResolutionFundV1::decode(&fund_data).expect("canonical Fund");
            assert_eq!(fund.market(), market_key.as_ref());
            assert_eq!(fund.sponsor_refund(), frame.sponsor.key.as_ref());
            assert_eq!(fund.provider_fee_reimbursement(), 3);
            assert_eq!(fund.success_bounty(), 5);
            assert_eq!(
                frame.resolution_material.data_len(),
                RESOLUTION_MATERIAL_BYTES
            );
        }
    }

    #[test]
    fn declared_market_root_width_remains_the_contract_width() {
        assert_eq!(MARKET_ROOT_BYTES, 232);
        assert_eq!(REALM_BYTES, 144);
    }
}
