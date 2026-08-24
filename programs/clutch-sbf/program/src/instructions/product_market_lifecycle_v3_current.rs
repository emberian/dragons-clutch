//! Hostile account authentication for the current Product lifecycle spine.
//!
//! These receipts decode only `0xaa/v3` and `0xad/v3`. Mutation remains narrow:
//! a bounded action15 stage must first move exact FoundationVault principal
//! into the canonical next GraphV4 account, consume that account owner's
//! move-only postwrite, then write and hostile-reopen RootV3. Family-specific
//! mutation is owned by separate atomic composers.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{transfer_data, SYSTEM_PROGRAM_ID};
use crate::instructions::product_market_replay_current::AuthenticatedMarketLifecycleReplayV2;
use crate::seeds;
use clutch_product_series::{
    ContentId, MarketFoundationAccountGraphV4, MarketFoundationScheduleV4,
    MarketFoundationSlotV4, MarketFoundationStepProjectionV4, MarketInstanceV2Id,
    MarketLifecycleBindingV3, MarketLifecyclePhaseV3, MarketLifecycleReplayPhaseV2,
    MarketLifecycleRootV3,
    SeriesMarketLinkBindingV3, SeriesMarketLinkV3, SeriesMarketLinkV3Id, SeriesPlanV5Id,
    MARKET_FOUNDATION_SLOT_COUNT_V4,
    MARKET_LIFECYCLE_ROOT_DOMAIN_V3, SERIES_MARKET_LINK_DOMAIN_V3,
};
use clutch_solana_layout::product_series::{
    series_market_link_authentication_id_v3, MarketLifecycleRootAccountV3,
    SeriesMarketLinkAccountV3, MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V3,
    SERIES_MARKET_LINK_ACCOUNT_BYTES_V3,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const MARKET_LIFECYCLE_ROOT_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/market-lifecycle-root-authentication/v3\0";
const PRODUCT_MARKET_FOUNDATION_CURRENT_MATERIAL_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-market-foundation-current-material/v4\0";
const PRODUCT_MARKET_FOUNDATION_CURRENT_STEPS_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-market-foundation-current-steps/v4\0";
const PRODUCT_MARKET_FOUNDATION_DEBIT_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-market-foundation-debit/v4\0";
const PRODUCT_MARKET_FOUNDATION_POSTWRITE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-market-foundation-postwrite/v4\0";

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

/// Move-only proof that Product transferred one exact ScheduleV4 principal
/// from the canonical FoundationVault into the next uninitialized GraphV4
/// account. Concrete account owners consume this value in the same
/// instruction and return a typed postwrite; there is no ID-only constructor.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductMarketFoundationDebitV4 {
    id: ContentId,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_steps_id: ContentId,
    market_binding_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    slot: MarketFoundationSlotV4,
    foundation_vault_account: Pubkey,
    destination_account: Pubkey,
    principal_lamports: u64,
    principal_before_lamports: u64,
    principal_after_lamports: u64,
    destination_donation_floor_lamports: u64,
    destination_balance_after_lamports: u64,
    vault_donation_before_lamports: u64,
    vault_donation_after_lamports: u64,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
}

impl AuthenticatedProductMarketFoundationDebitV4 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn founder_creation_receipt_id(&self) -> ContentId {
        self.founder_creation_receipt_id
    }
    pub(crate) const fn founder_preauthorization_id(&self) -> ContentId {
        self.founder_preauthorization_id
    }
    pub(crate) const fn foundation_steps_id(&self) -> ContentId { self.foundation_steps_id }
    pub(crate) const fn market_binding_id(&self) -> ContentId { self.market_binding_id }
    pub(crate) const fn foundation_schedule_id(&self) -> ContentId {
        self.foundation_schedule_id
    }
    pub(crate) const fn foundation_graph_id(&self) -> ContentId { self.foundation_graph_id }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn slot(&self) -> MarketFoundationSlotV4 { self.slot }
    pub(crate) const fn foundation_vault_account(&self) -> Pubkey {
        self.foundation_vault_account
    }
    pub(crate) const fn destination_account(&self) -> Pubkey { self.destination_account }
    pub(crate) const fn principal_lamports(&self) -> u64 { self.principal_lamports }
    pub(crate) const fn principal_before_lamports(&self) -> u64 {
        self.principal_before_lamports
    }
    pub(crate) const fn principal_after_lamports(&self) -> u64 {
        self.principal_after_lamports
    }
    pub(crate) const fn destination_donation_floor_lamports(&self) -> u64 {
        self.destination_donation_floor_lamports
    }
    pub(crate) const fn destination_balance_after_lamports(&self) -> u64 {
        self.destination_balance_after_lamports
    }
    pub(crate) const fn vault_donation_before_lamports(&self) -> u64 {
        self.vault_donation_before_lamports
    }
    pub(crate) const fn vault_donation_after_lamports(&self) -> u64 {
        self.vault_donation_after_lamports
    }
    pub(crate) const fn rent_refund_owner(&self) -> Pubkey { self.rent_refund_owner }
    pub(crate) const fn neutral_lamport_sink(&self) -> Pubkey { self.neutral_lamport_sink }
}

/// Move-only family proof for one exact current 50-slot physical postwrite.
/// Implementations live beside the concrete account writer and must consume
/// their private postwrite before returning the observed donation amount.
pub(crate) trait AuthenticatedProductMarketFoundationStepPostwriteV4: Sized {
    #[allow(clippy::too_many_arguments)]
    fn consume_product_market_foundation_step_postwrite_v4(
        self,
        _debit_id: ContentId,
        _founder_creation_receipt_id: ContentId,
        _founder_preauthorization_id: ContentId,
        _foundation_steps_id: ContentId,
        _market_binding_id: ContentId,
        _foundation_schedule_id: ContentId,
        _foundation_graph_id: ContentId,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _slot: MarketFoundationSlotV4,
        _account_id: ContentId,
        _principal_lamports: u64,
        _principal_before_lamports: u64,
        _principal_after_lamports: u64,
        _destination_donation_floor_lamports: u64,
        _destination_balance_after_lamports: u64,
        _vault_donation_before_lamports: u64,
        _vault_donation_after_lamports: u64,
        _foundation_vault_account: Pubkey,
        _rent_refund_owner: Pubkey,
        _neutral_lamport_sink: Pubkey,
    ) -> Outcome<(ContentId, u64)> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Move-only hostile RootV3 postwrite for one completed action15 stage.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductMarketFoundationStepV4<'state> {
    id: ContentId,
    debit_id: ContentId,
    accepted_poststate_receipt_id: ContentId,
    projection_id: ContentId,
    slot: MarketFoundationSlotV4,
    root_authentication_before_id: ContentId,
    root_semantic_before_id: ContentId,
    root_data_before_id: ContentId,
    root_after: AuthenticatedMarketLifecycleRootV3<'state>,
}

impl<'state> AuthenticatedProductMarketFoundationStepV4<'state> {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn debit_id(&self) -> ContentId { self.debit_id }
    pub(crate) const fn accepted_poststate_receipt_id(&self) -> ContentId {
        self.accepted_poststate_receipt_id
    }
    pub(crate) const fn projection_id(&self) -> ContentId { self.projection_id }
    pub(crate) const fn slot(&self) -> MarketFoundationSlotV4 { self.slot }
    pub(crate) const fn root_authentication_before_id(&self) -> ContentId {
        self.root_authentication_before_id
    }
    pub(crate) const fn root_semantic_before_id(&self) -> ContentId {
        self.root_semantic_before_id
    }
    pub(crate) const fn root_data_before_id(&self) -> ContentId { self.root_data_before_id }
    pub(crate) const fn root_after(&self) -> &AuthenticatedMarketLifecycleRootV3<'state> {
        &self.root_after
    }
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

/// Transfer exactly the next canonical ScheduleV4 principal out of the
/// Market-scoped FoundationVault. The returned debit is instruction-local and
/// must be consumed by a concrete physical slot writer before Product can
/// advance RootV3.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn debit_next_current_product_market_foundation_v4<'a>(
    program_id: &Pubkey,
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    replay: &AuthenticatedMarketLifecycleReplayV2,
    schedule: &MarketFoundationScheduleV4,
    graph: &MarketFoundationAccountGraphV4,
    foundation_vault: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> Outcome<AuthenticatedProductMarketFoundationDebitV4> {
    schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    graph
        .validate(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let state = root.state();
    let binding = state.binding_ref();
    let replay_binding = replay.state().binding();
    let replay_binding_id = replay_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let next_index = next_foundation_slot_index_v4(
        state.foundation().expected_bitmap,
        state.foundation().initialized_bitmap,
    )?
    .ok_or(ClutchError::MismatchedState)?;
    let slot = foundation_slot_v4(next_index)?;
    let account_id = graph
        .account(slot)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = binding.market_instance_id.bytes();
    let (expected_vault, vault_bump) = seeds::product_market_foundation_vault_pda(
        program_id,
        &market,
        binding.generation,
    );
    let slot13_bit = 1_u64
        .checked_shl(
            u32::try_from(
                MarketFoundationSlotV4::ProductReplayAnchor
                    .index()
                    .map_err(|_| ClutchError::MismatchedState)?,
            )
            .map_err(|_| ClutchError::Arithmetic)?,
        )
        .ok_or(ClutchError::Arithmetic)?;
    let expected_replay_phase = if state.foundation().initialized_bitmap & slot13_bit == 0 {
        MarketLifecycleReplayPhaseV2::Founding
    } else {
        MarketLifecycleReplayPhaseV2::FoundationSettled
    };
    require(
        root.is_writable()
            && root.owner_program() == *program_id
            && state.phase() == MarketLifecyclePhaseV3::Founding
            && binding.foundation_schedule_id == schedule_id
            && binding.foundation_account_graph_id == graph_id
            && binding.market_instance_id == graph.market_instance_id
            && binding.generation == graph.generation
            && binding.market_lifecycle_replay_account_id.bytes()
                == replay.account().to_bytes()
            && binding.market_lifecycle_generation_binding_id == replay_binding_id
            && replay_binding.market_instance_id == binding.market_instance_id
            && replay_binding.generation == binding.generation
            && replay_binding.foundation_schedule_id == schedule_id
            && replay_binding.foundation_account_graph_id == graph_id
            && replay.state().phase() == expected_replay_phase
            && slot != MarketFoundationSlotV4::ProductReplayAnchor
            && *foundation_vault.key == expected_vault
            && binding.foundation_vault_id.bytes() == foundation_vault.key.to_bytes()
            && foundation_vault.is_writable
            && !foundation_vault.is_signer
            && !foundation_vault.executable
            && foundation_vault.owner == &SYSTEM_PROGRAM_ID
            && foundation_vault.data_len() == 0
            && destination.is_writable
            && !destination.is_signer
            && !destination.executable
            && destination.owner == &SYSTEM_PROGRAM_ID
            && destination.data_len() == 0
            && destination.key.to_bytes() == account_id.bytes()
            && destination.key != foundation_vault.key
            && *system_program.key == SYSTEM_PROGRAM_ID
            && !system_program.is_writable
            && system_program.executable,
        ClutchError::MismatchedState,
    )?;
    let capital = state.capital();
    let principal_lamports = schedule.slot_principal_lamports[next_index];
    let principal_before_lamports = capital.principal_remaining_lamports;
    let principal_after_lamports = principal_before_lamports
        .checked_sub(principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let vault_lamports_before = foundation_vault.lamports();
    let vault_donation_before_lamports = vault_lamports_before
        .checked_sub(principal_before_lamports)
        .ok_or(ClutchError::SeriesCustodyDeltaMismatch)?;
    let vault_lamports_after = vault_lamports_before
        .checked_sub(principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let destination_donation_floor_lamports = destination.lamports();
    let destination_balance_after_lamports = destination_donation_floor_lamports
        .checked_add(principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        principal_lamports != 0
            && vault_donation_before_lamports >= capital.vault_current_donation_lamports
            && vault_lamports_after
                == principal_after_lamports
                    .checked_add(vault_donation_before_lamports)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let founder_preauthorization_id = replay_binding_id.content_id();
    let market_binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let founder_creation_receipt_id = hashv(&[
        PRODUCT_MARKET_FOUNDATION_CURRENT_MATERIAL_DOMAIN_V4,
        program_id.as_ref(),
        root.account().as_ref(),
        &root.binding_id().bytes(),
        &replay.authentication_id().bytes(),
        &replay_binding.physical_capitalization_receipt_id.bytes(),
        &founder_preauthorization_id.bytes(),
    ]);
    let foundation_steps_id = hashv(&[
        PRODUCT_MARKET_FOUNDATION_CURRENT_STEPS_DOMAIN_V4,
        program_id.as_ref(),
        root.account().as_ref(),
        &market_binding_id.bytes(),
        &schedule_id.bytes(),
        &graph_id.bytes(),
        foundation_vault.key.as_ref(),
        &capital.rent_refund_owner.bytes(),
        &capital.neutral_lamport_sink.bytes(),
    ]);
    require_live(founder_creation_receipt_id)?;
    require_live(foundation_steps_id)?;
    let debit_id = hashv(&[
        PRODUCT_MARKET_FOUNDATION_DEBIT_DOMAIN_V4,
        &founder_creation_receipt_id.bytes(),
        &founder_preauthorization_id.bytes(),
        &foundation_steps_id.bytes(),
        &market_binding_id.bytes(),
        &root.authentication_id().bytes(),
        &root.semantic_id().bytes(),
        &root.data_id().bytes(),
        &state.transition_sequence().to_le_bytes(),
        &state.foundation().transcript_id.bytes(),
        &[u8::try_from(next_index).map_err(|_| ClutchError::Arithmetic)?],
        destination.key.as_ref(),
        &principal_lamports.to_le_bytes(),
        &principal_before_lamports.to_le_bytes(),
        &principal_after_lamports.to_le_bytes(),
        &vault_donation_before_lamports.to_le_bytes(),
        &destination_donation_floor_lamports.to_le_bytes(),
        &destination_balance_after_lamports.to_le_bytes(),
    ]);
    require_live(debit_id)?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(principal_lamports),
        vec![
            AccountMeta::new(*foundation_vault.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[
            foundation_vault.clone(),
            destination.clone(),
            system_program.clone(),
        ],
        &[&[
            seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
            &market,
            &binding.generation.to_le_bytes(),
            &[vault_bump],
        ]],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        foundation_vault.lamports() == vault_lamports_after
            && destination.lamports() == destination_balance_after_lamports,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    Ok(AuthenticatedProductMarketFoundationDebitV4 {
        id: debit_id,
        founder_creation_receipt_id,
        founder_preauthorization_id,
        foundation_steps_id,
        market_binding_id,
        foundation_schedule_id: schedule_id.content_id(),
        foundation_graph_id: graph_id.content_id(),
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        slot,
        foundation_vault_account: *foundation_vault.key,
        destination_account: *destination.key,
        principal_lamports,
        principal_before_lamports,
        principal_after_lamports,
        destination_donation_floor_lamports,
        destination_balance_after_lamports,
        vault_donation_before_lamports,
        vault_donation_after_lamports: vault_donation_before_lamports,
        rent_refund_owner: Pubkey::new_from_array(capital.rent_refund_owner.bytes()),
        neutral_lamport_sink: Pubkey::new_from_array(capital.neutral_lamport_sink.bytes()),
    })
}

/// Consume one concrete physical postwrite, advance RootV3's sole persisted
/// cursor, write the exact successor, and hostile-reopen it. Rollback keeps the
/// preceding FoundationVault transfer and physical writer atomic with this
/// final Product state write.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn record_current_product_market_foundation_step_v4<'state, P>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root_before: AuthenticatedMarketLifecycleRootV3<'_>,
    schedule: &MarketFoundationScheduleV4,
    graph: &MarketFoundationAccountGraphV4,
    debit: AuthenticatedProductMarketFoundationDebitV4,
    postwrite: P,
    successor_state: &mut MarketLifecycleRootV3,
    rebound_output: &'state mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedProductMarketFoundationStepV4<'state>>
where
    P: AuthenticatedProductMarketFoundationStepPostwriteV4,
{
    require(
        root_before.is_writable()
            && root_before.account() == *root_account.key
            && root_before.owner_program() == *program_id
            && root_before.state().phase() == MarketLifecyclePhaseV3::Founding
            && debit.market_instance_id == root_before.binding().market_instance_id
            && debit.generation == root_before.binding().generation
            && debit.market_binding_id == root_before.binding_id()
            && debit.foundation_schedule_id
                == root_before.binding().foundation_schedule_id.content_id()
            && debit.foundation_graph_id
                == root_before.binding().foundation_account_graph_id.content_id()
            && debit.destination_account.to_bytes()
                == graph
                    .account(debit.slot)
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .bytes(),
        ClutchError::MismatchedState,
    )?;
    let (accepted_poststate_receipt_id, observed_donation_lamports) = postwrite
        .consume_product_market_foundation_step_postwrite_v4(
            debit.id,
            debit.founder_creation_receipt_id,
            debit.founder_preauthorization_id,
            debit.foundation_steps_id,
            debit.market_binding_id,
            debit.foundation_schedule_id,
            debit.foundation_graph_id,
            debit.market_instance_id,
            debit.generation,
            debit.slot,
            ContentId::from_bytes(debit.destination_account.to_bytes()),
            debit.principal_lamports,
            debit.principal_before_lamports,
            debit.principal_after_lamports,
            debit.destination_donation_floor_lamports,
            debit.destination_balance_after_lamports,
            debit.vault_donation_before_lamports,
            debit.vault_donation_after_lamports,
            debit.foundation_vault_account,
            debit.rent_refund_owner,
            debit.neutral_lamport_sink,
        )?;
    require(
        !accepted_poststate_receipt_id.is_zero()
            && observed_donation_lamports == debit.vault_donation_after_lamports,
        ClutchError::MismatchedState,
    )?;
    let projection = MarketFoundationStepProjectionV4 {
        binding_id: debit.market_binding_id,
        slot: debit.slot,
        root_transition_sequence: root_before
            .state()
            .transition_sequence()
            .checked_add(1)
            .ok_or(ClutchError::Arithmetic)?,
        principal_lamports: debit.principal_lamports,
        principal_before_lamports: debit.principal_before_lamports,
        principal_after_lamports: debit.principal_after_lamports,
        donation_before_lamports: observed_donation_lamports,
        donation_after_lamports: observed_donation_lamports,
        account_id: ContentId::from_bytes(debit.destination_account.to_bytes()),
        accepted_poststate_receipt_id,
    };
    let projection_id = projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    root_before
        .state()
        .record_foundation_step_into(schedule, graph, projection, successor_state)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let authentication_before_id = root_before.authentication_id();
    let semantic_before_id = root_before.semantic_id();
    let data_before_id = root_before.data_id();
    let rent_principal_lamports = root_before.value().rent_principal_lamports;
    let stored_bump = root_before.value().stored_bump;
    {
        let data = root_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            root_account.lamports() == root_before.observed_lamports()
                && hash_data(&data) == data_before_id,
            ClutchError::MismatchedState,
        )?;
    }
    {
        let mut data = root_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        MarketLifecycleRootAccountV3::encode_parts(
            successor_state,
            rent_principal_lamports,
            stored_bump,
            &mut data,
        )?;
    }
    let root_after = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        debit.market_instance_id,
        debit.generation,
        true,
        rebound_output,
    )?;
    require(
        root_after.state() == successor_state
            && root_after.observed_lamports() == root_before.observed_lamports()
            && root_after.authentication_id() != authentication_before_id
            && root_after.semantic_id() != semantic_before_id
            && root_after.data_id() != data_before_id
            && root_after.state().transition_sequence()
                == root_before
                    .state()
                    .transition_sequence()
                    .checked_add(1)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_MARKET_FOUNDATION_POSTWRITE_DOMAIN_V4,
        &debit.id.bytes(),
        &accepted_poststate_receipt_id.bytes(),
        &projection_id.bytes(),
        root_account.key.as_ref(),
        &authentication_before_id.bytes(),
        &root_after.authentication_id().bytes(),
        &semantic_before_id.bytes(),
        &root_after.semantic_id().bytes(),
        &data_before_id.bytes(),
        &root_after.data_id().bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductMarketFoundationStepV4 {
        id,
        debit_id: debit.id,
        accepted_poststate_receipt_id,
        projection_id,
        slot: debit.slot,
        root_authentication_before_id: authentication_before_id,
        root_semantic_before_id: semantic_before_id,
        root_data_before_id: data_before_id,
        root_after,
    })
}

fn next_foundation_slot_index_v4(
    expected_bitmap: u64,
    initialized_bitmap: u64,
) -> Outcome<Option<usize>> {
    let remaining = expected_bitmap & !initialized_bitmap;
    if remaining == 0 {
        return Ok(None);
    }
    let index = usize::try_from(remaining.trailing_zeros()).map_err(|_| ClutchError::Arithmetic)?;
    require(index < MARKET_FOUNDATION_SLOT_COUNT_V4, ClutchError::MismatchedState)?;
    Ok(Some(index))
}

fn foundation_slot_v4(index: usize) -> Outcome<MarketFoundationSlotV4> {
    match index {
        0 => Ok(MarketFoundationSlotV4::LifecycleRoot),
        1 => Ok(MarketFoundationSlotV4::MarketBinding),
        2 => Ok(MarketFoundationSlotV4::MarketRuntime),
        3 => Ok(MarketFoundationSlotV4::Hoard),
        4 => Ok(MarketFoundationSlotV4::ClaimLedger),
        5 => Ok(MarketFoundationSlotV4::FailureAdmissionRoot),
        6 => Ok(MarketFoundationSlotV4::FailureRuntimeRoot),
        7 => Ok(MarketFoundationSlotV4::FailureReplay),
        8 => Ok(MarketFoundationSlotV4::FailureIntervalWork),
        9 => Ok(MarketFoundationSlotV4::FailureIntervalHistory),
        10 => Ok(MarketFoundationSlotV4::ResolutionV5),
        11 => Ok(MarketFoundationSlotV4::FractionalPolicy),
        12 => Ok(MarketFoundationSlotV4::FractionalLedger),
        13 => Ok(MarketFoundationSlotV4::ProductReplayAnchor),
        14 => Ok(MarketFoundationSlotV4::HoardCollateralVault),
        15..=30 => Ok(MarketFoundationSlotV4::OutcomeMint(
            u8::try_from(index - 15).map_err(|_| ClutchError::Arithmetic)?,
        )),
        31..=46 => Ok(MarketFoundationSlotV4::OutcomeCustody(
            u8::try_from(index - 31).map_err(|_| ClutchError::Arithmetic)?,
        )),
        47 => Ok(MarketFoundationSlotV4::GeneralTreasuryPosition),
        48 => Ok(MarketFoundationSlotV4::GeneralTreasuryReplay),
        49 => Ok(MarketFoundationSlotV4::TreasuryServiceLedger),
        _ => Err(Refusal::Adapter(ClutchError::MismatchedState)),
    }
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
