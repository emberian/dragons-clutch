//! The flagship conditional market's shape, as three founding inputs.
//!
//! **"If feature `X` activates by slot `S`, does mainnet's slot time move?"** —
//! decision 0029's tenth item and
//! `docs/design/MECHANISM_CONDITIONAL_MARKETS_2026_09_04.md` §8. WHICH feature,
//! WHICH slot and WHICH metric are ember's to choose; this module is the shape
//! that choice fills in, so the answer is three flags and a calendar rather
//! than a new market design.
//!
//! # What each input decides, and what it does not
//!
//! * `feature_gate` — the mainnet Feature account the decision parent watches.
//!   It selects the parent's pinned account set (relay row 2's two positions);
//!   it changes no width and no cut.
//! * `activation_slot` `S` — the decision parent's ONE cut, at `S + 1`. Row 2
//!   yields `activated_at` when the feature is activated and a sentinel above
//!   every slot when it is not, so a cut at `S + 1` separates "activated by
//!   `S`" (cell 0) from "not activated by `S`, including never" (cell 1), and
//!   the parent is two ordinary cells wide.
//! * `metric` — the cuts that carve parent `B`. Their COUNT is the only thing
//!   the child reads.
//!
//! That last sentence is why the metric is a flag rather than a fork. **A child
//! is blind to what its parents observe**: [`ParentFactsV1`] carries a Market,
//! a generation, a Product-record digest, an ordinary count and a deadline, and
//! nothing about a venue, a feed or an observable. So a slot-time parent B
//! through the relay's native row 3 and a Pyth-priced parent B are the same
//! child at the same width, and choosing between them moves no code here.
//!
//! # What this module does not build
//!
//! The two parents are ordinary markets of existing families and are founded by
//! their own compilers; this states the SHAPE their widths must have and hands
//! the child's compilation input to [`crate::child_v1`]. It reads no chain and
//! signs nothing.

use dclutch_source::relay::FLAGSHIP_SLOT_TIME_CUTS_MILLIS_V1;
use dclutch_source::relay::decode::RelayedObservableV1;

use crate::child_v1::{
    ChildCompilationInputV1, ChildIdentitiesV1, ChildPortfolioV1, ChildQuestionV1, ParentFactsV1,
};
use crate::{Error, Result};

/// The decision parent's ordinary width: activated by `S`, or not.
///
/// Two, and it is not a parameter. Row 2 observes one number and the question
/// asks one threshold of it, so a third ordinary cell would be a cell no
/// observation can select.
pub const FLAGSHIP_DECISION_ORDINARY_COUNT_V1: u32 = 2;

/// The decision parent's cell for "activated by `S`".
pub const FLAGSHIP_DECISION_ACTIVATED_CELL_V1: u32 = 0;

/// Which metric parent `B` observes, and the cuts that carve it.
///
/// The variants differ in which market a founder points at, never in the
/// child's shape: both yield an ordinary count of `cuts + 1`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlagshipMetricV1 {
    /// Mainnet's own mean slot duration since the epoch began, in
    /// milliseconds, through the relay's native row 3
    /// (`RelayedObservableV1::MeanSlotTimeSinceEpochStartV1`, raw exponent
    /// −3). The cuts are milliseconds, at that same exponent.
    MeanSlotTimeMillis {
        /// Strictly increasing millisecond thresholds.
        cuts_millis: Vec<u32>,
    },
    /// A published price, through the Pyth family, cut at atoms in the feed's
    /// own exponent. The child cannot tell this apart from the row above and
    /// does not need to.
    PriceAtoms {
        /// The feed the parent's Source names.
        feed_id: [u8; 32],
        /// Strictly increasing thresholds, in the feed's own atoms.
        cuts_atoms: Vec<i128>,
    },
}

impl FlagshipMetricV1 {
    /// The metric parent's own ordinary width: one cell per interval the cuts
    /// carve, which is one more than the number of cuts.
    pub fn ordinary_count(&self) -> Result<u32> {
        let cuts = match self {
            Self::MeanSlotTimeMillis { cuts_millis } => cuts_millis.len(),
            Self::PriceAtoms { cuts_atoms, .. } => cuts_atoms.len(),
        };
        if cuts == 0 {
            // A metric with no cut is one ordinary cell: a parent that always
            // resolves the same way, which is a question the child would
            // inherit as a foregone conclusion.
            return Err(Error::DegenerateOutcomePartition);
        }
        u32::try_from(cuts)
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or(Error::WidthMismatch)
    }

    /// The cuts as the ordered thresholds a `ResultDomainV2` carries, refusing
    /// a non-increasing list by name.
    pub fn cuts(&self) -> Result<Vec<i128>> {
        let cuts: Vec<i128> = match self {
            Self::MeanSlotTimeMillis { cuts_millis } => {
                cuts_millis.iter().map(|value| i128::from(*value)).collect()
            }
            Self::PriceAtoms { cuts_atoms, .. } => cuts_atoms.clone(),
        };
        for window in cuts.windows(2) {
            let (low, high) = (window.first(), window.get(1));
            match (low, high) {
                (Some(low), Some(high)) if low < high => {}
                _ => return Err(Error::InvalidRecord),
            }
        }
        Ok(cuts)
    }

    /// The relay row this metric is observed through, when it is observed
    /// through the relay at all.
    pub const fn relayed_observable(&self) -> Option<RelayedObservableV1> {
        match self {
            Self::MeanSlotTimeMillis { .. } => {
                Some(RelayedObservableV1::MeanSlotTimeSinceEpochStartV1)
            }
            Self::PriceAtoms { .. } => None,
        }
    }
}

/// The metric the design note's own worked example carries: mainnet slot time
/// cut at the two millisecond thresholds the decoding rules emit.
///
/// The constant is the Lean's (`flagshipSlotTimeCutsMillis`), read here rather
/// than retyped, and this function is its only consumer — it was emitted and
/// unread until the shape existed to spend it.
pub fn flagship_slot_time_metric_v1() -> FlagshipMetricV1 {
    FlagshipMetricV1::MeanSlotTimeMillis {
        cuts_millis: FLAGSHIP_SLOT_TIME_CUTS_MILLIS_V1.to_vec(),
    }
}

/// The three choices, plus the calendar and the identities every market needs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlagshipInputV1 {
    /// WHICH FEATURE: the mainnet Feature account the decision parent watches.
    pub feature_gate: [u8; 32],
    /// WHICH SLOT: `S`, in "activated by slot `S`".
    pub activation_slot: u64,
    /// WHICH METRIC: what parent `B` observes and how it is carved.
    pub metric: FlagshipMetricV1,
    /// The decision parent, as its own market already exists.
    pub decision_parent: ParentFactsV1,
    /// The metric parent, as its own market already exists.
    pub metric_parent: ParentFactsV1,
    /// The child's deadline, Unix seconds.
    pub settle_by: u64,
    /// The margin past each parent's deadline, and the child's window
    /// `max_age`.
    pub margin_seconds: u32,
    /// Quantity of each branch cell in the founder's opening bundle.
    pub bundle_quantity: u64,
    /// The child's identities.
    pub identities: ChildIdentitiesV1,
}

/// The decision parent's single cut, in the atoms row 2 publishes.
///
/// `S + 1`, so an `activated_at` of exactly `S` is inside cell 0. An
/// `activation_slot` of `u64::MAX` has no cut above it — and that value is
/// itself row 2's not-activated sentinel, so the question would be "did it
/// activate by the slot that means it never did".
pub fn flagship_decision_cut_v1(activation_slot: u64) -> Result<i128> {
    let cut = activation_slot
        .checked_add(1)
        .ok_or(Error::DegenerateOutcomePartition)?;
    Ok(i128::from(cut))
}

/// The flagship child: the metric, conditioned on the feature having
/// activated.
///
/// A conditional and not a product, and the asymmetry is the design's: what is
/// bought is "the slot time, GIVEN activation", so the off-condition cell pays
/// the scale without reading `B` at all
/// (`ConditionalMarketV1.off_condition_ignores_B`) and the market does not
/// spend width on the joint cells nobody asked about.
///
/// Every width in the result is a function of the two parents' own widths,
/// which Core proves again against their result domains at founding
/// (`programs/dclutch-core-sbf/src/parents_v1.rs`).
pub fn flagship_child_input_v1(input: &FlagshipInputV1) -> Result<ChildCompilationInputV1> {
    if input.decision_parent.ordinary_count != FLAGSHIP_DECISION_ORDINARY_COUNT_V1 {
        // The decision parent is the one parent this shape constrains: its
        // width is the question's, not the founder's.
        return Err(Error::WidthMismatch);
    }
    if input.metric_parent.ordinary_count != input.metric.ordinary_count()? {
        return Err(Error::WidthMismatch);
    }
    flagship_decision_cut_v1(input.activation_slot)?;
    if input.feature_gate == [0_u8; 32] {
        return Err(Error::InvalidRecord);
    }
    Ok(ChildCompilationInputV1 {
        question: ChildQuestionV1::ConditionalOn {
            condition: FLAGSHIP_DECISION_ACTIVATED_CELL_V1,
        },
        parent_a: input.decision_parent,
        parent_b: input.metric_parent,
        settle_by: input.settle_by,
        margin_seconds: input.margin_seconds,
        portfolio: ChildPortfolioV1::BranchBundle {
            quantity: input.bundle_quantity,
        },
        identities: input.identities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_product::ContentId;

    fn id(tag: u8) -> ContentId {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        ContentId::new(bytes).expect("nonzero identity")
    }

    fn identities() -> ChildIdentitiesV1 {
        ChildIdentitiesV1 {
            product_id: id(1),
            coordinate_domain_id: id(2),
            result_unit_id: id(3),
            claim_basis_id: id(4),
            liability_basis_id: id(5),
            representation_release_id: id(6),
            mapping_release_id: id(7),
            evaluator_release_id: id(8),
            portfolio_denominator: 1,
        }
    }

    fn parent(tag: u8, ordinary_count: u32) -> ParentFactsV1 {
        let mut market = [0_u8; 32];
        market[0] = tag;
        let mut digest = [0_u8; 32];
        digest[0] = tag;
        digest[1] = 1;
        ParentFactsV1 {
            market,
            generation: 1,
            product_record_digest: digest,
            ordinary_count,
            deadline_unix_seconds: 1_800_000_000,
        }
    }

    fn flagship(metric: FlagshipMetricV1) -> Result<FlagshipInputV1> {
        let width = metric.ordinary_count()?;
        Ok(FlagshipInputV1 {
            feature_gate: [0x9f; 32],
            activation_slot: 400_000_000,
            metric,
            decision_parent: parent(0xA, FLAGSHIP_DECISION_ORDINARY_COUNT_V1),
            metric_parent: parent(0xB, width),
            settle_by: 1_800_600_000,
            margin_seconds: 3_600,
            bundle_quantity: 2,
            identities: identities(),
        })
    }

    /// The tenth item's three choices, and the shape that does not move when
    /// they do: two different metrics, the same child at the same width, and
    /// the decision parent's cut sitting one above the chosen slot.
    #[test]
    fn the_metric_is_a_flag_and_the_child_does_not_notice_which_one() {
        let slot_time = flagship(flagship_slot_time_metric_v1()).expect("slot-time flagship");
        let price = flagship(FlagshipMetricV1::PriceAtoms {
            feed_id: [0x7c; 32],
            cuts_atoms: vec![15_000, 25_000],
        })
        .expect("price flagship");

        assert_eq!(slot_time.metric.ordinary_count(), Ok(3));
        assert_eq!(price.metric.ordinary_count(), Ok(3));
        assert_eq!(
            slot_time.metric.relayed_observable(),
            Some(RelayedObservableV1::MeanSlotTimeSinceEpochStartV1)
        );
        assert_eq!(price.metric.relayed_observable(), None);

        let one = flagship_child_input_v1(&slot_time).expect("slot-time child");
        let other = flagship_child_input_v1(&price).expect("price child");
        assert_eq!(one.question, other.question);
        assert_eq!(one.parent_a.ordinary_count, other.parent_a.ordinary_count);
        assert_eq!(one.parent_b.ordinary_count, other.parent_b.ordinary_count);
        assert_eq!(
            one.question,
            ChildQuestionV1::ConditionalOn {
                condition: FLAGSHIP_DECISION_ACTIVATED_CELL_V1
            }
        );
    }

    /// The cuts are the Lean's, not this module's: `[390, 410]` at exponent
    /// −3, and the constant was emitted and read by nothing until now.
    #[test]
    fn the_worked_metric_carries_the_emitted_millisecond_cuts() {
        let metric = flagship_slot_time_metric_v1();
        assert_eq!(metric.cuts(), Ok(vec![390_i128, 410]));
        assert_eq!(
            metric,
            FlagshipMetricV1::MeanSlotTimeMillis {
                cuts_millis: FLAGSHIP_SLOT_TIME_CUTS_MILLIS_V1.to_vec()
            }
        );
    }

    /// `S + 1`: an `activated_at` of exactly `S` is "activated by `S`".
    #[test]
    fn the_decision_cut_sits_one_slot_above_the_chosen_slot() {
        assert_eq!(flagship_decision_cut_v1(0), Ok(1));
        assert_eq!(flagship_decision_cut_v1(400_000_000), Ok(400_000_001));
        assert_eq!(
            flagship_decision_cut_v1(u64::MAX),
            Err(Error::DegenerateOutcomePartition)
        );
    }

    /// Each malformed choice refuses on its own name rather than on one
    /// shared "bad input".
    #[test]
    fn each_malformed_choice_refuses_by_its_own_name() {
        let uncut = FlagshipMetricV1::MeanSlotTimeMillis {
            cuts_millis: vec![],
        };
        assert_eq!(
            uncut.ordinary_count(),
            Err(Error::DegenerateOutcomePartition)
        );

        let unordered = FlagshipMetricV1::MeanSlotTimeMillis {
            cuts_millis: vec![410, 390],
        };
        assert_eq!(unordered.cuts(), Err(Error::InvalidRecord));

        let mut wrong_decision = flagship(flagship_slot_time_metric_v1()).expect("flagship");
        wrong_decision.decision_parent = parent(0xA, 3);
        assert_eq!(
            flagship_child_input_v1(&wrong_decision),
            Err(Error::WidthMismatch)
        );

        let mut wrong_metric = flagship(flagship_slot_time_metric_v1()).expect("flagship");
        wrong_metric.metric_parent = parent(0xB, 4);
        assert_eq!(
            flagship_child_input_v1(&wrong_metric),
            Err(Error::WidthMismatch)
        );

        let mut nameless = flagship(flagship_slot_time_metric_v1()).expect("flagship");
        nameless.feature_gate = [0_u8; 32];
        assert_eq!(
            flagship_child_input_v1(&nameless),
            Err(Error::InvalidRecord)
        );
    }
}
