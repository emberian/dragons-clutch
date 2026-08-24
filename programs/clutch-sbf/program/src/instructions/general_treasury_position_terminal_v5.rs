//! Physical terminal owner for the canonical General Market treasury Position.
//!
//! The transition consumes only hostile current authority: General BindingV5
//! and its complete Product/Revenue graph, the durable settled fee-terminal
//! pair, the counted treasury-service ledger, and the exact PositionV3/GEN1
//! Replay bytes. It first seals the economically empty pair, reopens that
//! terminal postimage, shrinks Position to its permanent tombstone, deletes
//! Replay with exact persisted rent disposition, and finally latches the
//! resulting non-copy receipt into the already-Retiring Product RootV3.

use std::boxed::Box;

use clutch_fee_runtime_contract::terminal::FeeTerminalOutcomeV1;
use clutch_general_v2_contract::{
    prepare_general_treasury_position_terminal_v1,
    project_general_position_replay_prestate_v1, GeneralReplayExtensionV1,
    Id32, GENERAL_REPLAY_ACCOUNT_V1_BYTES, GENERAL_REPLAY_EXTENSION_SCHEMA_V1,
};
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_product_series::{
    ContentId, MarketLifecyclePhaseV3, MarketSharedCoreTerminalProjectionV3,
    MarketSharedCoreV3, SeriesFundingPhaseV5, SeriesMarketLinkPhaseV3,
};
use clutch_retirement::{
    plan_position_v3_replay_v3_retirement_v1, Identity32V1, PositionAccountV3,
    PositionPurposeV3, PositionTombstoneV3, PositionV3ReplayV3AccountsV1,
    PositionV3ReplayV3RetirementRequestV1, PositionV3Sha256Backend,
    RecipientBalanceBookV1, RecipientBalanceV1, ReplayV3Envelope,
    ReplayV3HashBackend, POSITION_ACCOUNT_VERSION_V3, POSITION_TOMBSTONE_V3_BYTES,
    POSITION_TOMBSTONE_VERSION_V3,
};
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV3;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::seeds;

use super::general_market_current_v5::AuthenticatedGeneralMarketCurrentV5;
use super::general_v2_fee_terminal_pair_v1::AuthenticatedFeeTerminalPairV1;
use super::product_market_lifecycle_v3_current::authenticate_market_lifecycle_root_v3;
use super::product_series_current::retirement_v5::write_market_lifecycle_root_v3;
use super::revenue_policy_v2::authenticate_treasury_service_ledger_v1;

const GENERAL_TREASURY_TERMINAL_AUTHORITY_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/general-treasury-terminal-authority/v5\0";
const GENERAL_TREASURY_POSITION_OWNER_RELEASE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/general-treasury-position-owner-release/v5\0";
const GENERAL_TREASURY_POSITION_PHYSICAL_TERMINAL_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/general-treasury-position-physical-terminal/v5\0";
const PRODUCT_POSITION_SHARED_CORE_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-position-shared-core-postwrite/v5\0";
const TREASURY_SERVICE_LEDGER_DATA_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/treasury-service-ledger/data/v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha256;

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for RuntimeSha256 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn identity(bytes: [u8; 32]) -> Outcome<Identity32V1> {
    Identity32V1::new(bytes)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn require_live(value: ContentId) -> Outcome<()> {
    require(!value.is_zero(), ClutchError::MismatchedState)
}

fn require_program_source(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: usize,
) -> Outcome<()> {
    require(
        account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len() == exact_len,
        ClutchError::MismatchedState,
    )
}

fn require_recipient(account: &AccountInfo<'_>) -> Outcome<()> {
    require(
        account.owner == &SYSTEM_PROGRAM_ID
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_is_empty(),
        ClutchError::MismatchedState,
    )
}

fn recipient<'a, 'info>(
    id: Identity32V1,
    position_payer: &'a AccountInfo<'info>,
    replay_payer: &'a AccountInfo<'info>,
    neutral_sink: &'a AccountInfo<'info>,
) -> Outcome<&'a AccountInfo<'info>> {
    let bytes = id.bytes();
    if bytes == position_payer.key.to_bytes() {
        Ok(position_payer)
    } else if bytes == replay_payer.key.to_bytes() {
        Ok(replay_payer)
    } else if bytes == neutral_sink.key.to_bytes() {
        Ok(neutral_sink)
    } else {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

fn insert_recipient(
    entries: &mut [Option<RecipientBalanceV1>; clutch_retirement::MAX_RETIREMENT_RECIPIENTS],
    count: &mut usize,
    id: Identity32V1,
    balance_before: u64,
) -> Outcome<()> {
    for existing in entries.iter().flatten() {
        if existing.recipient == id {
            return require(
                existing.balance_before == balance_before,
                ClutchError::MismatchedState,
            );
        }
    }
    require(*count < entries.len(), ClutchError::Arithmetic)?;
    entries[*count] = Some(RecipientBalanceV1 {
        recipient: id,
        balance_before,
    });
    *count += 1;
    Ok(())
}

/// Non-copy receipt proving both physical Position retirement and its unique
/// same-instruction Product RootV3 shared-core latch.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductPositionPhysicalTerminalV5 {
    id: ContentId,
    physical_terminal_id: ContentId,
    shared_core_projection_id: ContentId,
    owner_release_id: ContentId,
    position_account: Pubkey,
    replay_account: Pubkey,
    position_tombstone_semantic_id: ContentId,
    replay_terminal_semantic_id: ContentId,
    terminal_sequence: u64,
    root_account: Pubkey,
    market_instance_id: clutch_product_series::MarketInstanceV2Id,
    generation: u64,
    root_binding_id: ContentId,
    root_data_before_id: ContentId,
    root_authentication_before_id: ContentId,
    root_semantic_before_id: ContentId,
    root_transition_sequence_before: u64,
    root_data_after_id: ContentId,
    root_authentication_after_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_after: u64,
}

impl AuthenticatedProductPositionPhysicalTerminalV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn physical_terminal_id(&self) -> ContentId {
        self.physical_terminal_id
    }
    pub(crate) const fn shared_core_projection_id(&self) -> ContentId {
        self.shared_core_projection_id
    }
    pub(crate) const fn owner_release_id(&self) -> ContentId { self.owner_release_id }
    pub(crate) const fn position_account(&self) -> Pubkey { self.position_account }
    pub(crate) const fn replay_account(&self) -> Pubkey { self.replay_account }
    pub(crate) const fn position_tombstone_semantic_id(&self) -> ContentId {
        self.position_tombstone_semantic_id
    }
    pub(crate) const fn replay_terminal_semantic_id(&self) -> ContentId {
        self.replay_terminal_semantic_id
    }
    pub(crate) const fn terminal_sequence(&self) -> u64 { self.terminal_sequence }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn market_instance_id(
        &self,
    ) -> clutch_product_series::MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn root_binding_id(&self) -> ContentId { self.root_binding_id }
    pub(crate) const fn root_data_before_id(&self) -> ContentId {
        self.root_data_before_id
    }
    pub(crate) const fn root_authentication_before_id(&self) -> ContentId {
        self.root_authentication_before_id
    }
    pub(crate) const fn root_semantic_before_id(&self) -> ContentId {
        self.root_semantic_before_id
    }
    pub(crate) const fn root_transition_sequence_before(&self) -> u64 {
        self.root_transition_sequence_before
    }
    pub(crate) const fn root_data_after_id(&self) -> ContentId {
        self.root_data_after_id
    }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.root_authentication_after_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
}

/// Physically retire and immediately latch the canonical Market treasury
/// Position. The durable fee pair is borrowed because the enclosing General
/// root close must still consume and close those same accounts later in this
/// transaction; no detached Product receipt account is created.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn retire_current_general_treasury_position_into_product_v5(
    program_id: &Pubkey,
    current: AuthenticatedGeneralMarketCurrentV5,
    fee_terminal: &AuthenticatedFeeTerminalPairV1,
    product_root: &AccountInfo<'_>,
    treasury_service_ledger: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    position_refund_owner: &AccountInfo<'_>,
    replay_refund_owner: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
) -> Outcome<AuthenticatedProductPositionPhysicalTerminalV5> {
    require_program_source(program_id, position_account, clutch_retirement::POSITION_V3_BYTES)?;
    require_program_source(program_id, replay_account, GENERAL_REPLAY_ACCOUNT_V1_BYTES)?;
    require_recipient(position_refund_owner)?;
    require_recipient(replay_refund_owner)?;
    require_recipient(neutral_sink)?;
    require(
        product_root.is_writable
            && current.product_root_account() == *product_root.key
            && current.product_root_phase() == MarketLifecyclePhaseV3::Retiring
            && current.product_link_phase() == SeriesMarketLinkPhaseV3::Retired
            && current.funding_phase() == SeriesFundingPhaseV5::Closed
            && current.treasury().treasury_position_account() == *position_account.key
            && current.treasury().treasury_replay_account() == *replay_account.key
            && current.treasury().treasury_service_ledger_account()
                == *treasury_service_ledger.key
            && current.binding().base().base().neutral_sink.bytes()
                == neutral_sink.key.to_bytes()
            && fee_terminal.general().outcome == FeeTerminalOutcomeV1::Settled
            && fee_terminal.general().market.0 == current.runtime_account().to_bytes()
            && fee_terminal.revenue_policy().bytes()
                == current.revenue().policy_digest().bytes()
            && fee_terminal.treasury_position().bytes() == position_account.key.to_bytes()
            && product_root.key != treasury_service_ledger.key
            && product_root.key != position_account.key
            && product_root.key != replay_account.key
            && position_account.key != replay_account.key
            && treasury_service_ledger.key != position_account.key
            && treasury_service_ledger.key != replay_account.key,
        ClutchError::MismatchedState,
    )?;

    let service = authenticate_treasury_service_ledger_v1(
        program_id,
        treasury_service_ledger,
        current.treasury(),
        false,
    )?;
    let service_body = service.body();
    require(
        service_body.is_economically_closeable()
            && service_body.treasury_position_account.bytes()
                == position_account.key.to_bytes(),
        ClutchError::TreasuryServiceOutstanding,
    )?;
    let service_data = treasury_service_ledger
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let service_data_id = hashv(&[
        TREASURY_SERVICE_LEDGER_DATA_DOMAIN_V1,
        treasury_service_ledger.key.as_ref(),
        &service_data,
    ]);
    drop(service_data);

    let position_data = position_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let position = PositionAccountV3::decode(&position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_semantic_id = position
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_refund = position.rent().refundable_live_principal;
    let position_pda = seeds::position_v3_pda(
        program_id,
        &position.market_instance_id().bytes(),
        &position.owner().bytes(),
        position.purpose(),
        &position.purpose_binding_id().bytes(),
    );
    expect_pda(position_account.key, position_pda, Some(position.stored_bump()))?;
    let replay_pda = seeds::purpose_replay_v3_pda(
        program_id,
        &position_account.key.to_bytes(),
        position.purpose(),
        &position.purpose_binding_id().bytes(),
    );
    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observed_replay = ReplayV3Envelope::decode(&replay_data, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_sequence = observed_replay.header().next_sequence();
    let replay_refund = observed_replay.header().rent().refundable_principal();
    expect_pda(
        replay_account.key,
        replay_pda,
        Some(observed_replay.header().stored_bump()),
    )?;
    GeneralReplayExtensionV1::decode(observed_replay.extension())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let fields = position.fields();
    require(
        position.purpose() == PositionPurposeV3::General
            && fields.market_instance_id.bytes()
                == current.binding().base().base().market_instance_v2_id.bytes()
            && fields.realm_id.bytes() == current.product_root_realm_id().bytes()
            && fields.collateral_policy_id.bytes()
                == current.product_root_collateral_policy_id().bytes()
            && fields.collateral_release_id.bytes()
                == current.product_root_collateral_release_id().bytes()
            && fields.outcome_count == current.product_root_outcome_count()
            && fields.owner.bytes() == current.revenue().treasury_owner().bytes()
            && fields.controller == fields.owner
            && fields.purpose_binding_id.bytes() == current.runtime_account().to_bytes()
            && fields.replay_account.bytes() == replay_account.key.to_bytes()
            && fields.generation == service_body.treasury_position_generation
            && fields.cash_atoms == 0
            && fields.reserved_cash_atoms == 0
            && fields.native_eggs == [0; clutch_retirement::MAX_OUTCOMES]
            && fields.outstanding_reservations == 0
            && fields.rent.payer.bytes() == position_refund_owner.key.to_bytes()
            && observed_replay.header().rent().payer().bytes()
                == replay_refund_owner.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let authenticated_position = AuthenticatedPositionV3 {
        account: position_account.key.to_bytes(),
        general_market_runtime: current.runtime_account().to_bytes(),
        semantic: position,
        semantic_id: position_semantic_id.bytes(),
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    let position_replay = project_general_position_replay_prestate_v1(
        Id32::from_bytes(replay_account.key.to_bytes()),
        replay_pda.1,
        replay_sequence,
        &replay_data,
        authenticated_position,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(replay_data);
    drop(position_data);

    let terminal_authority_id = hashv(&[
        GENERAL_TREASURY_TERMINAL_AUTHORITY_DOMAIN_V5,
        program_id.as_ref(),
        &current.id().bytes(),
        &current.binding_data_id().bytes(),
        &current.runtime_data_id().bytes(),
        &fee_terminal.manifest_account_data_id().bytes(),
        &fee_terminal.terminal_account_data_id().bytes(),
        &fee_terminal.terminal_semantic_data_id().bytes(),
        &service_data_id.bytes(),
        position_account.key.as_ref(),
        &position_semantic_id.bytes(),
        replay_account.key.as_ref(),
        &position_replay.replay_semantic_id().bytes(),
        &replay_sequence.to_le_bytes(),
    ]);
    require_live(terminal_authority_id)?;
    let terminal = prepare_general_treasury_position_terminal_v1(
        position_replay,
        Id32::from_bytes(terminal_authority_id.bytes()),
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    {
        let mut data = position_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        data.copy_from_slice(terminal.position_terminal_body());
    }
    {
        let mut data = replay_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        data.copy_from_slice(terminal.replay_terminal_body());
    }
    let terminal_position_data = position_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let terminal_position = PositionAccountV3::decode(&terminal_position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_position_projection = terminal_position
        .terminal_projection()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        terminal_position_data.as_ref() == terminal.position_terminal_body()
            && terminal_position
                .semantic_id(&RuntimeSha256)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == terminal.position_terminal_semantic_id().bytes(),
        ClutchError::MismatchedState,
    )?;
    let terminal_replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let terminal_replay = ReplayV3Envelope::decode(&terminal_replay_data, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_replay_projection = terminal_replay
        .terminal_projection()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        terminal_replay_data.as_ref() == terminal.replay_terminal_body()
            && terminal_replay_projection
                .semantic_id(&RuntimeSha256)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == terminal.replay_terminal_semantic_id().bytes()
            && terminal_replay_projection.header().next_sequence()
                == terminal.terminal_sequence(),
        ClutchError::MismatchedState,
    )?;

    let position_payer_id = identity(position_refund_owner.key.to_bytes())?;
    let replay_payer_id = identity(replay_refund_owner.key.to_bytes())?;
    let neutral_sink_id = identity(neutral_sink.key.to_bytes())?;
    let mut recipient_entries = [None; clutch_retirement::MAX_RETIREMENT_RECIPIENTS];
    let mut recipient_count = 0usize;
    insert_recipient(
        &mut recipient_entries,
        &mut recipient_count,
        position_payer_id,
        position_refund_owner.lamports(),
    )?;
    insert_recipient(
        &mut recipient_entries,
        &mut recipient_count,
        replay_payer_id,
        replay_refund_owner.lamports(),
    )?;
    insert_recipient(
        &mut recipient_entries,
        &mut recipient_count,
        neutral_sink_id,
        neutral_sink.lamports(),
    )?;
    let retirement = plan_position_v3_replay_v3_retirement_v1(
        PositionV3ReplayV3RetirementRequestV1 {
            position: terminal_position_projection,
            replay: terminal_replay_projection,
            position_balance: position_account.lamports(),
            replay_balance: replay_account.lamports(),
            neutral_sink: neutral_sink_id,
            accounts: PositionV3ReplayV3AccountsV1 {
                position: identity(position_account.key.to_bytes())?,
                replay: identity(replay_account.key.to_bytes())?,
            },
            recipient_balances: RecipientBalanceBookV1 {
                entries: recipient_entries,
            },
            signed_sequence: terminal.terminal_sequence(),
        },
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let tombstone_bytes = retirement
        .position_tombstone
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let tombstone_semantic_id = retirement
        .position_tombstone
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_balance_before = position_account.lamports();
    let replay_balance_before = replay_account.lamports();
    let position_donation = position_balance_before
        .checked_sub(position_refund)
        .and_then(|value| value.checked_sub(retirement.position_balance_after))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let replay_donation = replay_balance_before
        .checked_sub(replay_refund)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    drop(terminal_replay_data);
    drop(terminal_position_data);

    for account in [
        position_account,
        replay_account,
        position_refund_owner,
        replay_refund_owner,
        neutral_sink,
        product_root,
    ] {
        let data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        drop(data);
        let lamports = account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        drop(lamports);
    }
    position_account
        .resize(POSITION_TOMBSTONE_V3_BYTES)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    position_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&tombstone_bytes);
    replay_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    for credit in retirement.recipient_credits.entries.into_iter().flatten() {
        let account = recipient(
            credit.recipient,
            position_refund_owner,
            replay_refund_owner,
            neutral_sink,
        )?;
        **account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? =
            credit.balance_after;
    }
    **position_account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? =
        retirement.position_balance_after;
    **replay_account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? =
        retirement.replay_balance_after;
    replay_account.assign(&SYSTEM_PROGRAM_ID);

    let reopened_position_data = position_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let reopened_tombstone = PositionTombstoneV3::decode(&reopened_position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        reopened_position_data.as_ref() == &tombstone_bytes[..]
            && reopened_tombstone == retirement.position_tombstone
            && position_account.owner == program_id
            && position_account.lamports() == retirement.position_balance_after
            && replay_account.owner == &SYSTEM_PROGRAM_ID
            && replay_account.data_is_empty()
            && replay_account.lamports() == 0,
        ClutchError::MismatchedState,
    )?;
    drop(reopened_position_data);
    for credit in retirement.recipient_credits.entries.into_iter().flatten() {
        require(
            recipient(
                credit.recipient,
                position_refund_owner,
                replay_refund_owner,
                neutral_sink,
            )?
            .lamports()
                == credit.balance_after,
            ClutchError::MismatchedState,
        )?;
    }

    let owner_release_id = hashv(&[
        GENERAL_TREASURY_POSITION_OWNER_RELEASE_DOMAIN_V5,
        program_id.as_ref(),
        &current.product_root_registry_release_id().bytes(),
        &[POSITION_ACCOUNT_VERSION_V3],
        &[POSITION_TOMBSTONE_VERSION_V3],
        &GENERAL_REPLAY_EXTENSION_SCHEMA_V1.to_le_bytes(),
        &(clutch_retirement::POSITION_V3_BYTES as u64).to_le_bytes(),
        &(POSITION_TOMBSTONE_V3_BYTES as u64).to_le_bytes(),
        &(GENERAL_REPLAY_ACCOUNT_V1_BYTES as u64).to_le_bytes(),
    ]);
    require_live(owner_release_id)?;
    let physical_terminal_id = hashv(&[
        GENERAL_TREASURY_POSITION_PHYSICAL_TERMINAL_DOMAIN_V5,
        program_id.as_ref(),
        &terminal_authority_id.bytes(),
        &owner_release_id.bytes(),
        position_account.key.as_ref(),
        &terminal.position_prestate_semantic_id().bytes(),
        &terminal.position_terminal_semantic_id().bytes(),
        &tombstone_semantic_id.bytes(),
        replay_account.key.as_ref(),
        &terminal.replay_prestate_semantic_id().bytes(),
        &retirement.terminal_replay_semantic_id.bytes(),
        &terminal.transition_id().bytes(),
        &terminal.delta_id().bytes(),
        &terminal.terminal_sequence().to_le_bytes(),
        position_refund_owner.key.as_ref(),
        replay_refund_owner.key.as_ref(),
        neutral_sink.key.as_ref(),
        &position_refund.to_le_bytes(),
        &replay_refund.to_le_bytes(),
        &position_donation.to_le_bytes(),
        &replay_donation.to_le_bytes(),
        &retirement.position_balance_after.to_le_bytes(),
    ]);
    require_live(physical_terminal_id)?;

    let market_instance_id = clutch_product_series::MarketInstanceV2Id::from_bytes(
        current.binding().base().base().market_instance_v2_id.bytes(),
    );
    let mut root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        product_root,
        market_instance_id,
        current.product_root_generation(),
        true,
        &mut root_value,
    )?;
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Retiring
            && root.binding_id() == current.product_root_binding_id()
            && root.data_id() == current.product_root_data_id()
            && root.semantic_id() == current.product_root_semantic_id()
            && root.authentication_id() == current.product_root_authentication_id()
            && root
                .state()
                .shared_core_terminal_receipt(MarketSharedCoreV3::Position)
                .is_zero(),
        ClutchError::MismatchedState,
    )?;
    let root_transition_sequence_before = root.state().transition_sequence();
    let root_transition_sequence_after = root_transition_sequence_before
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let projection = MarketSharedCoreTerminalProjectionV3::new(
        *root.binding(),
        MarketSharedCoreV3::Position,
        ContentId::from_bytes(position_account.key.to_bytes()),
        owner_release_id,
        physical_terminal_id,
        root_transition_sequence_after,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let next = (*root.state())
        .consume_shared_core_terminal(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_data_before_id = root.data_id();
    let root_authentication_before_id = root.authentication_id();
    let root_semantic_before_id = root.semantic_id();
    drop(root);
    write_market_lifecycle_root_v3(product_root, &root_value, &next)?;
    let mut reopened_root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let reopened = authenticate_market_lifecycle_root_v3(
        program_id,
        product_root,
        market_instance_id,
        current.product_root_generation(),
        true,
        &mut reopened_root_value,
    )?;
    require(
        reopened.state() == &next
            && reopened
                .state()
                .shared_core_terminal_receipt(MarketSharedCoreV3::Position)
                == projection.id()
            && reopened.state().transition_sequence() == root_transition_sequence_after,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_POSITION_SHARED_CORE_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &current.id().bytes(),
        &physical_terminal_id.bytes(),
        &projection.id().bytes(),
        product_root.key.as_ref(),
        &root_data_before_id.bytes(),
        &reopened.data_id().bytes(),
        &root_authentication_before_id.bytes(),
        &reopened.authentication_id().bytes(),
        &root_semantic_before_id.bytes(),
        &reopened.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &root_transition_sequence_after.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductPositionPhysicalTerminalV5 {
        id,
        physical_terminal_id,
        shared_core_projection_id: projection.id(),
        owner_release_id,
        position_account: *position_account.key,
        replay_account: *replay_account.key,
        position_tombstone_semantic_id: ContentId::from_bytes(
            tombstone_semantic_id.bytes(),
        ),
        replay_terminal_semantic_id: ContentId::from_bytes(
            retirement.terminal_replay_semantic_id.bytes(),
        ),
        terminal_sequence: terminal.terminal_sequence(),
        root_account: *product_root.key,
        market_instance_id,
        generation: current.product_root_generation(),
        root_binding_id: current.product_root_binding_id(),
        root_data_before_id,
        root_authentication_before_id,
        root_semantic_before_id,
        root_transition_sequence_before,
        root_data_after_id: reopened.data_id(),
        root_authentication_after_id: reopened.authentication_id(),
        root_semantic_after_id: reopened.semantic_id(),
        root_transition_sequence_after,
    })
}
