// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_retirement::{
    canonical_epoch_generation, AdapterDirectEpochProjectionV1,
    AdapterNeutralSinkBindingProjectionV1, ChildGenerationV1, DeletableRentOwnerV1,
    DirectEpochLifecyclePhaseV1, EpochRetirementTailV1, GeneralEpochPhaseV2, Identity32V1,
    LiveGeneralEpochProjectionV2, LivePositionV2, LiveReplaySuccessorV1, MarketEpochCursorV1,
    PositionEconomicStateV1, PositionLifecycleStateV2, PositionRetirementTailV1,
    ReplayLifecycleStateV1, ReservationCountTailV1, ReservationRetirementTailV2, RetirementErrorV1,
    RetirementErrorV2, DIRECT_RESERVATION_ACCOUNT_VERSION_V6,
    DIRECT_RESERVATION_ACCOUNT_VERSION_V8, DIRECT_RESERVATION_V2_BYTES,
    DIRECT_RESERVATION_V6_BYTES, DIRECT_RESERVATION_V8_BYTES, EPOCH_ACCOUNT_TAG,
    EPOCH_ACCOUNT_VERSION_V5, EPOCH_V2_BYTES, EPOCH_V5_BYTES, MARKET_ACCOUNT_TAG,
    MARKET_ACCOUNT_VERSION_V2, MARKET_V1_BYTES, MARKET_V2_BYTES, POSITION_ACCOUNT_TAG,
    POSITION_ACCOUNT_VERSION_V2, POSITION_V1_BYTES, POSITION_V2_BYTES,
    PROJECTED_REPLAY_SUCCESSOR_BYTES, REFERENCE_REPLAY_V1_BYTES, RESERVATION_ACCOUNT_TAG,
    RESERVATION_ACCOUNT_VERSION_V5, RESERVATION_ACCOUNT_VERSION_V7, RESERVATION_V4_BYTES,
    RESERVATION_V5_BYTES, RESERVATION_V7_BYTES,
};
use clutch_solana_layout::registry::{
    REPLAY_SUCCESSOR_ACCOUNT_TAG, REPLAY_SUCCESSOR_ACCOUNT_VERSION,
};
use clutch_solana_layout::{
    account_len, account_version,
    direct_selection_v3::{
        DirectReservationV2Account, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY,
        DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN, DIRECT_LIFECYCLE_PHASE_SELECTED,
        DIRECT_LIFECYCLE_PHASE_TERMINAL, DIRECT_LIFECYCLE_PHASE_VERIFYING,
        DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN,
    },
    reservation::{ReservationAccount, RESERVATION_STATE_ACTIVE, RESERVATION_STATE_ENTITLED},
    EpochAccount, MarketAccount, PositionAccount, EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN,
    EPOCH_PHASE_LAPSED, EPOCH_PHASE_OPEN, EPOCH_PHASE_SETTLED,
};
use clutch_solana_reference::{
    ReplayAccount, REPLAY_ACCOUNT_LEN, REPLAY_ACCOUNT_TAG, REPLAY_ACCOUNT_VERSION,
};

use crate::{
    AuthenticatedAccountV1, AuthenticatedAccountV2, CountedChildSchemaV1, RetirementAdapterErrorV1,
    RetirementAdapterErrorV2,
};

fn identity(value: clutch_solana_layout::Hash32) -> Result<Identity32V1, RetirementAdapterErrorV1> {
    Identity32V1::new(value.bytes()).map_err(Into::into)
}

fn project_direct_epoch_lifecycle_phase_v1(
    phase: u8,
) -> Result<DirectEpochLifecyclePhaseV1, RetirementAdapterErrorV2> {
    match phase {
        DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN => Ok(DirectEpochLifecyclePhaseV1::PrefreezeOpen),
        DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY => Ok(DirectEpochLifecyclePhaseV1::FrozenEmpty),
        DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN => Ok(DirectEpochLifecyclePhaseV1::WindowOpen),
        DIRECT_LIFECYCLE_PHASE_VERIFYING => Ok(DirectEpochLifecyclePhaseV1::Verifying),
        DIRECT_LIFECYCLE_PHASE_SELECTED => Ok(DirectEpochLifecyclePhaseV1::Selected),
        DIRECT_LIFECYCLE_PHASE_TERMINAL => Ok(DirectEpochLifecyclePhaseV1::Terminal),
        _ => Err(RetirementErrorV2::InvalidEnum.into()),
    }
}

/// Project the authoritative five-state general-Epoch wire phase without an
/// unchecked cast or permissive fallback.
pub fn project_general_epoch_phase_v2(
    phase: u8,
) -> Result<GeneralEpochPhaseV2, RetirementAdapterErrorV2> {
    match phase {
        EPOCH_PHASE_OPEN => Ok(GeneralEpochPhaseV2::Open),
        EPOCH_PHASE_FROZEN => Ok(GeneralEpochPhaseV2::Frozen),
        EPOCH_PHASE_CLEARED => Ok(GeneralEpochPhaseV2::Cleared),
        EPOCH_PHASE_SETTLED => Ok(GeneralEpochPhaseV2::Settled),
        EPOCH_PHASE_LAPSED => Ok(GeneralEpochPhaseV2::Lapsed),
        _ => Err(RetirementErrorV2::InvalidEnum.into()),
    }
}

fn exact(input: &[u8], expected: usize) -> Result<(), RetirementAdapterErrorV1> {
    if input.len() < expected {
        Err(RetirementErrorV1::Truncated.into())
    } else if input.len() > expected {
        Err(RetirementErrorV1::TrailingBytes.into())
    } else {
        Ok(())
    }
}

fn header(input: &[u8], tag: u8, version: u8) -> Result<(), RetirementAdapterErrorV1> {
    if input[0] != tag {
        return Err(RetirementErrorV1::WrongTag.into());
    }
    if input[1] != version {
        return Err(RetirementErrorV1::WrongVersion.into());
    }
    Ok(())
}

fn checked_base(
    input: &[u8],
    full_len: usize,
    base_len: usize,
    tag: u8,
    promoted_version: u8,
    legacy_version: u8,
    output: &mut [u8],
) -> Result<(), RetirementAdapterErrorV1> {
    exact(input, full_len)?;
    exact(output, base_len)?;
    header(input, tag, promoted_version)?;
    output.copy_from_slice(&input[..base_len]);
    output[1] = legacy_version;
    Ok(())
}

fn validate_position_count_marker(
    base_state: u8,
    count: ReservationCountTailV1,
) -> Result<(), RetirementAdapterErrorV1> {
    count.validate()?;
    let expected = matches!(
        base_state,
        RESERVATION_STATE_ACTIVE | RESERVATION_STATE_ENTITLED
    );
    if count.position_counted != expected {
        return Err(RetirementErrorV1::NonCanonicalState.into());
    }
    Ok(())
}

fn validate_direct_funding_mirror(
    funding: clutch_solana_layout::direct_selection_v3::DirectFundingLedgerV3,
    rent: clutch_retirement::DeletableRentOwnerV1,
) -> Result<(), RetirementAdapterErrorV2> {
    rent.validate()?;
    if funding.payer.bytes() != rent.payer().bytes()
        || funding.payer_principal_lamports != rent.refundable_principal()
        || funding.prior_donation_lamports != rent.donation_floor()
    {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }
    Ok(())
}

/// Exact Position V2 composition: authoritative Position V1 body plus tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionAccountV2 {
    /// Base value decoded and validated by `clutch-solana-layout`.
    pub base: PositionAccount,
    /// Counted-retirement facts owned by `clutch-retirement`.
    pub retirement: PositionRetirementTailV1,
}

impl PositionAccountV2 {
    /// Encode exactly 280 bytes under Position tag/version 2.
    pub fn encode(self) -> Result<[u8; POSITION_V2_BYTES], RetirementAdapterErrorV1> {
        let mut output = [0u8; POSITION_V2_BYTES];
        let written = self.base.encode(&mut output[..POSITION_V1_BYTES])?;
        if written != POSITION_V1_BYTES {
            return Err(RetirementAdapterErrorV1::BaseLengthMismatch);
        }
        output[1] = POSITION_ACCOUNT_VERSION_V2;
        output[POSITION_V1_BYTES..].copy_from_slice(&self.retirement.encode()?);
        Ok(output)
    }

    /// Decode an exact Position V2, delegating every base-field invariant to
    /// the authoritative Position V1 decoder after restoring its header.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementAdapterErrorV1> {
        let mut base_bytes = [0u8; POSITION_V1_BYTES];
        checked_base(
            input,
            POSITION_V2_BYTES,
            POSITION_V1_BYTES,
            POSITION_ACCOUNT_TAG,
            POSITION_ACCOUNT_VERSION_V2,
            account_version::POSITION,
            &mut base_bytes,
        )?;
        Ok(Self {
            base: PositionAccount::decode(&base_bytes)?,
            retirement: PositionRetirementTailV1::decode(&input[POSITION_V1_BYTES..])?,
        })
    }
}

/// Project one authenticated Position V2 into the pure retirement seam.
///
/// The economic fields and retirement identity are returned together so a
/// live caller cannot decode one byte image for identity and another for the
/// zero-balance proof.
pub fn project_authenticated_position_v2(
    account: AuthenticatedAccountV2<'_>,
) -> Result<(PositionLifecycleStateV2, PositionEconomicStateV1), RetirementAdapterErrorV2> {
    let value = PositionAccountV2::decode(account.data())?;
    Ok((
        PositionLifecycleStateV2::Live(LivePositionV2 {
            market: identity(value.base.market)?,
            owner: identity(value.base.owner)?,
            generation: value.base.generation,
            stored_bump: value.base.stored_bump,
            retirement: value.retirement,
        }),
        PositionEconomicStateV1 {
            cash_atoms: value.base.cash_atoms,
            reserved_cash_atoms: value.base.reserved_cash_atoms,
            internal_atoms: value.base.internal,
        },
    ))
}

/// Exact Replay-successor composition: the authoritative 84-byte reference
/// Replay body plus its independently funded 48-byte deletion owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplaySuccessorAccountV1 {
    /// Replay sequence and generation body owned by `clutch-solana-reference`.
    pub base: ReplayAccount,
    /// Exact payer, principal, and hostile-prefund floor for Replay deletion.
    pub rent: DeletableRentOwnerV1,
}

impl ReplaySuccessorAccountV1 {
    /// Encode exactly 132 bytes under the centrally reserved Replay-successor
    /// account coordinate.
    pub fn encode(
        self,
    ) -> Result<[u8; PROJECTED_REPLAY_SUCCESSOR_BYTES], RetirementAdapterErrorV2> {
        let mut output = [0u8; PROJECTED_REPLAY_SUCCESSOR_BYTES];
        let written = self.base.encode(&mut output[..REFERENCE_REPLAY_V1_BYTES])?;
        if written != REFERENCE_REPLAY_V1_BYTES {
            return Err(RetirementAdapterErrorV2::BaseLengthMismatch);
        }
        output[0] = REPLAY_SUCCESSOR_ACCOUNT_TAG;
        output[1] = REPLAY_SUCCESSOR_ACCOUNT_VERSION;
        output[REFERENCE_REPLAY_V1_BYTES..].copy_from_slice(&self.rent.encode()?);
        Ok(output)
    }

    /// Decode exact successor bytes by restoring the frozen reference Replay
    /// header before invoking its semantic owner.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementAdapterErrorV2> {
        if input.len() < PROJECTED_REPLAY_SUCCESSOR_BYTES {
            return Err(RetirementErrorV2::Truncated.into());
        }
        if input.len() > PROJECTED_REPLAY_SUCCESSOR_BYTES {
            return Err(RetirementErrorV2::TrailingBytes.into());
        }
        if input[0] != REPLAY_SUCCESSOR_ACCOUNT_TAG {
            return Err(RetirementErrorV2::WrongTag.into());
        }
        if input[1] != REPLAY_SUCCESSOR_ACCOUNT_VERSION {
            return Err(RetirementErrorV2::WrongVersion.into());
        }
        let mut base_bytes = [0u8; REFERENCE_REPLAY_V1_BYTES];
        base_bytes.copy_from_slice(&input[..REFERENCE_REPLAY_V1_BYTES]);
        base_bytes[0] = REPLAY_ACCOUNT_TAG;
        base_bytes[1] = REPLAY_ACCOUNT_VERSION;
        Ok(Self {
            base: ReplayAccount::decode(&base_bytes)?,
            rent: DeletableRentOwnerV1::decode(&input[REFERENCE_REPLAY_V1_BYTES..])?,
        })
    }
}

/// Decode one authenticated Replay successor into the exact pure lifecycle
/// projection consumed by atomic Position+Replay retirement planning.
pub fn project_authenticated_replay_successor_v1(
    account: AuthenticatedAccountV2<'_>,
) -> Result<ReplayLifecycleStateV1, RetirementAdapterErrorV2> {
    let value = ReplaySuccessorAccountV1::decode(account.data())?;
    Ok(ReplayLifecycleStateV1::Live(LiveReplaySuccessorV1 {
        market: identity(value.base.market)?,
        owner: identity(value.base.owner)?,
        position_generation: value.base.position_generation,
        sequence: value.base.sequence,
        stored_bump: value.base.stored_bump,
        rent: value.rent,
    }))
}

/// Decode one exact authenticated Budget and ask its semantic owner for the
/// terminal disposition consumed by atomic Epoch-root retirement.
pub fn project_authenticated_epoch_budget_semantic_disposition_v1(
    account: AuthenticatedAccountV2<'_>,
) -> Result<clutch_general_v2_contract::EpochBudgetRetirementDispositionV1, RetirementAdapterErrorV2>
{
    Ok(
        clutch_general_v2_contract::EpochBudgetV2AccountV1::decode(account.data())?
            .retirement_disposition()?,
    )
}

/// Legacy EpochV5 Budget lowering retained as an explicit fail-closed seam.
///
/// General V2 Budget stores its parent Epoch PDA, while the legacy EpochV5
/// planner expects a distinct semantic Epoch identity. Lowering between those
/// namespaces is unsound even when byte values happen to coincide. Production
/// General V2 callers must use
/// `authenticate_general_v2_budget_retirement_v2` and the fresh-family root
/// join instead.
pub fn project_authenticated_epoch_budget_retirement_v1(
    account: AuthenticatedAccountV2<'_>,
    neutral_sink: Identity32V1,
) -> Result<clutch_retirement::AuthenticatedEpochBudgetDispositionV1, RetirementAdapterErrorV2> {
    let _ = (account, neutral_sink);
    Err(RetirementErrorV2::BudgetRetirementUnauthenticated.into())
}

const _: () = assert!(REFERENCE_REPLAY_V1_BYTES == REPLAY_ACCOUNT_LEN);
const _: () = assert!(
    POSITION_ACCOUNT_TAG == clutch_solana_layout::registry::RETIREMENT_V2_POSITION_ACCOUNT_TAG
);
const _: () = assert!(
    POSITION_ACCOUNT_VERSION_V2
        == clutch_solana_layout::registry::RETIREMENT_V2_POSITION_ACCOUNT_VERSION
);
const _: () =
    assert!(MARKET_ACCOUNT_TAG == clutch_solana_layout::registry::RETIREMENT_V2_MARKET_ACCOUNT_TAG);
const _: () = assert!(
    MARKET_ACCOUNT_VERSION_V2
        == clutch_solana_layout::registry::RETIREMENT_V2_MARKET_ACCOUNT_VERSION
);
const _: () =
    assert!(EPOCH_ACCOUNT_TAG == clutch_solana_layout::registry::RETIREMENT_V2_EPOCH_ACCOUNT_TAG);
const _: () = assert!(
    EPOCH_ACCOUNT_VERSION_V5 == clutch_solana_layout::registry::RETIREMENT_V2_EPOCH_ACCOUNT_VERSION
);
const _: () = assert!(
    PROJECTED_REPLAY_SUCCESSOR_BYTES
        == REPLAY_ACCOUNT_LEN + clutch_retirement::DELETABLE_RENT_OWNER_V1_BYTES
);

/// Exact Market V2 composition: authoritative Market V1 body plus cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketAccountV2 {
    /// Base value decoded and validated by `clutch-solana-layout`.
    pub base: MarketAccount,
    /// Monotone general-Epoch cursor.
    pub cursor: MarketEpochCursorV1,
}

impl MarketAccountV2 {
    /// Encode exactly 734 bytes under Market tag/version 2.
    pub fn encode(self) -> Result<[u8; MARKET_V2_BYTES], RetirementAdapterErrorV1> {
        let mut output = [0u8; MARKET_V2_BYTES];
        let written = self.base.encode(&mut output[..MARKET_V1_BYTES])?;
        if written != MARKET_V1_BYTES {
            return Err(RetirementAdapterErrorV1::BaseLengthMismatch);
        }
        output[1] = MARKET_ACCOUNT_VERSION_V2;
        output[MARKET_V1_BYTES..].copy_from_slice(&self.cursor.encode());
        Ok(output)
    }

    /// Decode an exact Market V2 through the authoritative Market V1 decoder.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementAdapterErrorV1> {
        let mut base_bytes = [0u8; MARKET_V1_BYTES];
        checked_base(
            input,
            MARKET_V2_BYTES,
            MARKET_V1_BYTES,
            MARKET_ACCOUNT_TAG,
            MARKET_ACCOUNT_VERSION_V2,
            account_version::MARKET,
            &mut base_bytes,
        )?;
        Ok(Self {
            base: MarketAccount::decode(&base_bytes)?,
            cursor: MarketEpochCursorV1::decode(&input[MARKET_V1_BYTES..])?,
        })
    }
}

/// Exact general Epoch V5 composition: authoritative Epoch V2 body plus tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEpochAccountV5 {
    /// Base value decoded and validated by `clutch-solana-layout`.
    pub base: EpochAccount,
    /// Counted child, generation, and rent facts.
    pub retirement: EpochRetirementTailV1,
}

impl GeneralEpochAccountV5 {
    /// Encode exactly 429 bytes under Epoch tag/version 5.
    pub fn encode(self) -> Result<[u8; EPOCH_V5_BYTES], RetirementAdapterErrorV1> {
        let mut output = [0u8; EPOCH_V5_BYTES];
        let written = self.base.encode(&mut output[..EPOCH_V2_BYTES])?;
        if written != EPOCH_V2_BYTES {
            return Err(RetirementAdapterErrorV1::BaseLengthMismatch);
        }
        output[1] = EPOCH_ACCOUNT_VERSION_V5;
        output[EPOCH_V2_BYTES..].copy_from_slice(&self.retirement.encode()?);
        Ok(output)
    }

    /// Decode an exact general Epoch V5 through the Epoch V2 semantic owner.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementAdapterErrorV1> {
        let mut base_bytes = [0u8; EPOCH_V2_BYTES];
        checked_base(
            input,
            EPOCH_V5_BYTES,
            EPOCH_V2_BYTES,
            EPOCH_ACCOUNT_TAG,
            EPOCH_ACCOUNT_VERSION_V5,
            account_version::EPOCH,
            &mut base_bytes,
        )?;
        Ok(Self {
            base: EpochAccount::decode(&base_bytes)?,
            retirement: EpochRetirementTailV1::decode(&input[EPOCH_V2_BYTES..])?,
        })
    }
}

/// Project one already-decoded general Epoch V5 into the pure retirement
/// model, preserving distinct semantic Epoch and runtime account namespaces.
pub fn project_live_general_epoch_retirement_v2(
    account: GeneralEpochAccountV5,
) -> Result<LiveGeneralEpochProjectionV2, RetirementAdapterErrorV2> {
    if account.retirement.epoch_generation != canonical_epoch_generation(account.base.epoch_index)?
    {
        return Err(RetirementErrorV2::WrongGeneration.into());
    }
    Ok(LiveGeneralEpochProjectionV2 {
        market: identity(account.base.market)?,
        epoch: identity(account.base.epoch)?,
        epoch_index: account.base.epoch_index,
        phase: project_general_epoch_phase_v2(account.base.phase)?,
        stored_bump: account.base.stored_bump,
        retirement: account.retirement,
    })
}

/// Decode an authenticated Direct Epoch V4 and project the exact parent
/// identity, canonical index, and persisted Market/Realm neutral sink used by
/// direct Reservation retirement.
pub fn project_authenticated_direct_epoch_v4(
    account: AuthenticatedAccountV1<'_>,
) -> Result<
    (
        AdapterDirectEpochProjectionV1,
        AdapterNeutralSinkBindingProjectionV1,
    ),
    RetirementAdapterErrorV2,
> {
    let direct =
        clutch_solana_layout::direct_selection_v3::DirectEpochV4Account::decode(account.data())?;
    let market = identity(direct.direct.common.market)?;
    Ok((
        AdapterDirectEpochProjectionV1 {
            account: account.address(),
            market,
            epoch: identity(direct.direct.common.epoch)?,
            epoch_index: direct.direct.common.epoch_index,
            lifecycle_phase: project_direct_epoch_lifecycle_phase_v1(direct.lifecycle_phase)?,
        },
        AdapterNeutralSinkBindingProjectionV1 {
            market,
            neutral_sink: identity(direct.neutral_lamport_sink)?,
        },
    ))
}

/// Exact general Reservation V5 composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReservationAccountV5 {
    /// General Reservation V4 decoded by its authoritative owner.
    pub base: ReservationAccount,
    /// Parent generation and exact-once Position count marker.
    pub count: ReservationCountTailV1,
}

impl GeneralReservationAccountV5 {
    /// Encode exactly 627 bytes under shared Reservation tag/version 5.
    pub fn encode(self) -> Result<[u8; RESERVATION_V5_BYTES], RetirementAdapterErrorV1> {
        validate_position_count_marker(self.base.state, self.count)?;
        let mut output = [0u8; RESERVATION_V5_BYTES];
        let written = self.base.encode(&mut output[..RESERVATION_V4_BYTES])?;
        if written != RESERVATION_V4_BYTES {
            return Err(RetirementAdapterErrorV1::BaseLengthMismatch);
        }
        output[1] = RESERVATION_ACCOUNT_VERSION_V5;
        output[RESERVATION_V4_BYTES..].copy_from_slice(&self.count.encode()?);
        Ok(output)
    }

    /// Decode an exact general Reservation V5 through the V4 semantic owner.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementAdapterErrorV1> {
        let mut base_bytes = [0u8; RESERVATION_V4_BYTES];
        checked_base(
            input,
            RESERVATION_V5_BYTES,
            RESERVATION_V4_BYTES,
            RESERVATION_ACCOUNT_TAG,
            RESERVATION_ACCOUNT_VERSION_V5,
            clutch_solana_layout::reservation::RESERVATION_ACCOUNT_VERSION,
            &mut base_bytes,
        )?;
        let base = ReservationAccount::decode(&base_bytes)?;
        let count = ReservationCountTailV1::decode(&input[RESERVATION_V4_BYTES..])?;
        validate_position_count_marker(base.state, count)?;
        Ok(Self { base, count })
    }
}

/// Exact direct Reservation V6 composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationAccountV6 {
    /// Direct Reservation V2 decoded by its authoritative owner.
    pub base: DirectReservationV2Account,
    /// Parent generation and exact-once Position count marker.
    pub count: ReservationCountTailV1,
}

impl DirectReservationAccountV6 {
    /// Encode exactly 627 bytes under shared Reservation tag/version 6.
    pub fn encode(
        self,
        neutral_sink: clutch_solana_layout::Hash32,
    ) -> Result<[u8; DIRECT_RESERVATION_V6_BYTES], RetirementAdapterErrorV1> {
        validate_position_count_marker(self.base.reservation.state, self.count)?;
        let mut output = [0u8; DIRECT_RESERVATION_V6_BYTES];
        let written = self
            .base
            .encode(neutral_sink, &mut output[..DIRECT_RESERVATION_V2_BYTES])?;
        if written != DIRECT_RESERVATION_V2_BYTES {
            return Err(RetirementAdapterErrorV1::BaseLengthMismatch);
        }
        output[1] = DIRECT_RESERVATION_ACCOUNT_VERSION_V6;
        output[DIRECT_RESERVATION_V2_BYTES..].copy_from_slice(&self.count.encode()?);
        Ok(output)
    }

    /// Decode an exact direct Reservation V6 through the V2 semantic owner.
    pub fn decode(
        input: &[u8],
        neutral_sink: clutch_solana_layout::Hash32,
    ) -> Result<Self, RetirementAdapterErrorV1> {
        let mut base_bytes = [0u8; DIRECT_RESERVATION_V2_BYTES];
        checked_base(
            input,
            DIRECT_RESERVATION_V6_BYTES,
            DIRECT_RESERVATION_V2_BYTES,
            RESERVATION_ACCOUNT_TAG,
            DIRECT_RESERVATION_ACCOUNT_VERSION_V6,
            clutch_solana_layout::direct_selection_v3::DIRECT_RESERVATION_V2_VERSION,
            &mut base_bytes,
        )?;
        let base = DirectReservationV2Account::decode(&base_bytes, neutral_sink)?;
        let count = ReservationCountTailV1::decode(&input[DIRECT_RESERVATION_V2_BYTES..])?;
        validate_position_count_marker(base.reservation.state, count)?;
        Ok(Self { base, count })
    }
}

/// Exact deletable general Reservation V7 successor composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReservationAccountV7 {
    /// General Reservation V4 decoded by its authoritative owner.
    pub base: ReservationAccount,
    /// Count marker plus exact payer-owned deletion funding.
    pub retirement: ReservationRetirementTailV2,
}

impl GeneralReservationAccountV7 {
    /// Encode exactly 675 bytes under shared Reservation tag/version 7.
    pub fn encode(self) -> Result<[u8; RESERVATION_V7_BYTES], RetirementAdapterErrorV2> {
        validate_position_count_marker(self.base.state, self.retirement.count)?;
        let mut output = [0u8; RESERVATION_V7_BYTES];
        let written = self.base.encode(&mut output[..RESERVATION_V4_BYTES])?;
        if written != RESERVATION_V4_BYTES {
            return Err(RetirementAdapterErrorV2::BaseLengthMismatch);
        }
        output[1] = RESERVATION_ACCOUNT_VERSION_V7;
        output[RESERVATION_V4_BYTES..].copy_from_slice(&self.retirement.encode()?);
        Ok(output)
    }

    /// Decode exactly one V7 envelope through the V4 semantic owner.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementAdapterErrorV2> {
        let mut base_bytes = [0u8; RESERVATION_V4_BYTES];
        checked_base(
            input,
            RESERVATION_V7_BYTES,
            RESERVATION_V4_BYTES,
            RESERVATION_ACCOUNT_TAG,
            RESERVATION_ACCOUNT_VERSION_V7,
            clutch_solana_layout::reservation::RESERVATION_ACCOUNT_VERSION,
            &mut base_bytes,
        )?;
        let base = ReservationAccount::decode(&base_bytes)?;
        let retirement = ReservationRetirementTailV2::decode(&input[RESERVATION_V4_BYTES..])?;
        validate_position_count_marker(base.state, retirement.count)?;
        Ok(Self { base, retirement })
    }
}

/// Exact deletable direct Reservation V8 successor composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationAccountV8 {
    /// Direct Reservation V2 decoded by its authoritative owner.
    pub base: DirectReservationV2Account,
    /// Count marker plus exact mirrored payer-owned deletion funding.
    pub retirement: ReservationRetirementTailV2,
}

impl DirectReservationAccountV8 {
    /// Encode exactly 675 bytes under shared Reservation tag/version 8.
    pub fn encode(
        self,
        neutral_sink: clutch_solana_layout::Hash32,
    ) -> Result<[u8; DIRECT_RESERVATION_V8_BYTES], RetirementAdapterErrorV2> {
        validate_position_count_marker(self.base.reservation.state, self.retirement.count)?;
        validate_direct_funding_mirror(self.base.funding, self.retirement.rent)?;
        let mut output = [0u8; DIRECT_RESERVATION_V8_BYTES];
        let written = self
            .base
            .encode(neutral_sink, &mut output[..DIRECT_RESERVATION_V2_BYTES])?;
        if written != DIRECT_RESERVATION_V2_BYTES {
            return Err(RetirementAdapterErrorV2::BaseLengthMismatch);
        }
        output[1] = DIRECT_RESERVATION_ACCOUNT_VERSION_V8;
        output[DIRECT_RESERVATION_V2_BYTES..].copy_from_slice(&self.retirement.encode()?);
        Ok(output)
    }

    /// Decode exactly one V8 envelope through the direct V2 semantic owner.
    pub fn decode(
        input: &[u8],
        neutral_sink: clutch_solana_layout::Hash32,
    ) -> Result<Self, RetirementAdapterErrorV2> {
        let mut base_bytes = [0u8; DIRECT_RESERVATION_V2_BYTES];
        checked_base(
            input,
            DIRECT_RESERVATION_V8_BYTES,
            DIRECT_RESERVATION_V2_BYTES,
            RESERVATION_ACCOUNT_TAG,
            DIRECT_RESERVATION_ACCOUNT_VERSION_V8,
            clutch_solana_layout::direct_selection_v3::DIRECT_RESERVATION_V2_VERSION,
            &mut base_bytes,
        )?;
        let base = DirectReservationV2Account::decode(&base_bytes, neutral_sink)?;
        let retirement =
            ReservationRetirementTailV2::decode(&input[DIRECT_RESERVATION_V2_BYTES..])?;
        validate_position_count_marker(base.reservation.state, retirement.count)?;
        validate_direct_funding_mirror(base.funding, retirement.rent)?;
        Ok(Self { base, retirement })
    }
}

/// Promote one already-authoritatively-decoded child base and append its
/// parent-generation tail into an exact caller-provided output buffer.
pub fn encode_counted_child_after_base_validation(
    schema: CountedChildSchemaV1,
    legacy_base: &[u8],
    generation: ChildGenerationV1,
    output: &mut [u8],
) -> Result<(), RetirementAdapterErrorV1> {
    exact(legacy_base, schema.legacy_len())?;
    exact(output, schema.counted_len())?;
    header(legacy_base, schema.tag(), schema.legacy_version())?;
    output[..schema.legacy_len()].copy_from_slice(legacy_base);
    output[1] = schema.counted_version();
    output[schema.legacy_len()..].copy_from_slice(&generation.encode()?);
    Ok(())
}

/// Split one exact counted child into legacy-version base bytes for its
/// authoritative decoder and a validated parent-generation tail.
pub fn decode_counted_child(
    schema: CountedChildSchemaV1,
    input: &[u8],
    legacy_base_output: &mut [u8],
) -> Result<ChildGenerationV1, RetirementAdapterErrorV1> {
    checked_base(
        input,
        schema.counted_len(),
        schema.legacy_len(),
        schema.tag(),
        schema.counted_version(),
        schema.legacy_version(),
        legacy_base_output,
    )?;
    Ok(ChildGenerationV1::decode(&input[schema.legacy_len()..])?)
}

const _: () = assert!(POSITION_V1_BYTES == account_len::POSITION);
const _: () = assert!(MARKET_V1_BYTES == account_len::MARKET);
const _: () = assert!(EPOCH_V2_BYTES == account_len::EPOCH);
const _: () =
    assert!(RESERVATION_V4_BYTES == clutch_solana_layout::reservation::RESERVATION_ACCOUNT_BYTES);
const _: () = assert!(
    DIRECT_RESERVATION_V2_BYTES
        == clutch_solana_layout::direct_selection_v3::DIRECT_RESERVATION_V2_BYTES
);
const _: () = assert!(
    EPOCH_ACCOUNT_VERSION_V5 != clutch_solana_layout::direct_selection::DIRECT_EPOCH_VERSION
);
const _: () = assert!(
    EPOCH_ACCOUNT_VERSION_V5 != clutch_solana_layout::direct_selection_v3::DIRECT_EPOCH_V4_VERSION
);
const _: () = assert!(
    RESERVATION_ACCOUNT_VERSION_V5
        != clutch_solana_layout::reservation::RESERVATION_ACCOUNT_VERSION
);
const _: () = assert!(
    RESERVATION_ACCOUNT_VERSION_V5
        != clutch_solana_layout::direct_selection_v3::DIRECT_RESERVATION_V2_VERSION
);
const _: () = assert!(
    DIRECT_RESERVATION_ACCOUNT_VERSION_V6
        != clutch_solana_layout::reservation::RESERVATION_ACCOUNT_VERSION
);
const _: () = assert!(
    DIRECT_RESERVATION_ACCOUNT_VERSION_V6
        != clutch_solana_layout::direct_selection_v3::DIRECT_RESERVATION_V2_VERSION
);
const _: () = assert!(DIRECT_RESERVATION_ACCOUNT_VERSION_V6 != RESERVATION_ACCOUNT_VERSION_V5);
const _: () = assert!(RESERVATION_ACCOUNT_VERSION_V7 != RESERVATION_ACCOUNT_VERSION_V5);
const _: () = assert!(RESERVATION_ACCOUNT_VERSION_V7 != DIRECT_RESERVATION_ACCOUNT_VERSION_V6);
const _: () = assert!(DIRECT_RESERVATION_ACCOUNT_VERSION_V8 != RESERVATION_ACCOUNT_VERSION_V7);
const _: () =
    assert!(DIRECT_RESERVATION_ACCOUNT_VERSION_V8 != DIRECT_RESERVATION_ACCOUNT_VERSION_V6);
