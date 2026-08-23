//! Authenticated SBF boundary for the shared Product Market lifecycle.
//!
//! The pure Product crate owns deterministic state. This module owns hostile
//! account decoding, exact `0xaa/1` and `0xad/1` PDA/owner/full-body checks,
//! atomic state writes, and private non-decodable terminal authority. Merely
//! compiling these helpers does not enable an instruction route.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_product_series::{
    ContentId, MarketInstanceTerminalProjectionV1, MarketInstanceV2Id, MarketLifecyclePhaseV1,
    MarketLifecycleRootV1, MarketSharedCoreV1, SeriesMarketLinkPhaseV1, SeriesMarketLinkV1,
    SeriesPlanV5Id,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
    MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1, SERIES_MARKET_LINK_ACCOUNT_BYTES_V1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-lifecycle-account-authentication/v1";
const SERIES_MARKET_LINK_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-market-link-account-authentication/v1";
const MARKET_INSTANCE_TERMINAL_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-instance-terminal-authentication/v1";

/// Exact authenticated shared `0xaa/1` account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketLifecycleRootV1 {
    account: Pubkey,
    owner_program: Pubkey,
    value: MarketLifecycleRootAccountV1,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedMarketLifecycleRootV1 {
    /// Physical root account.
    pub const fn account(self) -> Pubkey {
        self.account
    }

    /// Program which authenticated and owns the account.
    pub const fn owner_program(self) -> Pubkey {
        self.owner_program
    }

    /// Complete hostile-decoded account value.
    pub const fn value(self) -> MarketLifecycleRootAccountV1 {
        self.value
    }

    /// Complete pure Market lifecycle state.
    pub const fn state(self) -> MarketLifecycleRootV1 {
        self.value.state
    }

    /// SHA-256 of the exact framed account bytes.
    pub const fn data_id(self) -> ContentId {
        self.data_id
    }

    /// Account/PDA/body/rent authentication identity.
    pub const fn authentication_id(self) -> ContentId {
        self.authentication_id
    }

    /// Exact persisted refundable root rent principal.
    pub const fn rent_principal_lamports(self) -> u64 {
        self.value.rent_principal_lamports
    }
}

/// Exact authenticated per-Series `0xad/1` link account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesMarketLinkV1 {
    account: Pubkey,
    owner_program: Pubkey,
    value: SeriesMarketLinkAccountV1,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedSeriesMarketLinkV1 {
    /// Physical link account.
    pub const fn account(self) -> Pubkey {
        self.account
    }

    /// Program which authenticated and owns the account.
    pub const fn owner_program(self) -> Pubkey {
        self.owner_program
    }

    /// Complete hostile-decoded account value.
    pub const fn value(self) -> SeriesMarketLinkAccountV1 {
        self.value
    }

    /// Complete pure link state.
    pub const fn state(self) -> SeriesMarketLinkV1 {
        self.value.state
    }

    /// SHA-256 of the exact framed account bytes.
    pub const fn data_id(self) -> ContentId {
        self.data_id
    }

    /// Account/PDA/body/rent authentication identity.
    pub const fn authentication_id(self) -> ContentId {
        self.authentication_id
    }
}

/// Private whole-Market terminal receipt re-derived only from authenticated `0xaa`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketInstanceTerminalV1 {
    id: ContentId,
    root_account: Pubkey,
    owner_program: Pubkey,
    root_semantic_id: ContentId,
    root_data_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    projection: MarketInstanceTerminalProjectionV1,
}

impl AuthenticatedMarketInstanceTerminalV1 {
    /// Authentication receipt identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Exact physical terminal root.
    pub const fn root_account(self) -> Pubkey {
        self.root_account
    }

    /// Program which owns the exact root.
    pub const fn owner_program(self) -> Pubkey {
        self.owner_program
    }

    /// Exact semantic identity of the terminal pure state.
    pub const fn root_semantic_id(self) -> ContentId {
        self.root_semantic_id
    }

    /// SHA-256 of the exact terminal framed bytes.
    pub const fn root_data_id(self) -> ContentId {
        self.root_data_id
    }

    /// Full-width shared Market identity.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact Market/Failure generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact Failure market-level terminal receipt consumed by `0xaa`.
    pub const fn failure_terminal_receipt_id(self) -> ContentId {
        self.projection.shared_core_terminal_receipts()[MarketSharedCoreV1::Failure.index()]
    }

    /// Private structural projection consumed only inside this program.
    pub(crate) const fn projection(self) -> MarketInstanceTerminalProjectionV1 {
        self.projection
    }
}

/// Authenticate the exact shared Market root without trusting a caller DTO.
pub fn authenticate_market_lifecycle_root_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    require_writable: bool,
) -> Outcome<AuthenticatedMarketLifecycleRootV1> {
    require(
        !account.is_signer
            && !account.executable
            && (!require_writable || account.is_writable)
            && account.owner == program_id
            && account.data_len() == MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = MarketLifecycleRootAccountV1::decode(&data)?;
    let binding = value.state.binding();
    require(
        binding.market_instance_id == expected_market_instance_id
            && binding.generation == expected_generation
            && account.lamports() >= value.rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::market_lifecycle_root_pda(
        program_id,
        &expected_market_instance_id.bytes(),
        expected_generation,
    );
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[&data[..]]).to_bytes());
    drop(data);
    let semantic_id = value
        .state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            program_id.as_ref(),
            &data_id.bytes(),
            &semantic_id.bytes(),
            &value.rent_principal_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(authentication_id)?;
    Ok(AuthenticatedMarketLifecycleRootV1 {
        account: *account.key,
        owner_program: *program_id,
        value,
        data_id,
        authentication_id,
    })
}

/// Authenticate an exact per-Series link and its shared-root association.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_series_market_link_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    expected_ordinal: u32,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    expected_market_root: Pubkey,
    require_writable: bool,
) -> Outcome<AuthenticatedSeriesMarketLinkV1> {
    require(
        !account.is_signer
            && !account.executable
            && (!require_writable || account.is_writable)
            && account.owner == program_id
            && account.data_len() == SERIES_MARKET_LINK_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesMarketLinkAccountV1::decode(&data)?;
    let binding = value.state.binding();
    require(
        binding.series_plan_id == expected_series_plan_id
            && binding.ordinal == expected_ordinal
            && binding.market_instance_id == expected_market_instance_id
            && binding.generation == expected_generation
            && binding.market_root_account_id.bytes() == expected_market_root.to_bytes()
            && account.lamports() >= value.state.rent_principal_lamports(),
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::series_market_link_pda(
        program_id,
        &expected_series_plan_id.bytes(),
        expected_ordinal,
    );
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[&data[..]]).to_bytes());
    drop(data);
    let semantic_id = value
        .state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_MARKET_LINK_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            program_id.as_ref(),
            &data_id.bytes(),
            &semantic_id.bytes(),
            expected_market_root.as_ref(),
        ])
        .to_bytes(),
    );
    require_live_content_id(authentication_id)?;
    Ok(AuthenticatedSeriesMarketLinkV1 {
        account: *account.key,
        owner_program: *program_id,
        value,
        data_id,
        authentication_id,
    })
}

/// Re-open a terminal root and mint the private whole-Market receipt.
pub fn authenticate_market_instance_terminal_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
) -> Outcome<AuthenticatedMarketInstanceTerminalV1> {
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        account,
        expected_market_instance_id,
        expected_generation,
        false,
    )?;
    require(
        root.state().phase() == MarketLifecyclePhaseV1::Terminal,
        ClutchError::MismatchedState,
    )?;
    let projection = root
        .state()
        .terminal_projection()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        projection.market_instance_id() == expected_market_instance_id
            && projection.generation() == expected_generation,
        ClutchError::MismatchedState,
    )?;
    let root_semantic_id = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_INSTANCE_TERMINAL_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            program_id.as_ref(),
            &root.data_id().bytes(),
            &root_semantic_id.bytes(),
            &projection.id().bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedMarketInstanceTerminalV1 {
        id,
        root_account: root.account(),
        owner_program: root.owner_program(),
        root_semantic_id,
        root_data_id: root.data_id(),
        market_instance_id: expected_market_instance_id,
        generation: expected_generation,
        projection,
    })
}

/// Atomically finalize a fully retired root and return its private terminal receipt.
pub fn finalize_market_lifecycle_terminal_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1,
) -> Outcome<AuthenticatedMarketInstanceTerminalV1> {
    let binding = authenticated.state().binding();
    let (successor, _) = authenticated
        .state()
        .finalize_terminal()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_market_lifecycle_root_v1(program_id, account, authenticated, successor)?;
    authenticate_market_instance_terminal_v1(
        program_id,
        account,
        binding.market_instance_id,
        binding.generation,
    )
}

/// Persist a pure successor and immediately reauthenticate the full root bytes.
pub fn write_market_lifecycle_root_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1,
    successor: MarketLifecycleRootV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1> {
    require(
        account.is_writable
            && *account.key == authenticated.account
            && account.owner == program_id
            && successor.binding() == authenticated.state().binding(),
        ClutchError::MismatchedState,
    )?;
    let value = MarketLifecycleRootAccountV1 {
        state: successor,
        ..authenticated.value
    };
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    drop(data);
    let rebound = authenticate_market_lifecycle_root_v1(
        program_id,
        account,
        successor.binding().market_instance_id,
        successor.binding().generation,
        true,
    )?;
    require(rebound.value == value, ClutchError::MismatchedState)?;
    Ok(rebound)
}

/// Persist a pure per-Series link successor and reauthenticate exact bytes.
pub fn write_series_market_link_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV1,
    successor: SeriesMarketLinkV1,
) -> Outcome<AuthenticatedSeriesMarketLinkV1> {
    require(
        account.is_writable
            && *account.key == authenticated.account
            && account.owner == program_id
            && successor.binding() == authenticated.state().binding(),
        ClutchError::MismatchedState,
    )?;
    let value = SeriesMarketLinkAccountV1 {
        state: successor,
        ..authenticated.value
    };
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    drop(data);
    let binding = successor.binding();
    let rebound = authenticate_series_market_link_v1(
        program_id,
        account,
        binding.series_plan_id,
        binding.ordinal,
        binding.market_instance_id,
        binding.generation,
        Pubkey::new_from_array(binding.market_root_account_id.bytes()),
        true,
    )?;
    require(rebound.value == value, ClutchError::MismatchedState)?;
    Ok(rebound)
}

/// Promote an authenticated active link into a private Failure pin successor.
pub(crate) fn pin_series_market_link_failure_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV1,
    failure_begin_receipt_id: ContentId,
) -> Outcome<AuthenticatedSeriesMarketLinkV1> {
    require(
        authenticated.state().phase() == SeriesMarketLinkPhaseV1::Active,
        ClutchError::MismatchedState,
    )?;
    let successor = authenticated
        .state()
        .pin_failure_session(failure_begin_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_series_market_link_v1(program_id, account, authenticated, successor)
}

fn require_live_content_id(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}
