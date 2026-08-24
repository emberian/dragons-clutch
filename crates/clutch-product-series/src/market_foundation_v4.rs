//! Canonical 50-account current Market foundation schedule and graph.
//!
//! Indices `0..=46` preserve the V3 coordinates exactly. The three appended
//! General treasury accounts are distinct mandatory principals owned by the
//! same MarketCore foundation decomposition; they are never caller-funded.

use crate::{
    content_id, ContentId, Error, MarketFoundationAccountGraphV4Id,
    MarketFoundationScheduleV4Id, MarketInstanceV2Id, Result,
};

/// Fifteen fixed roles, ending with the Hoard collateral vault at index 14.
pub const MARKET_FOUNDATION_CORE_SLOT_COUNT_V4: usize = 15;
/// Maximum outcome count represented by the current foundation schedule.
pub const MARKET_FOUNDATION_MAX_OUTCOMES_V4: usize = 16;
/// Three mandatory General treasury roles appended after the stable V3 slots.
pub const MARKET_FOUNDATION_GENERAL_TREASURY_SLOT_COUNT_V4: usize = 3;
/// Fifteen core roles, sixteen mints, sixteen custody accounts, and three
/// General treasury accounts.
pub const MARKET_FOUNDATION_SLOT_COUNT_V4: usize = MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
    + 2 * MARKET_FOUNDATION_MAX_OUTCOMES_V4
    + MARKET_FOUNDATION_GENERAL_TREASURY_SLOT_COUNT_V4;
/// Exact byte preimage of one V4 foundation schedule.
pub const MARKET_FOUNDATION_SCHEDULE_BYTES_V4: usize =
    16 + MARKET_FOUNDATION_SLOT_COUNT_V4 * 8;
/// Exact byte preimage of one V4 foundation graph.
pub const MARKET_FOUNDATION_ACCOUNT_GRAPH_BYTES_V4: usize =
    32 + 8 + 32 + MARKET_FOUNDATION_SLOT_COUNT_V4 * 32;
/// Semantic identity domain for the current foundation schedule.
pub const MARKET_FOUNDATION_SCHEDULE_V4_DOMAIN: &[u8] =
    b"dragons-clutch/market-foundation-schedule/v4";
/// Semantic identity domain for the current foundation graph.
pub const MARKET_FOUNDATION_ACCOUNT_GRAPH_V4_DOMAIN: &[u8] =
    b"dragons-clutch/market-foundation-account-graph/v4";

const _: () = {
    assert!(MARKET_FOUNDATION_SLOT_COUNT_V4 == 50);
    assert!(MARKET_FOUNDATION_SCHEDULE_BYTES_V4 == 416);
    assert!(MARKET_FOUNDATION_ACCOUNT_GRAPH_BYTES_V4 == 1_672);
};

/// Canonical ordered current foundation slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarketFoundationSlotV4 {
    /// Product lifecycle root.
    LifecycleRoot,
    /// General MarketBindingV4.
    MarketBinding,
    /// General MarketRuntimeV3.
    MarketRuntime,
    /// Hoard state.
    Hoard,
    /// Claim ledger.
    ClaimLedger,
    /// Failure market admission root.
    FailureAdmissionRoot,
    /// Failure market runtime root.
    FailureRuntimeRoot,
    /// Permanent Failure replay.
    FailureReplay,
    /// Reusable Failure interval work cell.
    FailureIntervalWork,
    /// Failure interval history.
    FailureIntervalHistory,
    /// Resolution V5.
    ResolutionV5,
    /// Fractional policy.
    FractionalPolicy,
    /// Fractional ledger.
    FractionalLedger,
    /// Permanent Product market lifecycle replay.
    ProductReplayAnchor,
    /// Distinct release-selected raw-collateral Hoard token account.
    HoardCollateralVault,
    /// Outcome mint `0..outcome_count`.
    OutcomeMint(u8),
    /// Outcome custody account `0..outcome_count`.
    OutcomeCustody(u8),
    /// General treasury PositionV3.
    GeneralTreasuryPosition,
    /// General treasury ReplayV3.
    GeneralTreasuryReplay,
    /// TreasuryServiceLedger at the current `0xbb` coordinate.
    TreasuryServiceLedger,
}

impl MarketFoundationSlotV4 {
    /// Exact fixed slot-table index.
    pub fn index(self) -> Result<usize> {
        match self {
            Self::LifecycleRoot => Ok(0),
            Self::MarketBinding => Ok(1),
            Self::MarketRuntime => Ok(2),
            Self::Hoard => Ok(3),
            Self::ClaimLedger => Ok(4),
            Self::FailureAdmissionRoot => Ok(5),
            Self::FailureRuntimeRoot => Ok(6),
            Self::FailureReplay => Ok(7),
            Self::FailureIntervalWork => Ok(8),
            Self::FailureIntervalHistory => Ok(9),
            Self::ResolutionV5 => Ok(10),
            Self::FractionalPolicy => Ok(11),
            Self::FractionalLedger => Ok(12),
            Self::ProductReplayAnchor => Ok(13),
            Self::HoardCollateralVault => Ok(14),
            Self::OutcomeMint(outcome) => {
                let index = usize::from(outcome);
                if index >= MARKET_FOUNDATION_MAX_OUTCOMES_V4 {
                    return Err(Error::InvalidParameter);
                }
                MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
                    .checked_add(index)
                    .ok_or(Error::ArithmeticOverflow)
            }
            Self::OutcomeCustody(outcome) => {
                let index = usize::from(outcome);
                if index >= MARKET_FOUNDATION_MAX_OUTCOMES_V4 {
                    return Err(Error::InvalidParameter);
                }
                MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
                    .checked_add(MARKET_FOUNDATION_MAX_OUTCOMES_V4)
                    .and_then(|base| base.checked_add(index))
                    .ok_or(Error::ArithmeticOverflow)
            }
            Self::GeneralTreasuryPosition => Ok(47),
            Self::GeneralTreasuryReplay => Ok(48),
            Self::TreasuryServiceLedger => Ok(49),
        }
    }

    fn from_index(index: usize) -> Result<Self> {
        match index {
            0 => Ok(Self::LifecycleRoot),
            1 => Ok(Self::MarketBinding),
            2 => Ok(Self::MarketRuntime),
            3 => Ok(Self::Hoard),
            4 => Ok(Self::ClaimLedger),
            5 => Ok(Self::FailureAdmissionRoot),
            6 => Ok(Self::FailureRuntimeRoot),
            7 => Ok(Self::FailureReplay),
            8 => Ok(Self::FailureIntervalWork),
            9 => Ok(Self::FailureIntervalHistory),
            10 => Ok(Self::ResolutionV5),
            11 => Ok(Self::FractionalPolicy),
            12 => Ok(Self::FractionalLedger),
            13 => Ok(Self::ProductReplayAnchor),
            14 => Ok(Self::HoardCollateralVault),
            15..=30 => Ok(Self::OutcomeMint(
                u8::try_from(index - MARKET_FOUNDATION_CORE_SLOT_COUNT_V4)
                    .map_err(|_| Error::InvalidParameter)?,
            )),
            31..=46 => Ok(Self::OutcomeCustody(
                u8::try_from(
                    index
                        - MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
                        - MARKET_FOUNDATION_MAX_OUTCOMES_V4,
                )
                .map_err(|_| Error::InvalidParameter)?,
            )),
            47 => Ok(Self::GeneralTreasuryPosition),
            48 => Ok(Self::GeneralTreasuryReplay),
            49 => Ok(Self::TreasuryServiceLedger),
            _ => Err(Error::InvalidParameter),
        }
    }
}

fn slot_is_active(index: usize, outcome_count: u8) -> Result<bool> {
    let outcomes = usize::from(outcome_count);
    if outcomes == 0 || outcomes > MARKET_FOUNDATION_MAX_OUTCOMES_V4 {
        return Err(Error::InvalidParameter);
    }
    let mint_end = MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
        .checked_add(outcomes)
        .ok_or(Error::ArithmeticOverflow)?;
    let custody_start = MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
        .checked_add(MARKET_FOUNDATION_MAX_OUTCOMES_V4)
        .ok_or(Error::ArithmeticOverflow)?;
    let custody_end = custody_start
        .checked_add(outcomes)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(index < MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
        || (index >= MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 && index < mint_end)
        || (index >= custody_start && index < custody_end)
        || index >= 47)
}

/// Current exact quote-owned principal for all 50 foundation accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFoundationScheduleV4 {
    /// Active outcome count; inactive mint/custody tails are zero.
    pub outcome_count: u8,
    /// Canonical one-account/one-principal slot array.
    pub slot_principal_lamports: [u64; MARKET_FOUNDATION_SLOT_COUNT_V4],
    /// Finite timeout after which an inert foundation may abort.
    pub founding_timeout_buckets: u64,
}

impl MarketFoundationScheduleV4 {
    /// Validate exact mandatory presence, active outcome presence, and zero tails.
    pub fn validate(&self) -> Result<()> {
        if self.founding_timeout_buckets == 0 {
            return Err(Error::InvalidParameter);
        }
        let mut index = 0usize;
        while index < MARKET_FOUNDATION_SLOT_COUNT_V4 {
            let active = slot_is_active(index, self.outcome_count)?;
            if active != (self.slot_principal_lamports[index] != 0) {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        self.total_principal_lamports()?;
        Ok(())
    }

    /// Checked sum of every active one-account principal.
    pub fn total_principal_lamports(&self) -> Result<u64> {
        let mut total = 0u64;
        for amount in self.slot_principal_lamports {
            total = total.checked_add(amount).ok_or(Error::ArithmeticOverflow)?;
        }
        if total == 0 {
            return Err(Error::InsufficientPrepayment);
        }
        Ok(total)
    }

    /// Typed identity of the exact 50-slot itemization and timeout.
    pub fn id(&self) -> Result<MarketFoundationScheduleV4Id> {
        self.validate()?;
        let mut body = [0u8; MARKET_FOUNDATION_SCHEDULE_BYTES_V4];
        body[0] = self.outcome_count;
        body[8..16].copy_from_slice(&self.founding_timeout_buckets.to_le_bytes());
        let mut at = 16usize;
        for amount in self.slot_principal_lamports {
            body[at..at + 8].copy_from_slice(&amount.to_le_bytes());
            at += 8;
        }
        Ok(MarketFoundationScheduleV4Id::from_bytes(
            content_id(MARKET_FOUNDATION_SCHEDULE_V4_DOMAIN, &body).bytes(),
        ))
    }
}

/// Canonical ordered physical account graph for one prepaid Market foundation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFoundationAccountGraphV4 {
    /// Full-width shared Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact shared generation.
    pub generation: u64,
    /// Exact quote-owned slot schedule.
    pub foundation_schedule_id: MarketFoundationScheduleV4Id,
    /// Accounts in [`MarketFoundationSlotV4`] order.
    pub account_ids: [ContentId; MARKET_FOUNDATION_SLOT_COUNT_V4],
}

impl MarketFoundationAccountGraphV4 {
    /// Validate exact presence, inactive tails, and pairwise role separation.
    pub fn validate(&self, schedule: &MarketFoundationScheduleV4) -> Result<()> {
        self.market_instance_id.validate()?;
        self.foundation_schedule_id.validate()?;
        schedule.validate()?;
        if self.generation == 0 || self.foundation_schedule_id != schedule.id()? {
            return Err(Error::MismatchedArtifact);
        }
        let mut index = 0usize;
        while index < MARKET_FOUNDATION_SLOT_COUNT_V4 {
            let active = slot_is_active(index, schedule.outcome_count)?;
            if active != !self.account_ids[index].is_zero() {
                return Err(Error::NonCanonicalPadding);
            }
            if active {
                self.account_ids[index].validate()?;
                let mut prior = 0usize;
                while prior < index {
                    if self.account_ids[prior] == self.account_ids[index] {
                        return Err(Error::MismatchedArtifact);
                    }
                    prior += 1;
                }
            }
            index += 1;
        }
        Ok(())
    }

    /// Typed identity of the complete Market/generation/schedule/account graph.
    pub fn id(
        &self,
        schedule: &MarketFoundationScheduleV4,
    ) -> Result<MarketFoundationAccountGraphV4Id> {
        self.validate(schedule)?;
        let mut body = [0u8; MARKET_FOUNDATION_ACCOUNT_GRAPH_BYTES_V4];
        body[..32].copy_from_slice(&self.market_instance_id.bytes());
        body[32..40].copy_from_slice(&self.generation.to_le_bytes());
        body[40..72].copy_from_slice(&self.foundation_schedule_id.bytes());
        let mut at = 72usize;
        for account in self.account_ids {
            body[at..at + 32].copy_from_slice(&account.bytes());
            at += 32;
        }
        Ok(MarketFoundationAccountGraphV4Id::from_bytes(
            content_id(MARKET_FOUNDATION_ACCOUNT_GRAPH_V4_DOMAIN, &body).bytes(),
        ))
    }

    /// Exact account occupying one canonical active slot.
    pub fn account(&self, slot: MarketFoundationSlotV4) -> Result<ContentId> {
        let account = self.account_ids[slot.index()?];
        account.validate()?;
        Ok(account)
    }
}

/// Borrowed, fully validated V4 graph preimage for stack-bounded adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketFoundationAccountGraphBytesV4<'a> {
    input: &'a [u8],
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    foundation_schedule_id: MarketFoundationScheduleV4Id,
    graph_id: MarketFoundationAccountGraphV4Id,
}

impl AuthenticatedMarketFoundationAccountGraphBytesV4<'_> {
    /// Full-width Market parsed from the complete preimage.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id { self.market_instance_id }
    /// Exact nonzero generation parsed from the complete preimage.
    pub const fn generation(self) -> u64 { self.generation }
    /// Exact schedule identity checked against the supplied schedule body.
    pub const fn foundation_schedule_id(self) -> MarketFoundationScheduleV4Id {
        self.foundation_schedule_id
    }
    /// Content identity of the complete canonical 1,672-byte preimage.
    pub const fn graph_id(self) -> MarketFoundationAccountGraphV4Id { self.graph_id }
    /// Read one active account from the authenticated borrowed preimage.
    pub fn account(self, slot: MarketFoundationSlotV4) -> Result<ContentId> {
        let at = 72usize
            .checked_add(slot.index()?.checked_mul(32).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)?;
        let account = ContentId::from_bytes(
            self.input[at..at + 32]
                .try_into()
                .map_err(|_| Error::InvalidCodec)?,
        );
        account.validate()?;
        Ok(account)
    }
}

/// Hostile-decode and identify a canonical V4 graph without materializing it.
pub fn authenticate_market_foundation_account_graph_bytes_v4<'a>(
    input: &'a [u8],
    schedule: &MarketFoundationScheduleV4,
) -> Result<AuthenticatedMarketFoundationAccountGraphBytesV4<'a>> {
    if input.len() != MARKET_FOUNDATION_ACCOUNT_GRAPH_BYTES_V4 {
        return Err(Error::InvalidCodec);
    }
    schedule.validate()?;
    let market_instance_id = MarketInstanceV2Id::from_bytes(
        input[..32].try_into().map_err(|_| Error::InvalidCodec)?,
    );
    let generation = u64::from_le_bytes(
        input[32..40].try_into().map_err(|_| Error::InvalidCodec)?,
    );
    let foundation_schedule_id = MarketFoundationScheduleV4Id::from_bytes(
        input[40..72].try_into().map_err(|_| Error::InvalidCodec)?,
    );
    market_instance_id.validate()?;
    foundation_schedule_id.validate()?;
    if generation == 0 || foundation_schedule_id != schedule.id()? {
        return Err(Error::MismatchedArtifact);
    }
    let mut index = 0usize;
    while index < MARKET_FOUNDATION_SLOT_COUNT_V4 {
        let at = 72usize
            .checked_add(index.checked_mul(32).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)?;
        let account = ContentId::from_bytes(
            input[at..at + 32]
                .try_into()
                .map_err(|_| Error::InvalidCodec)?,
        );
        let active = slot_is_active(index, schedule.outcome_count)?;
        if active != !account.is_zero() {
            return Err(Error::NonCanonicalPadding);
        }
        if active {
            account.validate()?;
            let mut prior = 0usize;
            while prior < index {
                let prior_at = 72usize
                    .checked_add(prior.checked_mul(32).ok_or(Error::ArithmeticOverflow)?)
                    .ok_or(Error::ArithmeticOverflow)?;
                if input[prior_at..prior_at + 32] == input[at..at + 32] {
                    return Err(Error::MismatchedArtifact);
                }
                prior += 1;
            }
        }
        index += 1;
    }
    let graph_id = MarketFoundationAccountGraphV4Id::from_bytes(
        content_id(MARKET_FOUNDATION_ACCOUNT_GRAPH_V4_DOMAIN, input).bytes(),
    );
    Ok(AuthenticatedMarketFoundationAccountGraphBytesV4 {
        input,
        market_instance_id,
        generation,
        foundation_schedule_id,
        graph_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule() -> MarketFoundationScheduleV4 {
        let mut slots = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V4];
        for principal in &mut slots[..MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 + 2] {
            *principal = 1;
        }
        let custody = MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
            + MARKET_FOUNDATION_MAX_OUTCOMES_V4;
        for principal in &mut slots[custody..custody + 2] {
            *principal = 1;
        }
        for principal in &mut slots[47..50] {
            *principal = 1;
        }
        MarketFoundationScheduleV4 {
            outcome_count: 2,
            slot_principal_lamports: slots,
            founding_timeout_buckets: 7,
        }
    }

    fn graph(schedule: MarketFoundationScheduleV4) -> MarketFoundationAccountGraphV4 {
        let mut ids = [ContentId::ZERO; MARKET_FOUNDATION_SLOT_COUNT_V4];
        let mut index = 0usize;
        while index < ids.len() {
            if slot_is_active(index, schedule.outcome_count).unwrap() {
                let byte = u8::try_from(index + 1).unwrap();
                ids[index] = ContentId::from_bytes([byte; 32]);
            }
            index += 1;
        }
        MarketFoundationAccountGraphV4 {
            market_instance_id: MarketInstanceV2Id::from_bytes([100; 32]),
            generation: 1,
            foundation_schedule_id: schedule.id().unwrap(),
            account_ids: ids,
        }
    }

    #[test]
    fn exact_slot_coordinates_are_frozen_and_v3_prefix_is_stable() {
        assert_eq!(MarketFoundationSlotV4::ProductReplayAnchor.index(), Ok(13));
        assert_eq!(MarketFoundationSlotV4::HoardCollateralVault.index(), Ok(14));
        assert_eq!(MarketFoundationSlotV4::OutcomeMint(0).index(), Ok(15));
        assert_eq!(MarketFoundationSlotV4::OutcomeMint(15).index(), Ok(30));
        assert_eq!(MarketFoundationSlotV4::OutcomeCustody(0).index(), Ok(31));
        assert_eq!(MarketFoundationSlotV4::OutcomeCustody(15).index(), Ok(46));
        assert_eq!(MarketFoundationSlotV4::GeneralTreasuryPosition.index(), Ok(47));
        assert_eq!(MarketFoundationSlotV4::GeneralTreasuryReplay.index(), Ok(48));
        assert_eq!(MarketFoundationSlotV4::TreasuryServiceLedger.index(), Ok(49));
        assert!(MarketFoundationSlotV4::OutcomeMint(16).index().is_err());
    }

    #[test]
    fn appended_general_principals_are_mandatory() {
        let valid = schedule();
        assert!(valid.validate().is_ok());
        for index in 47..50 {
            let mut missing = valid;
            missing.slot_principal_lamports[index] = 0;
            assert_eq!(missing.validate(), Err(Error::NonCanonicalPadding));
        }
    }

    #[test]
    fn graph_refuses_alias_and_missing_treasury_account() {
        let schedule = schedule();
        let valid = graph(schedule);
        assert!(valid.validate(&schedule).is_ok());
        let mut alias = valid;
        alias.account_ids[47] = alias.account_ids[1];
        assert_eq!(alias.validate(&schedule), Err(Error::MismatchedArtifact));
        let mut missing = valid;
        missing.account_ids[49] = ContentId::ZERO;
        assert_eq!(missing.validate(&schedule), Err(Error::NonCanonicalPadding));
    }
}
