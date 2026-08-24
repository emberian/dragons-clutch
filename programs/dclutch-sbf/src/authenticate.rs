//! Exact account-frame, replay, funding, and provider authentication.

use dclutch_pyth_contract::{
    funding::{BalanceClassification, ResolutionFundV1},
    market::MarketStateV1,
};
use dclutch_pyth_svm::{
    PostUpdateParamsView, ProgramDataV3View, ProgramV3View, PythReleaseV1, ReceiverConfigV2View,
};
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey, rent::Rent,
    sysvar::Sysvar,
};

use crate::AdapterError;

pub(crate) const FUND_SEED: &[u8] = b"dclutch/resolution-fund/v1";
pub(crate) const MARKET_SEED: &[u8] = b"dclutch/market-root/v1";
const RECEIVER_CONFIG_SEED: &[u8] = b"config";
const RECEIVER_TREASURY_SEED: &[u8] = b"treasury";
const UPGRADEABLE_LOADER: Pubkey = Pubkey::new_from_array([
    2, 168, 246, 145, 78, 136, 161, 176, 226, 16, 21, 62, 247, 99, 174, 43, 0, 194, 185, 61, 22,
    193, 36, 210, 192, 83, 122, 16, 4, 128, 0, 0,
]);

/// Canonical System Program address.
pub(crate) const SYSTEM_PROGRAM: Pubkey = Pubkey::new_from_array([0; 32]);

/// Exact 13-role price-resolution account frame.
pub(crate) struct PriceFrame<'a, 'info> {
    pub(crate) resolver: &'a AccountInfo<'info>,
    pub(crate) update: &'a AccountInfo<'info>,
    pub(crate) market: &'a AccountInfo<'info>,
    pub(crate) fund: &'a AccountInfo<'info>,
    pub(crate) sponsor: &'a AccountInfo<'info>,
    pub(crate) receiver: &'a AccountInfo<'info>,
    pub(crate) receiver_programdata: &'a AccountInfo<'info>,
    pub(crate) config: &'a AccountInfo<'info>,
    pub(crate) encoded_vaa: &'a AccountInfo<'info>,
    pub(crate) router: &'a AccountInfo<'info>,
    pub(crate) router_programdata: &'a AccountInfo<'info>,
    pub(crate) treasury: &'a AccountInfo<'info>,
    pub(crate) system: &'a AccountInfo<'info>,
}

impl<'a, 'info> PriceFrame<'a, 'info> {
    pub(crate) fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != 13 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            resolver: account(accounts, 0)?,
            update: account(accounts, 1)?,
            market: account(accounts, 2)?,
            fund: account(accounts, 3)?,
            sponsor: account(accounts, 4)?,
            receiver: account(accounts, 5)?,
            receiver_programdata: account(accounts, 6)?,
            config: account(accounts, 7)?,
            encoded_vaa: account(accounts, 8)?,
            router: account(accounts, 9)?,
            router_programdata: account(accounts, 10)?,
            treasury: account(accounts, 11)?,
            system: account(accounts, 12)?,
        };
        frame.validate_privileges()?;
        Ok(frame)
    }

    fn validate_privileges(&self) -> Result<(), ProgramError> {
        expect(self.resolver, true, true, false)?;
        expect(self.update, true, true, false)?;
        expect(self.market, false, true, false)?;
        expect(self.fund, false, true, false)?;
        // The immutable sponsor-refund destination never grants authority.
        // A duplicate fee-payer role can legitimately union a signer bit.
        expect_writable_executable(self.sponsor, true, false)?;
        expect(self.receiver, false, false, true)?;
        expect(self.receiver_programdata, false, false, false)?;
        expect(self.config, false, false, false)?;
        expect(self.encoded_vaa, false, false, false)?;
        expect(self.router, false, false, true)?;
        expect(self.router_programdata, false, false, false)?;
        expect(self.treasury, false, true, false)?;
        expect(self.system, false, false, true)?;

        // The two payout roles may alias each other. They may not alias the
        // Fund, persistent Market, or mutable provider accounts.
        if self.fund.key == self.resolver.key
            || self.fund.key == self.sponsor.key
            || self.resolver.key == self.update.key
            || self.resolver.key == self.treasury.key
            || self.update.key == self.treasury.key
            || self.sponsor.key == self.update.key
            || self.sponsor.key == self.market.key
            || self.sponsor.key == self.treasury.key
        {
            return Err(AdapterError::AccountIdentity.into());
        }
        Ok(())
    }
}

/// Exact four-role permissionless failure-resolution account frame.
pub(crate) struct FailureFrame<'a, 'info> {
    pub(crate) bounty_recipient: &'a AccountInfo<'info>,
    pub(crate) market: &'a AccountInfo<'info>,
    pub(crate) fund: &'a AccountInfo<'info>,
    pub(crate) sponsor: &'a AccountInfo<'info>,
}

impl<'a, 'info> FailureFrame<'a, 'info> {
    pub(crate) fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != 4 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            bounty_recipient: account(accounts, 0)?,
            market: account(accounts, 1)?,
            fund: account(accounts, 2)?,
            sponsor: account(accounts, 3)?,
        };
        expect_writable_executable(frame.bounty_recipient, true, false)?;
        expect(frame.market, false, true, false)?;
        expect(frame.fund, false, true, false)?;
        expect_writable_executable(frame.sponsor, true, false)?;
        // Only the two payout roles may alias; neither may alias owned state.
        if frame.fund.key == frame.bounty_recipient.key
            || frame.fund.key == frame.sponsor.key
            || frame.market.key == frame.bounty_recipient.key
            || frame.market.key == frame.sponsor.key
        {
            return Err(AdapterError::AccountIdentity.into());
        }
        Ok(frame)
    }
}

/// Authenticated immutable Market facts needed outside generic outcome dispatch.
#[derive(Clone, Copy)]
pub(crate) struct MarketFacts {
    pub(crate) release_id: [u8; 32],
    pub(crate) provider_feed_id: [u8; 32],
    pub(crate) outcome_count: u8,
}

/// Authenticated immutable Fund and exact live-balance classification.
#[derive(Clone, Copy)]
pub(crate) struct FundFacts {
    pub(crate) fund: ResolutionFundV1,
    pub(crate) classification: BalanceClassification,
    pub(crate) required_rent: u64,
}

/// Exact receiver fee and temporary-account rent expected from `post_update`.
#[derive(Clone, Copy)]
pub(crate) struct ProviderFacts {
    pub(crate) update_rent: u64,
    pub(crate) fee: u64,
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
    let outcomes = data.get(10).copied().ok_or(AdapterError::AccountData)?;
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
    let market = MarketStateV1::<N>::decode(bytes).map_err(|_| AdapterError::AccountData)?;
    let root = market.root();
    if root.identity().generation() != generation || root.outstanding_children() != child_count {
        return Err(AdapterError::ReplayMismatch.into());
    }
    if hash(&market.policy().to_bytes()).to_bytes()
        != root.identity().resolution_policy_id().to_bytes()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    if hash(&market.feed_profile().to_bytes()).to_bytes() != *market.policy().feed_profile_id() {
        return Err(AdapterError::ContentIdentity.into());
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

    // Refuse an unretirable child count before paying the provider.
    let mut next_root = root;
    next_root
        .transition_phase(generation, dclutch_core_contract::Phase::Resolved)
        .map_err(|_| AdapterError::ReplayMismatch)?;
    next_root
        .retire_child(generation, child_count)
        .map_err(|_| AdapterError::ReplayMismatch)?;

    Ok(MarketFacts {
        release_id: *market.policy().release_id(),
        provider_feed_id: *market.feed_profile().provider_feed_id(),
        outcome_count: u8::try_from(N).map_err(|_| AdapterError::AccountData)?,
    })
}

pub(crate) fn authenticate_fund(
    program_id: &Pubkey,
    fund_account: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    sponsor: &AccountInfo<'_>,
    generation: u64,
) -> Result<FundFacts, ProgramError> {
    if fund_account.owner != program_id || fund_account.key == sponsor.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let (expected, _) = Pubkey::find_program_address(&[FUND_SEED, market.key.as_ref()], program_id);
    if fund_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = fund_account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let fund = ResolutionFundV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    if fund.market() != market.key.as_ref()
        || fund.generation() != generation
        || fund.sponsor_refund() != sponsor.key.as_ref()
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let rent = Rent::get().map_err(|_| AdapterError::AccountData)?;
    let required_rent = rent.minimum_balance(data.len());
    let classification = fund
        .classify_balance(fund_account.lamports(), required_rent)
        .map_err(|_| AdapterError::FundUnderfunded)?;
    Ok(FundFacts {
        fund,
        classification,
        required_rent,
    })
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
    if funding.fund.provider_fee_reimbursement() != config.fee() {
        return Err(AdapterError::FundUnderfunded.into());
    }
    let rent = Rent::get().map_err(|_| AdapterError::AccountData)?;
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

fn expect(
    account: &AccountInfo<'_>,
    signer: bool,
    writable: bool,
    executable: bool,
) -> Result<(), ProgramError> {
    if account.is_signer != signer {
        return Err(AdapterError::AccountPrivilege.into());
    }
    expect_writable_executable(account, writable, executable)
}

fn expect_writable_executable(
    account: &AccountInfo<'_>,
    writable: bool,
    executable: bool,
) -> Result<(), ProgramError> {
    if account.is_writable != writable || account.executable != executable {
        return Err(AdapterError::AccountPrivilege.into());
    }
    Ok(())
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

    fn price_accounts() -> [AccountInfo<'static>; 13] {
        [
            test_account(true, true, false),
            test_account(true, true, false),
            test_account(false, true, false),
            test_account(false, true, false),
            test_account(false, true, false),
            test_account(false, false, true),
            test_account(false, false, false),
            test_account(false, false, false),
            test_account(false, false, false),
            test_account(false, false, true),
            test_account(false, false, false),
            test_account(false, true, false),
            test_account(false, false, true),
        ]
    }

    fn failure_accounts(bounty_signer: bool, sponsor_signer: bool) -> [AccountInfo<'static>; 4] {
        [
            test_account(bounty_signer, true, false),
            test_account(false, true, false),
            test_account(false, true, false),
            test_account(sponsor_signer, true, false),
        ]
    }

    #[test]
    fn fund_pda_is_market_bound() {
        let program = Pubkey::new_from_array([3; 32]);
        let market = Pubkey::new_from_array([4; 32]);
        let first = Pubkey::find_program_address(&[FUND_SEED, market.as_ref()], &program);
        let other_market = Pubkey::new_from_array([5; 32]);
        let second = Pubkey::find_program_address(&[FUND_SEED, other_market.as_ref()], &program);
        assert_ne!(first.0, second.0);
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
        writable_receiver[5] = test_account(false, true, true);
        assert_eq!(
            PriceFrame::parse(&writable_receiver).err(),
            Some(AdapterError::AccountPrivilege.into())
        );
    }

    #[test]
    fn price_frame_allows_resolver_sponsor_alias_and_refuses_extras() {
        let mut aliased_sponsor = price_accounts();
        aliased_sponsor[4] = aliased_sponsor[0].clone();
        assert!(PriceFrame::parse(&aliased_sponsor).is_ok());

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
        price[4] = price[3].clone();
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
    fn failure_frame_is_permissionless_and_allows_destination_alias() {
        assert!(FailureFrame::parse(&failure_accounts(false, false)).is_ok());
        assert!(FailureFrame::parse(&failure_accounts(true, true)).is_ok());
        let mut aliased = failure_accounts(false, false);
        aliased[3] = aliased[0].clone();
        assert!(FailureFrame::parse(&aliased).is_ok());
    }
}
