//! Exact fixed-layout provider-neutral categorical Market state.

use core::convert::TryInto;

use dclutch_core_contract::{MARKET_ROOT_BYTES, MarketRoot, Phase as RootPhase};
use dclutch_kernel::{CategoricalLedger, MAX_OUTCOMES, MIN_OUTCOMES, Phase as LedgerPhase};
use dclutch_product_contract::{ContentId, terminal::ResolutionKind};

use crate::{Error, Result};

/// Canonical provider-neutral categorical Market magic.
pub const MARKET_MAGIC: [u8; 8] = *b"DCLTCAT1";
/// Implemented provider-neutral Market schema version.
pub const MARKET_SCHEMA_VERSION: u16 = 1;
/// Provisional exact categorical profile discriminator.
///
/// This profile supports `N = 2..=16` because that is the current measured
/// kernel envelope, not because sixteen is a mathematical protocol limit. A
/// lift requires a new reviewed profile discriminator and matching kernel
/// implementation; it does not require adding provider fields to Market state.
pub const PROVISIONAL_CATEGORICAL_PROFILE_V1: u8 = 1;
/// Exact header width of the provider-neutral Market account.
pub const MARKET_HEADER_BYTES: usize = 16;
/// Byte offset of the exact categorical outcome count.
pub const MARKET_OUTCOME_COUNT_OFFSET: usize = 10;
/// Byte offset of the provisional categorical profile discriminator.
pub const MARKET_PROFILE_OFFSET: usize = 11;
/// Byte offset of the compact [`MarketRoot`].
pub const MARKET_ROOT_OFFSET: usize = MARKET_HEADER_BYTES;
/// Byte offset of claimant-backing Hoard atoms.
pub const MARKET_HOARD_OFFSET: usize = MARKET_ROOT_OFFSET + MARKET_ROOT_BYTES;
/// Byte offset of the first exact aggregate supply entry.
pub const MARKET_SUPPLY_OFFSET: usize = MARKET_HOARD_OFFSET + 8;
/// Exact encoded width of [`CategoricalSettlementSummaryV1`].
pub const SETTLEMENT_SUMMARY_BYTES: usize = 64;
/// Fixed Market width excluding the `N` eight-byte supply entries.
pub const MARKET_BASE_BYTES: usize = MARKET_SUPPLY_OFFSET + SETTLEMENT_SUMMARY_BYTES;
/// Exact width of a binary provider-neutral Market.
pub const BINARY_MARKET_BYTES: usize = MARKET_BASE_BYTES + MIN_OUTCOMES * 8;
/// Exact width of the largest Market in provisional profile V1.
pub const MAX_MARKET_BYTES: usize = MARKET_BASE_BYTES + MAX_OUTCOMES * 8;

const HEADER_RESERVED_OFFSET: usize = 12;
const HEADER_RESERVED_BYTES: usize = 4;
const SETTLEMENT_STATUS_OFFSET: usize = 0;
const SETTLEMENT_ROUTE_OFFSET: usize = 1;
const SETTLEMENT_WINNER_OFFSET: usize = 2;
const SETTLEMENT_HEADER_RESERVED_OFFSET: usize = 3;
const SETTLEMENT_HEADER_RESERVED_BYTES: usize = 5;
const SETTLEMENT_SEQUENCE_OFFSET: usize = 8;
const SETTLEMENT_EVIDENCE_OFFSET: usize = 16;
const SETTLEMENT_TAIL_RESERVED_OFFSET: usize = 48;
const SETTLEMENT_TAIL_RESERVED_BYTES: usize = 16;
const SETTLEMENT_RESOLVED: u8 = 1;

/// Exact terminal categorical truth retained for redemption and replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalResolutionV1 {
    resolution_evidence_id: ContentId,
    resolution_kind: ResolutionKind,
    winner: u8,
    terminal_sequence: u64,
}

impl CategoricalResolutionV1 {
    /// Construct a resolution for one exact compile-time Market width.
    pub fn for_market<const N: usize>(
        resolution_evidence_id: ContentId,
        resolution_kind: ResolutionKind,
        winner: usize,
        terminal_sequence: u64,
    ) -> Result<Self> {
        outcome_count::<N>()?;
        if winner >= N {
            return Err(Error::InvalidWinner);
        }
        if terminal_sequence == 0 {
            return Err(Error::ZeroTerminalSequence);
        }
        let winner = u8::try_from(winner).map_err(|_| Error::InvalidWinner)?;
        Ok(Self {
            resolution_evidence_id,
            resolution_kind,
            winner,
            terminal_sequence,
        })
    }

    /// Return the accepted resolution-evidence content identity.
    pub const fn resolution_evidence_id(self) -> ContentId {
        self.resolution_evidence_id
    }

    /// Return the Product-owned provider-neutral resolution route.
    pub const fn resolution_kind(self) -> ResolutionKind {
        self.resolution_kind
    }

    /// Return the zero-based winning state cell.
    pub const fn winner(self) -> u8 {
        self.winner
    }

    /// Return the positive monotone terminal sequence.
    pub const fn terminal_sequence(self) -> u64 {
        self.terminal_sequence
    }

    fn validate<const N: usize>(self) -> Result<()> {
        outcome_count::<N>()?;
        if usize::from(self.winner) >= N {
            return Err(Error::InvalidWinner);
        }
        if self.terminal_sequence == 0 {
            return Err(Error::ZeroTerminalSequence);
        }
        Ok(())
    }
}

/// Canonical optional 64-byte categorical settlement summary.
///
/// Empty is encoded as sixty-four zero bytes. Resolved stores only status,
/// Product-owned route, winner, positive sequence, and evidence content ID;
/// Market identity, Product identity, generation, and outcome count already
/// have one semantic owner in the root or exact `N` profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalSettlementSummaryV1 {
    resolution: Option<CategoricalResolutionV1>,
}

impl CategoricalSettlementSummaryV1 {
    /// Return the unique canonical empty summary.
    pub const fn empty() -> Self {
        Self { resolution: None }
    }

    /// Construct a canonical resolved summary for one exact Market width.
    pub fn resolved<const N: usize>(
        resolution_evidence_id: ContentId,
        resolution_kind: ResolutionKind,
        winner: usize,
        terminal_sequence: u64,
    ) -> Result<Self> {
        Ok(Self {
            resolution: Some(CategoricalResolutionV1::for_market::<N>(
                resolution_evidence_id,
                resolution_kind,
                winner,
                terminal_sequence,
            )?),
        })
    }

    /// Decode one exact summary against the active compile-time Market width.
    pub fn decode<const N: usize>(bytes: &[u8]) -> Result<Self> {
        outcome_count::<N>()?;
        if bytes.len() != SETTLEMENT_SUMMARY_BYTES {
            return Err(Error::InvalidLength);
        }
        match byte(bytes, SETTLEMENT_STATUS_OFFSET)? {
            0 => {
                require_zero(bytes, 0, SETTLEMENT_SUMMARY_BYTES)?;
                Ok(Self::empty())
            }
            SETTLEMENT_RESOLVED => {
                require_zero(
                    bytes,
                    SETTLEMENT_HEADER_RESERVED_OFFSET,
                    SETTLEMENT_HEADER_RESERVED_BYTES,
                )?;
                require_zero(
                    bytes,
                    SETTLEMENT_TAIL_RESERVED_OFFSET,
                    SETTLEMENT_TAIL_RESERVED_BYTES,
                )?;
                let resolution_kind = ResolutionKind::decode(byte(bytes, SETTLEMENT_ROUTE_OFFSET)?)
                    .map_err(|error| Error::InvalidProductContract { error })?;
                let winner = usize::from(byte(bytes, SETTLEMENT_WINNER_OFFSET)?);
                let terminal_sequence =
                    u64::from_le_bytes(array(bytes, SETTLEMENT_SEQUENCE_OFFSET)?);
                let resolution_evidence_id = ContentId::decode(
                    bytes
                        .get(SETTLEMENT_EVIDENCE_OFFSET..SETTLEMENT_EVIDENCE_OFFSET + 32)
                        .ok_or(Error::InvalidLength)?,
                )
                .map_err(|error| Error::InvalidProductContract { error })?;
                Self::resolved::<N>(
                    resolution_evidence_id,
                    resolution_kind,
                    winner,
                    terminal_sequence,
                )
            }
            _ => Err(Error::UnknownSettlementStatus),
        }
    }

    /// Encode the exact reusable 64-byte settlement summary.
    pub fn to_bytes<const N: usize>(self) -> Result<[u8; SETTLEMENT_SUMMARY_BYTES]> {
        self.validate::<N>()?;
        let mut output = [0u8; SETTLEMENT_SUMMARY_BYTES];
        if let Some(resolution) = self.resolution {
            copy_at(
                &mut output,
                SETTLEMENT_STATUS_OFFSET,
                &[SETTLEMENT_RESOLVED],
            );
            copy_at(
                &mut output,
                SETTLEMENT_ROUTE_OFFSET,
                &[resolution.resolution_kind.byte()],
            );
            copy_at(&mut output, SETTLEMENT_WINNER_OFFSET, &[resolution.winner]);
            copy_at(
                &mut output,
                SETTLEMENT_SEQUENCE_OFFSET,
                &resolution.terminal_sequence.to_le_bytes(),
            );
            copy_at(
                &mut output,
                SETTLEMENT_EVIDENCE_OFFSET,
                resolution.resolution_evidence_id.as_bytes(),
            );
        }
        Ok(output)
    }

    /// Return the exact terminal truth, or `None` for canonical empty.
    pub const fn resolution(self) -> Option<CategoricalResolutionV1> {
        self.resolution
    }

    /// Return whether this is the unique canonical empty summary.
    pub const fn is_empty(self) -> bool {
        self.resolution.is_none()
    }

    fn validate<const N: usize>(self) -> Result<()> {
        outcome_count::<N>()?;
        if let Some(resolution) = self.resolution {
            resolution.validate::<N>()?;
        }
        Ok(())
    }
}

/// Provider-neutral active categorical Market for exactly `N` ordered cells.
///
/// Each state cell owns one native claim. A complete set contains exactly one
/// unit of every cell claim and is backed by one Hoard collateral atom. Richer
/// payoff shapes belong in separately committed portfolio templates, not in
/// this elementary state-claim basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CategoricalMarketV1<const N: usize> {
    root: MarketRoot,
    hoard_atoms: u64,
    supply: [u64; N],
    settlement: CategoricalSettlementSummaryV1,
}

impl<const N: usize> CategoricalMarketV1<N> {
    /// Construct and cross-validate one provider-neutral categorical Market.
    pub fn new(
        root: MarketRoot,
        hoard_atoms: u64,
        supply: [u64; N],
        settlement: CategoricalSettlementSummaryV1,
    ) -> Result<Self> {
        let market = Self {
            root,
            hoard_atoms,
            supply,
            settlement,
        };
        market.validate()?;
        Ok(market)
    }

    /// Return the checked exact account width, `320 + 8N` bytes.
    pub fn encoded_len() -> Result<usize> {
        required_bytes::<N>()
    }

    /// Return the exact active categorical outcome count.
    pub fn outcome_count() -> Result<u8> {
        outcome_count::<N>()
    }

    /// Decode and validate one exact provider-neutral Market account.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let stored_outcome_count = decode_market_outcome_count(bytes)?;
        if stored_outcome_count != outcome_count::<N>()? {
            return Err(Error::InvalidOutcomeCount);
        }
        let root_end = MARKET_ROOT_OFFSET
            .checked_add(MARKET_ROOT_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let root = MarketRoot::decode(
            bytes
                .get(MARKET_ROOT_OFFSET..root_end)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|error| Error::InvalidMarketRoot { error })?;
        let hoard_atoms = u64::from_le_bytes(array(bytes, MARKET_HOARD_OFFSET)?);
        let mut supply = [0u64; N];
        let mut index = 0usize;
        while index < N {
            let offset = supply_entry_offset(index)?;
            let destination = supply.get_mut(index).ok_or(Error::ArithmeticOverflow)?;
            *destination = u64::from_le_bytes(array(bytes, offset)?);
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let settlement_offset = settlement_offset::<N>()?;
        let settlement_end = settlement_offset
            .checked_add(SETTLEMENT_SUMMARY_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let settlement = CategoricalSettlementSummaryV1::decode::<N>(
            bytes
                .get(settlement_offset..settlement_end)
                .ok_or(Error::InvalidLength)?,
        )?;
        Self::new(root, hoard_atoms, supply, settlement)
    }

    /// Encode into one exact caller-owned account buffer.
    ///
    /// All fallible validation and layout arithmetic completes before the
    /// output is changed, so any refusal leaves caller memory untouched.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        let expected = required_bytes::<N>()?;
        if output.len() != expected {
            return Err(Error::OutputLength);
        }
        self.validate()?;
        let settlement_offset = settlement_offset::<N>()?;
        let root = self.root.to_bytes();
        let settlement = self.settlement.to_bytes::<N>()?;
        let outcome_count = outcome_count::<N>()?;

        output.fill(0);
        copy_at(output, 0, &MARKET_MAGIC);
        copy_at(output, 8, &MARKET_SCHEMA_VERSION.to_le_bytes());
        copy_at(output, MARKET_OUTCOME_COUNT_OFFSET, &[outcome_count]);
        copy_at(
            output,
            MARKET_PROFILE_OFFSET,
            &[PROVISIONAL_CATEGORICAL_PROFILE_V1],
        );
        copy_at(output, MARKET_ROOT_OFFSET, &root);
        copy_at(output, MARKET_HOARD_OFFSET, &self.hoard_atoms.to_le_bytes());
        for (index, amount) in self.supply.iter().enumerate() {
            copy_at(
                output,
                MARKET_SUPPLY_OFFSET + index * 8,
                &amount.to_le_bytes(),
            );
        }
        copy_at(output, settlement_offset, &settlement);
        Ok(())
    }

    /// Validate root, exact width, lifecycle/summary canonicality, and solvency.
    pub fn validate(&self) -> Result<()> {
        outcome_count::<N>()?;
        self.root
            .validate()
            .map_err(|error| Error::InvalidMarketRoot { error })?;
        self.settlement.validate::<N>()?;
        validate_phase_and_economics(
            self.root.phase(),
            self.hoard_atoms,
            &self.supply,
            self.settlement,
        )?;
        self.to_kernel_ledger()?;
        Ok(())
    }

    /// Reconstruct the total allocation-free categorical liability kernel.
    pub fn to_kernel_ledger(&self) -> Result<CategoricalLedger<N>> {
        let phase = match self.settlement.resolution() {
            None => LedgerPhase::Open,
            Some(resolution) => LedgerPhase::Resolved {
                winner: usize::from(resolution.winner()),
            },
        };
        CategoricalLedger::from_parts(self.hoard_atoms, self.supply, phase)
            .map_err(|error| Error::InvalidLedger { error })
    }

    /// Deposit collateral and issue one complete set while open.
    pub fn split_complete_set(&mut self, quantity: u64) -> Result<()> {
        if self.root.phase() != RootPhase::Open {
            return Err(Error::InvalidLedger {
                error: dclutch_kernel::Error::InvalidPhase,
            });
        }
        let mut candidate = *self;
        let mut ledger = candidate.to_kernel_ledger()?;
        ledger
            .split_complete_set(quantity)
            .map_err(|error| Error::InvalidLedger { error })?;
        candidate.replace_economic_state(ledger);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Burn one complete set and release its exact backing while open.
    pub fn merge_complete_set(&mut self, quantity: u64) -> Result<()> {
        if self.root.phase() != RootPhase::Open {
            return Err(Error::InvalidLedger {
                error: dclutch_kernel::Error::InvalidPhase,
            });
        }
        let mut candidate = *self;
        let mut ledger = candidate.to_kernel_ledger()?;
        ledger
            .merge_complete_set(quantity)
            .map_err(|error| Error::InvalidLedger { error })?;
        candidate.replace_economic_state(ledger);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Burn resolved claims and return their exact categorical payout.
    ///
    /// Redemption remains active during resolved retirement. The composing
    /// adapter must burn the matching claim and transfer Hoard collateral in
    /// the same atomic instruction as persistence of this transition.
    pub fn redeem_outcome(&mut self, outcome: usize, quantity: u64) -> Result<u64> {
        if !matches!(self.root.phase(), RootPhase::Resolved | RootPhase::Retiring) {
            return Err(Error::InvalidLedger {
                error: dclutch_kernel::Error::InvalidPhase,
            });
        }
        let mut candidate = *self;
        let mut ledger = candidate.to_kernel_ledger()?;
        let payout = ledger
            .redeem(outcome, quantity)
            .map_err(|error| Error::InvalidLedger { error })?;
        candidate.replace_economic_state(ledger);
        candidate.validate()?;
        *self = candidate;
        Ok(payout)
    }

    /// Register one direct physical child with generation and count guards.
    pub fn register_child(
        &mut self,
        expected_generation: u64,
        expected_prior_count: u64,
    ) -> Result<()> {
        let mut candidate = *self;
        candidate
            .root
            .register_child(expected_generation, expected_prior_count)
            .map_err(|error| Error::InvalidMarketRoot { error })?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Retire one direct physical child with generation and count guards.
    pub fn retire_child(
        &mut self,
        expected_generation: u64,
        expected_prior_count: u64,
    ) -> Result<()> {
        let mut candidate = *self;
        candidate
            .root
            .retire_child(expected_generation, expected_prior_count)
            .map_err(|error| Error::InvalidMarketRoot { error })?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Resolve one open Market with compact provider-neutral terminal truth.
    ///
    /// Provider-specific evidence is authenticated before this boundary and
    /// retained only by content identity. Provider children are retired via
    /// [`Self::retire_child`]; resolution does not assume which child supplied
    /// the accepted evidence.
    pub fn resolve_with_summary(
        &mut self,
        expected_generation: u64,
        settlement: CategoricalSettlementSummaryV1,
    ) -> Result<()> {
        let Some(resolution) = settlement.resolution() else {
            return Err(Error::PhaseSettlementMismatch);
        };
        settlement.validate::<N>()?;
        let mut candidate = *self;
        let mut ledger = candidate.to_kernel_ledger()?;
        ledger
            .resolve(usize::from(resolution.winner()))
            .map_err(|error| Error::InvalidLedger { error })?;
        candidate
            .root
            .transition_phase(expected_generation, RootPhase::Resolved)
            .map_err(|error| Error::InvalidMarketRoot { error })?;
        candidate.settlement = settlement;
        candidate.replace_economic_state(ledger);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Advance a non-resolution root edge and revalidate the composed Market.
    ///
    /// `Open -> Resolved` is intentionally refused because only
    /// [`Self::resolve_with_summary`] can atomically install terminal truth.
    pub fn transition_phase(&mut self, expected_generation: u64, next: RootPhase) -> Result<()> {
        if next == RootPhase::Resolved {
            return Err(Error::PhaseSettlementMismatch);
        }
        let mut candidate = *self;
        candidate
            .root
            .transition_phase(expected_generation, next)
            .map_err(|error| Error::InvalidMarketRoot { error })?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Return the compact Market root.
    pub const fn root(&self) -> MarketRoot {
        self.root
    }

    /// Return claimant-backing Hoard collateral atoms.
    pub const fn hoard_atoms(&self) -> u64 {
        self.hoard_atoms
    }

    /// Borrow the exact aggregate supply vector.
    pub const fn supply(&self) -> &[u64; N] {
        &self.supply
    }

    /// Return the compact optional categorical settlement summary.
    pub const fn settlement(&self) -> CategoricalSettlementSummaryV1 {
        self.settlement
    }

    fn replace_economic_state(&mut self, ledger: CategoricalLedger<N>) {
        let (hoard_atoms, supply, _) = ledger.into_parts();
        self.hoard_atoms = hoard_atoms;
        self.supply = supply;
    }
}

/// Decode the exact outcome count after validating the complete account header
/// and count-derived account width.
///
/// This helper supports bounded adapter dispatch from hostile bytes. It does
/// not validate the root, liabilities, or settlement body; callers must then
/// dispatch to [`CategoricalMarketV1::decode`].
pub fn decode_market_outcome_count(bytes: &[u8]) -> Result<u8> {
    if bytes.len() < MARKET_HEADER_BYTES {
        return Err(Error::InvalidLength);
    }
    if array::<8>(bytes, 0)? != MARKET_MAGIC {
        return Err(Error::InvalidMagic);
    }
    if u16::from_le_bytes(array(bytes, 8)?) != MARKET_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema);
    }
    if byte(bytes, MARKET_PROFILE_OFFSET)? != PROVISIONAL_CATEGORICAL_PROFILE_V1 {
        return Err(Error::UnsupportedProfile);
    }
    require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
    let outcome_count = byte(bytes, MARKET_OUTCOME_COUNT_OFFSET)?;
    if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&usize::from(outcome_count)) {
        return Err(Error::InvalidOutcomeCount);
    }
    let expected = usize::from(outcome_count)
        .checked_mul(8)
        .and_then(|supply_bytes| MARKET_BASE_BYTES.checked_add(supply_bytes))
        .ok_or(Error::ArithmeticOverflow)?;
    if bytes.len() != expected {
        return Err(Error::InvalidLength);
    }
    Ok(outcome_count)
}

fn validate_phase_and_economics<const N: usize>(
    phase: RootPhase,
    hoard_atoms: u64,
    supply: &[u64; N],
    settlement: CategoricalSettlementSummaryV1,
) -> Result<()> {
    let empty_economics = economic_state_is_empty(hoard_atoms, supply);
    match phase {
        RootPhase::Founding => {
            if !settlement.is_empty() {
                return Err(Error::PhaseSettlementMismatch);
            }
            if !empty_economics {
                return Err(Error::NonemptyEconomicState);
            }
        }
        RootPhase::Open => {
            if !settlement.is_empty() {
                return Err(Error::PhaseSettlementMismatch);
            }
        }
        RootPhase::Resolved => {
            if settlement.is_empty() {
                return Err(Error::PhaseSettlementMismatch);
            }
        }
        RootPhase::Retiring => {
            if settlement.is_empty() && !empty_economics {
                return Err(Error::NonemptyEconomicState);
            }
        }
        RootPhase::Retired => {
            if !empty_economics {
                return Err(Error::NonemptyEconomicState);
            }
        }
    }
    Ok(())
}

fn outcome_count<const N: usize>() -> Result<u8> {
    if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&N) {
        return Err(Error::InvalidOutcomeCount);
    }
    u8::try_from(N).map_err(|_| Error::InvalidOutcomeCount)
}

fn required_bytes<const N: usize>() -> Result<usize> {
    outcome_count::<N>()?;
    N.checked_mul(8)
        .and_then(|supply_bytes| MARKET_BASE_BYTES.checked_add(supply_bytes))
        .ok_or(Error::ArithmeticOverflow)
}

fn settlement_offset<const N: usize>() -> Result<usize> {
    outcome_count::<N>()?;
    N.checked_mul(8)
        .and_then(|supply_bytes| MARKET_SUPPLY_OFFSET.checked_add(supply_bytes))
        .ok_or(Error::ArithmeticOverflow)
}

fn supply_entry_offset(index: usize) -> Result<usize> {
    index
        .checked_mul(8)
        .and_then(|relative| MARKET_SUPPLY_OFFSET.checked_add(relative))
        .ok_or(Error::ArithmeticOverflow)
}

fn economic_state_is_empty<const N: usize>(hoard_atoms: u64, supply: &[u64; N]) -> bool {
    hoard_atoms == 0 && supply.iter().all(|amount| *amount == 0)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn copy_at(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(destination) = output.get_mut(offset..offset.saturating_add(value.len())) {
        destination.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::{ContentId as RootContentId, Error as RootError, MarketIdentity};
    use dclutch_kernel::Error as LedgerError;
    use dclutch_product_contract::{ContentId as ProductContentId, Error as ProductError};

    use super::*;

    const GENERATION: u64 = 7;

    fn root_id(fill: u8) -> RootContentId {
        RootContentId::new([fill; 32]).expect("nonzero root identity")
    }

    fn evidence(fill: u8) -> ProductContentId {
        ProductContentId::new([fill; 32]).expect("nonzero evidence identity")
    }

    fn founding_root() -> MarketRoot {
        MarketRoot::founding(
            MarketIdentity::new(
                root_id(1),
                root_id(2),
                root_id(3),
                root_id(4),
                root_id(5),
                GENERATION,
            ),
            [9; 32],
        )
        .expect("valid founding root")
    }

    fn root_in_phase(phase: RootPhase) -> MarketRoot {
        let mut root = founding_root();
        match phase {
            RootPhase::Founding => {}
            RootPhase::Open => root
                .transition_phase(GENERATION, RootPhase::Open)
                .expect("open root"),
            RootPhase::Resolved => {
                root.transition_phase(GENERATION, RootPhase::Open)
                    .expect("open root");
                root.transition_phase(GENERATION, RootPhase::Resolved)
                    .expect("resolved root");
            }
            RootPhase::Retiring => root
                .transition_phase(GENERATION, RootPhase::Retiring)
                .expect("retiring root"),
            RootPhase::Retired => {
                root.transition_phase(GENERATION, RootPhase::Retiring)
                    .expect("retiring root");
                root.transition_phase(GENERATION, RootPhase::Retired)
                    .expect("retired root");
            }
        }
        root
    }

    fn open_market<const N: usize>() -> CategoricalMarketV1<N> {
        let mut market = CategoricalMarketV1::new(
            founding_root(),
            0,
            [0; N],
            CategoricalSettlementSummaryV1::empty(),
        )
        .expect("valid founding market");
        market
            .transition_phase(GENERATION, RootPhase::Open)
            .expect("open market");
        market
    }

    fn resolved_summary<const N: usize>(winner: usize) -> CategoricalSettlementSummaryV1 {
        CategoricalSettlementSummaryV1::resolved::<N>(
            evidence(6),
            ResolutionKind::Occurrence,
            winner,
            11,
        )
        .expect("valid resolved summary")
    }

    #[test]
    fn layout_is_exact_compact_and_width_dispatch_is_hostile_safe() {
        assert_eq!(MARKET_ROOT_OFFSET, 16);
        assert_eq!(MARKET_HOARD_OFFSET, 248);
        assert_eq!(MARKET_SUPPLY_OFFSET, 256);
        assert_eq!(MARKET_BASE_BYTES, 320);
        assert_eq!(CategoricalMarketV1::<2>::encoded_len(), Ok(336));
        assert_eq!(CategoricalMarketV1::<16>::encoded_len(), Ok(448));
        assert_eq!(BINARY_MARKET_BYTES, 336);
        assert_eq!(MAX_MARKET_BYTES, 448);
        assert_eq!(
            CategoricalMarketV1::<1>::encoded_len(),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            CategoricalMarketV1::<17>::encoded_len(),
            Err(Error::InvalidOutcomeCount)
        );

        let market = open_market::<2>();
        let mut bytes = [0u8; BINARY_MARKET_BYTES];
        assert_eq!(market.encode(&mut bytes), Ok(()));
        assert_eq!(bytes.get(MARKET_OUTCOME_COUNT_OFFSET), Some(&2));
        assert_eq!(bytes.get(MARKET_PROFILE_OFFSET), Some(&1));
        assert_eq!(decode_market_outcome_count(&bytes), Ok(2));
        assert_eq!(CategoricalMarketV1::<2>::decode(&bytes), Ok(market));
        assert_eq!(
            CategoricalMarketV1::<3>::decode(&bytes),
            Err(Error::InvalidOutcomeCount)
        );
    }

    #[test]
    fn account_header_and_output_length_refuse_without_alias_or_mutation() {
        let market = open_market::<2>();
        let mut canonical = [0u8; BINARY_MARKET_BYTES];
        market.encode(&mut canonical).expect("encode market");

        let mut wrong_magic = canonical;
        *wrong_magic.get_mut(0).expect("magic byte") ^= 1;
        assert_eq!(
            CategoricalMarketV1::<2>::decode(&wrong_magic),
            Err(Error::InvalidMagic)
        );

        let mut wrong_schema = canonical;
        *wrong_schema.get_mut(8).expect("schema byte") = 2;
        assert_eq!(
            CategoricalMarketV1::<2>::decode(&wrong_schema),
            Err(Error::UnsupportedSchema)
        );

        let mut wrong_profile = canonical;
        *wrong_profile
            .get_mut(MARKET_PROFILE_OFFSET)
            .expect("profile byte") = 2;
        assert_eq!(
            CategoricalMarketV1::<2>::decode(&wrong_profile),
            Err(Error::UnsupportedProfile)
        );

        let mut reserved = canonical;
        *reserved.get_mut(12).expect("reserved byte") = 1;
        assert_eq!(
            CategoricalMarketV1::<2>::decode(&reserved),
            Err(Error::NonCanonicalReservedBytes)
        );

        let mut wrong_count = canonical;
        *wrong_count
            .get_mut(MARKET_OUTCOME_COUNT_OFFSET)
            .expect("outcome count") = 3;
        assert_eq!(
            CategoricalMarketV1::<2>::decode(&wrong_count),
            Err(Error::InvalidLength)
        );

        assert_eq!(
            CategoricalMarketV1::<2>::decode(&canonical[..335]),
            Err(Error::InvalidLength)
        );
        let mut wrong_output = [0xa5; 335];
        assert_eq!(market.encode(&mut wrong_output), Err(Error::OutputLength));
        assert!(wrong_output.iter().all(|byte| *byte == 0xa5));
    }

    #[test]
    fn settlement_summary_has_one_exact_empty_and_resolved_encoding() {
        let empty = CategoricalSettlementSummaryV1::empty();
        assert_eq!(empty.to_bytes::<3>(), Ok([0; SETTLEMENT_SUMMARY_BYTES]));
        assert_eq!(
            CategoricalSettlementSummaryV1::decode::<3>(&[0; SETTLEMENT_SUMMARY_BYTES]),
            Ok(empty)
        );

        let summary = CategoricalSettlementSummaryV1::resolved::<3>(
            evidence(4),
            ResolutionKind::Recovery,
            2,
            17,
        )
        .expect("valid summary");
        let bytes = summary.to_bytes::<3>().expect("encode summary");
        assert_eq!(bytes.get(SETTLEMENT_STATUS_OFFSET), Some(&1));
        assert_eq!(bytes.get(SETTLEMENT_ROUTE_OFFSET), Some(&2));
        assert_eq!(bytes.get(SETTLEMENT_WINNER_OFFSET), Some(&2));
        assert_eq!(
            bytes.get(SETTLEMENT_SEQUENCE_OFFSET..SETTLEMENT_SEQUENCE_OFFSET + 8),
            Some(17u64.to_le_bytes().as_slice())
        );
        assert_eq!(
            CategoricalSettlementSummaryV1::decode::<3>(&bytes),
            Ok(summary)
        );
        let resolution = summary.resolution().expect("resolved summary");
        assert_eq!(resolution.resolution_evidence_id(), evidence(4));
        assert_eq!(resolution.resolution_kind(), ResolutionKind::Recovery);
        assert_eq!(resolution.winner(), 2);
        assert_eq!(resolution.terminal_sequence(), 17);
    }

    #[test]
    fn hostile_settlement_tags_reserved_bytes_and_bounds_are_refused() {
        let mut noncanonical_empty = [0u8; SETTLEMENT_SUMMARY_BYTES];
        *noncanonical_empty.get_mut(8).expect("sequence byte") = 1;
        assert_eq!(
            CategoricalSettlementSummaryV1::decode::<2>(&noncanonical_empty),
            Err(Error::NonCanonicalReservedBytes)
        );

        let mut unknown_status = [0u8; SETTLEMENT_SUMMARY_BYTES];
        *unknown_status
            .get_mut(SETTLEMENT_STATUS_OFFSET)
            .expect("status") = 2;
        assert_eq!(
            CategoricalSettlementSummaryV1::decode::<2>(&unknown_status),
            Err(Error::UnknownSettlementStatus)
        );

        let canonical = resolved_summary::<2>(1)
            .to_bytes::<2>()
            .expect("encode summary");
        let mut unknown_route = canonical;
        *unknown_route
            .get_mut(SETTLEMENT_ROUTE_OFFSET)
            .expect("route") = 9;
        assert_eq!(
            CategoricalSettlementSummaryV1::decode::<2>(&unknown_route),
            Err(Error::InvalidProductContract {
                error: ProductError::UnknownResolutionKind,
            })
        );

        let mut invalid_winner = canonical;
        *invalid_winner
            .get_mut(SETTLEMENT_WINNER_OFFSET)
            .expect("winner") = 2;
        assert_eq!(
            CategoricalSettlementSummaryV1::decode::<2>(&invalid_winner),
            Err(Error::InvalidWinner)
        );

        let mut zero_sequence = canonical;
        zero_sequence
            .get_mut(SETTLEMENT_SEQUENCE_OFFSET..SETTLEMENT_SEQUENCE_OFFSET + 8)
            .expect("sequence")
            .fill(0);
        assert_eq!(
            CategoricalSettlementSummaryV1::decode::<2>(&zero_sequence),
            Err(Error::ZeroTerminalSequence)
        );

        let mut zero_evidence = canonical;
        zero_evidence
            .get_mut(SETTLEMENT_EVIDENCE_OFFSET..SETTLEMENT_EVIDENCE_OFFSET + 32)
            .expect("evidence")
            .fill(0);
        assert_eq!(
            CategoricalSettlementSummaryV1::decode::<2>(&zero_evidence),
            Err(Error::InvalidProductContract {
                error: ProductError::ZeroIdentifier,
            })
        );

        let mut reserved = canonical;
        *reserved
            .get_mut(SETTLEMENT_TAIL_RESERVED_OFFSET)
            .expect("reserved") = 1;
        assert_eq!(
            CategoricalSettlementSummaryV1::decode::<2>(&reserved),
            Err(Error::NonCanonicalReservedBytes)
        );
    }

    #[test]
    fn phase_summary_and_canceled_economic_rules_are_canonical() {
        assert_eq!(
            CategoricalMarketV1::<2>::new(
                founding_root(),
                1,
                [1, 1],
                CategoricalSettlementSummaryV1::empty(),
            ),
            Err(Error::NonemptyEconomicState)
        );
        assert_eq!(
            CategoricalMarketV1::<2>::new(
                root_in_phase(RootPhase::Open),
                1,
                [1, 1],
                resolved_summary::<2>(0),
            ),
            Err(Error::PhaseSettlementMismatch)
        );
        assert_eq!(
            CategoricalMarketV1::<2>::new(
                root_in_phase(RootPhase::Resolved),
                1,
                [1, 1],
                CategoricalSettlementSummaryV1::empty(),
            ),
            Err(Error::PhaseSettlementMismatch)
        );
        assert_eq!(
            CategoricalMarketV1::<2>::new(
                root_in_phase(RootPhase::Retiring),
                1,
                [1, 1],
                CategoricalSettlementSummaryV1::empty(),
            ),
            Err(Error::NonemptyEconomicState)
        );
        assert!(
            CategoricalMarketV1::<2>::new(
                root_in_phase(RootPhase::Retiring),
                1,
                [1, 1],
                resolved_summary::<2>(0),
            )
            .is_ok()
        );
        assert!(
            CategoricalMarketV1::<2>::new(
                root_in_phase(RootPhase::Retired),
                0,
                [0, 0],
                resolved_summary::<2>(0),
            )
            .is_ok()
        );
    }

    #[test]
    fn open_and_resolved_solvency_use_the_exact_native_claim_basis() {
        assert_eq!(
            CategoricalMarketV1::<3>::new(
                root_in_phase(RootPhase::Open),
                4,
                [5, 1, 3],
                CategoricalSettlementSummaryV1::empty(),
            ),
            Err(Error::InvalidLedger {
                error: LedgerError::Insolvent,
            })
        );
        assert_eq!(
            CategoricalMarketV1::<3>::new(
                root_in_phase(RootPhase::Resolved),
                4,
                [u64::MAX, 5, u64::MAX],
                resolved_summary::<3>(1),
            ),
            Err(Error::InvalidLedger {
                error: LedgerError::Insolvent,
            })
        );
        assert!(
            CategoricalMarketV1::<3>::new(
                root_in_phase(RootPhase::Resolved),
                5,
                [u64::MAX, 5, u64::MAX],
                resolved_summary::<3>(1),
            )
            .is_ok()
        );
    }

    #[test]
    fn split_merge_and_refusals_are_atomic() {
        let mut market = open_market::<3>();
        assert_eq!(market.split_complete_set(10), Ok(()));
        assert_eq!(market.hoard_atoms(), 10);
        assert_eq!(market.supply(), &[10, 10, 10]);
        assert_eq!(market.merge_complete_set(4), Ok(()));
        assert_eq!(market.hoard_atoms(), 6);
        assert_eq!(market.supply(), &[6, 6, 6]);

        let before = market;
        assert_eq!(
            market.merge_complete_set(7),
            Err(Error::InvalidLedger {
                error: LedgerError::InsufficientSupply,
            })
        );
        assert_eq!(market, before);
        assert_eq!(
            market.split_complete_set(0),
            Err(Error::InvalidLedger {
                error: LedgerError::ZeroQuantity,
            })
        );
        assert_eq!(market, before);

        let mut saturated = CategoricalMarketV1::<2>::new(
            root_in_phase(RootPhase::Open),
            u64::MAX,
            [u64::MAX; 2],
            CategoricalSettlementSummaryV1::empty(),
        )
        .expect("solvent saturated market");
        let saturated_before = saturated;
        assert_eq!(
            saturated.split_complete_set(1),
            Err(Error::InvalidLedger {
                error: LedgerError::ArithmeticOverflow,
            })
        );
        assert_eq!(saturated, saturated_before);
    }

    #[test]
    fn resolution_redemption_and_terminal_replay_are_exact_and_atomic() {
        let mut market = open_market::<3>();
        market.split_complete_set(10).expect("split set");
        let summary = resolved_summary::<3>(1);
        assert_eq!(market.resolve_with_summary(GENERATION, summary), Ok(()));
        assert_eq!(market.root().phase(), RootPhase::Resolved);
        assert_eq!(market.settlement(), summary);

        let before_replay = market;
        assert_eq!(
            market.resolve_with_summary(GENERATION, summary),
            Err(Error::InvalidLedger {
                error: LedgerError::InvalidPhase,
            })
        );
        assert_eq!(market, before_replay);

        assert_eq!(market.redeem_outcome(0, 10), Ok(0));
        assert_eq!(market.hoard_atoms(), 10);
        assert_eq!(market.redeem_outcome(1, 4), Ok(4));
        assert_eq!(market.hoard_atoms(), 6);
        let before_invalid = market;
        assert_eq!(
            market.redeem_outcome(3, 1),
            Err(Error::InvalidLedger {
                error: LedgerError::InvalidOutcome,
            })
        );
        assert_eq!(market, before_invalid);
        market
            .transition_phase(GENERATION, RootPhase::Retiring)
            .expect("begin resolved retirement");
        assert_eq!(market.redeem_outcome(1, 6), Ok(6));
    }

    #[test]
    fn child_and_lifecycle_guards_refuse_without_partial_mutation() {
        let mut market = open_market::<2>();
        let initial = market;
        assert_eq!(
            market.register_child(GENERATION + 1, 0),
            Err(Error::InvalidMarketRoot {
                error: RootError::GenerationMismatch,
            })
        );
        assert_eq!(market, initial);
        assert_eq!(market.register_child(GENERATION, 0), Ok(()));
        let registered = market;
        assert_eq!(
            market.retire_child(GENERATION, 0),
            Err(Error::InvalidMarketRoot {
                error: RootError::ChildCountMismatch,
            })
        );
        assert_eq!(market, registered);
        assert_eq!(market.retire_child(GENERATION, 1), Ok(()));

        market.split_complete_set(1).expect("split set");
        let before_cancel = market;
        assert_eq!(
            market.transition_phase(GENERATION, RootPhase::Retiring),
            Err(Error::NonemptyEconomicState)
        );
        assert_eq!(market, before_cancel);
        assert_eq!(
            market.transition_phase(GENERATION, RootPhase::Resolved),
            Err(Error::PhaseSettlementMismatch)
        );
        assert_eq!(market, before_cancel);
    }
}
