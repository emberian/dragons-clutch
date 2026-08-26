//! Stateless terminal retirement for recurring Series V3.
//!
//! A terminal Ticket or Series root never pays a wallet directly. Every
//! program-owned lamport first credits the exact Market+generation
//! [`LifecycleRentCreditV2`]. The immutable V3 refund owner remains an
//! attribution fact and must equal the credit's immutable refund wallet. Only
//! the generic Core retirement and Rent V2 close continuation may later close
//! that credit to the wallet.

use dclutch_core_contract::ContentId;
use dclutch_rent_contract::lifecycle_v2::LifecycleRentCreditV2;

use crate::{
    AccountKeyV3, AdmittedTicketV3, FoundingFundsV3, TemplateV3,
    plan::{ReplayCandidateV3, SeriesReplayActionV3, evaluate_replay_v3},
    replay::{SeriesStateV3, TicketStateV3},
};

/// Stable refusal from terminal replay or lifecycle-credit admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesTerminalErrorV3 {
    /// Lifecycle Rent V2 bytes were hostile or selected another schema.
    RentEncoding,
    /// The credit did not bind the exact Market, release, generation, or wallet.
    RentBinding,
    /// Mutable root/Ticket state or an optimistic revision refused.
    Replay,
    /// A terminal account balance could not contain its exact Rent principal.
    Balance,
    /// Checked fixed-width arithmetic overflowed.
    Arithmetic,
}

/// Authenticated lifecycle-scoped sink for all Series-owned terminal lamports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLifecycleRentSinkV3 {
    credit_account: AccountKeyV3,
    refund_wallet: AccountKeyV3,
    market: AccountKeyV3,
    release_set: ContentId,
    generation: u64,
    pda_bump: u8,
}

impl SeriesLifecycleRentSinkV3 {
    /// Hostile-decode and bind the exact lifecycle credit selected by the root.
    pub fn admit(
        credit_account: AccountKeyV3,
        credit_bytes: &[u8],
        expected_market: AccountKeyV3,
        expected_release_set: ContentId,
        expected_generation: u64,
        expected_refund_wallet: AccountKeyV3,
    ) -> Result<Self, SeriesTerminalErrorV3> {
        let credit = LifecycleRentCreditV2::decode(credit_bytes)
            .map_err(|_| SeriesTerminalErrorV3::RentEncoding)?;
        if credit.market().to_bytes() != expected_market.to_bytes()
            || credit.release_set().to_bytes() != expected_release_set.to_bytes()
            || credit.generation() != expected_generation
            || credit.refund_wallet().to_bytes() != expected_refund_wallet.to_bytes()
        {
            return Err(SeriesTerminalErrorV3::RentBinding);
        }
        Ok(Self {
            credit_account,
            refund_wallet: AccountKeyV3::new(credit.refund_wallet().to_bytes())
                .map_err(|_| SeriesTerminalErrorV3::RentEncoding)?,
            market: AccountKeyV3::new(credit.market().to_bytes())
                .map_err(|_| SeriesTerminalErrorV3::RentEncoding)?,
            release_set: ContentId::new(credit.release_set().to_bytes())
                .map_err(|_| SeriesTerminalErrorV3::RentEncoding)?,
            generation: credit.generation(),
            pda_bump: credit.pda_bump(),
        })
    }

    /// Exact lifecycle-credit account receiving the resource close.
    pub const fn credit_account(self) -> AccountKeyV3 {
        self.credit_account
    }

    /// Immutable wallet which may receive funds only after generic retirement.
    pub const fn refund_wallet(self) -> AccountKeyV3 {
        self.refund_wallet
    }

    /// Bound Market identity.
    pub const fn market(self) -> AccountKeyV3 {
        self.market
    }

    /// Bound release-set identity.
    pub const fn release_set(self) -> ContentId {
        self.release_set
    }

    /// Bound Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Persisted PDA bump; the SDK adapter independently derives the account.
    pub const fn pda_bump(self) -> u8 {
        self.pda_bump
    }

    fn requires_wallet(self, expected: AccountKeyV3) -> Result<(), SeriesTerminalErrorV3> {
        if self.refund_wallet == expected {
            Ok(())
        } else {
            Err(SeriesTerminalErrorV3::RentBinding)
        }
    }

    /// Require one immutable Series/Ticket refund attribution to select this
    /// lifecycle's wallet. This does not authorize a direct wallet transfer.
    pub fn admit_refund_owner(self, expected: AccountKeyV3) -> Result<(), SeriesTerminalErrorV3> {
        self.requires_wallet(expected)
    }
}

/// Three disjoint native compartments remaining in Ticket custody.
///
/// Hoard principal is Realm collateral and can never enter this lamport plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TicketNativeRemaindersV3 {
    market_rent: u64,
    capability_native: u64,
    founding_work: u64,
}

impl TicketNativeRemaindersV3 {
    /// Derive exact native compartments from the sole immutable occurrence.
    pub const fn from_founding_funds(funds: FoundingFundsV3) -> Self {
        Self {
            market_rent: funds.market_rent(),
            capability_native: funds.capability_native(),
            founding_work: funds.founding_work(),
        }
    }

    /// Exact prepaid Market account Rent.
    pub const fn market_rent(self) -> u64 {
        self.market_rent
    }

    /// Exact capability-native account funding and Rent.
    pub const fn capability_native(self) -> u64 {
        self.capability_native
    }

    /// Exact founding work capital.
    pub const fn founding_work(self) -> u64 {
        self.founding_work
    }

    /// Checked native total; Hoard collateral is deliberately absent.
    pub fn total(self) -> Result<u64, SeriesTerminalErrorV3> {
        self.market_rent
            .checked_add(self.capability_native)
            .and_then(|value| value.checked_add(self.founding_work))
            .ok_or(SeriesTerminalErrorV3::Arithmetic)
    }
}

/// Exact typed refund from deleting one terminal Ticket replay account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TicketRetirementPlanV3 {
    series_after: SeriesStateV3,
    rent_sink: SeriesLifecycleRentSinkV3,
    ticket_rent: u64,
    donation: u64,
}

impl TicketRetirementPlanV3 {
    /// Candidate Series root persisted only after Ticket deletion succeeds.
    pub const fn series_after(self) -> SeriesStateV3 {
        self.series_after
    }

    /// Sole lifecycle-scoped destination for both typed components.
    pub const fn rent_sink(self) -> SeriesLifecycleRentSinkV3 {
        self.rent_sink
    }

    /// Exact Ticket account Rent reserve.
    pub const fn ticket_rent(self) -> u64 {
        self.ticket_rent
    }

    /// Unsolicited Ticket lamports; never funding or Hoard principal.
    pub const fn donation(self) -> u64 {
        self.donation
    }

    /// Complete Ticket balance credited to Rent V2.
    pub fn total_credit(self) -> Result<u64, SeriesTerminalErrorV3> {
        self.ticket_rent
            .checked_add(self.donation)
            .ok_or(SeriesTerminalErrorV3::Arithmetic)
    }
}

/// Plan deletion of one already-terminal, non-replayable Ticket.
#[allow(clippy::too_many_arguments)]
pub fn plan_ticket_retirement_v3(
    occurrence_count: u32,
    series: SeriesStateV3,
    ticket_state: TicketStateV3,
    admitted_ticket: AdmittedTicketV3,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    observed_ticket_lamports: u64,
    exact_ticket_rent: u64,
    rent_sink: SeriesLifecycleRentSinkV3,
) -> Result<TicketRetirementPlanV3, SeriesTerminalErrorV3> {
    rent_sink.requires_wallet(admitted_ticket.ticket().refund_owner())?;
    if exact_ticket_rent == 0 {
        return Err(SeriesTerminalErrorV3::Balance);
    }
    let series_bytes = series
        .encode(occurrence_count)
        .map_err(|_| SeriesTerminalErrorV3::Replay)?;
    let ticket_bytes = ticket_state.encode();
    let witness = evaluate_replay_v3(
        SeriesReplayActionV3::Retire {
            ticket_record: admitted_ticket.content_id(),
            expected_ticket_revision,
        },
        occurrence_count,
        expected_series_revision,
        &series_bytes,
        Some(&ticket_bytes),
    )
    .map_err(|_| SeriesTerminalErrorV3::Replay)?;
    let series_after = match witness.series() {
        ReplayCandidateV3::Replace(bytes) => SeriesStateV3::decode(&bytes, occurrence_count)
            .map_err(|_| SeriesTerminalErrorV3::Replay)?,
        ReplayCandidateV3::Unchanged | ReplayCandidateV3::Delete => {
            return Err(SeriesTerminalErrorV3::Replay);
        }
    };
    if witness.ticket() != ReplayCandidateV3::Delete {
        return Err(SeriesTerminalErrorV3::Replay);
    }
    let donation = observed_ticket_lamports
        .checked_sub(exact_ticket_rent)
        .ok_or(SeriesTerminalErrorV3::Balance)?;
    Ok(TicketRetirementPlanV3 {
        series_after,
        rent_sink,
        ticket_rent: exact_ticket_rent,
        donation,
    })
}

/// Exact typed refund from deleting the zero-outstanding terminal Series root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRootClosurePlanV3 {
    rent_sink: SeriesLifecycleRentSinkV3,
    close_rent: u64,
    root_rent: u64,
    donation: u64,
}

impl SeriesRootClosurePlanV3 {
    /// Sole lifecycle-scoped destination for every typed component.
    pub const fn rent_sink(self) -> SeriesLifecycleRentSinkV3 {
        self.rent_sink
    }

    /// Separately classified Template close principal.
    pub const fn close_rent(self) -> u64 {
        self.close_rent
    }

    /// Exact composite-root Rent reserve.
    pub const fn root_rent(self) -> u64 {
        self.root_rent
    }

    /// Unsolicited root lamports; never funding or Hoard principal.
    pub const fn donation(self) -> u64 {
        self.donation
    }

    /// Complete root balance credited to Rent V2.
    pub fn total_credit(self) -> Result<u64, SeriesTerminalErrorV3> {
        self.root_rent
            .checked_add(self.close_rent)
            .and_then(|value| value.checked_add(self.donation))
            .ok_or(SeriesTerminalErrorV3::Arithmetic)
    }
}

/// Plan root deletion after every occurrence settled and every Ticket retired.
pub fn plan_series_root_closure_v3(
    template: TemplateV3,
    series: SeriesStateV3,
    expected_series_revision: u64,
    observed_root_lamports: u64,
    exact_root_rent: u64,
    rent_sink: SeriesLifecycleRentSinkV3,
) -> Result<SeriesRootClosurePlanV3, SeriesTerminalErrorV3> {
    rent_sink.requires_wallet(template.refund_owner())?;
    if exact_root_rent == 0 {
        return Err(SeriesTerminalErrorV3::Balance);
    }
    let series_bytes = series
        .encode(template.occurrence_count())
        .map_err(|_| SeriesTerminalErrorV3::Replay)?;
    let witness = evaluate_replay_v3(
        SeriesReplayActionV3::Close,
        template.occurrence_count(),
        expected_series_revision,
        &series_bytes,
        None,
    )
    .map_err(|_| SeriesTerminalErrorV3::Replay)?;
    if witness.series() != ReplayCandidateV3::Delete
        || witness.ticket() != ReplayCandidateV3::Unchanged
    {
        return Err(SeriesTerminalErrorV3::Replay);
    }
    let classified = exact_root_rent
        .checked_add(series.close_rent_remaining())
        .ok_or(SeriesTerminalErrorV3::Arithmetic)?;
    let donation = observed_root_lamports
        .checked_sub(classified)
        .ok_or(SeriesTerminalErrorV3::Balance)?;
    Ok(SeriesRootClosurePlanV3 {
        rent_sink,
        close_rent: series.close_rent_remaining(),
        root_rent: exact_root_rent,
        donation,
    })
}

#[cfg(test)]
mod tests {
    use dclutch_rent_contract::{
        RefundAuthority,
        lifecycle_v2::{LifecycleAccountIdV2, LifecycleRentCreditV2},
    };

    use super::*;
    use crate::{
        SERIES_TICKET_BYTES_V3, admit_ticket, generated,
        replay::{TicketPhaseV3, TicketStateV3},
    };

    fn key(byte: u8) -> AccountKeyV3 {
        AccountKeyV3::new([byte; 32]).expect("nonzero")
    }

    fn content(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero")
    }

    fn sink(wallet: AccountKeyV3) -> SeriesLifecycleRentSinkV3 {
        let credit = LifecycleRentCreditV2::new(
            RefundAuthority::new(wallet.to_bytes()).expect("wallet"),
            LifecycleAccountIdV2::new([31; 32]).expect("Market"),
            LifecycleAccountIdV2::new([32; 32]).expect("release"),
            7,
            9,
        )
        .expect("credit");
        SeriesLifecycleRentSinkV3::admit(
            key(30),
            &credit.to_bytes(),
            key(31),
            content(32),
            7,
            wallet,
        )
        .expect("sink")
    }

    fn admitted_ticket(wallet: AccountKeyV3) -> AdmittedTicketV3 {
        let mut bytes: [u8; SERIES_TICKET_BYTES_V3] = generated::SERIES_EXAMPLE_TICKET_V3;
        bytes[176..208].copy_from_slice(&wallet.to_bytes());
        // The fixture owns no outer commitment here; hostile Ticket decoding is
        // still exact and its content identity follows the changed bytes.
        admit_ticket(&bytes).expect("Ticket")
    }

    #[test]
    fn terminal_ticket_credits_rent_v2_and_refuses_replay_or_wallet_substitution() {
        let wallet = key(41);
        let ticket = admitted_ticket(wallet);
        let ticket_id = ticket.content_id();
        let initial = SeriesStateV3::new(19);
        let prepared = initial.prepare_ticket(0).expect("prepare");
        let settled = prepared.settle_current(1, 1).expect("settle");
        let consumed = TicketStateV3::prepared(ticket_id)
            .settle(0, TicketPhaseV3::Consumed)
            .expect("consume");
        let plan =
            plan_ticket_retirement_v3(1, settled, consumed, ticket, 2, 1, 29, 23, sink(wallet))
                .expect("retire");
        assert_eq!(plan.ticket_rent(), 23);
        assert_eq!(plan.donation(), 6);
        assert_eq!(plan.total_credit(), Ok(29));
        assert_eq!(plan.series_after().outstanding_ticket_accounts(), 0);

        assert_eq!(
            plan_ticket_retirement_v3(
                1,
                plan.series_after(),
                consumed,
                ticket,
                3,
                1,
                29,
                23,
                sink(wallet),
            ),
            Err(SeriesTerminalErrorV3::Replay)
        );
        assert_eq!(
            plan_ticket_retirement_v3(1, settled, consumed, ticket, 2, 1, 29, 23, sink(key(42)),),
            Err(SeriesTerminalErrorV3::RentBinding)
        );
    }

    #[test]
    fn rent_sink_hostile_decode_and_stale_lifecycle_refuse() {
        let wallet = key(51);
        let credit = LifecycleRentCreditV2::new(
            RefundAuthority::new(wallet.to_bytes()).expect("wallet"),
            LifecycleAccountIdV2::new([31; 32]).expect("Market"),
            LifecycleAccountIdV2::new([32; 32]).expect("release"),
            7,
            9,
        )
        .expect("credit");
        let mut hostile = credit.to_bytes();
        hostile[127] = 1;
        assert_eq!(
            SeriesLifecycleRentSinkV3::admit(key(30), &hostile, key(31), content(32), 7, wallet,),
            Err(SeriesTerminalErrorV3::RentEncoding)
        );
        assert_eq!(
            SeriesLifecycleRentSinkV3::admit(
                key(30),
                &credit.to_bytes(),
                key(31),
                content(32),
                8,
                wallet,
            ),
            Err(SeriesTerminalErrorV3::RentBinding)
        );
    }

    #[test]
    fn root_close_waits_for_every_occurrence_and_ticket_then_classifies_all_lamports() {
        let wallet = key(61);
        let mut template_bytes = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        template_bytes[generated::SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V3
            ..generated::SERIES_TEMPLATE_OCCURRENCE_COUNT_OFFSET_V3 + 4]
            .copy_from_slice(&1_u32.to_le_bytes());
        template_bytes[generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3
            ..generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3 + 32]
            .copy_from_slice(&wallet.to_bytes());
        let template = TemplateV3::decode(&template_bytes).expect("Template");
        let initial = SeriesStateV3::new(template.close_rent());
        assert_eq!(
            plan_series_root_closure_v3(template, initial, 0, 40, 10, sink(wallet),),
            Err(SeriesTerminalErrorV3::Replay)
        );

        let prepared = initial.prepare_ticket(0).expect("prepare");
        let settled = prepared.settle_current(1, 1).expect("settle");
        assert_eq!(
            plan_series_root_closure_v3(template, settled, 2, 40, 10, sink(wallet),),
            Err(SeriesTerminalErrorV3::Replay)
        );
        let retired = settled.retire_ticket(2).expect("retire");
        let plan =
            plan_series_root_closure_v3(template, retired, 3, 40, 10, sink(wallet)).expect("close");
        assert_eq!(plan.root_rent(), 10);
        assert_eq!(plan.close_rent(), template.close_rent());
        assert_eq!(
            plan.donation(),
            30_u64
                .checked_sub(template.close_rent())
                .expect("fixture close rent")
        );
        assert_eq!(plan.total_credit(), Ok(40));

        assert_eq!(
            plan_series_root_closure_v3(template, retired, 2, 40, 10, sink(wallet),),
            Err(SeriesTerminalErrorV3::Replay)
        );
        assert_eq!(
            plan_series_root_closure_v3(template, retired, 3, 40, 10, sink(key(62)),),
            Err(SeriesTerminalErrorV3::RentBinding)
        );
    }
}
