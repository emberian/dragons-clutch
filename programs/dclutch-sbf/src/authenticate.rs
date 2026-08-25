//! Exact account-frame, replay, funding, and provider authentication.

use alloc::boxed::Box;

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, FundingCustodyObservationV1,
};
use dclutch_core_contract::ContentId;
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_pyth_contract::{
    frame::{
        ResolutionAccountPrivilegeV1, ResolutionFrameErrorV1, validate_failure_resolution_frame_v1,
        validate_price_resolution_frame_v1,
    },
    funding::{
        FundingStateV1, required_resolution_minimum_balance, validate_required_resolution_funding,
    },
};
use dclutch_pyth_svm::{
    PostUpdateParamsView, ProgramDataV3View, ProgramV3View, PythReleaseV1, ReceiverConfigV2View,
};
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
};
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey, rent::Rent,
    sysvar::SysvarSerialize,
};

use crate::{
    AdapterError,
    records::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, with_authenticated_finalized_record_v1},
};
use dclutch_product_contract::result_domain::FiniteResultDomainV1;
use dclutch_source_contract::{
    ContentId as SourceContentId, ProviderReleaseV1, PythProviderAdapterObligationV1,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SourceAccessProfile, SourceMaterialViewV1, SourceSpecV1,
    WindowSpecV1,
};

pub(crate) const MARKET_SEED: &[u8] = b"dclutch/market-root/v1";
const RECEIVER_CONFIG_SEED: &[u8] = b"config";
const RECEIVER_TREASURY_SEED: &[u8] = b"treasury";
const UPGRADEABLE_LOADER: Pubkey = Pubkey::new_from_array([
    2, 168, 246, 145, 78, 136, 161, 176, 226, 16, 21, 62, 247, 99, 174, 43, 0, 194, 185, 61, 22,
    193, 36, 210, 192, 83, 122, 16, 4, 128, 0, 0,
]);

/// Canonical System Program address.
pub(crate) const SYSTEM_PROGRAM: Pubkey = Pubkey::new_from_array([0; 32]);

/// Exact 18-role price-resolution account frame.
pub(crate) struct PriceFrame<'a, 'info> {
    pub(crate) resolver: &'a AccountInfo<'info>,
    pub(crate) update: &'a AccountInfo<'info>,
    pub(crate) market: &'a AccountInfo<'info>,
    pub(crate) fund: &'a AccountInfo<'info>,
    pub(crate) material: &'a AccountInfo<'info>,
    pub(crate) manifest: &'a AccountInfo<'info>,
    pub(crate) rent_credit: &'a AccountInfo<'info>,
    pub(crate) receiver: &'a AccountInfo<'info>,
    pub(crate) receiver_programdata: &'a AccountInfo<'info>,
    pub(crate) config: &'a AccountInfo<'info>,
    pub(crate) encoded_vaa: &'a AccountInfo<'info>,
    pub(crate) router: &'a AccountInfo<'info>,
    pub(crate) router_programdata: &'a AccountInfo<'info>,
    pub(crate) treasury: &'a AccountInfo<'info>,
    pub(crate) material_staging_cursor: &'a AccountInfo<'info>,
    pub(crate) manifest_staging_cursor: &'a AccountInfo<'info>,
    pub(crate) system: &'a AccountInfo<'info>,
    pub(crate) rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> PriceFrame<'a, 'info> {
    pub(crate) fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != 18 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            resolver: account(accounts, 0)?,
            update: account(accounts, 1)?,
            market: account(accounts, 2)?,
            fund: account(accounts, 3)?,
            material: account(accounts, 4)?,
            manifest: account(accounts, 5)?,
            rent_credit: account(accounts, 6)?,
            receiver: account(accounts, 7)?,
            receiver_programdata: account(accounts, 8)?,
            config: account(accounts, 9)?,
            encoded_vaa: account(accounts, 10)?,
            router: account(accounts, 11)?,
            router_programdata: account(accounts, 12)?,
            treasury: account(accounts, 13)?,
            material_staging_cursor: account(accounts, 14)?,
            manifest_staging_cursor: account(accounts, 15)?,
            system: account(accounts, 16)?,
            rent_sysvar: account(accounts, 17)?,
        };
        frame.validate_privileges()?;
        Ok(frame)
    }

    fn validate_privileges(&self) -> Result<(), ProgramError> {
        let privileges = [
            resolution_privilege(self.resolver),
            resolution_privilege(self.update),
            resolution_privilege(self.market),
            resolution_privilege(self.fund),
            resolution_privilege(self.material),
            resolution_privilege(self.manifest),
            resolution_privilege(self.rent_credit),
            resolution_privilege(self.receiver),
            resolution_privilege(self.receiver_programdata),
            resolution_privilege(self.config),
            resolution_privilege(self.encoded_vaa),
            resolution_privilege(self.router),
            resolution_privilege(self.router_programdata),
            resolution_privilege(self.treasury),
            resolution_privilege(self.material_staging_cursor),
            resolution_privilege(self.manifest_staging_cursor),
            resolution_privilege(self.system),
            resolution_privilege(self.rent_sysvar),
        ];
        validate_price_resolution_frame_v1(&privileges).map_err(map_resolution_frame_error)
    }
}

/// Exact nine-role permissionless failure-resolution account frame.
pub(crate) struct FailureFrame<'a, 'info> {
    pub(crate) bounty_recipient: &'a AccountInfo<'info>,
    pub(crate) market: &'a AccountInfo<'info>,
    pub(crate) fund: &'a AccountInfo<'info>,
    pub(crate) material: &'a AccountInfo<'info>,
    pub(crate) manifest: &'a AccountInfo<'info>,
    pub(crate) rent_credit: &'a AccountInfo<'info>,
    pub(crate) material_staging_cursor: &'a AccountInfo<'info>,
    pub(crate) manifest_staging_cursor: &'a AccountInfo<'info>,
    pub(crate) rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> FailureFrame<'a, 'info> {
    pub(crate) fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != 9 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            bounty_recipient: account(accounts, 0)?,
            market: account(accounts, 1)?,
            fund: account(accounts, 2)?,
            material: account(accounts, 3)?,
            manifest: account(accounts, 4)?,
            rent_credit: account(accounts, 5)?,
            material_staging_cursor: account(accounts, 6)?,
            manifest_staging_cursor: account(accounts, 7)?,
            rent_sysvar: account(accounts, 8)?,
        };
        let privileges = [
            resolution_privilege(frame.bounty_recipient),
            resolution_privilege(frame.market),
            resolution_privilege(frame.fund),
            resolution_privilege(frame.material),
            resolution_privilege(frame.manifest),
            resolution_privilege(frame.rent_credit),
            resolution_privilege(frame.material_staging_cursor),
            resolution_privilege(frame.manifest_staging_cursor),
            resolution_privilege(frame.rent_sysvar),
        ];
        validate_failure_resolution_frame_v1(&privileges).map_err(map_resolution_frame_error)?;
        Ok(frame)
    }
}

/// Authenticated immutable Market facts needed outside generic outcome dispatch.
#[derive(Clone, Copy)]
pub(crate) struct MarketFacts {
    pub(crate) outcome_count: u8,
    pub(crate) resolution_policy_id: ContentId,
    pub(crate) capability_manifest_id: ContentId,
    pub(crate) generation: u64,
    pub(crate) rent_refund: [u8; 32],
}

/// Authenticated immutable Fund and exact live-balance classification.
#[derive(Clone, Copy)]
pub(crate) struct FundFacts {
    pub(crate) funding: FundingStateV1,
    pub(crate) required_rent: u64,
    pub(crate) rent_credit: RentCreditV1,
    pub(crate) credit_excess: u64,
}

/// Exact receiver fee and temporary-account rent expected from `post_update`.
#[derive(Clone, Copy)]
pub(crate) struct ProviderFacts {
    pub(crate) update_rent: u64,
    pub(crate) fee: u64,
}

/// Compact Pyth-relevant facts selected from the canonical Source authority.
#[derive(Clone, Copy)]
pub(crate) struct SourceMaterialFacts {
    pub(crate) obligation: PythProviderAdapterObligationV1,
    pub(crate) result_domain: FiniteResultDomainV1,
    pub(crate) window: WindowSpecV1,
    pub(crate) source_id: SourceContentId,
    pub(crate) source: SourceSpecV1,
    pub(crate) provider_release_id: SourceContentId,
    pub(crate) provider_release: ProviderReleaseV1,
}

#[inline(never)]
pub(crate) fn authenticate_market(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    generation: u64,
    child_count: u64,
) -> Result<MarketFacts, ProgramError> {
    if market.owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = market
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let outcomes = decode_market_outcome_count(&data).map_err(|_| AdapterError::AccountData)?;
    match outcomes {
        2 => market_facts::<2>(program_id, market.key, &data, generation, child_count),
        3 => market_facts::<3>(program_id, market.key, &data, generation, child_count),
        4 => market_facts::<4>(program_id, market.key, &data, generation, child_count),
        5 => market_facts::<5>(program_id, market.key, &data, generation, child_count),
        6 => market_facts::<6>(program_id, market.key, &data, generation, child_count),
        7 => market_facts::<7>(program_id, market.key, &data, generation, child_count),
        8 => market_facts::<8>(program_id, market.key, &data, generation, child_count),
        9 => market_facts::<9>(program_id, market.key, &data, generation, child_count),
        10 => market_facts::<10>(program_id, market.key, &data, generation, child_count),
        11 => market_facts::<11>(program_id, market.key, &data, generation, child_count),
        12 => market_facts::<12>(program_id, market.key, &data, generation, child_count),
        13 => market_facts::<13>(program_id, market.key, &data, generation, child_count),
        14 => market_facts::<14>(program_id, market.key, &data, generation, child_count),
        15 => market_facts::<15>(program_id, market.key, &data, generation, child_count),
        16 => market_facts::<16>(program_id, market.key, &data, generation, child_count),
        _ => Err(AdapterError::AccountData.into()),
    }
}

#[inline(never)]
fn market_facts<const N: usize>(
    program_id: &Pubkey,
    market_key: &Pubkey,
    bytes: &[u8],
    generation: u64,
    child_count: u64,
) -> Result<MarketFacts, ProgramError> {
    let market = CategoricalMarketV1::<N>::decode(bytes).map_err(|_| AdapterError::AccountData)?;
    let root = market.root();
    if root.identity().generation() != generation || root.outstanding_children() != child_count {
        return Err(AdapterError::ReplayMismatch.into());
    }
    if root.phase() != dclutch_core_contract::Phase::Open {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected_market, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], program_id);
    if market_key != &expected_market {
        return Err(AdapterError::AccountIdentity.into());
    }

    Ok(MarketFacts {
        outcome_count: u8::try_from(N).map_err(|_| AdapterError::AccountData)?,
        resolution_policy_id: root.identity().resolution_policy_id(),
        capability_manifest_id: root.identity().capability_manifest_id(),
        generation: root.identity().generation(),
        rent_refund: root.rent_refund(),
    })
}

#[allow(clippy::too_many_arguments)] // Exact raw/cursor/Rent finality witnesses are independent frame roles.
pub(crate) fn authenticate_fund<'info>(
    program_id: &Pubkey,
    fund_account: &AccountInfo<'info>,
    market: &AccountInfo<'info>,
    material_account: &AccountInfo<'info>,
    manifest_account: &AccountInfo<'info>,
    material_staging_cursor: &AccountInfo<'info>,
    manifest_staging_cursor: &AccountInfo<'info>,
    rent_credit_account: &AccountInfo<'info>,
    rent_sysvar: &AccountInfo<'info>,
    market_facts: MarketFacts,
) -> Result<(FundFacts, Box<SourceMaterialFacts>), ProgramError> {
    if fund_account.owner != program_id || fund_account.key == rent_credit_account.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let material = with_authenticated_finalized_record_v1(
        program_id,
        material_account,
        material_staging_cursor,
        rent_sysvar,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        market_facts.resolution_policy_id.to_bytes(),
        |record| {
            let material = SourceMaterialViewV1::decode(record.exact_content())
                .map_err(|_| AdapterError::AccountData)?;
            let (source_id, source) = material
                .primary_source()
                .map_err(|_| AdapterError::ContentIdentity)?;
            let window = material
                .window()
                .map_err(|_| AdapterError::ContentIdentity)?;
            let domain = material
                .result_domain()
                .map_err(|_| AdapterError::ContentIdentity)?;
            if source.access_profile() != SourceAccessProfile::PythTerminalOneTransaction
                || domain.outcome_count() != market_facts.outcome_count
            {
                return Err(AdapterError::ContentIdentity.into());
            }
            let obligation =
                PythProviderAdapterObligationV1::from_material_view(material, source_id)
                    .map_err(|_| AdapterError::ReleaseUnavailable)?;
            let (provider_release_id, provider_release) = material
                .primary_provider_release()
                .map_err(|_| AdapterError::ContentIdentity)?;
            Ok(Box::new(SourceMaterialFacts {
                obligation,
                result_domain: domain,
                window,
                source_id,
                source,
                provider_release_id,
                provider_release,
            }))
        },
    )?;

    let data = fund_account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let funding = FundingStateV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    let (required_rent, credit_excess) = with_authenticated_finalized_record_v1(
        program_id,
        manifest_account,
        manifest_staging_cursor,
        rent_sysvar,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        market_facts.capability_manifest_id.to_bytes(),
        |record| {
            let manifest = CapabilityManifestV1::decode(record.exact_content())
                .map_err(|_| AdapterError::AccountData)?;
            let selected = manifest
                .required_founding_entry_for_config(market_facts.resolution_policy_id)
                .map_err(|_| AdapterError::AccountData)?;
            if selected.entry().release_id().to_bytes()
                != material
                    .obligation
                    .provider_release()
                    .adapter_release_id()
                    .to_bytes()
            {
                return Err(AdapterError::ContentIdentity.into());
            }
            let derivation = CapabilityFundingDerivationV1::new(
                market.key.to_bytes(),
                market_facts.generation,
                market_facts.capability_manifest_id,
                manifest,
                funding,
            )
            .map_err(|_| AdapterError::AccountIdentity)?;
            let (expected, _) =
                Pubkey::find_program_address(&derivation.seed_components(), program_id);
            if fund_account.key != &expected {
                return Err(AdapterError::AccountIdentity.into());
            }
            let rent =
                Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
            let required_rent = rent.minimum_balance(data.len());
            let custody = FundingCustodyObservationV1::native_only(
                fund_account
                    .try_lamports()
                    .map_err(|_| AdapterError::AccountData)?,
                required_rent,
            )
            .map_err(|_| AdapterError::FundUnderfunded)?;
            let minimum = required_resolution_minimum_balance(funding)
                .map_err(|_| AdapterError::FundUnderfunded)?;
            let credit_excess = fund_account
                .lamports()
                .checked_sub(minimum)
                .ok_or(AdapterError::FundUnderfunded)?;
            validate_required_resolution_funding(
                funding,
                market_facts.capability_manifest_id,
                manifest,
                selected,
                required_rent,
                custody,
            )
            .map_err(|_| AdapterError::FundUnderfunded)?;
            Ok((required_rent, credit_excess))
        },
    )?;
    let rent_credit = authenticate_rent_credit(
        program_id,
        rent_credit_account,
        Pubkey::new_from_array(market_facts.rent_refund),
    )?;
    Ok((
        FundFacts {
            funding,
            required_rent,
            rent_credit,
            credit_excess,
        },
        material,
    ))
}

#[inline(never)]
pub(crate) fn authenticate_provider(
    frame: &PriceFrame<'_, '_>,
    release: PythReleaseV1,
    body: &[u8],
    funding: FundFacts,
) -> Result<ProviderFacts, ProgramError> {
    let receiver = Pubkey::new_from_array(release.receiver_program());
    let router = Pubkey::new_from_array(release.router_program());
    let (canonical_config, _) = Pubkey::find_program_address(&[RECEIVER_CONFIG_SEED], &receiver);
    if frame.update.owner != &SYSTEM_PROGRAM
        || !frame.update.data_is_empty()
        || frame.update.lamports() != 0
        || release.receiver_config() != canonical_config.to_bytes()
        || frame.config.owner != &receiver
        || frame.encoded_vaa.owner != &router
        || frame.receiver.key != &receiver
        || frame.receiver_programdata.key.to_bytes() != release.receiver_programdata()
        || frame.config.key != &canonical_config
        || frame.router.key != &router
        || frame.router_programdata.key.to_bytes() != release.router_programdata()
    {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    if frame.receiver.owner != &UPGRADEABLE_LOADER
        || frame.receiver_programdata.owner != &UPGRADEABLE_LOADER
        || frame.router.owner != &UPGRADEABLE_LOADER
        || frame.router_programdata.owner != &UPGRADEABLE_LOADER
    {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    authenticate_loader_link(
        frame.receiver,
        frame.receiver_programdata,
        release.receiver_programdata(),
        release.receiver_deployment_slot(),
    )?;
    authenticate_loader_link(
        frame.router,
        frame.router_programdata,
        release.router_programdata(),
        release.router_deployment_slot(),
    )?;
    let config_data = frame
        .config
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let config =
        ReceiverConfigV2View::parse(&config_data).map_err(|_| AdapterError::AccountData)?;
    if hash(&config_data).to_bytes() != release.config_digest()
        || config.router_program() != release.router_program()
    {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    let params = PostUpdateParamsView::parse(body).map_err(|_| AdapterError::AccountData)?;
    let treasury_id = [params.treasury_id()];
    let (expected_treasury, _) =
        Pubkey::find_program_address(&[RECEIVER_TREASURY_SEED, &treasury_id], &receiver);
    if frame.treasury.key != &expected_treasury {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    if funding.funding.remaining().provider().amount() != config.fee() {
        return Err(AdapterError::FundUnderfunded.into());
    }
    let rent = Rent::from_account_info(frame.rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    let update_rent = rent.minimum_balance(dclutch_pyth_svm::FULL_PRICE_UPDATE_V2_LEN);
    let required_resolver_lamports = update_rent
        .checked_add(config.fee())
        .ok_or(AdapterError::Arithmetic)?;
    if frame.resolver.owner != &SYSTEM_PROGRAM
        || !frame.resolver.data_is_empty()
        || frame.system.key != &SYSTEM_PROGRAM
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    if frame.resolver.lamports() < required_resolver_lamports {
        return Err(AdapterError::FundUnderfunded.into());
    }
    Ok(ProviderFacts {
        update_rent,
        fee: config.fee(),
    })
}

fn authenticate_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    beneficiary: Pubkey,
) -> Result<RentCreditV1, ProgramError> {
    let authority =
        RefundAuthority::new(beneficiary.to_bytes()).map_err(|_| AdapterError::AccountIdentity)?;
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
        .map_err(|_| AdapterError::AccountData)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    credit
        .validate_binding(authority, bump)
        .map_err(|_| AdapterError::AccountIdentity)?;
    if credit.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::AccountData.into());
    }
    Ok(credit)
}

fn authenticate_loader_link(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    expected_programdata: [u8; 32],
    expected_slot: u64,
) -> Result<(), ProgramError> {
    let program_data = program
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let view = ProgramV3View::parse(&program_data).map_err(|_| AdapterError::AccountData)?;
    let (derived_programdata, _) =
        Pubkey::find_program_address(&[program.key.as_ref()], &UPGRADEABLE_LOADER);
    if view.programdata_key() != expected_programdata
        || programdata.key.to_bytes() != expected_programdata
        || programdata.key != &derived_programdata
    {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    let programdata_data = programdata
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let view =
        ProgramDataV3View::parse(&programdata_data).map_err(|_| AdapterError::AccountData)?;
    if view.deployment_slot() != expected_slot {
        return Err(AdapterError::ProviderAuthentication.into());
    }
    Ok(())
}

pub(crate) fn selected_release(
    release_id: [u8; 32],
    clock_time: i64,
) -> Result<PythReleaseV1, ProgramError> {
    for release in &dclutch_pyth_svm::PRODUCTION_RELEASES {
        if hash(&release.to_bytes()).to_bytes() == release_id
            && clock_time >= release.activation_time()
        {
            return Ok(*release);
        }
    }
    #[cfg(feature = "non-production-real-pyth-lab")]
    {
        let release = crate::synthetic_release::release()?;
        if hash(&release.to_bytes()).to_bytes() == release_id
            && clock_time >= release.activation_time()
        {
            return Ok(release);
        }
    }
    Err(AdapterError::ReleaseUnavailable.into())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(AdapterError::AccountFrameLength.into())
}

fn resolution_privilege(account: &AccountInfo<'_>) -> ResolutionAccountPrivilegeV1 {
    ResolutionAccountPrivilegeV1 {
        key: account.key.to_bytes(),
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        is_executable: account.executable,
    }
}

fn map_resolution_frame_error(error: ResolutionFrameErrorV1) -> ProgramError {
    match error {
        ResolutionFrameErrorV1::InvalidAccountCount => AdapterError::AccountFrameLength.into(),
        ResolutionFrameErrorV1::UnsafeAlias => AdapterError::AccountIdentity.into(),
        ResolutionFrameErrorV1::InsufficientPrivilege
        | ResolutionFrameErrorV1::UnexpectedPrivilege
        | ResolutionFrameErrorV1::InvalidExecutablePrivilege => {
            AdapterError::AccountPrivilege.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{boxed::Box, vec::Vec};

    fn test_account(signer: bool, writable: bool, executable: bool) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0_u64));
        let data: &'static mut [u8] = Box::leak(Vec::<u8>::new().into_boxed_slice());
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        AccountInfo::new(key, signer, writable, lamports, data, owner, executable)
    }

    fn price_accounts() -> [AccountInfo<'static>; 18] {
        [
            test_account(true, true, false),
            test_account(true, true, false),
            test_account(false, true, false),
            test_account(false, true, false),
            test_account(false, false, false),
            test_account(false, false, false),
            test_account(false, true, false),
            test_account(false, false, true),
            test_account(false, false, false),
            test_account(false, false, false),
            test_account(false, false, false),
            test_account(false, false, true),
            test_account(false, false, false),
            test_account(false, true, false),
            test_account(false, false, false),
            test_account(false, false, false),
            test_account(false, false, true),
            test_account(false, false, false),
        ]
    }

    fn failure_accounts(bounty_signer: bool, sponsor_signer: bool) -> [AccountInfo<'static>; 9] {
        [
            test_account(bounty_signer, true, false),
            test_account(false, true, false),
            test_account(false, true, false),
            test_account(false, false, false),
            test_account(false, false, false),
            test_account(sponsor_signer, true, false),
            test_account(false, false, false),
            test_account(false, false, false),
            test_account(false, false, false),
        ]
    }

    #[test]
    fn production_catalog_does_not_select_a_release() {
        assert_eq!(dclutch_pyth_svm::PRODUCTION_RELEASES.len(), 0);
    }

    #[test]
    fn price_frame_refuses_missing_update_signer_and_writable_provider() {
        let mut missing_update_signature = price_accounts();
        missing_update_signature[1] = test_account(false, true, false);
        assert_eq!(
            PriceFrame::parse(&missing_update_signature).err(),
            Some(AdapterError::AccountPrivilege.into())
        );

        let mut writable_receiver = price_accounts();
        writable_receiver[7] = test_account(false, true, true);
        assert_eq!(
            PriceFrame::parse(&writable_receiver).err(),
            Some(AdapterError::AccountPrivilege.into())
        );
    }

    #[test]
    fn price_frame_refuses_rent_credit_alias_and_extras() {
        let mut aliased_sponsor = price_accounts();
        aliased_sponsor[6] = aliased_sponsor[0].clone();
        assert_eq!(
            PriceFrame::parse(&aliased_sponsor).err(),
            Some(AdapterError::AccountPrivilege.into())
        );

        let mut extras = Vec::from(price_accounts());
        extras.push(test_account(false, false, false));
        assert_eq!(
            PriceFrame::parse(&extras).err(),
            Some(AdapterError::AccountFrameLength.into())
        );
    }

    #[test]
    fn price_and_failure_frames_refuse_a_self_refunding_fund() {
        let mut price = price_accounts();
        price[6] = price[3].clone();
        assert_eq!(
            PriceFrame::parse(&price).err(),
            Some(AdapterError::AccountIdentity.into())
        );

        let mut failure = failure_accounts(false, false);
        failure[0] = failure[2].clone();
        assert_eq!(
            FailureFrame::parse(&failure).err(),
            Some(AdapterError::AccountIdentity.into())
        );
    }

    #[test]
    fn failure_frame_is_permissionless_but_rent_credit_is_not_a_destination_alias() {
        assert!(FailureFrame::parse(&failure_accounts(false, false)).is_ok());
        assert!(FailureFrame::parse(&failure_accounts(true, false)).is_ok());
        assert_eq!(
            FailureFrame::parse(&failure_accounts(true, true)).err(),
            Some(AdapterError::AccountPrivilege.into())
        );
        let mut aliased = failure_accounts(false, false);
        aliased[5] = aliased[0].clone();
        assert_eq!(
            FailureFrame::parse(&aliased).err(),
            Some(AdapterError::AccountIdentity.into())
        );
    }
}
