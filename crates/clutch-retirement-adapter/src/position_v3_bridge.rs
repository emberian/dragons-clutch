// SPDX-License-Identifier: AGPL-3.0-or-later

//! Authenticated close and generation-safe reopen bridge for global Position V3.
//!
//! This module produces complete byte images and checked lamport schedules. It
//! does not execute writes or CPI. A live SBF route must prepare every account
//! in the bundle before its first mutation and commit the returned schedule
//! atomically. Legacy Position versions remain outside this successor bridge.

use clutch_collateral_adapter_v2::BoundCollateralProfileV2;
use clutch_retirement::{
    admit_deletable_rent, admit_reopen_rent_split, plan_position_v3_replay_v3_retirement_v1,
    reopen_position_with_replay_v2, AdapterNeutralSinkBindingProjectionV1,
    AdapterPositionAccountProjectionV1, AdapterPositionMarketBindingV3,
    AdapterReplayAbsenceProjectionV1, AdapterReplayAccountProjectionV1,
    CoalescedRecipientCreditsV1, Identity32V1, PositionLifecycleStateV2,
    PositionReplayReopenAccountsV1, PositionReplayReopenPlanV2, PositionReplayReopenRequestV1,
    PositionReplayReopenRequestV2, PositionTombstoneV1, PositionTombstoneV3, PositionV3Fields,
    PositionV3ReplayV3AccountsV1, PositionV3ReplayV3RetirementRequestV1, RecipientBalanceBookV1,
    RecipientBalanceV1, ReplayV3Envelope, ReplayV3HashBackend, ReplayV3Lifecycle,
    RetirementErrorV2, MAX_OUTCOMES, MAX_RETIREMENT_RECIPIENTS, POSITION_TOMBSTONE_V3_BYTES,
    POSITION_V3_BYTES,
};

use crate::{AuthenticatedAccountV2, CanonicalPdaV1};
use crate::{
    FundingPayerViewV1, NeutralSinkBalanceViewV1, RetirementAdapterErrorV2,
    RetirementRecipientViewV1, VacantPdaAccountViewV2,
};

const SYSTEM_PROGRAM_OWNER: [u8; 32] = [0; 32];

fn identity(bytes: [u8; 32]) -> Result<Identity32V1, RetirementAdapterErrorV2> {
    Ok(Identity32V1::new(bytes)?)
}

/// Immutable Realm-selected retirement context joined to collateral authority.
///
/// [`BoundCollateralProfileV2`] proves the Market → Realm → policy → compiled
/// release chain. The neutral sink is intentionally a separate Realm-owned
/// fact: the live adapter must read it from that same authenticated immutable
/// Realm account before calling this constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionV3RetirementRealmV1 {
    market_instance_id: Identity32V1,
    outcome_count: u8,
    realm_id: Identity32V1,
    collateral_policy_id: Identity32V1,
    collateral_release_id: Identity32V1,
    neutral_sink: Identity32V1,
}

impl PositionV3RetirementRealmV1 {
    /// Join canonical collateral authority with the outcome width read from
    /// the authenticated MarketInstance and the neutral sink read from the
    /// same authenticated immutable Realm semantic owner.
    pub fn after_immutable_realm_authentication(
        collateral: BoundCollateralProfileV2,
        authenticated_market_outcome_count: u8,
        neutral_sink: Identity32V1,
    ) -> Result<Self, RetirementAdapterErrorV2> {
        let market = collateral.market();
        let realm = identity(market.realm.bytes())?;
        let market_instance = identity(market.market.bytes())?;
        let policy = identity(collateral.policy_id().bytes())?;
        let release = identity(collateral.release().id()?.bytes())?;
        if authenticated_market_outcome_count == 0
            || usize::from(authenticated_market_outcome_count) > MAX_OUTCOMES
            || neutral_sink == market_instance
            || neutral_sink == realm
            || neutral_sink == policy
            || neutral_sink == release
        {
            return Err(RetirementErrorV2::AccountAlias.into());
        }
        Ok(Self {
            market_instance_id: market_instance,
            outcome_count: authenticated_market_outcome_count,
            realm_id: realm,
            collateral_policy_id: policy,
            collateral_release_id: release,
            neutral_sink,
        })
    }

    /// Exact immutable neutral destination for unowned lamports.
    pub const fn neutral_sink(self) -> Identity32V1 {
        self.neutral_sink
    }

    fn authenticate_position(
        self,
        position: clutch_retirement::PositionAccountV3,
    ) -> Result<(), RetirementAdapterErrorV2> {
        let market = AdapterPositionMarketBindingV3 {
            market_instance_id: self.market_instance_id,
            outcome_count: self.outcome_count,
            realm_id: self.realm_id,
            collateral_policy_id: self.collateral_policy_id,
            collateral_release_id: self.collateral_release_id,
        };
        if position.market_instance_id() != market.market_instance_id
            || position.outcome_count() != market.outcome_count
            || position.realm_id() != market.realm_id
            || position.collateral_policy_id() != market.collateral_policy_id
            || position.collateral_release_id() != market.collateral_release_id
        {
            return Err(RetirementErrorV2::WrongParent.into());
        }
        Ok(())
    }
}

/// Complete authenticated inputs for one Position V3 plus Replay close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayCloseRuntimeRequestV4<'a> {
    /// Exact writable Position V3 account authentication result.
    pub position: AuthenticatedAccountV2<'a>,
    /// Exact writable current-generation Replay authentication result.
    pub replay: AuthenticatedAccountV2<'a>,
    /// Authenticated immutable Realm/collateral retirement binding.
    pub realm: PositionV3RetirementRealmV1,
    /// Sequence authenticated from the signed close instruction envelope.
    /// It must equal the terminal Replay's next expected ordinal.
    pub signed_sequence: u64,
    /// Actual Position lamports before retirement.
    pub position_lamports: u64,
    /// Actual Replay lamports before deletion.
    pub replay_lamports: u64,
    /// Runtime recipient balances for both persisted payers and neutral sink.
    pub recipients: [Option<RetirementRecipientViewV1>; MAX_RETIREMENT_RECIPIENTS],
}

/// Complete prospective V3 close image and lamport schedule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedPositionReplayCloseV3 {
    position_account: Identity32V1,
    replay_account: Identity32V1,
    replay_terminal_semantic_id: Identity32V1,
    signed_sequence: u64,
    position_lamports_before: u64,
    replay_lamports_before: u64,
    position_tombstone_bytes: [u8; POSITION_TOMBSTONE_V3_BYTES],
    recipient_credits: CoalescedRecipientCreditsV1,
    position_lamports_after: u64,
    replay_lamports_after: u64,
}

impl PreparedPositionReplayCloseV3 {
    /// Position PDA rewritten to the permanent V3 tombstone image.
    pub const fn position_account(&self) -> Identity32V1 {
        self.position_account
    }

    /// Current-generation Replay PDA deleted in the same atomic commit.
    pub const fn replay_account(&self) -> Identity32V1 {
        self.replay_account
    }

    /// Semantic ID of the exact terminal Replay bytes authenticated before
    /// deletion. The permanent Position tombstone retains the deleted Replay
    /// address and generation for replay refusal.
    pub const fn replay_terminal_semantic_id(&self) -> Identity32V1 {
        self.replay_terminal_semantic_id
    }

    /// Exact sequence authenticated from the signed terminal instruction.
    pub const fn signed_sequence(&self) -> u64 {
        self.signed_sequence
    }

    /// Position lamports authenticated before any retirement mutation.
    pub const fn position_lamports_before(&self) -> u64 {
        self.position_lamports_before
    }

    /// Replay lamports authenticated before any retirement mutation.
    pub const fn replay_lamports_before(&self) -> u64 {
        self.replay_lamports_before
    }

    /// Exact canonical Position V3 tombstone bytes.
    pub const fn position_tombstone_bytes(&self) -> [u8; POSITION_TOMBSTONE_V3_BYTES] {
        self.position_tombstone_bytes
    }

    /// Alias-coalesced payer and neutral-sink credits.
    pub const fn recipient_credits(&self) -> CoalescedRecipientCreditsV1 {
        self.recipient_credits
    }

    /// Exact permanent tombstone balance retained after close.
    pub const fn position_lamports_after(&self) -> u64 {
        self.position_lamports_after
    }

    /// Replay must be physically absent after the same commit.
    pub const fn replay_lamports_after(&self) -> u64 {
        self.replay_lamports_after
    }
}

fn recipient_book(
    views: [Option<RetirementRecipientViewV1>; MAX_RETIREMENT_RECIPIENTS],
) -> Result<RecipientBalanceBookV1, RetirementAdapterErrorV2> {
    let mut entries = [None; MAX_RETIREMENT_RECIPIENTS];
    let mut index = 0usize;
    while index < views.len() {
        if let Some(view) = views[index] {
            if !view.is_writable {
                return Err(RetirementAdapterErrorV2::NotWritable);
            }
            if view.is_executable {
                return Err(RetirementAdapterErrorV2::ExecutableAccount);
            }
            entries[index] = Some(RecipientBalanceV1 {
                recipient: view.address,
                balance_before: view.lamports,
            });
        }
        index += 1;
    }
    Ok(RecipientBalanceBookV1 { entries })
}

/// Authenticate and prepare the only Position V3 close shape.
///
/// An economically empty Position is insufficient by itself: this also
/// consumes the exact persisted Replay sibling, binds the signed sequence,
/// authenticates the full Realm/collateral identity, coalesces payer credits,
/// and retains only the V3 tombstone principal.
pub fn authenticate_and_prepare_position_replay_close_v4<B: ReplayV3HashBackend>(
    request: PositionReplayCloseRuntimeRequestV4<'_>,
    hash_backend: &B,
) -> Result<PreparedPositionReplayCloseV3, RetirementAdapterErrorV2> {
    if !request.position.is_writable() || !request.replay.is_writable() {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }
    if request.position.program_id() != request.replay.program_id() {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if request.position.address() == request.replay.address() {
        return Err(RetirementErrorV2::AccountAlias.into());
    }

    let position = clutch_retirement::PositionAccountV3::decode(request.position.data())?;
    request.realm.authenticate_position(position)?;
    let terminal = position.terminal_projection()?;
    if position.replay_account() != request.replay.address() {
        return Err(RetirementErrorV2::ReplayMismatch.into());
    }

    let replay = ReplayV3Envelope::decode(request.replay.data(), hash_backend)?;
    let replay_terminal = replay.terminal_projection()?;
    let replay_header = replay_terminal.header();
    if replay_header.position_account() != request.position.address()
        || replay_header.replay_account() != request.replay.address()
        || replay_header.purpose() != position.purpose()
        || replay_header.purpose_binding_id() != position.purpose_binding_id()
        || replay_header.position_generation() != position.generation()
        || replay_header.next_sequence() != request.signed_sequence
        || replay_header.stored_bump() != request.replay.bump()
    {
        return Err(RetirementErrorV2::ReplayMismatch.into());
    }
    let book = recipient_book(request.recipients)?;
    let plan = plan_position_v3_replay_v3_retirement_v1(
        PositionV3ReplayV3RetirementRequestV1 {
            position: terminal,
            replay: replay_terminal,
            position_balance: request.position_lamports,
            replay_balance: request.replay_lamports,
            neutral_sink: request.realm.neutral_sink,
            accounts: PositionV3ReplayV3AccountsV1 {
                position: request.position.address(),
                replay: request.replay.address(),
            },
            recipient_balances: book,
            signed_sequence: request.signed_sequence,
        },
        hash_backend,
    )?;
    if plan.position_balance_after
        != plan
            .position_tombstone
            .fields()
            .permanent_tombstone_principal
        || plan.replay_balance_after != 0
    {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }
    Ok(PreparedPositionReplayCloseV3 {
        position_account: request.position.address(),
        replay_account: request.replay.address(),
        replay_terminal_semantic_id: plan.terminal_replay_semantic_id,
        signed_sequence: request.signed_sequence,
        position_lamports_before: request.position_lamports,
        replay_lamports_before: request.replay_lamports,
        position_tombstone_bytes: plan.position_tombstone.encode()?,
        recipient_credits: plan.recipient_credits,
        position_lamports_after: plan.position_balance_after,
        replay_lamports_after: plan.replay_balance_after,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VacantPdaV3 {
    address: Identity32V1,
    lamports: u64,
    bump: u8,
}

fn vacant(
    view: VacantPdaAccountViewV2,
    pda: CanonicalPdaV1,
) -> Result<VacantPdaV3, RetirementAdapterErrorV2> {
    if view.address != pda.address() {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if !view.is_writable {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }
    if view.owner != SYSTEM_PROGRAM_OWNER || view.data_len != 0 || view.is_executable {
        return Err(RetirementAdapterErrorV2::AccountNotAbsent);
    }
    Ok(VacantPdaV3 {
        address: view.address,
        lamports: view.lamports,
        bump: pda.bump(),
    })
}

fn funding_payer(
    payer: FundingPayerViewV1,
) -> Result<FundingPayerViewV1, RetirementAdapterErrorV2> {
    if payer.owner != SYSTEM_PROGRAM_OWNER || payer.is_executable {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if !payer.is_signer {
        return Err(RetirementAdapterErrorV2::MissingSigner);
    }
    if !payer.is_writable {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }
    Ok(payer)
}

fn refundable_rent(minima: PositionReplayRentMinimumsV3) -> Result<u64, RetirementAdapterErrorV2> {
    if minima.position_tombstone == 0 || minima.replay == 0 {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }
    let value = minima
        .position_live
        .checked_sub(minima.position_tombstone)
        .ok_or(RetirementErrorV2::NonCanonicalState)?;
    if value == 0 {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }
    Ok(value)
}

/// Rent minima for the exact canonical V3 live/tombstone/Replay byte widths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayRentMinimumsV3 {
    /// Rent minimum for the 480-byte live Position V3 account.
    pub position_live: u64,
    /// Rent minimum permanently retained by the 280-byte V3 tombstone.
    pub position_tombstone: u64,
    /// Rent minimum for this purpose's exact Replay V3 envelope and extension.
    pub replay: u64,
}

/// Complete authenticated inputs for a V3 tombstone reopen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayReopenRuntimeRequestV3<'a> {
    /// Exact writable V3 Position tombstone authentication result.
    pub position_tombstone: AuthenticatedAccountV2<'a>,
    /// Actual tombstone balance, including any later unsolicited lamports.
    pub position_lamports: u64,
    /// Read-only canonical absence proof for the just-closed Replay generation.
    pub prior_replay: crate::AbsentAccountViewV1,
    /// Exact balance of the absent prior Replay role; absence requires zero.
    pub prior_replay_lamports: u64,
    /// Canonical prior-generation Replay PDA derivation.
    pub prior_replay_pda: CanonicalPdaV1,
    /// Writable vacant next-generation Replay PDA, including hostile prefund.
    pub next_replay: VacantPdaAccountViewV2,
    /// Canonical next-generation Replay PDA derivation.
    pub next_replay_pda: CanonicalPdaV1,
    /// Immutable Realm/collateral retirement binding.
    pub realm: PositionV3RetirementRealmV1,
    /// Position live-rent payer.
    pub position_payer: FundingPayerViewV1,
    /// Replay rent payer.
    pub replay_payer: FundingPayerViewV1,
    /// Authenticated neutral-sink runtime balance.
    pub neutral_sink: NeutralSinkBalanceViewV1,
    /// Exact rent minima for Position V3, tombstone V3, and Replay successor.
    pub rent_minimums: PositionReplayRentMinimumsV3,
    /// Complete prospective next-generation Position V3 body.
    pub position_after: clutch_retirement::PositionAccountV3,
    /// Complete purpose-owned next-generation Replay V3 envelope.
    ///
    /// Its purpose handler owns and has already validated the extension. This
    /// bridge authenticates the common Position/replay/binding/generation/rent
    /// joins and full extension hash without interpreting those bytes.
    pub replay_after: ReplayV3Envelope<'a>,
}

/// Complete prospective Position V3 reopen images and checked funding plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedPositionReplayReopenV3<'a> {
    position_account: Identity32V1,
    prior_replay: Identity32V1,
    next_replay: Identity32V1,
    position_bytes_after: [u8; POSITION_V3_BYTES],
    replay_after: ReplayV3Envelope<'a>,
    replay_semantic_id: Identity32V1,
    plan: PositionReplayReopenPlanV2,
}

impl PreparedPositionReplayReopenV3<'_> {
    /// Permanent Position PDA rewritten from tombstone to live V3 bytes.
    pub const fn position_account(self) -> Identity32V1 {
        self.position_account
    }

    /// Closed-generation Replay PDA whose exact absence remains required.
    pub const fn prior_replay(self) -> Identity32V1 {
        self.prior_replay
    }

    /// Fresh next-generation Replay PDA created atomically.
    pub const fn next_replay(self) -> Identity32V1 {
        self.next_replay
    }

    /// Complete canonical next Position V3 image.
    pub const fn position_bytes_after(self) -> [u8; POSITION_V3_BYTES] {
        self.position_bytes_after
    }

    /// Exact total byte width of the next purpose-owned Replay.
    pub fn replay_bytes_after_len(self) -> Result<usize, RetirementErrorV2> {
        self.replay_after.encoded_len()
    }

    /// Semantic ID of the complete next Replay envelope and extension.
    pub const fn replay_semantic_id(self) -> Identity32V1 {
        self.replay_semantic_id
    }

    /// Encode the complete prospective Replay image into an exact scratch
    /// buffer before any account mutation starts.
    pub fn encode_replay_after<B: ReplayV3HashBackend>(
        self,
        output: &mut [u8],
        hash_backend: &B,
    ) -> Result<(), RetirementErrorV2> {
        self.replay_after.encode_into(output, hash_backend)
    }

    /// Checked full-principal, prefund, payer-coalescing, and sink plan.
    pub const fn plan(self) -> PositionReplayReopenPlanV2 {
        self.plan
    }
}

fn same_retained_identity(tombstone: PositionTombstoneV3, next: PositionV3Fields) -> bool {
    let prior = tombstone.fields();
    next.purpose == prior.purpose
        && next.stored_bump == prior.stored_bump
        && next.market_instance_id == prior.market_instance_id
        && next.realm_id == prior.realm_id
        && next.collateral_policy_id == prior.collateral_policy_id
        && next.collateral_release_id == prior.collateral_release_id
        && next.owner == prior.owner
        && next.controller == prior.controller
        && next.purpose_binding_id == prior.purpose_binding_id
}

/// Authenticate and prepare a generation-safe Position V3 tombstone reopen.
pub fn authenticate_and_prepare_position_replay_reopen_v3<'a, B: ReplayV3HashBackend>(
    request: PositionReplayReopenRuntimeRequestV3<'a>,
    hash_backend: &B,
) -> Result<PreparedPositionReplayReopenV3<'a>, RetirementAdapterErrorV2> {
    if !request.position_tombstone.is_writable() {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }
    let tombstone = PositionTombstoneV3::decode(request.position_tombstone.data())?;
    let tombstone_fields = tombstone.fields();
    let next = request.position_after;
    let next_fields = next.fields();
    request.realm.authenticate_position(next)?;
    if request.realm.market_instance_id != tombstone_fields.market_instance_id
        || request.realm.realm_id != tombstone_fields.realm_id
        || request.realm.collateral_policy_id != tombstone_fields.collateral_policy_id
        || request.realm.collateral_release_id != tombstone_fields.collateral_release_id
        || !same_retained_identity(tombstone, next_fields)
    {
        return Err(RetirementErrorV2::WrongParent.into());
    }
    let next_generation = tombstone_fields
        .generation
        .checked_add(1)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    if next_fields.generation != next_generation
        || next_fields.lifecycle != clutch_retirement::PositionLifecycleV3::Open
        || next_fields.cash_atoms != 0
        || next_fields.reserved_cash_atoms != 0
        || next_fields.native_eggs != [0; MAX_OUTCOMES]
        || next_fields.outstanding_reservations != 0
    {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }

    let market = tombstone_fields.market_instance_id;
    let owner = tombstone_fields.owner;
    if request.prior_replay.address != request.prior_replay_pda.address()
        || request.prior_replay.address != tombstone_fields.replay_account
        || request.prior_replay.is_writable
        || request.prior_replay.owner != SYSTEM_PROGRAM_OWNER
        || request.prior_replay.data_len != 0
        || request.prior_replay.is_executable
        || request.prior_replay_lamports != 0
    {
        return Err(RetirementErrorV2::ReplayMismatch.into());
    }
    let prior_replay = request.prior_replay.address;
    let next_vacant = vacant(request.next_replay, request.next_replay_pda)?;
    if next_vacant.address != next_fields.replay_account
        || next_vacant.address == request.position_tombstone.address()
        || next_vacant.address == prior_replay
    {
        return Err(RetirementErrorV2::AccountAlias.into());
    }
    if request.neutral_sink.address != request.realm.neutral_sink {
        return Err(RetirementErrorV2::WrongNeutralSink.into());
    }
    if !request.neutral_sink.is_writable || request.neutral_sink.is_executable {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }
    if request.position_tombstone.program_id() == request.realm.neutral_sink
        || request.position_tombstone.address() == request.realm.neutral_sink
        || next_vacant.address == request.realm.neutral_sink
    {
        return Err(RetirementErrorV2::AccountAlias.into());
    }

    let position_payer = funding_payer(request.position_payer)?;
    let replay_payer = funding_payer(request.replay_payer)?;
    let refundable = refundable_rent(request.rent_minimums)?;
    if request.rent_minimums.position_tombstone != tombstone_fields.permanent_tombstone_principal {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }
    let position_funding = admit_reopen_rent_split(
        request.position_tombstone.address(),
        position_payer.address,
        refundable,
        tombstone_fields.permanent_tombstone_principal,
        request.position_lamports,
        position_payer.lamports,
        request.realm.neutral_sink,
    )?;
    let replay_funding = admit_deletable_rent(
        next_vacant.address,
        replay_payer.address,
        request.rent_minimums.replay,
        next_vacant.lamports,
        replay_payer.lamports,
        request.realm.neutral_sink,
    )?;
    if next_fields.rent != position_funding.rent() {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }
    let replay = request.replay_after;
    let replay_header = replay.header();
    if replay_header.lifecycle() != ReplayV3Lifecycle::Live
        || replay_header.position_account() != request.position_tombstone.address()
        || replay_header.replay_account() != next_vacant.address
        || replay_header.purpose() != next_fields.purpose
        || replay_header.purpose_binding_id() != next_fields.purpose_binding_id
        || replay_header.position_generation() != next_generation
        || replay_header.next_sequence() != 0
        || replay_header.stored_bump() != next_vacant.bump
        || replay_header.rent() != replay_funding.rent()
    {
        return Err(RetirementErrorV2::ReplayMismatch.into());
    }
    let replay_semantic_id = replay.semantic_id(hash_backend)?;

    let reopen = PositionReplayReopenRequestV1 {
        position: PositionLifecycleStateV2::Tombstone(PositionTombstoneV1 {
            market,
            owner,
            generation: tombstone_fields.generation,
            stored_bump: tombstone_fields.stored_bump,
        }),
        prior_replay: AdapterReplayAbsenceProjectionV1 {
            account: prior_replay,
            market,
            owner,
            position_generation: tombstone_fields.generation,
        },
        position_funding,
        replay_stored_bump: next_vacant.bump,
        replay_funding,
        neutral_sink: request.realm.neutral_sink,
        neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1 {
            market,
            neutral_sink: request.realm.neutral_sink,
        },
        accounts: PositionReplayReopenAccountsV1 {
            position: AdapterPositionAccountProjectionV1 {
                account: request.position_tombstone.address(),
                market,
                owner,
            },
            next_replay: AdapterReplayAccountProjectionV1 {
                account: next_vacant.address,
                market,
                owner,
                position_generation: next_generation,
            },
        },
    };
    let plan = reopen_position_with_replay_v2(PositionReplayReopenRequestV2 {
        reopen,
        prior_replay_prefund_lamports: 0,
        neutral_sink_balance_before: request.neutral_sink.lamports,
    })?;
    Ok(PreparedPositionReplayReopenV3 {
        position_account: request.position_tombstone.address(),
        prior_replay,
        next_replay: next_vacant.address,
        position_bytes_after: next.encode()?,
        replay_after: replay,
        replay_semantic_id,
        plan,
    })
}
