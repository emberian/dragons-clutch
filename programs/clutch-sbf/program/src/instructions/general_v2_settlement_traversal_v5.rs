//! Single SBF authentication owner for General V5 settlement traversal.
//!
//! The returned value exists only after the adapter has authenticated the
//! exact retained Feed, complete canonical V5 page set, MarketBinding V2,
//! MarketRuntime, EconomicDomain, PriceGrid, Product/Genesis artifacts, and
//! Realm-selected collateral profile. It remains structural: a mutable root,
//! receipt, reservation, Position, or Replay must be authenticated separately
//! by the action-specific composer.

use core::cell::Ref;

use clutch_batch::portfolio_execution_v2::{
    authenticate_exact_portfolio_pair_v2,
    authenticate_portfolio_receipt_sibling_set_v2,
    authenticate_selected_portfolio_order_for_materialization_v2,
    AuthenticatedPortfolioReceiptSiblingSetV2, PortfolioAccountExpectationV2,
    PortfolioAccountRoleV2, PortfolioAdapterV2, PortfolioReceiptSiblingTraversalSetV2,
    PortfolioReceiptSiblingTraversalV2, PortfolioSelectionMembershipExpectationV2,
    PortfolioSettlementReceiptV5TransitionExpectationV2, PortfolioSourceOrderKindV2,
    PortfolioTransitionExpectationV2, PortfolioValuationBoundaryV2,
    SelectedPortfolioOrderRecordV2, PORTFOLIO_EXECUTION_VERSION_V2,
    PORTFOLIO_PAIR_MAX_RECEIPTS_V2,
};
use clutch_batch::relation_v1::{MAX_ORDERS, MAX_OUTCOMES};
use clutch_batch::relation_v2::{
    EconomicCandidateV2, EconomicOrderV2, PricePreconditionV2,
};
use clutch_batch::Side;
use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, BoundCollateralProfileV2, Id as CollateralId,
    MarketCollateralBindingV2,
};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    CandidateFeedHeaderV2, Id32, MarketBindingV2, Sha256BackendV1,
};
use clutch_general_v2_runtime::{
    bind_settlement_root_traversal_v4, derive_settlement_traversal_projection_v4,
    project_owner_blind_book_costed_v1, GeneralOrderPageInputV5,
    SettlementTraversalProjectionV4,
};
use clutch_product_series::{ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2};
use clutch_retirement::{
    Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3,
    PositionV3Sha256Backend, POSITION_V3_BYTES,
};
use clutch_solana_layout::order_page_v5::{verify_page_v5, ORDER_PAGE_V5_BYTES};
use clutch_solana_layout::reservation::RESERVATION_STATE_ACTIVE;
use clutch_solana_layout::reservation_v9::{
    ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9,
};
use clutch_solana_layout::{account_len, PriceGridAccount, MAX_ORDER_PAGES, ORDER_KIND_PORTFOLIO};
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

static EMPTY_PORTFOLIO_MATERIALIZATION_CANDIDATE_V5: EconomicCandidateV2 =
    EconomicCandidateV2 {
        fills: [0; MAX_ORDERS],
        honored_aon_mask: 0,
        virtual_split: 0,
        virtual_merge: 0,
    };
static EMPTY_PORTFOLIO_MATERIALIZATION_SIBLINGS_V5:
    [PortfolioReceiptSiblingTraversalV2; PORTFOLIO_PAIR_MAX_RECEIPTS_V2] = [
    PortfolioReceiptSiblingTraversalV2::EMPTY;
    PORTFOLIO_PAIR_MAX_RECEIPTS_V2
];

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
    frame_accounts: [Id32; 11],
    page_semantic_ids: [Id32; MAX_ORDER_PAGES],
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

    fn order_placement(
        &self,
        order_index: u8,
    ) -> Option<AuthenticatedPortfolioOrderPlacementV5> {
        let projection = self.traversal.order_projection();
        let page_index = projection.order_page_index(order_index)?;
        let page_slot = projection.order_page_slot(order_index)?;
        let page_account = projection.order_page_account(order_index)?;
        let page_semantic_id = *self.page_semantic_ids.get(usize::from(page_index))?;
        if page_semantic_id.is_zero() {
            return None;
        }
        Some(AuthenticatedPortfolioOrderPlacementV5 {
            page_index,
            page_slot,
            page_account,
            page_semantic_id,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedPortfolioOrderPlacementV5 {
    page_index: u16,
    page_slot: u8,
    page_account: Id32,
    page_semantic_id: Id32,
}

/// Existing SettlementRoot and immutable traversal after both SBF account
/// authentication and the pure exhaustive equality bind succeed.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedRootSettlementTraversalV5<'a> {
    root: AuthenticatedGeneralSettlementRootV1,
    traversal: &'a AuthenticatedSettlementTraversalV5,
    access: RootTraversalAccessV5,
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

#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; contract::ID_BYTES] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
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

fn require_writable_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(account.data_len() == exact_len, ClutchError::WrongDataLength)
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
    let mut page_semantic_ids = [Id32::ZERO; MAX_ORDER_PAGES];
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
        page_semantic_ids[page_index] = Id32::new(page.page_digest.bytes())?;
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
        frame_accounts: [
            id(frame.retained_feed.key),
            id(frame.market_binding.key),
            id(frame.market_runtime.key),
            id(frame.economic_domain.key),
            id(frame.price_grid.key),
            id(frame.realm.key),
            id(frame.profile.key),
            id(frame.collateral_policy.key),
            id(frame.token_program.key),
            id(frame.market_instance.key),
            id(frame.market_genesis.key),
        ],
        page_semantic_ids,
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
    let root_id = id(root_account.key);
    for account in traversal.frame_accounts {
        require(root_id != account, ClutchError::AccountAlias)?;
    }
    let mut page = 0u16;
    while usize::from(page) < usize::from(traversal.traversal().order_projection().page_count()) {
        require(
            traversal.traversal().order_projection().page_account(page) != Some(root_id),
            ClutchError::AccountAlias,
        )?;
        page = page
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
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
    Ok(AuthenticatedRootSettlementTraversalV5 {
        root,
        traversal,
        access,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedPortfolioMaterializationEndpointV5 {
    membership: clutch_owner_settlement::AuthenticatedOrderMembershipV2,
    placement: AuthenticatedPortfolioOrderPlacementV5,
    position_account: Id32,
    position_semantic_id: Id32,
}

#[derive(Clone, Copy, Debug)]
struct PortfolioMaterializationSelectionAdapterV5 {
    program_id: [u8; 32],
    root_account: Id32,
    root_semantic_id: Id32,
    root_generation: u64,
    feed_account: Id32,
    feed_semantic_id: Id32,
    endpoints: [AuthenticatedPortfolioMaterializationEndpointV5; 2],
}

impl PortfolioAdapterV2 for PortfolioMaterializationSelectionAdapterV5 {
    fn authenticate_account(&self, expected: &PortfolioAccountExpectationV2) -> bool {
        if expected.owner_program_id != self.program_id || !expected.must_exist {
            return false;
        }
        match expected.role {
            PortfolioAccountRoleV2::SettlementRoot => {
                expected.account_id == self.root_account.bytes()
                    && expected.data_semantic_id == self.root_semantic_id.bytes()
                    && expected.generation == Some(self.root_generation)
                    && expected.writable
            }
            PortfolioAccountRoleV2::RetainedFeed => {
                expected.account_id == self.feed_account.bytes()
                    && expected.data_semantic_id == self.feed_semantic_id.bytes()
                    && expected.generation.is_none()
                    && !expected.writable
            }
            PortfolioAccountRoleV2::OrderPage => self.endpoints.iter().any(|endpoint| {
                expected.account_id == endpoint.placement.page_account.bytes()
                    && expected.data_semantic_id
                        == endpoint.placement.page_semantic_id.bytes()
                    && expected.generation.is_none()
                    && !expected.writable
            }),
            PortfolioAccountRoleV2::Position => self.endpoints.iter().any(|endpoint| {
                expected.account_id == endpoint.position_account.bytes()
                    && expected.data_semantic_id == endpoint.position_semantic_id.bytes()
                    && expected.generation == Some(endpoint.membership.position_generation)
                    && !expected.writable
            }),
            PortfolioAccountRoleV2::Reservation
            | PortfolioAccountRoleV2::Replay
            | PortfolioAccountRoleV2::SettlementReceipt => false,
        }
    }

    fn authenticate_selection_membership(
        &self,
        expected: &PortfolioSelectionMembershipExpectationV2,
        relation_order: &EconomicOrderV2,
        candidate: &EconomicCandidateV2,
    ) -> bool {
        let record = expected.record;
        let Some(endpoint) = self
            .endpoints
            .iter()
            .find(|endpoint| endpoint.membership.order_index == record.order_index)
        else {
            return false;
        };
        let at = usize::from(record.order_index);
        endpoint.membership.order_id == record.order_id
            && endpoint.membership.owner == record.owner_id
            && endpoint.placement.page_index == record.page_index
            && endpoint.placement.page_slot == record.page_slot
            && endpoint.placement.page_account.bytes() == record.order_page_account_id
            && endpoint.placement.page_semantic_id.bytes() == record.order_page_semantic_id
            && endpoint.position_account.bytes() == record.position_account_id
            && endpoint.position_semantic_id.bytes() == record.position_pre_semantic_id
            && relation_order.order_id == record.order_id
            && relation_order.side == record.side
            && candidate.fills.get(at).copied() == Some(record.selected_fill_units)
    }

    fn authenticate_transition(&self, _expected: &PortfolioTransitionExpectationV2) -> bool {
        false
    }

    fn derive_settlement_receipt_v5_post_data_ids(
        &self,
        _expected: &PortfolioSettlementReceiptV5TransitionExpectationV2,
    ) -> Option<[[u8; 32]; PORTFOLIO_PAIR_MAX_RECEIPTS_V2]> {
        None
    }
}

/// Derive the exhaustive private portfolio sibling capability for action 24.
///
/// The writable root authority and immutable traversal already authenticate
/// the complete page/book/Feed set. The two Reservation V9 accounts are exact
/// writable active prestates; the two Position V3 accounts are exact read-only
/// prestates. No packet count, page slot, coefficient, fill, or scalar sibling
/// enters this constructor from the caller.
#[inline(never)]
pub fn authenticate_portfolio_materialization_sibling_set_v5(
    program_id: &Pubkey,
    authority: &AuthenticatedRootSettlementTraversalV5<'_>,
    buyer_reservation: &AccountInfo<'_>,
    buyer_position: &AccountInfo<'_>,
    seller_reservation: &AccountInfo<'_>,
    seller_position: &AccountInfo<'_>,
) -> Outcome<AuthenticatedPortfolioReceiptSiblingSetV2> {
    require(
        authority.access == RootTraversalAccessV5::Writable,
        ClutchError::NotWritable,
    )?;
    let endpoint_accounts = [
        buyer_reservation,
        buyer_position,
        seller_reservation,
        seller_position,
    ];
    let mut left = 0usize;
    while left < endpoint_accounts.len() {
        let endpoint_id = id(endpoint_accounts[left].key);
        require(!endpoint_id.is_zero(), ClutchError::MismatchedState)?;
        for fixed in authority.traversal.frame_accounts {
            require(endpoint_id != fixed, ClutchError::AccountAlias)?;
        }
        require(endpoint_id != authority.root.account(), ClutchError::AccountAlias)?;
        let mut page = 0u16;
        while usize::from(page)
            < usize::from(authority.traversal.traversal().order_projection().page_count())
        {
            require(
                authority
                    .traversal
                    .traversal()
                    .order_projection()
                    .page_account(page)
                    != Some(endpoint_id),
                ClutchError::AccountAlias,
            )?;
            page = page
                .checked_add(1)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        let mut right = left + 1;
        while right < endpoint_accounts.len() {
            require(
                endpoint_accounts[left].key != endpoint_accounts[right].key,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    require_writable_program_state(
        program_id,
        buyer_reservation,
        RESERVATION_ACCOUNT_BYTES_V9,
    )?;
    require_writable_program_state(
        program_id,
        seller_reservation,
        RESERVATION_ACCOUNT_BYTES_V9,
    )?;
    require_readonly_program_state(program_id, buyer_position, Some(POSITION_V3_BYTES))?;
    require_readonly_program_state(program_id, seller_position, Some(POSITION_V3_BYTES))?;

    let root = authority.root.root();
    let traversal = authority.traversal.traversal();
    let feed = authority.traversal.feed();
    let counts = root.counts();
    require(
        root.phase() == contract::SettlementRootPhaseV1::Materializing
            && counts.admitted_receipts == 0
            && counts.live_receipts == 0
            && (1..=u16::try_from(PORTFOLIO_PAIR_MAX_RECEIPTS_V2)
                .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?)
                .contains(&counts.expected_receipts)
            && feed.slice_count == counts.expected_receipts
            && counts.expected_owner_rows == 2
            && counts.admitted_owner_rows == 0
            && counts.live_owner_rows == 0
            && counts.expected_filled_reservations == 2
            && counts.admitted_reservations == 0
            && counts.live_reservations == 0
            && counts.expected_merge_payments == 0
            && feed.candidate_kind == contract::SettlementCandidateKindV1::Direct
            && feed.settlement_candidate_id == feed.base_relation_candidate_id
            && feed.virtual_split == 0
            && feed.virtual_merge == 0,
        ClutchError::MismatchedState,
    )?;
    let entry = traversal
        .slice(0)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let (buy_order_index, sell_order_index) = match (
        entry.buy(),
        entry.sell(),
        entry.route(),
    ) {
        (
            clutch_general_v2_runtime::SettlementLegV1::Order(buy),
            clutch_general_v2_runtime::SettlementLegV1::Order(sell),
            clutch_general_v2_runtime::SettlementRouteV1::Direct,
        ) => (buy, sell),
        _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
    };
    require(
        traversal.first_slice(buy_order_index) == Some(0)
            && traversal.first_slice(sell_order_index) == Some(0),
        ClutchError::MismatchedState,
    )?;
    let buy_membership = traversal
        .settlement_membership(buy_order_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let sell_membership = traversal
        .settlement_membership(sell_order_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        buy_membership.side == clutch_owner_settlement::SettlementSideV1::Buy
            && sell_membership.side == clutch_owner_settlement::SettlementSideV1::Sell
            && buy_membership.order_kind == clutch_owner_settlement::OrderKindV1::Portfolio
            && sell_membership.order_kind == clutch_owner_settlement::OrderKindV1::Portfolio,
        ClutchError::MismatchedState,
    )?;

    let buyer = authenticate_portfolio_materialization_endpoint_v5(
        program_id,
        authority,
        buy_membership,
        buyer_reservation,
        buyer_position,
        0,
    )?;
    let seller = authenticate_portfolio_materialization_endpoint_v5(
        program_id,
        authority,
        sell_membership,
        seller_reservation,
        seller_position,
        1,
    )?;
    let root_semantic_id = root.data_id(&RuntimeSha256, authority.root.account())?;
    let feed_semantic_id = traversal.candidate_bundle_digest();
    require(
        feed_semantic_id == root.candidate_bundle_digest(),
        ClutchError::MismatchedState,
    )?;

    let mut candidate = super::orders_batch::boxed_copy_of(
        &EMPTY_PORTFOLIO_MATERIALIZATION_CANDIDATE_V5,
    )?;
    candidate.honored_aon_mask = feed.honored_aon_mask;
    let mut order = 0usize;
    while order < usize::from(feed.order_count) {
        let order_index = u8::try_from(order)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        if let Some(membership) = traversal.settlement_membership(order_index) {
            candidate.fills[order] = membership.entitled_units;
        }
        order += 1;
    }
    let mut prices = [0u64; MAX_OUTCOMES];
    let mut outcome = 0usize;
    while outcome < usize::from(feed.outcome_count) {
        let outcome_index = u8::try_from(outcome)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        prices[outcome] = traversal
            .price(outcome_index)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        outcome += 1;
    }
    let domain = *traversal.order_projection().base().domain();
    let price = PricePreconditionV2 {
        policy_digest: domain.price_policy_digest,
        semantic_price_digest: feed.candidate_price_digest.bytes(),
        prices,
    };
    let buyer_record = portfolio_materialization_record_v5(
        authority,
        buyer,
        Side::Buy,
        root_semantic_id,
        feed_semantic_id,
    )?;
    let seller_record = portfolio_materialization_record_v5(
        authority,
        seller,
        Side::Sell,
        root_semantic_id,
        feed_semantic_id,
    )?;
    let adapter = PortfolioMaterializationSelectionAdapterV5 {
        program_id: program_id.to_bytes(),
        root_account: authority.root.account(),
        root_semantic_id,
        root_generation: root.epoch_generation(),
        feed_account: authority.traversal.feed_account(),
        feed_semantic_id,
        endpoints: [buyer, seller],
    };
    let authenticated_buyer = authenticate_selected_portfolio_order_for_materialization_v2(
        &adapter,
        program_id.to_bytes(),
        &domain,
        traversal.order_projection().base().book(),
        &candidate,
        feed.base_relation_candidate_id.bytes(),
        buyer_record,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authenticated_seller = authenticate_selected_portfolio_order_for_materialization_v2(
        &adapter,
        program_id.to_bytes(),
        &domain,
        traversal.order_projection().base().book(),
        &candidate,
        feed.base_relation_candidate_id.bytes(),
        seller_record,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let pair = authenticate_exact_portfolio_pair_v2(
        &domain,
        traversal.order_projection().base().book(),
        &price,
        &candidate,
        PortfolioValuationBoundaryV2::ExactReceiptDivisionV1,
        authenticated_buyer,
        authenticated_seller,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut derived_count = 0usize;
    outcome = 0;
    while outcome < usize::from(feed.outcome_count) {
        if pair.payoff()[outcome] != 0 {
            derived_count = derived_count
                .checked_add(1)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        outcome += 1;
    }
    require(
        u16::try_from(derived_count)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?
            == counts.expected_receipts,
        ClutchError::MismatchedState,
    )?;
    let mut sibling_set = super::orders_batch::boxed_copy_of(
        &EMPTY_PORTFOLIO_MATERIALIZATION_SIBLINGS_V5,
    )?;
    let mut sibling_index = 0usize;
    while sibling_index < derived_count {
        let slice_index = u16::try_from(sibling_index)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        let slice = traversal
            .slice(slice_index)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let (slice_buy, slice_sell) = match (slice.buy(), slice.sell(), slice.route()) {
            (
                clutch_general_v2_runtime::SettlementLegV1::Order(buy),
                clutch_general_v2_runtime::SettlementLegV1::Order(sell),
                clutch_general_v2_runtime::SettlementRouteV1::Direct,
            ) => (buy, sell),
            _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        };
        require(
            slice_buy == buy_order_index && slice_sell == sell_order_index,
            ClutchError::MismatchedState,
        )?;
        sibling_set[sibling_index] = PortfolioReceiptSiblingTraversalV2 {
            slice_index,
            sequence: u64::from(slice_index)
                .checked_add(1)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
            buy_order_index,
            sell_order_index,
            outcome: slice.outcome(),
            quantity: slice.quantity(),
            price: traversal
                .price(slice.outcome())
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
        };
        sibling_index += 1;
    }
    authenticate_portfolio_receipt_sibling_set_v2(
        pair,
        PortfolioReceiptSiblingTraversalSetV2 {
            sibling_count: u8::try_from(derived_count)
                .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
            siblings: *sibling_set,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn authenticate_portfolio_materialization_endpoint_v5(
    program_id: &Pubkey,
    authority: &AuthenticatedRootSettlementTraversalV5<'_>,
    membership: clutch_owner_settlement::AuthenticatedOrderMembershipV2,
    reservation_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    expected_side: u8,
) -> Outcome<AuthenticatedPortfolioMaterializationEndpointV5> {
    let root = authority.root.root();
    let traversal = authority.traversal.traversal();
    let placement = authority
        .traversal
        .order_placement(membership.order_index)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let reservation = ReservationAccountV9::decode(&borrow_data(reservation_account)?)?;
    let body = reservation.body();
    let expected_reservation = seeds::general_v2_reservation_v9_pda(
        program_id,
        &body.reservation.bytes(),
    );
    require(
        *reservation_account.key == expected_reservation.0
            && body.stored_bump == expected_reservation.1
            && body.reservation.bytes() == membership.reservation
            && body.market.bytes() == root.market().bytes()
            && body.epoch.bytes() == root.epoch().bytes()
            && body.owner.bytes() == membership.owner
            && body.order_id.bytes() == membership.order_id
            && body.price_grid.bytes()
                == traversal.order_projection().base().price_grid_id().bytes()
            && body.terms.bytes() == traversal.terms().bytes()
            && body.policy.bytes() == traversal.reservation_policy().bytes()
            && body.position_generation == membership.position_generation
            && body.order_generation == membership.order_generation
            && body.page_index == placement.page_index
            && body.order_kind == ORDER_KIND_PORTFOLIO
            && body.side == expected_side
            && body.state == RESERVATION_STATE_ACTIVE
            && body.outcome_count == root.outcome_count(),
        ClutchError::MismatchedState,
    )?;

    let position = PositionAccountV3::decode(&borrow_data(position_account)?)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let fields = position.fields();
    let purpose_binding = Identity32V1::new(root.market().bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_position = seeds::position_v3_pda(
        program_id,
        &root.market_instance_v2_id().bytes(),
        &membership.owner,
        PositionPurposeV3::General,
        &purpose_binding.bytes(),
    );
    let expected_replay = seeds::purpose_replay_v3_pda(
        program_id,
        &position_account.key.to_bytes(),
        PositionPurposeV3::General,
        &purpose_binding.bytes(),
    );
    let position_binding = traversal.position_market_binding();
    require(
        *position_account.key == expected_position.0
            && fields.stored_bump == expected_position.1
            && fields.purpose == PositionPurposeV3::General
            && fields.lifecycle == PositionLifecycleV3::Open
            && fields.generation == membership.position_generation
            && fields.market_instance_id == position_binding.market_instance_id
            && fields.outcome_count == position_binding.outcome_count
            && fields.realm_id == position_binding.realm_id
            && fields.collateral_policy_id == position_binding.collateral_policy_id
            && fields.collateral_release_id == position_binding.collateral_release_id
            && fields.owner.bytes() == membership.owner
            && fields.controller.bytes() == membership.owner
            && fields.purpose_binding_id == purpose_binding
            && fields.replay_account.bytes() == expected_replay.0.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let position_semantic_id = Id32::new(
        position
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    )?;
    Ok(AuthenticatedPortfolioMaterializationEndpointV5 {
        membership,
        placement,
        position_account: id(position_account.key),
        position_semantic_id,
    })
}

fn portfolio_materialization_record_v5(
    authority: &AuthenticatedRootSettlementTraversalV5<'_>,
    endpoint: AuthenticatedPortfolioMaterializationEndpointV5,
    side: Side,
    root_semantic_id: Id32,
    feed_semantic_id: Id32,
) -> Outcome<SelectedPortfolioOrderRecordV2> {
    let root = authority.root.root();
    let traversal = authority.traversal.traversal();
    let domain = traversal.order_projection().base().domain();
    let feed = authority.traversal.feed();
    Ok(SelectedPortfolioOrderRecordV2 {
        version: PORTFOLIO_EXECUTION_VERSION_V2,
        outcome_count: root.outcome_count(),
        source_kind: PortfolioSourceOrderKindV2::Portfolio,
        side,
        order_index: endpoint.membership.order_index,
        page_slot: endpoint.placement.page_slot,
        traversal_index: 0,
        page_index: endpoint.placement.page_index,
        settlement_root_epoch_generation: root.epoch_generation(),
        position_generation: endpoint.membership.position_generation,
        selected_fill_units: endpoint.membership.entitled_units,
        market_semantics_digest: domain.market_semantics_digest,
        epoch_semantics_digest: domain.epoch_semantics_digest,
        economic_candidate_digest: feed.base_relation_candidate_id.bytes(),
        order_set_digest: root.order_set().bytes(),
        settlement_root_account_id: authority.root.account().bytes(),
        settlement_root_pre_semantic_id: root_semantic_id.bytes(),
        settlement_candidate_id: root.settlement_candidate_id().bytes(),
        retained_feed_account_id: authority.traversal.feed_account().bytes(),
        retained_feed_semantic_id: feed_semantic_id.bytes(),
        settlement_witness_id: root.settlement_witness_digest().bytes(),
        order_page_account_id: endpoint.placement.page_account.bytes(),
        order_page_semantic_id: endpoint.placement.page_semantic_id.bytes(),
        position_account_id: endpoint.position_account.bytes(),
        position_pre_semantic_id: endpoint.position_semantic_id.bytes(),
        order_id: endpoint.membership.order_id,
        owner_id: endpoint.membership.owner,
    })
}
