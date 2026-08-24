//! Hostile account authentication for the current Product lifecycle spine.
//!
//! These receipts decode only `0xaa/v3` and `0xad/v3`. They deliberately do
//! not expose mutation: bounded action15 stages and family-specific atomic
//! composers consume the receipts through narrower private owners.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_product_series::{
    ContentId, MarketInstanceV2Id, MarketLifecycleBindingV3, MarketLifecycleRootV3,
    SeriesMarketLinkBindingV3, SeriesMarketLinkV3, SeriesMarketLinkV3Id, SeriesPlanV5Id,
    MARKET_LIFECYCLE_ROOT_DOMAIN_V3, SERIES_MARKET_LINK_DOMAIN_V3,
};
use clutch_solana_layout::product_series::{
    series_market_link_authentication_id_v3, MarketLifecycleRootAccountV3,
    SeriesMarketLinkAccountV3, MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V3,
    SERIES_MARKET_LINK_ACCOUNT_BYTES_V3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const MARKET_LIFECYCLE_ROOT_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/market-lifecycle-root-authentication/v3\0";

/// Move-only authentication of one exact current `0xaa/v3` account.
#[derive(Debug)]
pub(crate) struct AuthenticatedMarketLifecycleRootV3<'state> {
    account: Pubkey,
    owner_program: Pubkey,
    value: &'state MarketLifecycleRootAccountV3,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    semantic_id: ContentId,
    binding_id: ContentId,
    authentication_id: ContentId,
}

impl<'state> AuthenticatedMarketLifecycleRootV3<'state> {
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn owner_program(&self) -> Pubkey { self.owner_program }
    pub(crate) const fn value(&self) -> &MarketLifecycleRootAccountV3 { &self.value }
    pub(crate) const fn state(&self) -> &MarketLifecycleRootV3 { &self.value.state }
    pub(crate) const fn binding(&self) -> &MarketLifecycleBindingV3 {
        self.value.state.binding_ref()
    }
    pub(crate) const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(&self) -> bool { self.writable }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn semantic_id(&self) -> ContentId { self.semantic_id }
    pub(crate) const fn binding_id(&self) -> ContentId { self.binding_id }
    pub(crate) const fn authentication_id(&self) -> ContentId { self.authentication_id }
}

/// Move-only authentication of one exact current `0xad/v3` account.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesMarketLinkV3<'state> {
    account: Pubkey,
    owner_program: Pubkey,
    value: &'state SeriesMarketLinkAccountV3,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    semantic_id: SeriesMarketLinkV3Id,
    binding_id: ContentId,
    authentication_id: ContentId,
}

impl<'state> AuthenticatedSeriesMarketLinkV3<'state> {
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn owner_program(&self) -> Pubkey { self.owner_program }
    pub(crate) const fn value(&self) -> &SeriesMarketLinkAccountV3 { &self.value }
    pub(crate) const fn state(&self) -> &SeriesMarketLinkV3 { &self.value.state }
    pub(crate) const fn binding(&self) -> &SeriesMarketLinkBindingV3 {
        self.value.state.binding_ref()
    }
    pub(crate) const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(&self) -> bool { self.writable }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn semantic_id(&self) -> SeriesMarketLinkV3Id { self.semantic_id }
    pub(crate) const fn binding_id(&self) -> ContentId { self.binding_id }
    pub(crate) const fn authentication_id(&self) -> ContentId { self.authentication_id }
}

/// Hostile-authenticate the sole current shared Product lifecycle root.
#[inline(never)]
pub(crate) fn authenticate_market_lifecycle_root_v3<'state>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    require_writable: bool,
    output: &'state mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedMarketLifecycleRootV3<'state>> {
    require(
        !account.is_signer
            && !account.executable
            && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V3,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV3::decode_into(&data, output)?;
    let value = &*output;
    let binding = value.state.binding_ref();
    require(
        expected_generation != 0
            && binding.market_instance_id == expected_market_instance_id
            && binding.generation == expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::product_market_lifecycle_root_pda(
        program_id,
        &expected_market_instance_id.bytes(),
        expected_generation,
    );
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let observed_lamports = account.lamports();
    require(
        observed_lamports >= value.rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let data_id = hash_data(&data);
    let semantic_id = hashv(&[
        MARKET_LIFECYCLE_ROOT_DOMAIN_V3,
        &data[clutch_solana_layout::product_series::PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1..],
    ]);
    drop(data);
    let binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = hashv(&[
        MARKET_LIFECYCLE_ROOT_AUTHENTICATION_DOMAIN_V3,
        account.key.as_ref(),
        program_id.as_ref(),
        &data_id.bytes(),
        &semantic_id.bytes(),
        &binding_id.bytes(),
        &observed_lamports.to_le_bytes(),
        &value.rent_principal_lamports.to_le_bytes(),
        &[value.stored_bump, u8::from(require_writable)],
    ]);
    require_live(authentication_id)?;
    Ok(AuthenticatedMarketLifecycleRootV3 {
        account: *account.key,
        owner_program: *program_id,
        value,
        observed_lamports,
        writable: require_writable,
        data_id,
        semantic_id,
        binding_id,
        authentication_id,
    })
}

/// Hostile-authenticate one current RootV3-bound per-Series link.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn authenticate_series_market_link_v3<'state>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    expected_ordinal: u32,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    expected_market_root: Pubkey,
    require_writable: bool,
    output: &'state mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedSeriesMarketLinkV3<'state>> {
    require(
        !account.is_signer
            && !account.executable
            && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == SERIES_MARKET_LINK_ACCOUNT_BYTES_V3,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV3::decode_into(&data, output)?;
    let value = &*output;
    let binding = value.state.binding_ref();
    require(
        expected_generation != 0
            && binding.series_plan_id == expected_series_plan_id
            && binding.ordinal == expected_ordinal
            && binding.market_instance_id == expected_market_instance_id
            && binding.generation == expected_generation
            && binding.market_root_account_id.bytes() == expected_market_root.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::product_series_market_link_pda(
        program_id,
        &expected_series_plan_id.bytes(),
        expected_ordinal,
    );
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let accounted_lamports = value
        .state
        .rent_principal_lamports()
        .checked_add(value.state.current_donation_lamports())
        .ok_or(ClutchError::Arithmetic)?;
    let observed_lamports = account.lamports();
    require(observed_lamports >= accounted_lamports, ClutchError::MismatchedState)?;
    let data_id = hash_data(&data);
    let semantic_id = SeriesMarketLinkV3Id::from_bytes(hashv(&[
        SERIES_MARKET_LINK_DOMAIN_V3,
        &data[clutch_solana_layout::product_series::PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1..],
    ]).bytes());
    drop(data);
    let binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = ContentId::from_bytes(series_market_link_authentication_id_v3(
        account.key.to_bytes(),
        program_id.to_bytes(),
        data_id.bytes(),
        semantic_id.bytes(),
        expected_market_root.to_bytes(),
        observed_lamports,
    ).0);
    require_live(authentication_id)?;
    Ok(AuthenticatedSeriesMarketLinkV3 {
        account: *account.key,
        owner_program: *program_id,
        value,
        observed_lamports,
        writable: require_writable,
        data_id,
        semantic_id,
        binding_id,
        authentication_id,
    })
}

fn hash_data(data: &[u8]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(&[data]).to_bytes())
}

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}
