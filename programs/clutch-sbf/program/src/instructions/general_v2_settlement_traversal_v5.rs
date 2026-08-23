//! Single SBF authentication owner for General V5 settlement traversal.
//!
//! The returned value exists only after the adapter has authenticated the
//! exact retained Feed, complete canonical V5 page set, MarketBinding V2,
//! MarketRuntime, EconomicDomain, PriceGrid, Product/Genesis artifacts, and
//! Realm-selected collateral profile. It remains structural: a mutable root,
//! receipt, reservation, Position, or Replay must be authenticated separately
//! by the action-specific composer.

use core::cell::Ref;

use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, BoundCollateralProfileV2, Id as CollateralId,
    MarketCollateralBindingV2,
};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{CandidateFeedHeaderV2, Id32, MarketBindingV2};
use clutch_general_v2_runtime::{
    bind_settlement_root_traversal_v4, derive_settlement_traversal_projection_v4,
    project_owner_blind_book_costed_v1, GeneralOrderPageInputV5,
    SettlementTraversalProjectionV4,
};
use clutch_product_series::{ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2};
use clutch_solana_layout::order_page_v5::{verify_page_v5, ORDER_PAGE_V5_BYTES};
use clutch_solana_layout::{account_len, PriceGridAccount, MAX_ORDER_PAGES};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::seeds;

use super::collateral_position_v3::authenticate_general_market_v2;
use super::general_v2_settlement_root::{
    authenticate_readonly_general_settlement_root_v1,
    authenticate_writable_general_settlement_root_v1, AuthenticatedGeneralSettlementRootV1,
};
use super::product_artifact::authenticate_product_artifact_v1;

/// Named immutable account frame shared by settlement-root creation and every
/// later action that must reproduce the candidate-wide V5 traversal.
#[derive(Clone, Copy, Debug)]
pub struct SettlementTraversalAccountFrameV5<'a, 'info> {
    /// Counted retained CandidateFeed V2 account.
    pub retained_feed: &'a AccountInfo<'info>,
    /// Immutable MarketBinding V2 PDA.
    pub market_binding: &'a AccountInfo<'info>,
    /// Stable MarketRuntime V3 PDA.
    pub market_runtime: &'a AccountInfo<'info>,
    /// Canonical EconomicDomain V2 PDA.
    pub economic_domain: &'a AccountInfo<'info>,
    /// Canonical PriceGrid PDA.
    pub price_grid: &'a AccountInfo<'info>,
    /// Immutable Realm account.
    pub realm: &'a AccountInfo<'info>,
    /// Realm-selected collateral Profile V2.
    pub profile: &'a AccountInfo<'info>,
    /// Profile-selected collateral-policy artifact.
    pub collateral_policy: &'a AccountInfo<'info>,
    /// Canonical Token-2022 program.
    pub token_program: &'a AccountInfo<'info>,
    /// Exact content-addressed Product MarketInstance V2 artifact.
    pub market_instance: &'a AccountInfo<'info>,
    /// Exact content-addressed MarketGenesisProfile V2 artifact.
    pub market_genesis: &'a AccountInfo<'info>,
    /// Complete one-to-four canonical OrderPage V5 accounts in page order.
    pub pages: &'a [AccountInfo<'info>],
}

/// Program-authenticated immutable traversal facts.
#[derive(Debug)]
pub struct AuthenticatedSettlementTraversalV5 {
    market: MarketBindingV2,
    collateral: BoundCollateralProfileV2,
    genesis: MarketGenesisProfileV2,
    feed_account: Id32,
    feed: CandidateFeedHeaderV2,
    traversal: Box<SettlementTraversalProjectionV4>,
}

impl AuthenticatedSettlementTraversalV5 {
    /// Exact MarketBinding V2 body.
    pub const fn market(&self) -> &MarketBindingV2 {
        &self.market
    }

    /// Exact Realm-selected collateral binding used by the traversal.
    pub const fn collateral(&self) -> BoundCollateralProfileV2 {
        self.collateral
    }

    /// Exact authenticated Genesis V2 body.
    pub const fn genesis(&self) -> &MarketGenesisProfileV2 {
        &self.genesis
    }

    /// Canonical retained Feed account.
    pub const fn feed_account(&self) -> Id32 {
        self.feed_account
    }

    /// Exact sealed Feed header.
    pub const fn feed(&self) -> CandidateFeedHeaderV2 {
        self.feed
    }

    /// Exhaustive candidate-wide settlement projection.
    pub const fn traversal(&self) -> &SettlementTraversalProjectionV4 {
        &self.traversal
    }
}

/// Existing SettlementRoot and immutable traversal after both SBF account
/// authentication and the pure exhaustive equality bind succeed.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedRootSettlementTraversalV5<'a> {
    root: AuthenticatedGeneralSettlementRootV1,
    traversal: &'a AuthenticatedSettlementTraversalV5,
}

impl<'a> AuthenticatedRootSettlementTraversalV5<'a> {
    /// Program-owned canonical root account and exact decoded body.
    pub const fn root(&self) -> &AuthenticatedGeneralSettlementRootV1 {
        &self.root
    }

    /// Exact immutable traversal equality-bound to the root.
    pub const fn traversal(&self) -> &'a AuthenticatedSettlementTraversalV5 {
        self.traversal
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootTraversalAccessV5 {
    ReadOnly,
    Writable,
}

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

fn require_readonly_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: Option<usize>,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    if let Some(len) = exact_len {
        require(account.data_len() == len, ClutchError::WrongDataLength)?;
    }
    Ok(())
}

fn require_frame_distinct(frame: SettlementTraversalAccountFrameV5<'_, '_>) -> Outcome<()> {
    let fixed = [
        frame.retained_feed,
        frame.market_binding,
        frame.market_runtime,
        frame.economic_domain,
        frame.price_grid,
        frame.realm,
        frame.profile,
        frame.collateral_policy,
        frame.token_program,
        frame.market_instance,
        frame.market_genesis,
    ];
    let mut left = 0usize;
    while left < fixed.len() {
        let mut right = left + 1;
        while right < fixed.len() {
            require(fixed[left].key != fixed[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        let mut page = 0usize;
        while page < frame.pages.len() {
            require(fixed[left].key != frame.pages[page].key, ClutchError::AccountAlias)?;
            page += 1;
        }
        left += 1;
    }
    left = 0;
    while left < frame.pages.len() {
        let mut right = left + 1;
        while right < frame.pages.len() {
            require(
                frame.pages[left].key != frame.pages[right].key,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Reproduce the one canonical immutable settlement traversal.
///
/// No caller-supplied count, owner aggregate, Position identity, Reservation
/// identity, or child expectation enters this constructor. Page count and all
/// settlement expectations come from the exact page set and sealed Feed.
#[inline(never)]
pub fn authenticate_settlement_traversal_v5(
    program_id: &Pubkey,
    frame: SettlementTraversalAccountFrameV5<'_, '_>,
) -> Outcome<AuthenticatedSettlementTraversalV5> {
    require(
        (1..=MAX_ORDER_PAGES).contains(&frame.pages.len()),
        ClutchError::WrongAccountCount,
    )?;
    require_frame_distinct(frame)?;
    require_readonly_program_state(program_id, frame.retained_feed, None)?;
    require_readonly_program_state(
        program_id,
        frame.economic_domain,
        Some(contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES),
    )?;
    require_readonly_program_state(
        program_id,
        frame.price_grid,
        Some(account_len::PRICE_GRID),
    )?;
    for page in frame.pages {
        require_readonly_program_state(program_id, page, Some(ORDER_PAGE_V5_BYTES))?;
    }

    let feed_data = borrow_data(frame.retained_feed)?;
    let (feed, _) = contract::complete_candidate_feed_v2(&feed_data, true)?;
    expect_pda(
        frame.retained_feed.key,
        seeds::general_v2_feed_pda(program_id, &feed.node.bytes()),
        Some(feed.stored_bump),
    )?;
    let domain = contract::EconomicDomainV2AccountV1::decode(&borrow_data(frame.economic_domain)?)?;
    expect_pda(
        frame.economic_domain.key,
        seeds::general_v2_economic_domain_pda(program_id, &feed.epoch.bytes()),
        Some(domain.stored_bump),
    )?;
    let grid = PriceGridAccount::decode(&borrow_data(frame.price_grid)?)?;
    expect_pda(
        frame.price_grid.key,
        seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes()),
        Some(grid.stored_bump),
    )?;

    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        frame.realm,
        frame.profile,
        frame.collateral_policy,
        frame.token_program,
    )?;
    let (market, runtime) =
        authenticate_general_market_v2(program_id, frame.market_binding, frame.market_runtime)?;
    let base = market.base();
    let instance = *authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        frame.market_instance,
        ContentId::from_bytes(base.market_instance_v2_id.bytes()),
    )?
    .value();
    let genesis = *authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        frame.market_genesis,
        ContentId::from_bytes(base.market_genesis_profile_v2_id.bytes()),
    )?
    .value();
    require(
        instance
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes()
            == base.market_instance_v2_id.bytes()
            && runtime.market_instance_v2_id == base.market_instance_v2_id
            && instance.market_genesis_profile_id.content_id().bytes()
                == base.market_genesis_profile_v2_id.bytes()
            && genesis.realm_id.bytes() == realm.realm().realm.bytes()
            && genesis.profile_id.bytes() == realm.realm().profile.bytes()
            && genesis.price_grid_id.bytes() == grid.grid.bytes()
            && genesis.price_measure_policy_id.content_id().bytes()
                == base.price_measure_policy_v1_id.bytes()
            && genesis.relation_policy_id.bytes() == base.relation_policy_id.bytes()
            && genesis.score_policy_id.bytes() == base.score_policy_id.bytes()
            && genesis.capability_profile_id.bytes() == capabilities::PROFILE_ID
            && grid.realm.bytes() == genesis.realm_id.bytes()
            && grid.price_scale == base.price_scale
            && feed.market == base.market
            && feed.epoch == domain.epoch,
        ClutchError::MismatchedState,
    )?;

    let market_bytes = base.market_instance_v2_id.bytes();
    let collateral = refine_market_collateral_v2(
        realm,
        MarketCollateralBindingV2 {
            market: CollateralId::from_bytes(market_bytes),
            realm: CollateralId::from_bytes(realm.realm().realm.bytes()),
            profile: CollateralId::from_bytes(realm.realm().profile.bytes()),
            collateral_cap_atoms: instance.collateral_cap,
            hoard_authority: CollateralId::from_bytes(
                seeds::hoard_authority_v2_pda(program_id, &market_bytes)
                    .0
                    .to_bytes(),
            ),
            hoard_token_account: CollateralId::from_bytes(
                seeds::hoard_token_v2_pda(program_id, &market_bytes)
                    .0
                    .to_bytes(),
            ),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;

    let mut page_refs: [Option<Ref<'_, [u8]>>; MAX_ORDER_PAGES] = [None, None, None, None];
    let mut page_inputs = [GeneralOrderPageInputV5 {
        account: Id32::ZERO,
        body: &[],
    }; MAX_ORDER_PAGES];
    let mut page_index = 0usize;
    while page_index < frame.pages.len() {
        let page_account = &frame.pages[page_index];
        let canonical_index =
            u16::try_from(page_index).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        let page_pda = seeds::general_v2_order_page_v5_pda(
            program_id,
            &feed.epoch.bytes(),
            canonical_index,
        );
        require(*page_account.key == page_pda.0, ClutchError::WrongPda)?;
        let data = borrow_data(page_account)?;
        let page = verify_page_v5(&data)?;
        require(
            page.page_index == canonical_index
                && usize::from(page.page_count) == frame.pages.len()
                && page.epoch.bytes() == feed.epoch.bytes()
                && page.market.bytes() == feed.market.bytes()
                && page.order_set.bytes() == feed.order_set.bytes()
                && page.stored_bump == page_pda.1,
            ClutchError::MismatchedState,
        )?;
        page_refs[page_index] = Some(data);
        page_index += 1;
    }
    page_index = 0;
    while page_index < frame.pages.len() {
        page_inputs[page_index] = GeneralOrderPageInputV5 {
            account: id(frame.pages[page_index].key),
            body: page_refs[page_index]
                .as_ref()
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
        };
        page_index += 1;
    }
    let order_projection = Box::new(project_owner_blind_book_costed_v1(
        &page_inputs[..frame.pages.len()],
        feed.order_set,
        &domain,
        &market,
        &grid,
    )?);
    let traversal = Box::new(derive_settlement_traversal_projection_v4(
        id(frame.retained_feed.key),
        &feed_data,
        &order_projection,
        base.series_funding_terms_v2_id,
        base.settlement_policy_id,
        collateral,
    )?);
    Ok(AuthenticatedSettlementTraversalV5 {
        market,
        collateral,
        genesis,
        feed_account: id(frame.retained_feed.key),
        feed,
        traversal,
    })
}

/// Authenticate one writable counted root and equality-bind the immutable
/// traversal before any action-specific root mutation is prepared.
pub fn authenticate_writable_root_settlement_traversal_v5<'a>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    traversal: &'a AuthenticatedSettlementTraversalV5,
) -> Outcome<AuthenticatedRootSettlementTraversalV5<'a>> {
    authenticate_root_settlement_traversal_v5(
        program_id,
        root_account,
        traversal,
        RootTraversalAccessV5::Writable,
    )
}

/// Authenticate one read-only counted root and equality-bind the immutable
/// traversal before any action-specific child transition is prepared.
pub fn authenticate_readonly_root_settlement_traversal_v5<'a>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    traversal: &'a AuthenticatedSettlementTraversalV5,
) -> Outcome<AuthenticatedRootSettlementTraversalV5<'a>> {
    authenticate_root_settlement_traversal_v5(
        program_id,
        root_account,
        traversal,
        RootTraversalAccessV5::ReadOnly,
    )
}

fn authenticate_root_settlement_traversal_v5<'a>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    traversal: &'a AuthenticatedSettlementTraversalV5,
    access: RootTraversalAccessV5,
) -> Outcome<AuthenticatedRootSettlementTraversalV5<'a>> {
    require(
        id(root_account.key) != traversal.feed_account(),
        ClutchError::AccountAlias,
    )?;
    let feed = traversal.feed();
    let root = match access {
        RootTraversalAccessV5::ReadOnly => authenticate_readonly_general_settlement_root_v1(
            program_id,
            core::slice::from_ref(root_account),
            feed.epoch,
            feed.settlement_candidate_id,
        )?,
        RootTraversalAccessV5::Writable => authenticate_writable_general_settlement_root_v1(
            program_id,
            core::slice::from_ref(root_account),
            feed.epoch,
            feed.settlement_candidate_id,
        )?,
    };
    bind_settlement_root_traversal_v4(root.account(), root.root(), traversal.traversal())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedRootSettlementTraversalV5 { root, traversal })
}
