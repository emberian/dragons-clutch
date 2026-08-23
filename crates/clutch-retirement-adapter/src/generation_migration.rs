// SPDX-License-Identifier: AGPL-3.0-or-later

//! Authenticated generation-safe Position/Replay reopen bridge.
//!
//! This module intentionally implements tombstone reopen, not in-place legacy
//! migration. Position V1 has no authoritative outstanding-reservation count,
//! so its local bytes cannot prove an exhaustive empty child set. A live V1
//! Position must remain on the fail-closed route.

use clutch_retirement::{
    admit_deletable_rent, admit_reopen_rent_split, plan_position_replay_retirement_v3,
    reopen_position_with_replay_v2, AdapterNeutralSinkBindingProjectionV1,
    AdapterPositionAccountProjectionV1, AdapterReplayAbsenceProjectionV1,
    AdapterReplayAccountProjectionV1, Identity32V1, PositionLifecycleStateV2,
    PositionReplayAccountsV1, PositionReplayReopenAccountsV1, PositionReplayReopenPlanV2,
    PositionReplayReopenRequestV1, PositionReplayReopenRequestV2,
    PositionReplayRetirementRequestV1, PositionReplayRetirementRequestV2,
    PositionReplayRetirementRequestV3, PositionTombstoneV2, RecipientBalanceBookV1,
    RecipientBalanceV1, ReplayLifecycleStateV1, RetirementErrorV2, MAX_RETIREMENT_RECIPIENTS,
};

use crate::runtime_commit::{
    prepare_position_replay_close_v2, prepare_position_replay_reopen_v2,
    PreparedPositionReplayCloseV2, PreparedPositionReplayReopenV2,
};
use crate::{
    project_authenticated_position_v2, project_authenticated_replay_successor_v1,
    AuthenticatedAccountV2, AuthenticatedGeneralV2NeutralSinkBindingV1, CanonicalPdaV1,
    PositionAccountV2, ReplaySuccessorAccountV1, RetirementAdapterErrorV2,
};

const SYSTEM_PROGRAM_OWNER: [u8; 32] = [0u8; 32];

/// Authenticated runtime balance role for one retirement recipient.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementRecipientViewV1 {
    /// Recipient account identity.
    pub address: Identity32V1,
    /// Actual balance used by the pure close plan.
    pub lamports: u64,
    /// Recipient must be writable.
    pub is_writable: bool,
    /// Recipient cannot be executable.
    pub is_executable: bool,
}

/// Complete exact-account inputs for a production Position/Replay close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayCloseRuntimeRequestV3<'a> {
    /// Exact writable Position V2 authentication result.
    pub position: AuthenticatedAccountV2<'a>,
    /// Exact writable Replay-successor authentication result.
    pub replay: AuthenticatedAccountV2<'a>,
    /// Exact immutable General V2 MarketBinding sink capability.
    pub market_binding: AuthenticatedGeneralV2NeutralSinkBindingV1,
    /// Sequence authenticated from the signed instruction envelope.
    pub signed_sequence: u64,
    /// Actual Position balance before close.
    pub position_lamports: u64,
    /// Actual Replay balance before close.
    pub replay_lamports: u64,
    /// Up to four runtime recipient roles; required identities are derived from
    /// persisted funding owners and the immutable sink.
    pub recipients: [Option<RetirementRecipientViewV1>; MAX_RETIREMENT_RECIPIENTS],
}

/// Authenticate every semantic/account join and prepare the only production
/// Position/Replay close schedule.
pub fn authenticate_and_prepare_position_replay_close_v3(
    request: PositionReplayCloseRuntimeRequestV3<'_>,
) -> Result<PreparedPositionReplayCloseV2, RetirementAdapterErrorV2> {
    if !request.position.is_writable() || !request.replay.is_writable() {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }
    if request.position.program_id() != request.replay.program_id()
        || request.position.program_id() != request.market_binding.program_id()
    {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    let (position, economic) = project_authenticated_position_v2(request.position)?;
    let live_position = match position {
        PositionLifecycleStateV2::Live(value) => value,
        PositionLifecycleStateV2::Tombstone(_) => {
            return Err(RetirementErrorV2::AlreadyTerminal.into())
        }
    };
    let replay = project_authenticated_replay_successor_v1(request.replay)?;
    let live_replay = match replay {
        ReplayLifecycleStateV1::Live(value) => value,
        ReplayLifecycleStateV1::Absent => return Err(RetirementErrorV2::ReplayMismatch.into()),
    };
    if request.market_binding.market() != live_position.market {
        return Err(RetirementErrorV2::WrongParent.into());
    }

    let mut entries = [None; MAX_RETIREMENT_RECIPIENTS];
    let mut index = 0usize;
    while index < request.recipients.len() {
        if let Some(recipient) = request.recipients[index] {
            if !recipient.is_writable {
                return Err(RetirementAdapterErrorV2::NotWritable);
            }
            if recipient.is_executable {
                return Err(RetirementAdapterErrorV2::ExecutableAccount);
            }
            entries[index] = Some(RecipientBalanceV1 {
                recipient: recipient.address,
                balance_before: recipient.lamports,
            });
        }
        index += 1;
    }
    let neutral_sink = request.market_binding.neutral_sink();
    let pure = PositionReplayRetirementRequestV1 {
        position,
        replay,
        economic,
        position_balance: request.position_lamports,
        replay_balance: request.replay_lamports,
        neutral_sink,
        neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1 {
            market: live_position.market,
            neutral_sink,
        },
        accounts: PositionReplayAccountsV1 {
            position: AdapterPositionAccountProjectionV1 {
                account: request.position.address(),
                market: live_position.market,
                owner: live_position.owner,
            },
            replay: AdapterReplayAccountProjectionV1 {
                account: request.replay.address(),
                market: live_replay.market,
                owner: live_replay.owner,
                position_generation: live_replay.position_generation,
            },
        },
        recipient_balances: RecipientBalanceBookV1 { entries },
    };
    let plan = plan_position_replay_retirement_v3(PositionReplayRetirementRequestV3 {
        retirement: PositionReplayRetirementRequestV2 {
            retirement: pure,
            signed_sequence: request.signed_sequence,
        },
    })?;
    prepare_position_replay_close_v2(
        request.position.address(),
        request.replay.address(),
        request.position_lamports,
        request.replay_lamports,
        plan,
    )
}

/// Runtime facts for one writable, semantically vacant canonical PDA.
///
/// Positive lamports are an admitted hostile prefund, not semantic existence
/// and never a discount against a payer's full rent principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VacantPdaAccountViewV2 {
    /// Actual runtime address.
    pub address: Identity32V1,
    /// Raw owner bytes; vacancy requires the System program.
    pub owner: [u8; 32],
    /// Actual lamports, including hostile prefund.
    pub lamports: u64,
    /// Actual data length; vacancy requires zero.
    pub data_len: usize,
    /// Creation/sweep requires writable access.
    pub is_writable: bool,
    /// A vacant PDA cannot be executable.
    pub is_executable: bool,
}

/// Runtime facts for a System-transfer funding payer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingPayerViewV1 {
    /// Payer identity.
    pub address: Identity32V1,
    /// Raw runtime owner bytes; funding requires the System program.
    pub owner: [u8; 32],
    /// Actual balance before the complete coalesced funding bundle.
    pub lamports: u64,
    /// Funding authority must sign.
    pub is_signer: bool,
    /// Funding authority must be writable.
    pub is_writable: bool,
    /// A funding authority cannot be executable.
    pub is_executable: bool,
}

/// Runtime facts for the immutable neutral-sink recipient.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeutralSinkBalanceViewV1 {
    /// Recipient identity, cross-bound to MarketBinding.
    pub address: Identity32V1,
    /// Actual balance before predecessor-prefund sweep.
    pub lamports: u64,
    /// Recipient must be writable.
    pub is_writable: bool,
    /// Recipient cannot be executable.
    pub is_executable: bool,
}

/// Exact rent minima queried by the SBF adapter for the three relevant fixed
/// lengths: Position V2, Position tombstone, and Replay successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayRentMinimumsV1 {
    /// Minimum for the full live Position V2 account.
    pub position_live: u64,
    /// Minimum permanently retained by the Position tombstone.
    pub position_tombstone: u64,
    /// Full minimum for the deletable Replay successor.
    pub replay: u64,
}

impl PositionReplayRentMinimumsV1 {
    fn position_refundable(self) -> Result<u64, RetirementAdapterErrorV2> {
        if self.position_tombstone == 0 || self.replay == 0 {
            return Err(RetirementErrorV2::NonCanonicalState.into());
        }
        let refundable = self
            .position_live
            .checked_sub(self.position_tombstone)
            .ok_or(RetirementErrorV2::NonCanonicalState)?;
        if refundable == 0 {
            return Err(RetirementErrorV2::NonCanonicalState.into());
        }
        Ok(refundable)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedVacantPdaV2 {
    address: Identity32V1,
    lamports: u64,
    bump: u8,
}

fn authenticate_vacant_pda_v2(
    view: VacantPdaAccountViewV2,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedVacantPdaV2, RetirementAdapterErrorV2> {
    if view.address != canonical_pda.address() {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if !view.is_writable {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }
    if view.owner != SYSTEM_PROGRAM_OWNER || view.data_len != 0 || view.is_executable {
        return Err(RetirementAdapterErrorV2::AccountNotAbsent);
    }
    Ok(AuthenticatedVacantPdaV2 {
        address: view.address,
        lamports: view.lamports,
        bump: canonical_pda.bump(),
    })
}

fn authenticate_funding_payer_v1(
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

/// Complete authenticated runtime inputs for one tombstone reopen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionReplayReopenRuntimeRequestV2<'a> {
    /// Exact writable Position tombstone authentication result.
    pub position_tombstone: AuthenticatedAccountV2<'a>,
    /// Actual Position tombstone lamports before funding.
    pub position_lamports: u64,
    /// Writable System-owned predecessor Replay PDA, including prefund.
    pub prior_replay: VacantPdaAccountViewV2,
    /// Canonical predecessor Replay derivation for the closed generation.
    pub prior_replay_pda: CanonicalPdaV1,
    /// Writable System-owned next-generation Replay PDA, including prefund.
    pub next_replay: VacantPdaAccountViewV2,
    /// Canonical next-generation Replay derivation.
    pub next_replay_pda: CanonicalPdaV1,
    /// Exact immutable General V2 MarketBinding sink capability.
    pub market_binding: AuthenticatedGeneralV2NeutralSinkBindingV1,
    /// Position live-rent payer.
    pub position_payer: FundingPayerViewV1,
    /// Replay rent payer.
    pub replay_payer: FundingPayerViewV1,
    /// Authenticated sink starting balance.
    pub neutral_sink: NeutralSinkBalanceViewV1,
    /// Rent-sysvar results for exact fixed account lengths.
    pub rent_minimums: PositionReplayRentMinimumsV1,
}

/// Private-field output ready for full byte-image preparation and ordered SBF
/// execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedPositionReplayReopenV2 {
    prior_replay: Identity32V1,
    next_replay: Identity32V1,
    neutral_sink: Identity32V1,
    plan: PositionReplayReopenPlanV2,
}

impl AuthenticatedPositionReplayReopenV2 {
    /// Canonical predecessor Replay PDA swept during reopen.
    pub const fn prior_replay(self) -> Identity32V1 {
        self.prior_replay
    }

    /// Canonical next-generation Replay PDA created during reopen.
    pub const fn next_replay(self) -> Identity32V1 {
        self.next_replay
    }

    /// Exact immutable neutral sink receiving predecessor prefund.
    pub const fn neutral_sink(self) -> Identity32V1 {
        self.neutral_sink
    }

    /// Complete checked pure plan consumed by runtime commit preparation.
    pub const fn plan(self) -> PositionReplayReopenPlanV2 {
        self.plan
    }

    /// Cross-check complete authoritative Position/Replay byte images and
    /// prepare the fixed ordered runtime commit without exposing a mix-and-
    /// match gap between authentication and mutation preparation.
    pub fn prepare(
        self,
        position: PositionAccountV2,
        replay: ReplaySuccessorAccountV1,
    ) -> Result<PreparedPositionReplayReopenV2, RetirementAdapterErrorV2> {
        prepare_position_replay_reopen_v2(self.prior_replay, position, replay, self.plan)
    }
}

/// Authenticate and plan a complete Position tombstone reopen plus Replay
/// generation rotation.
pub fn authenticate_and_plan_position_replay_reopen_v2(
    request: PositionReplayReopenRuntimeRequestV2<'_>,
) -> Result<AuthenticatedPositionReplayReopenV2, RetirementAdapterErrorV2> {
    if !request.position_tombstone.is_writable() {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }
    if request.position_tombstone.program_id() != request.market_binding.program_id() {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    let tombstone_v2 = PositionTombstoneV2::decode(request.position_tombstone.data())?;
    let tombstone = tombstone_v2.identity_v1();
    let market = tombstone.market;
    let owner = tombstone.owner;
    if request.market_binding.market() != market {
        return Err(RetirementErrorV2::WrongParent.into());
    }
    let neutral_sink = request.market_binding.neutral_sink();
    if request.neutral_sink.address != neutral_sink {
        return Err(RetirementErrorV2::WrongNeutralSink.into());
    }
    if !request.neutral_sink.is_writable || request.neutral_sink.is_executable {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }

    let prior = authenticate_vacant_pda_v2(request.prior_replay, request.prior_replay_pda)?;
    let next = authenticate_vacant_pda_v2(request.next_replay, request.next_replay_pda)?;
    let position_payer = authenticate_funding_payer_v1(request.position_payer)?;
    let replay_payer = authenticate_funding_payer_v1(request.replay_payer)?;
    let next_generation = tombstone
        .generation
        .checked_add(1)
        .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
    let position_refundable = request.rent_minimums.position_refundable()?;
    if request.rent_minimums.position_tombstone != tombstone_v2.permanent_tombstone_principal {
        return Err(RetirementErrorV2::NonCanonicalState.into());
    }

    let position_funding = admit_reopen_rent_split(
        request.position_tombstone.address(),
        position_payer.address,
        position_refundable,
        tombstone_v2.permanent_tombstone_principal,
        request.position_lamports,
        position_payer.lamports,
        neutral_sink,
    )?;
    let replay_funding = admit_deletable_rent(
        next.address,
        replay_payer.address,
        request.rent_minimums.replay,
        next.lamports,
        replay_payer.lamports,
        neutral_sink,
    )?;
    let prior_projection = AdapterReplayAbsenceProjectionV1 {
        account: prior.address,
        market,
        owner,
        position_generation: tombstone.generation,
    };
    let accounts = PositionReplayReopenAccountsV1 {
        position: AdapterPositionAccountProjectionV1 {
            account: request.position_tombstone.address(),
            market,
            owner,
        },
        next_replay: AdapterReplayAccountProjectionV1 {
            account: next.address,
            market,
            owner,
            position_generation: next_generation,
        },
    };
    let reopen = PositionReplayReopenRequestV1 {
        position: PositionLifecycleStateV2::Tombstone(tombstone),
        prior_replay: prior_projection,
        position_funding,
        replay_stored_bump: next.bump,
        replay_funding,
        neutral_sink,
        neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1 {
            market,
            neutral_sink,
        },
        accounts,
    };
    let plan = reopen_position_with_replay_v2(PositionReplayReopenRequestV2 {
        reopen,
        prior_replay_prefund_lamports: prior.lamports,
        neutral_sink_balance_before: request.neutral_sink.lamports,
    })?;
    Ok(AuthenticatedPositionReplayReopenV2 {
        prior_replay: prior.address,
        next_replay: next.address,
        neutral_sink,
        plan,
    })
}
