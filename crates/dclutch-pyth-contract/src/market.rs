//! One-account categorical Market state composed from existing contracts.
//!
//! The embedded [`CategoricalPythPolicyRecordV1`] and [`PythFeedProfileV1`] are
//! shape- and semantics-validated here. This SDK-free crate deliberately
//! performs no hashing. A composing SBF adapter **must** hash the canonical
//! policy bytes and compare that content identity with
//! [`MarketIdentity::resolution_policy_id`](dclutch_core_contract::MarketIdentity::resolution_policy_id).
//! It must separately hash the exact canonical feed-profile bytes and compare
//! that content identity with [`CategoricalPythPolicyRecordV1::feed_profile_id`].
//! Neither [`MarketStateV1::new`] nor [`MarketStateV1::decode`] claims that
//! either required identity comparison has occurred.

use dclutch_core_contract::{MARKET_ROOT_BYTES, MarketRoot, Phase as RootPhase};
use dclutch_kernel::{CategoricalLedger, Phase as LedgerPhase};

use crate::{
    Error, Result, array,
    feed_profile::{FEED_PROFILE_BYTES, PythFeedProfileV1},
    policy::{CategoricalPythPolicyRecordV1, POLICY_BYTES},
    receipt::{RECEIPT_BYTES, ReceiptKind, ResolutionReceiptV1},
    zero,
};

/// Market account magic.
pub const MARKET_MAGIC: [u8; 8] = *b"DCLTMKT1";
/// Implemented composed Market schema.
pub const MARKET_SCHEMA_VERSION: u16 = 1;
/// Fixed Market bytes excluding the `N` eight-byte supply entries.
pub const MARKET_BASE_BYTES: usize = 810;
/// Exact width of a two-outcome Market account.
pub const BINARY_MARKET_BYTES: usize = 826;
/// Exact width of a sixteen-outcome Market account.
pub const MAX_MARKET_BYTES: usize = 938;

const ROOT_OFFSET: usize = 16;
const POLICY_OFFSET: usize = 184;
const FEED_PROFILE_OFFSET: usize = 568;
const HOARD_OFFSET: usize = 674;
const SUPPLY_OFFSET: usize = 682;

/// Private-field V1 composition of root, policy, liabilities, and receipt.
///
/// The only persistent lifecycle phase is the phase inside `root`. The
/// transient kernel ledger phase is reconstructed solely from `receipt` every
/// time validation or composition access needs it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketStateV1<const N: usize> {
    root: MarketRoot,
    policy: CategoricalPythPolicyRecordV1,
    feed_profile: PythFeedProfileV1,
    hoard_atoms: u64,
    supply: [u64; N],
    receipt: ResolutionReceiptV1,
}

impl<const N: usize> MarketStateV1<N> {
    /// Construct and cross-validate one composed Market account.
    ///
    /// This validates only canonical shape, lifecycle, policy, receipt, and
    /// ledger solvency. The composing SBF adapter remains obligated to hash
    /// both canonical records and compare them with their respective content
    /// identities in the root and policy.
    pub fn new(
        root: MarketRoot,
        policy: CategoricalPythPolicyRecordV1,
        feed_profile: PythFeedProfileV1,
        hoard_atoms: u64,
        supply: [u64; N],
        receipt: ResolutionReceiptV1,
    ) -> Result<Self> {
        let state = Self {
            root,
            policy,
            feed_profile,
            hoard_atoms,
            supply,
            receipt,
        };
        state.validate()?;
        Ok(state)
    }

    /// Return the checked exact account width for this outcome count.
    pub fn encoded_len() -> Result<usize> {
        required_bytes::<N>()
    }

    /// Decode one exact account and validate every owned cross-field rule.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let expected = required_bytes::<N>()?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != MARKET_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array::<2>(bytes, 8)?) != MARKET_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        let outcome_count = outcome_count::<N>()?;
        if bytes.get(10).copied().ok_or(Error::InvalidLength)? != outcome_count {
            return Err(Error::InvalidOutcomeCount);
        }
        if !zero(bytes.get(11..16).ok_or(Error::InvalidLength)?) {
            return Err(Error::NonCanonicalReservedBytes);
        }

        let root_end = ROOT_OFFSET
            .checked_add(MARKET_ROOT_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let policy_end = POLICY_OFFSET
            .checked_add(POLICY_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let feed_profile_end = FEED_PROFILE_OFFSET
            .checked_add(FEED_PROFILE_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let receipt_offset = receipt_offset::<N>()?;
        let receipt_end = receipt_offset
            .checked_add(RECEIPT_BYTES)
            .ok_or(Error::ArithmeticOverflow)?;
        let root = MarketRoot::decode(
            bytes
                .get(ROOT_OFFSET..root_end)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|error| Error::InvalidMarketRoot { error })?;
        let policy = CategoricalPythPolicyRecordV1::decode(
            bytes
                .get(POLICY_OFFSET..policy_end)
                .ok_or(Error::InvalidLength)?,
        )?;
        let feed_profile = PythFeedProfileV1::decode(
            bytes
                .get(FEED_PROFILE_OFFSET..feed_profile_end)
                .ok_or(Error::InvalidLength)?,
        )?;
        let hoard_atoms = u64::from_le_bytes(array(bytes, HOARD_OFFSET)?);
        let mut supply = [0u64; N];
        let mut index = 0usize;
        while index < N {
            let offset = supply_entry_offset(index)?;
            let destination = supply.get_mut(index).ok_or(Error::ArithmeticOverflow)?;
            *destination = u64::from_le_bytes(array(bytes, offset)?);
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let receipt = ResolutionReceiptV1::decode(
            bytes
                .get(receipt_offset..receipt_end)
                .ok_or(Error::InvalidLength)?,
            outcome_count,
        )?;
        Self::new(root, policy, feed_profile, hoard_atoms, supply, receipt)
    }

    /// Encode into the exact caller-owned account buffer without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        let expected = required_bytes::<N>()?;
        if output.len() != expected {
            return Err(Error::OutputLength);
        }
        self.validate()?;
        let outcome_count = outcome_count::<N>()?;

        let root = self.root.to_bytes();
        let policy = self.policy.to_bytes();
        let feed_profile = self.feed_profile.to_bytes();
        let receipt = self.receipt.to_bytes();
        let receipt_offset = receipt_offset::<N>()?;

        output.fill(0);
        put(output, 0, &MARKET_MAGIC);
        put(output, 8, &MARKET_SCHEMA_VERSION.to_le_bytes());
        put(output, 10, &[outcome_count]);
        put(output, ROOT_OFFSET, &root);
        put(output, POLICY_OFFSET, &policy);
        put(output, FEED_PROFILE_OFFSET, &feed_profile);
        put(output, HOARD_OFFSET, &self.hoard_atoms.to_le_bytes());
        for (index, amount) in self.supply.iter().enumerate() {
            put(output, SUPPLY_OFFSET + index * 8, &amount.to_le_bytes());
        }
        put(output, receipt_offset, &receipt);
        Ok(())
    }

    /// Validate the composed account without asserting either content hash.
    pub fn validate(&self) -> Result<()> {
        let outcome_count = outcome_count::<N>()?;
        self.root
            .validate()
            .map_err(|error| Error::InvalidMarketRoot { error })?;
        self.policy.to_kernel_policy()?;
        self.feed_profile.validate()?;
        if usize::from(self.policy.price_cell_count())
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?
            != N
        {
            return Err(Error::PolicyOutcomeCountMismatch);
        }
        ResolutionReceiptV1::decode(&self.receipt.to_bytes(), outcome_count)?;
        validate_phase_receipt(self.root.phase(), self.receipt.kind())?;
        validate_receipt_policy(&self.receipt, &self.policy)?;
        if matches!(self.root.phase(), RootPhase::Founding | RootPhase::Retired)
            && !economic_state_is_empty(self.hoard_atoms, &self.supply)
        {
            return Err(Error::NonemptyEconomicState);
        }
        self.to_kernel_ledger()?;
        Ok(())
    }

    /// Reconstruct the validated kernel ledger from economic state and receipt.
    pub fn to_kernel_ledger(&self) -> Result<CategoricalLedger<N>> {
        let phase = match self.receipt.kind() {
            ReceiptKind::Empty => LedgerPhase::Open,
            ReceiptKind::Price | ReceiptKind::Failure => LedgerPhase::Resolved {
                winner: usize::from(self.receipt.winner()),
            },
        };
        CategoricalLedger::from_parts(self.hoard_atoms, self.supply, phase)
            .map_err(|error| Error::InvalidLedger { error })
    }

    /// Return the embedded Market root.
    pub const fn root(&self) -> MarketRoot {
        self.root
    }

    /// Borrow the embedded validated categorical Pyth policy record.
    pub const fn policy(&self) -> &CategoricalPythPolicyRecordV1 {
        &self.policy
    }

    /// Borrow the inline canonical Pyth feed-semantics profile.
    pub const fn feed_profile(&self) -> &PythFeedProfileV1 {
        &self.feed_profile
    }

    /// Return claimant-backing collateral atoms.
    pub const fn hoard_atoms(&self) -> u64 {
        self.hoard_atoms
    }

    /// Return the exact conservative outcome-supply vector.
    pub const fn supply(&self) -> &[u64; N] {
        &self.supply
    }

    /// Borrow the inline canonical resolution receipt.
    pub const fn receipt(&self) -> &ResolutionReceiptV1 {
        &self.receipt
    }
}

fn outcome_count<const N: usize>() -> Result<u8> {
    if !(2..=16).contains(&N) {
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

fn receipt_offset<const N: usize>() -> Result<usize> {
    N.checked_mul(8)
        .and_then(|supply_bytes| SUPPLY_OFFSET.checked_add(supply_bytes))
        .ok_or(Error::ArithmeticOverflow)
}

fn supply_entry_offset(index: usize) -> Result<usize> {
    index
        .checked_mul(8)
        .and_then(|relative| SUPPLY_OFFSET.checked_add(relative))
        .ok_or(Error::ArithmeticOverflow)
}

fn validate_phase_receipt(phase: RootPhase, receipt: ReceiptKind) -> Result<()> {
    let canonical = match phase {
        RootPhase::Founding | RootPhase::Open => matches!(receipt, ReceiptKind::Empty),
        RootPhase::Resolved => matches!(receipt, ReceiptKind::Price | ReceiptKind::Failure),
        RootPhase::Retiring => true,
        RootPhase::Retired => true,
    };
    if !canonical {
        return Err(Error::PhaseReceiptMismatch);
    }
    Ok(())
}

fn validate_receipt_policy(
    receipt: &ResolutionReceiptV1,
    policy: &CategoricalPythPolicyRecordV1,
) -> Result<()> {
    let canonical = match receipt.kind() {
        ReceiptKind::Empty => true,
        ReceiptKind::Price => u16::from(receipt.winner()) < policy.price_cell_count(),
        ReceiptKind::Failure => u16::from(receipt.winner()) == policy.failure_outcome_index(),
    };
    if !canonical {
        return Err(Error::ReceiptPolicyWinnerMismatch);
    }
    Ok(())
}

fn economic_state_is_empty<const N: usize>(hoard_atoms: u64, supply: &[u64; N]) -> bool {
    hoard_atoms == 0 && supply.iter().all(|amount| *amount == 0)
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::{CapabilitySet, ContentId, MarketIdentity};
    use dclutch_kernel::{Error as LedgerError, MAX_OUTCOMES};

    use crate::{
        feed_profile::FEED_PROFILE_MAGIC,
        policy::POLICY_MAGIC,
        receipt::{Clock, PriceInput},
    };

    use super::*;
    use dclutch_kernel::resolution::categorical_pyth_v1::{
        CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
    };

    fn content(value: u8) -> Result<ContentId> {
        ContentId::new([value; 32]).map_err(|error| Error::InvalidMarketRoot { error })
    }

    fn root(phase: RootPhase) -> Result<MarketRoot> {
        let identity = MarketIdentity::new(
            content(1)?,
            content(2)?,
            content(3)?,
            content(4)?,
            7,
            CapabilitySet::NONE,
        );
        let mut root = MarketRoot::founding(identity);
        match phase {
            RootPhase::Founding => {}
            RootPhase::Open => root
                .transition_phase(7, RootPhase::Open)
                .map_err(|error| Error::InvalidMarketRoot { error })?,
            RootPhase::Resolved => {
                root.transition_phase(7, RootPhase::Open)
                    .map_err(|error| Error::InvalidMarketRoot { error })?;
                root.transition_phase(7, RootPhase::Resolved)
                    .map_err(|error| Error::InvalidMarketRoot { error })?;
            }
            RootPhase::Retiring => root
                .transition_phase(7, RootPhase::Retiring)
                .map_err(|error| Error::InvalidMarketRoot { error })?,
            RootPhase::Retired => {
                root.transition_phase(7, RootPhase::Retiring)
                    .map_err(|error| Error::InvalidMarketRoot { error })?;
                root.transition_phase(7, RootPhase::Retired)
                    .map_err(|error| Error::InvalidMarketRoot { error })?;
            }
        }
        Ok(root)
    }

    fn policy<const N: usize>() -> Result<CategoricalPythPolicyRecordV1> {
        let cell_count = N.checked_sub(1).ok_or(Error::InvalidOutcomeCount)?;
        let price_cell_count = u16::try_from(cell_count).map_err(|_| Error::InvalidOutcomeCount)?;
        let mut upper_edges = [0u128; MAX_PRICE_CELLS];
        let active_edges = cell_count
            .checked_sub(1)
            .ok_or(Error::InvalidOutcomeCount)?;
        for (index, edge) in upper_edges.iter_mut().take(active_edges).enumerate() {
            *edge = u128::try_from(index)
                .map_err(|_| Error::ArithmeticOverflow)?
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        CategoricalPythPolicyRecordV1::new(CategoricalPythV1PolicyInput {
            pyth_release_id: [1; 32],
            feed_profile_id: [2; 32],
            target_time: 100,
            grace: 5,
            window: 10,
            max_crossing_lag: 5,
            max_age: 20,
            max_future_skew: 5,
            confidence_multiplier: 1,
            max_confidence_bps: 10_000,
            max_normalized_confidence_atoms: 100,
            normalized_decimals: 8,
            price_cell_count,
            upper_edges,
            failure_outcome_index: price_cell_count,
        })
    }

    fn feed_profile() -> Result<PythFeedProfileV1> {
        PythFeedProfileV1::new([4; 32], [5; 32], [6; 32])
    }

    fn empty<const N: usize>() -> Result<ResolutionReceiptV1> {
        ResolutionReceiptV1::empty(u8::try_from(N).map_err(|_| Error::InvalidOutcomeCount)?)
    }

    fn price_receipt<const N: usize>(winner: u8) -> Result<ResolutionReceiptV1> {
        ResolutionReceiptV1::price(
            PriceInput {
                winner,
                posted_slot: 9,
                consumed_slot: 9,
                consumed_unix_timestamp: 105,
                previous_publish_time: 99,
                publish_time: 100,
                price: 1,
                confidence: 0,
                exponent: 0,
                post_params_body_digest: [3; 32],
            },
            u8::try_from(N).map_err(|_| Error::InvalidOutcomeCount)?,
        )
    }

    fn round_trip<const N: usize>(state: MarketStateV1<N>) -> Result<()> {
        let mut bytes = [0u8; MAX_MARKET_BYTES];
        let exact = bytes
            .get_mut(..MarketStateV1::<N>::encoded_len()?)
            .ok_or(Error::OutputLength)?;
        state.encode(exact)?;
        assert_eq!(MarketStateV1::<N>::decode(exact), Ok(state));
        Ok(())
    }

    #[test]
    fn exact_binary_and_maximum_widths_and_offsets_are_canonical() -> Result<()> {
        assert_eq!(MarketStateV1::<2>::encoded_len(), Ok(BINARY_MARKET_BYTES));
        assert_eq!(
            MarketStateV1::<MAX_OUTCOMES>::encoded_len(),
            Ok(MAX_MARKET_BYTES)
        );

        let binary = MarketStateV1::new(
            root(RootPhase::Open)?,
            policy::<2>()?,
            feed_profile()?,
            9,
            [9, 8],
            empty::<2>()?,
        )?;
        let mut binary_bytes = [0u8; BINARY_MARKET_BYTES];
        binary.encode(&mut binary_bytes)?;
        assert_eq!(binary_bytes.get(0..8), Some(&MARKET_MAGIC[..]));
        assert_eq!(binary_bytes.get(8..10), Some(&1u16.to_le_bytes()[..]));
        assert_eq!(binary_bytes.get(10), Some(&2));
        assert_eq!(binary_bytes.get(11..16), Some(&[0; 5][..]));
        assert_eq!(binary_bytes.get(16..24), Some(&b"DCLTROOT"[..]));
        assert_eq!(binary_bytes.get(184..192), Some(&POLICY_MAGIC[..]));
        assert_eq!(binary_bytes.get(568..576), Some(&FEED_PROFILE_MAGIC[..]));
        assert_eq!(binary_bytes.get(674..682), Some(&9u64.to_le_bytes()[..]));
        assert_eq!(binary_bytes.get(682..690), Some(&9u64.to_le_bytes()[..]));
        assert_eq!(binary_bytes.get(690..698), Some(&8u64.to_le_bytes()[..]));
        assert_eq!(binary_bytes.get(698..706), Some(&b"DCLTRCP1"[..]));
        round_trip(binary)?;

        let maximum = MarketStateV1::new(
            root(RootPhase::Open)?,
            policy::<MAX_OUTCOMES>()?,
            feed_profile()?,
            16,
            [16; MAX_OUTCOMES],
            empty::<MAX_OUTCOMES>()?,
        )?;
        let mut maximum_bytes = [0u8; MAX_MARKET_BYTES];
        maximum.encode(&mut maximum_bytes)?;
        assert_eq!(maximum_bytes.get(682..690), Some(&16u64.to_le_bytes()[..]));
        assert_eq!(maximum_bytes.get(802..810), Some(&16u64.to_le_bytes()[..]));
        assert_eq!(maximum_bytes.get(810..818), Some(&b"DCLTRCP1"[..]));
        round_trip(maximum)?;
        Ok(())
    }

    #[test]
    fn every_material_lifecycle_phase_round_trips() -> Result<()> {
        round_trip(MarketStateV1::new(
            root(RootPhase::Founding)?,
            policy::<2>()?,
            feed_profile()?,
            0,
            [0, 0],
            empty::<2>()?,
        )?)?;
        round_trip(MarketStateV1::new(
            root(RootPhase::Open)?,
            policy::<2>()?,
            feed_profile()?,
            7,
            [7, 6],
            empty::<2>()?,
        )?)?;
        round_trip(MarketStateV1::new(
            root(RootPhase::Resolved)?,
            policy::<2>()?,
            feed_profile()?,
            4,
            [4, 99],
            price_receipt::<2>(0)?,
        )?)?;
        round_trip(MarketStateV1::new(
            root(RootPhase::Resolved)?,
            policy::<2>()?,
            feed_profile()?,
            4,
            [99, 4],
            ResolutionReceiptV1::failure(
                1,
                2,
                Clock {
                    slot: 9,
                    unix_timestamp: 116,
                },
            )?,
        )?)?;
        round_trip(MarketStateV1::new(
            root(RootPhase::Retiring)?,
            policy::<2>()?,
            feed_profile()?,
            3,
            [3, 2],
            empty::<2>()?,
        )?)?;
        round_trip(MarketStateV1::new(
            root(RootPhase::Retiring)?,
            policy::<2>()?,
            feed_profile()?,
            2,
            [88, 2],
            ResolutionReceiptV1::failure(
                1,
                2,
                Clock {
                    slot: 9,
                    unix_timestamp: 116,
                },
            )?,
        )?)?;
        round_trip(MarketStateV1::new(
            root(RootPhase::Retired)?,
            policy::<2>()?,
            feed_profile()?,
            0,
            [0, 0],
            empty::<2>()?,
        )?)?;
        let terminal_retired = MarketStateV1::new(
            root(RootPhase::Retired)?,
            policy::<2>()?,
            feed_profile()?,
            0,
            [0, 0],
            ResolutionReceiptV1::failure(
                1,
                2,
                Clock {
                    slot: 9,
                    unix_timestamp: 116,
                },
            )?,
        )?;
        let mut terminal_bytes = [0u8; BINARY_MARKET_BYTES];
        terminal_retired.encode(&mut terminal_bytes)?;
        let retained = MarketStateV1::<2>::decode(&terminal_bytes)?;
        assert_eq!(retained.feed_profile(), terminal_retired.feed_profile());
        assert_eq!(retained.receipt(), terminal_retired.receipt());
        round_trip(terminal_retired)?;
        Ok(())
    }

    #[test]
    fn phase_receipt_policy_and_economic_mismatches_refuse() -> Result<()> {
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Founding)?,
                policy::<2>()?,
                feed_profile()?,
                0,
                [0, 0],
                price_receipt::<2>(0)?,
            ),
            Err(Error::PhaseReceiptMismatch)
        );
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Open)?,
                policy::<2>()?,
                feed_profile()?,
                0,
                [0, 0],
                price_receipt::<2>(0)?,
            ),
            Err(Error::PhaseReceiptMismatch)
        );
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Resolved)?,
                policy::<2>()?,
                feed_profile()?,
                0,
                [0, 0],
                empty::<2>()?,
            ),
            Err(Error::PhaseReceiptMismatch)
        );
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Open)?,
                policy::<3>()?,
                feed_profile()?,
                0,
                [0, 0],
                empty::<2>()?,
            ),
            Err(Error::PolicyOutcomeCountMismatch)
        );
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Resolved)?,
                policy::<2>()?,
                feed_profile()?,
                0,
                [0, 0],
                price_receipt::<2>(1)?,
            ),
            Err(Error::ReceiptPolicyWinnerMismatch)
        );
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Resolved)?,
                policy::<2>()?,
                feed_profile()?,
                0,
                [0, 0],
                ResolutionReceiptV1::failure(
                    0,
                    2,
                    Clock {
                        slot: 9,
                        unix_timestamp: 116,
                    },
                )?,
            ),
            Err(Error::ReceiptPolicyWinnerMismatch)
        );
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Founding)?,
                policy::<2>()?,
                feed_profile()?,
                1,
                [0, 0],
                empty::<2>()?,
            ),
            Err(Error::NonemptyEconomicState)
        );
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Founding)?,
                policy::<2>()?,
                feed_profile()?,
                0,
                [1, 0],
                empty::<2>()?,
            ),
            Err(Error::NonemptyEconomicState)
        );
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Retired)?,
                policy::<2>()?,
                feed_profile()?,
                1,
                [0, 0],
                empty::<2>()?,
            ),
            Err(Error::NonemptyEconomicState)
        );
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Retired)?,
                policy::<2>()?,
                feed_profile()?,
                0,
                [0, 1],
                empty::<2>()?,
            ),
            Err(Error::NonemptyEconomicState)
        );
        Ok(())
    }

    #[test]
    fn open_and_resolved_insolvency_are_kernel_refusals() -> Result<()> {
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Open)?,
                policy::<2>()?,
                feed_profile()?,
                4,
                [5, 4],
                empty::<2>()?,
            ),
            Err(Error::InvalidLedger {
                error: LedgerError::Insolvent
            })
        );
        assert_eq!(
            MarketStateV1::new(
                root(RootPhase::Resolved)?,
                policy::<2>()?,
                feed_profile()?,
                4,
                [5, 99],
                price_receipt::<2>(0)?,
            ),
            Err(Error::InvalidLedger {
                error: LedgerError::Insolvent
            })
        );
        Ok(())
    }

    #[test]
    fn hostile_market_headers_lengths_and_embedded_records_refuse() -> Result<()> {
        let state = MarketStateV1::new(
            root(RootPhase::Open)?,
            policy::<2>()?,
            feed_profile()?,
            1,
            [1, 1],
            empty::<2>()?,
        )?;
        let mut bytes = [0u8; BINARY_MARKET_BYTES];
        state.encode(&mut bytes)?;
        for length in 0..BINARY_MARKET_BYTES {
            let short = bytes.get(..length).ok_or(Error::InvalidLength)?;
            assert_eq!(MarketStateV1::<2>::decode(short), Err(Error::InvalidLength));
        }
        assert_eq!(
            MarketStateV1::<2>::decode(&[0; BINARY_MARKET_BYTES + 1]),
            Err(Error::InvalidLength)
        );
        let mut bad_magic = bytes;
        *bad_magic.get_mut(0).ok_or(Error::InvalidLength)? ^= 0xff;
        assert_eq!(
            MarketStateV1::<2>::decode(&bad_magic),
            Err(Error::InvalidMagic)
        );
        let mut bad_schema = bytes;
        put(&mut bad_schema, 8, &2u16.to_le_bytes());
        assert_eq!(
            MarketStateV1::<2>::decode(&bad_schema),
            Err(Error::UnsupportedSchema)
        );
        let mut wrong_count = bytes;
        *wrong_count.get_mut(10).ok_or(Error::InvalidLength)? = 3;
        assert_eq!(
            MarketStateV1::<2>::decode(&wrong_count),
            Err(Error::InvalidOutcomeCount)
        );
        let mut reserved = bytes;
        *reserved.get_mut(11).ok_or(Error::InvalidLength)? = 1;
        assert_eq!(
            MarketStateV1::<2>::decode(&reserved),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut bad_root = bytes;
        *bad_root.get_mut(ROOT_OFFSET).ok_or(Error::InvalidLength)? = 0;
        assert!(matches!(
            MarketStateV1::<2>::decode(&bad_root),
            Err(Error::InvalidMarketRoot { .. })
        ));
        let mut bad_policy = bytes;
        *bad_policy
            .get_mut(POLICY_OFFSET)
            .ok_or(Error::InvalidLength)? = 0;
        assert_eq!(
            MarketStateV1::<2>::decode(&bad_policy),
            Err(Error::InvalidMagic)
        );
        let mut bad_feed_profile = bytes;
        *bad_feed_profile
            .get_mut(FEED_PROFILE_OFFSET)
            .ok_or(Error::InvalidLength)? = 0;
        assert_eq!(
            MarketStateV1::<2>::decode(&bad_feed_profile),
            Err(Error::InvalidMagic)
        );
        let before = [0x5a; BINARY_MARKET_BYTES - 1];
        let mut wrong = before;
        assert_eq!(state.encode(&mut wrong), Err(Error::OutputLength));
        assert_eq!(wrong, before);
        assert_eq!(
            MarketStateV1::<1>::decode(&[0; MARKET_BASE_BYTES + 8]),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            MarketStateV1::<17>::decode(&[0; MARKET_BASE_BYTES + 136]),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            MarketStateV1::<1>::encoded_len(),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            MarketStateV1::<17>::encoded_len(),
            Err(Error::InvalidOutcomeCount)
        );
        Ok(())
    }
}
