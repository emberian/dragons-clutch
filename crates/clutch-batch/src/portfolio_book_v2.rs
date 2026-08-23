//! Private capability binding the complete General page set to RelationV2.
//!
//! The caller never supplies an [`EconomicBookV2`]. An authenticated adapter
//! decodes the counted SettlementRoot, retained Feed, and every active
//! OrderPage V5 account, checks the exact page-set/order-set joins, and returns
//! the owner-blind RelationV2 projection. Only then does this module mint a
//! private-field capability carrying that checked book. No coefficient vector
//! is persisted or copied into the fixed membership record.

use crate::relation_v1::MAX_OUTCOMES;
use crate::relation_v2::{EconomicBookV2, EconomicDomainV2, EconomicErrorV2};
use crate::MAX_ORDERS;

pub const PORTFOLIO_BOOK_AUTHORITY_VERSION_V2: u8 = 2;
pub const PORTFOLIO_BOOK_MAX_PAGES_V2: usize = 4;
pub const PORTFOLIO_BOOK_ORDERS_PER_PAGE_V2: usize = 16;
pub const PORTFOLIO_BOOK_PAGE_SET_RECORD_V2_BYTES: usize = 568;

const PORTFOLIO_BOOK_PAGE_SET_MAGIC_V2: [u8; 8] = *b"DCPBKS2\0";

const _: () = assert!(MAX_OUTCOMES == 16);
const _: () = assert!(MAX_ORDERS == 64);
const _: () = assert!(
    MAX_ORDERS == PORTFOLIO_BOOK_MAX_PAGES_V2 * PORTFOLIO_BOOK_ORDERS_PER_PAGE_V2
);

pub type PortfolioBookIdentityV2 = [u8; 32];

/// Canonical identity-only membership for the complete bounded page set.
///
/// Active pages occupy the prefix `0..page_count` in canonical page-index
/// order. Every inactive page account and semantic identity is zero. OrderPage
/// V5 has no generation field; exact PDA and V5 digest/body are authoritative.
/// Coefficients, quantities, sides, limits, and policies are absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioBookPageSetRecordV2 {
    pub version: u8,
    pub outcome_count: u8,
    pub page_count: u8,
    pub order_count: u8,
    pub traversal_index: u16,
    pub settlement_root_epoch_generation: u64,
    pub market_semantics_digest: PortfolioBookIdentityV2,
    pub epoch_semantics_digest: PortfolioBookIdentityV2,
    pub order_set_digest: PortfolioBookIdentityV2,
    pub settlement_root_account_id: PortfolioBookIdentityV2,
    pub settlement_root_pre_semantic_id: PortfolioBookIdentityV2,
    pub retained_feed_account_id: PortfolioBookIdentityV2,
    pub retained_feed_semantic_id: PortfolioBookIdentityV2,
    pub settlement_candidate_id: PortfolioBookIdentityV2,
    pub settlement_witness_id: PortfolioBookIdentityV2,
    pub page_account_ids: [PortfolioBookIdentityV2; PORTFOLIO_BOOK_MAX_PAGES_V2],
    pub page_semantic_ids: [PortfolioBookIdentityV2; PORTFOLIO_BOOK_MAX_PAGES_V2],
}

impl PortfolioBookPageSetRecordV2 {
    pub fn encode_into(
        &self,
        output: &mut [u8; PORTFOLIO_BOOK_PAGE_SET_RECORD_V2_BYTES],
    ) -> Result<(), PortfolioBookAuthorityErrorV2> {
        self.validate()?;
        *output = [0; PORTFOLIO_BOOK_PAGE_SET_RECORD_V2_BYTES];
        output[..8].copy_from_slice(&PORTFOLIO_BOOK_PAGE_SET_MAGIC_V2);
        output[8] = self.version;
        output[9] = self.outcome_count;
        output[10] = self.page_count;
        output[11] = self.order_count;
        output[12..14].copy_from_slice(&self.traversal_index.to_le_bytes());
        output[16..24].copy_from_slice(&self.settlement_root_epoch_generation.to_le_bytes());
        let identities = self.identities();
        let mut cursor = 24usize;
        let mut identity = 0usize;
        while identity < identities.len() {
            output[cursor..cursor + 32].copy_from_slice(identities[identity]);
            cursor += 32;
            identity += 1;
        }
        if cursor != PORTFOLIO_BOOK_PAGE_SET_RECORD_V2_BYTES {
            return Err(PortfolioBookAuthorityErrorV2::InvalidCodec);
        }
        Ok(())
    }

    pub fn decode(input: &[u8]) -> Result<Self, PortfolioBookAuthorityErrorV2> {
        if input.len() != PORTFOLIO_BOOK_PAGE_SET_RECORD_V2_BYTES
            || input[..8] != PORTFOLIO_BOOK_PAGE_SET_MAGIC_V2
        {
            return Err(PortfolioBookAuthorityErrorV2::InvalidCodec);
        }
        if input[14..16].iter().any(|byte| *byte != 0) {
            return Err(PortfolioBookAuthorityErrorV2::NonCanonicalPadding);
        }
        let mut cursor = 24usize;
        let mut next_identity = || -> Result<PortfolioBookIdentityV2, PortfolioBookAuthorityErrorV2> {
            let end = cursor
                .checked_add(32)
                .ok_or(PortfolioBookAuthorityErrorV2::ArithmeticOverflow)?;
            let bytes = input
                .get(cursor..end)
                .ok_or(PortfolioBookAuthorityErrorV2::InvalidCodec)?;
            let mut value = [0u8; 32];
            value.copy_from_slice(bytes);
            cursor = end;
            Ok(value)
        };
        let market_semantics_digest = next_identity()?;
        let epoch_semantics_digest = next_identity()?;
        let order_set_digest = next_identity()?;
        let settlement_root_account_id = next_identity()?;
        let settlement_root_pre_semantic_id = next_identity()?;
        let retained_feed_account_id = next_identity()?;
        let retained_feed_semantic_id = next_identity()?;
        let settlement_candidate_id = next_identity()?;
        let settlement_witness_id = next_identity()?;
        let mut page_account_ids = [[0u8; 32]; PORTFOLIO_BOOK_MAX_PAGES_V2];
        let mut page = 0usize;
        while page < PORTFOLIO_BOOK_MAX_PAGES_V2 {
            page_account_ids[page] = next_identity()?;
            page += 1;
        }
        let mut page_semantic_ids = [[0u8; 32]; PORTFOLIO_BOOK_MAX_PAGES_V2];
        page = 0;
        while page < PORTFOLIO_BOOK_MAX_PAGES_V2 {
            page_semantic_ids[page] = next_identity()?;
            page += 1;
        }
        if cursor != PORTFOLIO_BOOK_PAGE_SET_RECORD_V2_BYTES {
            return Err(PortfolioBookAuthorityErrorV2::InvalidCodec);
        }
        let value = Self {
            version: input[8],
            outcome_count: input[9],
            page_count: input[10],
            order_count: input[11],
            traversal_index: read_u16(input, 12)?,
            settlement_root_epoch_generation: read_u64(input, 16)?,
            market_semantics_digest,
            epoch_semantics_digest,
            order_set_digest,
            settlement_root_account_id,
            settlement_root_pre_semantic_id,
            retained_feed_account_id,
            retained_feed_semantic_id,
            settlement_candidate_id,
            settlement_witness_id,
            page_account_ids,
            page_semantic_ids,
        };
        value.validate()?;
        Ok(value)
    }

    fn identities(&self) -> [&PortfolioBookIdentityV2; 17] {
        [
            &self.market_semantics_digest,
            &self.epoch_semantics_digest,
            &self.order_set_digest,
            &self.settlement_root_account_id,
            &self.settlement_root_pre_semantic_id,
            &self.retained_feed_account_id,
            &self.retained_feed_semantic_id,
            &self.settlement_candidate_id,
            &self.settlement_witness_id,
            &self.page_account_ids[0],
            &self.page_account_ids[1],
            &self.page_account_ids[2],
            &self.page_account_ids[3],
            &self.page_semantic_ids[0],
            &self.page_semantic_ids[1],
            &self.page_semantic_ids[2],
            &self.page_semantic_ids[3],
        ]
    }

    fn validate(&self) -> Result<(), PortfolioBookAuthorityErrorV2> {
        if self.version != PORTFOLIO_BOOK_AUTHORITY_VERSION_V2 {
            return Err(PortfolioBookAuthorityErrorV2::UnknownVersion);
        }
        if !(2..=MAX_OUTCOMES).contains(&usize::from(self.outcome_count)) {
            return Err(PortfolioBookAuthorityErrorV2::InvalidOutcomeCount);
        }
        let page_count = usize::from(self.page_count);
        let order_count = usize::from(self.order_count);
        if page_count == 0
            || page_count > PORTFOLIO_BOOK_MAX_PAGES_V2
            || order_count == 0
            || order_count > MAX_ORDERS
        {
            return Err(PortfolioBookAuthorityErrorV2::InvalidPageGeometry);
        }
        let expected_page_count = order_count
            .checked_sub(1)
            .ok_or(PortfolioBookAuthorityErrorV2::InvalidPageGeometry)?
            / PORTFOLIO_BOOK_ORDERS_PER_PAGE_V2
            + 1;
        if page_count != expected_page_count || self.settlement_root_epoch_generation == 0 {
            return Err(PortfolioBookAuthorityErrorV2::InvalidPageGeometry);
        }
        let base_identities = [
            self.market_semantics_digest,
            self.epoch_semantics_digest,
            self.order_set_digest,
            self.settlement_root_account_id,
            self.settlement_root_pre_semantic_id,
            self.retained_feed_account_id,
            self.retained_feed_semantic_id,
            self.settlement_candidate_id,
            self.settlement_witness_id,
        ];
        if base_identities.iter().any(is_zero_identity) {
            return Err(PortfolioBookAuthorityErrorV2::ZeroIdentity);
        }
        if self.settlement_root_account_id == self.retained_feed_account_id {
            return Err(PortfolioBookAuthorityErrorV2::AliasedAccount);
        }
        let mut page = 0usize;
        while page < PORTFOLIO_BOOK_MAX_PAGES_V2 {
            if page < page_count {
                if is_zero_identity(&self.page_account_ids[page])
                    || is_zero_identity(&self.page_semantic_ids[page])
                    || self.page_account_ids[page] == self.settlement_root_account_id
                    || self.page_account_ids[page] == self.retained_feed_account_id
                {
                    return Err(PortfolioBookAuthorityErrorV2::InvalidPageGeometry);
                }
                let mut earlier = 0usize;
                while earlier < page {
                    if self.page_account_ids[earlier] == self.page_account_ids[page] {
                        return Err(PortfolioBookAuthorityErrorV2::AliasedAccount);
                    }
                    earlier += 1;
                }
            } else if !is_zero_identity(&self.page_account_ids[page])
                || !is_zero_identity(&self.page_semantic_ids[page])
            {
                return Err(PortfolioBookAuthorityErrorV2::NonCanonicalPadding);
            }
            page += 1;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortfolioBookAccountRoleV2 {
    SettlementRoot,
    RetainedFeed,
    OrderPage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioBookAccountExpectationV2 {
    pub role: PortfolioBookAccountRoleV2,
    pub account_id: PortfolioBookIdentityV2,
    pub owner_program_id: PortfolioBookIdentityV2,
    pub data_semantic_id: PortfolioBookIdentityV2,
    pub generation: Option<u64>,
    pub page_index: Option<u8>,
    pub writable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioCompleteBookProjectionExpectationV2 {
    pub page_set: PortfolioBookPageSetRecordV2,
}

/// Account authentication and page-to-Relation projection trust boundary.
pub trait PortfolioBookAdapterV2 {
    fn authenticate_book_account(&self, expected: &PortfolioBookAccountExpectationV2) -> bool;

    /// Return a book decoded from the authenticated page bytes. The adapter
    /// must compare every page slot directly with the returned RelationV2 row;
    /// it must never source a coefficient, side, quantity, or limit from the
    /// instruction caller.
    fn project_complete_economic_book(
        &self,
        expected: &PortfolioCompleteBookProjectionExpectationV2,
    ) -> Option<EconomicBookV2> {
        let _ = expected;
        None
    }

    /// Borrow the exact adapter-owned book without copying the 64-row value.
    /// SBF adapters should implement this variant and keep the projection in
    /// bounded heap storage so no 4-KiB call frame owns the full book.
    fn project_complete_economic_book_ref<'a>(
        &'a self,
        expected: &PortfolioCompleteBookProjectionExpectationV2,
    ) -> Option<&'a EconomicBookV2> {
        let _ = expected;
        None
    }
}

/// Frame-bounded projection boundary for allocation-owning adapters.
///
/// The caller supplies storage for the complete fixed-capacity book. Success
/// must fill that storage solely from the same authenticated page bytes used
/// by [`PortfolioBookAdapterV2::authenticate_book_account`].
pub trait PortfolioBookInPlaceAdapterV2: PortfolioBookAdapterV2 {
    fn project_complete_economic_book_into(
        &self,
        expected: &PortfolioCompleteBookProjectionExpectationV2,
        output: &mut EconomicBookV2,
    ) -> bool;
}

/// Private proof that one complete General page set equals one RelationV2 book.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedCompletePortfolioBookV2 {
    page_set: PortfolioBookPageSetRecordV2,
    economic_book: EconomicBookV2,
}

impl AuthenticatedCompletePortfolioBookV2 {
    pub const fn economic_book(&self) -> &EconomicBookV2 {
        &self.economic_book
    }

    pub const fn order_set_digest(&self) -> PortfolioBookIdentityV2 {
        self.page_set.order_set_digest
    }

    pub const fn settlement_root_account_id(&self) -> PortfolioBookIdentityV2 {
        self.page_set.settlement_root_account_id
    }

    pub const fn retained_feed_account_id(&self) -> PortfolioBookIdentityV2 {
        self.page_set.retained_feed_account_id
    }

    pub const fn settlement_candidate_id(&self) -> PortfolioBookIdentityV2 {
        self.page_set.settlement_candidate_id
    }

    pub const fn settlement_witness_id(&self) -> PortfolioBookIdentityV2 {
        self.page_set.settlement_witness_id
    }

    pub const fn traversal_index(&self) -> u16 {
        self.page_set.traversal_index
    }

    pub const fn page_count(&self) -> u8 {
        self.page_set.page_count
    }

    pub const fn order_count(&self) -> u8 {
        self.page_set.order_count
    }
}

/// Borrowed private proof over adapter-owned complete-book storage.
///
/// This is the SBF-safe equivalent of [`AuthenticatedCompletePortfolioBookV2`].
/// Its fields remain private, so a caller cannot wrap an untrusted book after
/// the account/page authentication constructor returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedCompletePortfolioBookRefV2<'a> {
    page_set: PortfolioBookPageSetRecordV2,
    economic_book: &'a EconomicBookV2,
}

impl<'a> AuthenticatedCompletePortfolioBookRefV2<'a> {
    pub const fn economic_book(&self) -> &'a EconomicBookV2 {
        self.economic_book
    }

    pub const fn order_set_digest(&self) -> PortfolioBookIdentityV2 {
        self.page_set.order_set_digest
    }

    pub const fn settlement_root_account_id(&self) -> PortfolioBookIdentityV2 {
        self.page_set.settlement_root_account_id
    }

    pub const fn retained_feed_account_id(&self) -> PortfolioBookIdentityV2 {
        self.page_set.retained_feed_account_id
    }

    pub const fn settlement_candidate_id(&self) -> PortfolioBookIdentityV2 {
        self.page_set.settlement_candidate_id
    }

    pub const fn settlement_witness_id(&self) -> PortfolioBookIdentityV2 {
        self.page_set.settlement_witness_id
    }

    pub const fn traversal_index(&self) -> u16 {
        self.page_set.traversal_index
    }
    pub const fn page_count(&self) -> u8 {
        self.page_set.page_count
    }

    pub const fn order_count(&self) -> u8 {
        self.page_set.order_count
    }
}

/// Authenticate the complete page set and privately construct its RelationV2 book.
pub fn authenticate_complete_portfolio_book_v2<A: PortfolioBookAdapterV2>(
    adapter: &A,
    owner_program_id: PortfolioBookIdentityV2,
    domain: &EconomicDomainV2,
    page_set: PortfolioBookPageSetRecordV2,
) -> Result<AuthenticatedCompletePortfolioBookV2, PortfolioBookAuthorityErrorV2> {
    authenticate_complete_portfolio_book_with_root_privilege_v2(
        adapter,
        owner_program_id,
        domain,
        page_set,
        false,
    )
}

/// Authenticate the read-only complete page set while borrowing the decoded
/// RelationV2 book from adapter-owned storage.
///
/// This is semantically identical to [`authenticate_complete_portfolio_book_v2`]
/// but does not copy the maximum-width book into either the capability or the
/// caller's SBF frame.
pub fn authenticate_complete_portfolio_book_ref_v2<'a, A: PortfolioBookAdapterV2>(
    adapter: &'a A,
    owner_program_id: PortfolioBookIdentityV2,
    domain: &EconomicDomainV2,
    page_set: PortfolioBookPageSetRecordV2,
) -> Result<AuthenticatedCompletePortfolioBookRefV2<'a>, PortfolioBookAuthorityErrorV2> {
    let projection = authenticate_complete_portfolio_book_accounts_v2(
        adapter,
        owner_program_id,
        domain,
        page_set,
        false,
    )?;
    let economic_book = adapter
        .project_complete_economic_book_ref(&projection)
        .ok_or(PortfolioBookAuthorityErrorV2::ProjectionAuthenticationFailed)?;
    economic_book
        .validate(domain)
        .map_err(PortfolioBookAuthorityErrorV2::Economic)?;
    if economic_book.len != page_set.order_count {
        return Err(PortfolioBookAuthorityErrorV2::ProjectedOrderCountMismatch);
    }
    Ok(AuthenticatedCompletePortfolioBookRefV2 {
        page_set,
        economic_book,
    })
}

/// Authenticate the same complete owner-blind book while requiring the exact
/// counted SettlementRoot to be writable in the caller's atomic transition.
///
/// This is a distinct private capability constructor for actions which both
/// consume the frozen book and apply a checked SettlementRoot successor. Feed
/// and OrderPage accounts remain strictly read-only; no record or coefficient
/// schema is duplicated or widened.
pub fn authenticate_complete_portfolio_book_for_root_transition_v2<A: PortfolioBookAdapterV2>(
    adapter: &A,
    owner_program_id: PortfolioBookIdentityV2,
    domain: &EconomicDomainV2,
    page_set: PortfolioBookPageSetRecordV2,
) -> Result<AuthenticatedCompletePortfolioBookV2, PortfolioBookAuthorityErrorV2> {
    authenticate_complete_portfolio_book_with_root_privilege_v2(
        adapter,
        owner_program_id,
        domain,
        page_set,
        true,
    )
}

/// Authenticate a writable-root complete book directly into caller-owned
/// storage and return only a borrowed private capability.
pub fn authenticate_complete_portfolio_book_for_root_transition_into_v2<
    'a,
    A: PortfolioBookInPlaceAdapterV2,
>(
    adapter: &A,
    owner_program_id: PortfolioBookIdentityV2,
    domain: &EconomicDomainV2,
    page_set: PortfolioBookPageSetRecordV2,
    output: &'a mut EconomicBookV2,
) -> Result<AuthenticatedCompletePortfolioBookRefV2<'a>, PortfolioBookAuthorityErrorV2> {
    let projection = authenticate_complete_portfolio_book_accounts_v2(
        adapter,
        owner_program_id,
        domain,
        page_set,
        true,
    )?;
    if !adapter.project_complete_economic_book_into(&projection, output) {
        return Err(PortfolioBookAuthorityErrorV2::ProjectionAuthenticationFailed);
    }
    validate_projected_complete_book_v2(output, domain, projection.page_set.order_count)?;
    Ok(AuthenticatedCompletePortfolioBookRefV2 {
        page_set: projection.page_set,
        economic_book: output,
    })
}

fn authenticate_complete_portfolio_book_with_root_privilege_v2<A: PortfolioBookAdapterV2>(
    adapter: &A,
    owner_program_id: PortfolioBookIdentityV2,
    domain: &EconomicDomainV2,
    page_set: PortfolioBookPageSetRecordV2,
    root_writable: bool,
) -> Result<AuthenticatedCompletePortfolioBookV2, PortfolioBookAuthorityErrorV2> {
    let projection = authenticate_complete_portfolio_book_accounts_v2(
        adapter,
        owner_program_id,
        domain,
        page_set,
        root_writable,
    )?;
    let economic_book = adapter
        .project_complete_economic_book(&projection)
        .ok_or(PortfolioBookAuthorityErrorV2::ProjectionAuthenticationFailed)?;
    validate_projected_complete_book_v2(
        &economic_book,
        domain,
        projection.page_set.order_count,
    )?;
    Ok(AuthenticatedCompletePortfolioBookV2 {
        page_set: projection.page_set,
        economic_book,
    })
}

fn authenticate_complete_portfolio_book_accounts_v2<A: PortfolioBookAdapterV2>(
    adapter: &A,
    owner_program_id: PortfolioBookIdentityV2,
    domain: &EconomicDomainV2,
    page_set: PortfolioBookPageSetRecordV2,
    root_writable: bool,
) -> Result<PortfolioCompleteBookProjectionExpectationV2, PortfolioBookAuthorityErrorV2> {
    page_set.validate()?;
    domain
        .validate()
        .map_err(PortfolioBookAuthorityErrorV2::Economic)?;
    if is_zero_identity(&owner_program_id) {
        return Err(PortfolioBookAuthorityErrorV2::ZeroIdentity);
    }
    if page_set.market_semantics_digest != domain.market_semantics_digest
        || page_set.epoch_semantics_digest != domain.epoch_semantics_digest
        || page_set.outcome_count != domain.outcome_count
    {
        return Err(PortfolioBookAuthorityErrorV2::DomainMismatch);
    }
    let root = PortfolioBookAccountExpectationV2 {
        role: PortfolioBookAccountRoleV2::SettlementRoot,
        account_id: page_set.settlement_root_account_id,
        owner_program_id,
        data_semantic_id: page_set.settlement_root_pre_semantic_id,
        generation: Some(page_set.settlement_root_epoch_generation),
        page_index: None,
        writable: root_writable,
    };
    if !adapter.authenticate_book_account(&root) {
        return Err(PortfolioBookAuthorityErrorV2::AuthenticationFailed {
            role: root.role,
        });
    }
    let feed = PortfolioBookAccountExpectationV2 {
        role: PortfolioBookAccountRoleV2::RetainedFeed,
        account_id: page_set.retained_feed_account_id,
        owner_program_id,
        data_semantic_id: page_set.retained_feed_semantic_id,
        generation: None,
        page_index: None,
        writable: false,
    };
    if !adapter.authenticate_book_account(&feed) {
        return Err(PortfolioBookAuthorityErrorV2::AuthenticationFailed {
            role: feed.role,
        });
    }
    let mut page = 0usize;
    while page < usize::from(page_set.page_count) {
        let page_index = u8::try_from(page)
            .map_err(|_| PortfolioBookAuthorityErrorV2::ArithmeticOverflow)?;
        let expected = PortfolioBookAccountExpectationV2 {
            role: PortfolioBookAccountRoleV2::OrderPage,
            account_id: page_set.page_account_ids[page],
            owner_program_id,
            data_semantic_id: page_set.page_semantic_ids[page],
            // OrderPage V5 has no generation field. Exact PDA plus the
            // canonical V5 page digest/body is its complete authority.
            generation: None,
            page_index: Some(page_index),
            writable: false,
        };
        if !adapter.authenticate_book_account(&expected) {
            return Err(PortfolioBookAuthorityErrorV2::AuthenticationFailed {
                role: expected.role,
            });
        }
        page += 1;
    }
    Ok(PortfolioCompleteBookProjectionExpectationV2 { page_set })
}

fn validate_projected_complete_book_v2(
    economic_book: &EconomicBookV2,
    domain: &EconomicDomainV2,
    order_count: u8,
) -> Result<(), PortfolioBookAuthorityErrorV2> {
    economic_book
        .validate(domain)
        .map_err(PortfolioBookAuthorityErrorV2::Economic)?;
    if economic_book.len != order_count {
        return Err(PortfolioBookAuthorityErrorV2::ProjectedOrderCountMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortfolioBookAuthorityErrorV2 {
    Economic(EconomicErrorV2),
    UnknownVersion,
    InvalidCodec,
    NonCanonicalPadding,
    InvalidOutcomeCount,
    InvalidPageGeometry,
    ZeroIdentity,
    AliasedAccount,
    DomainMismatch,
    AuthenticationFailed { role: PortfolioBookAccountRoleV2 },
    ProjectionAuthenticationFailed,
    ProjectedOrderCountMismatch,
    ArithmeticOverflow,
}

fn is_zero_identity(identity: &PortfolioBookIdentityV2) -> bool {
    identity.iter().all(|byte| *byte == 0)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, PortfolioBookAuthorityErrorV2> {
    let bytes = input
        .get(offset..offset + 2)
        .ok_or(PortfolioBookAuthorityErrorV2::InvalidCodec)?;
    let mut value = [0u8; 2];
    value.copy_from_slice(bytes);
    Ok(u16::from_le_bytes(value))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, PortfolioBookAuthorityErrorV2> {
    let bytes = input
        .get(offset..offset + 8)
        .ok_or(PortfolioBookAuthorityErrorV2::InvalidCodec)?;
    let mut value = [0u8; 8];
    value.copy_from_slice(bytes);
    Ok(u64::from_le_bytes(value))
}
