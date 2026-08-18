//! `clutch-batch`: the scalar lab (surface S3) and the coupled relation (S7).

use clutch_batch::relation_v1 as v1;
use clutch_batch::{
    Candidate, DustPolicy, Error, FixedBook, FrozenPolicy, Order, PartialPolicy, PriceGrid, Side,
    TieRule, MAX_GRID_TICKS, MAX_ORDERS,
};

use super::*;
use crate::json::Value;
use crate::taxonomy::{Observed, Refusal};

/// S3's variant to taxonomy-code map, per §2.4.
pub fn scalar_code(error: Error) -> Refusal {
    let (code, variant) = match error {
        Error::InvalidGrid => (2049, "InvalidGrid"),
        Error::InvalidTick => (2050, "InvalidTick"),
        Error::TooManyOrders => (8001, "TooManyOrders"),
        Error::InvalidQuantity => (2051, "InvalidQuantity"),
        Error::InvalidMinimumFill => (2052, "InvalidMinimumFill"),
        Error::NonCanonicalOrderOrder => (2053, "NonCanonicalOrderOrder"),
        Error::NonCanonicalPadding => (2022, "NonCanonicalPadding"),
        Error::NoGridTick => (2070, "NoGridTick"),
        Error::ArithmeticOverflow => (1001, "ArithmeticOverflow"),
        Error::CandidateMismatch => (5009, "CandidateMismatch"),
        Error::ConservationFailure => (5004, "ConservationFailure"),
        Error::FillExceedsQuantity => (5005, "FillExceedsQuantity"),
        Error::IneligibleFill => (5006, "IneligibleFill"),
        Error::MinimumFillViolation => (5007, "MinimumFillViolation"),
        Error::AllOrNoneViolation => (5008, "AllOrNoneViolation"),
        Error::DustRejected => (5010, "DustRejected"),
    };
    Refusal::new(code, "batch", variant)
}

/// S7's variant to taxonomy-code map.  `VECTOR_SPINE_PROPOSAL.md` §2.4 does not
/// map this surface at all; the codes come from the extension block of
/// `fixtures/vectors/TAXONOMY.json`.
pub fn relation_code(error: v1::ErrorV1) -> Refusal {
    let (code, variant) = match error {
        v1::ErrorV1::UnknownRelationVersion => (2031, "UnknownRelationVersion"),
        v1::ErrorV1::InvalidPriceScale => (2057, "InvalidPriceScale"),
        v1::ErrorV1::PolicyVariantUnimplemented => (9004, "PolicyVariantUnimplemented"),
        v1::ErrorV1::InvalidOwner => (2056, "InvalidOwner"),
        v1::ErrorV1::InvalidOutcome => (2062, "InvalidOutcome"),
        v1::ErrorV1::InvalidQuantity => (2051, "InvalidQuantity"),
        v1::ErrorV1::InvalidMinimumFill => (2052, "InvalidMinimumFill"),
        v1::ErrorV1::NonCanonicalOrderOrder => (2053, "NonCanonicalOrderOrder"),
        v1::ErrorV1::NonCanonicalPadding => (2022, "NonCanonicalPadding"),
        v1::ErrorV1::AonNotAdmitted => (9005, "AonNotAdmitted"),
        v1::ErrorV1::MinimumFillNotAdmitted => (9006, "MinimumFillNotAdmitted"),
        v1::ErrorV1::SelfCrossRefused => (4018, "SelfCrossRefused"),
        v1::ErrorV1::ExpiredOrder => (3011, "ExpiredOrder"),
        v1::ErrorV1::TooManyOrders => (8001, "TooManyOrders"),
        v1::ErrorV1::TooManyPortfolios => (8005, "TooManyPortfolios"),
        v1::ErrorV1::SimplexSumMismatch => (2058, "SimplexSumMismatch"),
        v1::ErrorV1::PriceOutOfRange => (2059, "PriceOutOfRange"),
        v1::ErrorV1::IneligibleFill => (5006, "IneligibleFill"),
        v1::ErrorV1::CandidateMismatch => (5009, "CandidateMismatch"),
        v1::ErrorV1::StrictUnderfill => (5023, "StrictUnderfill"),
        v1::ErrorV1::FillExceedsQuantity => (5005, "FillExceedsQuantity"),
        v1::ErrorV1::MinimumFillViolation => (5007, "MinimumFillViolation"),
        v1::ErrorV1::AllOrNoneViolation => (5008, "AllOrNoneViolation"),
        v1::ErrorV1::AonMaskDishonored => (5024, "AonMaskDishonored"),
        v1::ErrorV1::AonMaskLeak => (5025, "AonMaskLeak"),
        v1::ErrorV1::AonMaskNotApplicable => (2066, "AonMaskNotApplicable"),
        v1::ErrorV1::DustRejected => (5010, "DustRejected"),
        v1::ErrorV1::OutcomeConservationMismatch => (5026, "OutcomeConservationMismatch"),
        v1::ErrorV1::ChurnNotCanonical => (5027, "ChurnNotCanonical"),
        v1::ErrorV1::InfeasibleVirtualLeg => (5028, "InfeasibleVirtualLeg"),
        v1::ErrorV1::PairingInfeasible { .. } => (5029, "PairingInfeasible"),
        v1::ErrorV1::SliceNotExecutable => (5030, "SliceNotExecutable"),
        v1::ErrorV1::SliceSumMismatch => (5031, "SliceSumMismatch"),
        v1::ErrorV1::PairingWitnessNotAdmitted => (2067, "PairingWitnessNotAdmitted"),
        v1::ErrorV1::PairingWitnessMissing => (2068, "PairingWitnessMissing"),
        v1::ErrorV1::ConstructorStalled => (5032, "ConstructorStalled"),
        v1::ErrorV1::SliceCapacityExceeded => (8006, "SliceCapacityExceeded"),
        v1::ErrorV1::ConsiderationMismatch => (5015, "ConsiderationMismatch"),
        v1::ErrorV1::RemainderRequired => (1004, "RemainderRequired"),
        v1::ErrorV1::FeeMismatch => (5033, "FeeMismatch"),
        v1::ErrorV1::FeePayerUnfunded => (5034, "FeePayerUnfunded"),
        v1::ErrorV1::ConservationFailure => (5004, "ConservationFailure"),
        v1::ErrorV1::ScoreMismatch => (2071, "ScoreMismatch"),
        v1::ErrorV1::DigestMismatch => (2072, "DigestMismatch"),
        v1::ErrorV1::ArithmeticOverflow => (1001, "ArithmeticOverflow"),
        v1::ErrorV1::NoValidCandidate => (9007, "NoValidCandidate"),
        v1::ErrorV1::SearchBudgetExceeded => (8007, "SearchBudgetExceeded"),
    };
    Refusal::new(code, "batch", variant)
}

fn read_side(value: &Value) -> Result<Side, String> {
    match value.as_str()? {
        "buy" => Ok(Side::Buy),
        "sell" => Ok(Side::Sell),
        other => Err(format!("ENUM-1: unknown side {other:?}")),
    }
}

fn read_partial(value: &Value) -> Result<PartialPolicy, String> {
    match value.as_str()? {
        "allow" => Ok(PartialPolicy::Allow),
        "all-or-none" => Ok(PartialPolicy::AllOrNone),
        other => Err(format!("ENUM-1: unknown partial policy {other:?}")),
    }
}

fn read_dust(value: &Value) -> Result<DustPolicy, String> {
    match value.as_str()? {
        "assign-canonical" => Ok(DustPolicy::AssignCanonical),
        "reject" => Ok(DustPolicy::Reject),
        other => Err(format!("ENUM-1: unknown dust policy {other:?}")),
    }
}

// ------------------------------------------------------- the scalar lab -----

pub struct ScalarExecutor {
    book: FixedBook,
}

impl ScalarExecutor {
    pub fn open(constructed_by: &str, value: &Value) -> Result<Self, String> {
        let policy_value = field(value, "policy")?;
        let grid_value = field(policy_value, "grid")?;
        let len = small_field(grid_value, "len")? as u8;
        let mut ticks = [0u64; MAX_GRID_TICKS];
        let items = field(grid_value, "ticks")?.as_array()?;
        if items.len() != usize::from(len) {
            return Err("ARR-1: grid.ticks must be exactly `len` entries".into());
        }
        for (index, item) in items.iter().enumerate() {
            ticks[index] = item.as_u64()?;
        }
        let grid = PriceGrid { ticks, len };
        match str_field(policy_value, "tie_rule")? {
            "max-volume-min-imbalance-highest-tick" => {}
            other => return Err(format!("ENUM-1: unknown tie rule {other:?}")),
        }
        let policy = FrozenPolicy {
            grid,
            tie_rule: TieRule::MaxVolumeMinImbalanceHighestTick,
            dust_policy: read_dust(field(policy_value, "dust_policy")?)?,
            remainder_seed: u64_field(policy_value, "remainder_seed")?,
        };

        let book_len = small_field(value, "len")? as u8;
        let order_items = field(value, "orders")?.as_array()?;
        if order_items.len() != usize::from(book_len) {
            return Err("ARR-1: orders must be exactly `len` entries".into());
        }
        let mut orders = [Order {
            canonical_order_id: 0,
            side: Side::Buy,
            limit_tick: 0,
            quantity: 0,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
        }; MAX_ORDERS];
        for (index, item) in order_items.iter().enumerate() {
            orders[index] = Order {
                canonical_order_id: u64_field(item, "canonical_order_id")?,
                side: read_side(field(item, "side")?)?,
                limit_tick: small_field(item, "limit_tick")? as u8,
                quantity: u64_field(item, "quantity")?,
                minimum_fill: u64_field(item, "minimum_fill")?,
                partial_policy: read_partial(field(item, "partial_policy")?)?,
            };
        }
        let book = FixedBook {
            policy,
            orders,
            len: book_len,
        };
        if constructed_by == "constructor" {
            FixedBook::new(policy, orders, book_len)
                .map_err(|error| format!("initial_state is not constructible: {error:?}"))?;
        }
        Ok(Self { book })
    }

    fn read_candidate(&self, value: &Value) -> Result<Candidate, String> {
        let len = small_field(value, "len")? as u8;
        let items = field(value, "fills")?.as_array()?;
        if items.len() != usize::from(len) {
            return Err("ARR-1: fills must be exactly `len` entries".into());
        }
        let mut fills = [0u64; MAX_ORDERS];
        for (index, item) in items.iter().enumerate() {
            fills[index] = item.as_u64()?;
        }
        Ok(Candidate {
            clearing_tick: small_field(value, "clearing_tick")? as u8,
            fills,
            len,
            matched: u64_field(value, "matched")?,
        })
    }

    fn render_candidate(candidate: &Candidate) -> Value {
        obj(vec![
            ("clearing_tick", small(u64::from(candidate.clearing_tick))),
            ("len", small(u64::from(candidate.len))),
            ("matched", dec(u128::from(candidate.matched))),
            (
                "fills",
                prefix(&candidate.fills, usize::from(candidate.len)),
            ),
        ])
    }
}

impl Executor for ScalarExecutor {
    fn apply(&mut self, op: &str, args: &Value) -> Result<Observed, String> {
        let result = match op {
            "propose" => self.book.propose(),
            "verify" => {
                let candidate = self.read_candidate(field(args, "candidate")?)?;
                self.book.verify(&candidate)
            }
            "validate" => {
                return Ok(match self.book.validate() {
                    Ok(()) => Observed::Ok(Value::Null),
                    Err(error) => Observed::Error(scalar_code(error)),
                })
            }
            other => return Err(format!("clutch-batch has no scalar operation {other:?}")),
        };
        Ok(match result {
            Ok(candidate) => Observed::Ok(Self::render_candidate(&candidate)),
            Err(error) => Observed::Error(scalar_code(error)),
        })
    }

    fn render_state(&self) -> Value {
        // The scalar book is immutable: `propose` and `verify` take `&self`.
        obj(vec![("len", small(u64::from(self.book.len)))])
    }
}

// -------------------------------------------------- the coupled relation ----

pub struct RelationExecutor {
    domain: v1::RelationDomainV1,
    book: v1::BookV1,
}

fn read_relation_policy(value: &Value) -> Result<v1::FrozenPolicyV1, String> {
    let allocation = match str_field(value, "allocation")? {
        "price-priority-marginal-pro-rata" => v1::AllocationPolicyV1::PricePriorityMarginalProRata,
        "full-pro-rata" => v1::AllocationPolicyV1::FullProRata,
        other => return Err(format!("ENUM-1: unknown allocation {other:?}")),
    };
    let self_cross = match str_field(value, "self_cross")? {
        "refuse-overlap" => v1::SelfCrossPolicyV1::RefuseOverlap,
        "net-at-admission" => v1::SelfCrossPolicyV1::NetAtAdmission,
        "allow-gate-at-pairing" => v1::SelfCrossPolicyV1::AllowGateAtPairing,
        other => return Err(format!("ENUM-1: unknown self-cross policy {other:?}")),
    };
    let aon = match str_field(value, "aon")? {
        "refuse-admission" => v1::AonPolicyV1::RefuseAdmission,
        "witnessed-honored-mask" => v1::AonPolicyV1::WitnessedHonoredMask,
        "full-size-counting" => v1::AonPolicyV1::FullSizeCounting,
        other => return Err(format!("ENUM-1: unknown AON policy {other:?}")),
    };
    let rounding = match str_field(value, "rounding")? {
        "none" => v1::RoundingBoundaryV1::None,
        "terminal-owner-floor" => v1::RoundingBoundaryV1::TerminalOwnerFloor,
        "receipt-floor" => v1::RoundingBoundaryV1::ReceiptFloor,
        other => return Err(format!("ENUM-1: unknown rounding boundary {other:?}")),
    };
    let residual_settlement = match str_field(value, "residual_settlement")? {
        "full-pair-only" => v1::ResidualSettlementV1::FullPairOnly,
        "cumulative-pair-canonical" => v1::ResidualSettlementV1::CumulativePairCanonical,
        "cumulative-pair-free" => v1::ResidualSettlementV1::CumulativePairFree,
        "unique-slice-receipts" => v1::ResidualSettlementV1::UniqueSliceReceipts,
        other => return Err(format!("ENUM-1: unknown residual settlement {other:?}")),
    };
    let transfer_phase = match str_field(value, "transfer_phase")? {
        "active-only" => v1::TransferPhaseV1::ActiveOnly,
        "active-or-resolved" => v1::TransferPhaseV1::ActiveOrResolved,
        other => return Err(format!("ENUM-1: unknown transfer phase {other:?}")),
    };
    let portfolio_lots = match str_field(value, "portfolio_lots")? {
        "strict-whole-order" => v1::PortfolioLotPolicyV1::StrictWholeOrder,
        "marginal-pro-rata-lots" => v1::PortfolioLotPolicyV1::MarginalProRataLots,
        other => return Err(format!("ENUM-1: unknown portfolio lot policy {other:?}")),
    };
    let pairing_witness = match str_field(value, "pairing_witness")? {
        "recomputed-constructor" => v1::PairingWitnessPolicyV1::RecomputedConstructor,
        "explicit-slices" => v1::PairingWitnessPolicyV1::ExplicitSlices,
        other => return Err(format!("ENUM-1: unknown pairing witness policy {other:?}")),
    };
    let score = match str_field(value, "score")? {
        "lexicographic-dispersion-v1" => v1::ScorePolicyV1::LexicographicDispersionV1,
        other => return Err(format!("ENUM-1: unknown score policy {other:?}")),
    };
    let fee_value = field(value, "fee_base")?;
    let fee_base = match str_field(fee_value, "kind")? {
        "none" => v1::FeeBaseV1::None,
        "flat-notional" => v1::FeeBaseV1::FlatNotional {
            bps: u32::try_from(small_field(fee_value, "bps")?)
                .map_err(|_| "bps out of range".to_string())?,
        },
        other => return Err(format!("ENUM-1: unknown fee base {other:?}")),
    };
    Ok(v1::FrozenPolicyV1 {
        allocation,
        self_cross,
        aon,
        rounding,
        residual_settlement,
        transfer_phase,
        portfolio_lots,
        pairing_witness,
        dust: read_dust(field(value, "dust")?)?,
        score,
        fee_base,
    })
}

fn read_order(value: &Value, outcomes: usize) -> Result<v1::OrderV1, String> {
    match str_field(value, "kind")? {
        "single-egg" => Ok(v1::OrderV1::SingleEgg(v1::SingleEggOrderV1 {
            canonical_order_id: u64_field(value, "canonical_order_id")?,
            owner: u16::try_from(small_field(value, "owner")?)
                .map_err(|_| "owner out of range".to_string())?,
            outcome: small_field(value, "outcome")? as u8,
            side: read_side(field(value, "side")?)?,
            quantity: u64_field(value, "quantity")?,
            limit_price: u64_field(value, "limit_price")?,
            minimum_fill: u64_field(value, "minimum_fill")?,
            partial_policy: read_partial(field(value, "partial_policy")?)?,
            expiry_epoch: u64_field(value, "expiry_epoch")?,
        })),
        "portfolio" => {
            let active_len = small_field(value, "active_len")? as u8;
            let coefficients: [u64; v1::MAX_OUTCOMES] =
                read_prefix(field(value, "coefficients")?, usize::from(active_len))?;
            let _ = outcomes;
            Ok(v1::OrderV1::Portfolio(v1::PortfolioOrderV1 {
                canonical_order_id: u64_field(value, "canonical_order_id")?,
                owner: u16::try_from(small_field(value, "owner")?)
                    .map_err(|_| "owner out of range".to_string())?,
                side: read_side(field(value, "side")?)?,
                coefficients,
                active_len,
                lots: u64_field(value, "lots")?,
                limit_collateral_per_lot: u64_field(value, "limit_collateral_per_lot")?,
                minimum_fill_lots: u64_field(value, "minimum_fill_lots")?,
                partial_policy: read_partial(field(value, "partial_policy")?)?,
                expiry_epoch: u64_field(value, "expiry_epoch")?,
            }))
        }
        other => Err(format!("ENUM-1: unknown order kind {other:?}")),
    }
}

/// A signed exact integer: INT-1 forbids a sign character, so the sign is a
/// closed enum beside an unsigned magnitude.
fn signed(value: i128) -> Value {
    obj(vec![
        (
            "sign",
            Value::Str(if value < 0 {
                "-".to_string()
            } else {
                "+".to_string()
            }),
        ),
        ("magnitude", dec(value.unsigned_abs())),
    ])
}

fn read_signed(value: &Value) -> Result<i128, String> {
    let magnitude = u128_field(value, "magnitude")?;
    let magnitude = i128::try_from(magnitude).map_err(|_| "magnitude exceeds i128".to_string())?;
    match str_field(value, "sign")? {
        "+" => Ok(magnitude),
        "-" => Ok(-magnitude),
        other => Err(format!("ENUM-1: unknown sign {other:?}")),
    }
}

fn render_score(score: &v1::ScoreV1) -> Value {
    obj(vec![
        // INT-1 admits no sign, so a signed component ships as an explicit
        // sign/magnitude pair rather than as a signed decimal string.
        (
            "weighted_direct_volume",
            signed(score.weighted_direct_volume),
        ),
        (
            "limit_surplus_price_units",
            dec(score.limit_surplus_price_units),
        ),
        ("distinct_owners", small(u64::from(score.distinct_owners))),
        ("churn", dec(u128::from(score.churn))),
        ("digest", dec(score.digest)),
    ])
}

impl RelationExecutor {
    pub fn open(constructed_by: &str, value: &Value) -> Result<Self, String> {
        let domain_value = field(value, "domain")?;
        let outcome_count = small_field(domain_value, "outcome_count")? as u8;
        let domain = v1::RelationDomainV1 {
            relation_version: u32::try_from(small_field(domain_value, "relation_version")?)
                .map_err(|_| "relation_version out of range".to_string())?,
            market_id: u64_field(domain_value, "market_id")?,
            book_id: u64_field(domain_value, "book_id")?,
            epoch: u64_field(domain_value, "epoch")?,
            policy_id: u64_field(domain_value, "policy_id")?,
            order_set_id: u64_field(domain_value, "order_set_id")?,
            outcome_count,
            owner_count: u16::try_from(small_field(domain_value, "owner_count")?)
                .map_err(|_| "owner_count out of range".to_string())?,
            price_scale: u64_field(domain_value, "price_scale")?,
            remainder_seed: u64_field(domain_value, "remainder_seed")?,
            policy: read_relation_policy(field(domain_value, "policy")?)?,
        };
        let book_value = field(value, "book")?;
        let len = small_field(book_value, "len")? as u8;
        let items = field(book_value, "orders")?.as_array()?;
        if items.len() != usize::from(len) {
            return Err("ARR-1: book.orders must be exactly `len` entries".into());
        }
        let mut book = v1::BookV1::empty();
        for (index, item) in items.iter().enumerate() {
            book.orders[index] = read_order(item, usize::from(outcome_count))?;
        }
        book.len = len;
        if constructed_by == "constructor" {
            book.validate(&domain)
                .map_err(|error| format!("initial_state is not admissible: {error:?}"))?;
        }
        Ok(Self { domain, book })
    }

    fn read_prices(&self, value: &Value) -> Result<[u64; v1::MAX_OUTCOMES], String> {
        read_prefix(value, usize::from(self.domain.outcome_count))
    }

    fn read_candidate(&self, value: &Value) -> Result<v1::CandidateV1, String> {
        let order_len = small_field(value, "order_len")? as u8;
        let items = field(value, "fills")?.as_array()?;
        if items.len() != usize::from(order_len) {
            return Err("ARR-1: candidate.fills must be exactly `order_len` entries".into());
        }
        let mut fills = [0u64; clutch_batch::MAX_ORDERS];
        for (index, item) in items.iter().enumerate() {
            fills[index] = item.as_u64()?;
        }
        let score_value = field(value, "claimed_score")?;
        Ok(v1::CandidateV1 {
            order_len,
            prices: self.read_prices(field(value, "prices")?)?,
            virtual_split: u64_field(value, "virtual_split")?,
            virtual_merge: u64_field(value, "virtual_merge")?,
            fills,
            honored_aon_mask: u64_field(value, "honored_aon_mask")?,
            claimed_score: v1::ScoreV1 {
                weighted_direct_volume: read_signed(field(score_value, "weighted_direct_volume")?)?,
                limit_surplus_price_units: u128_field(score_value, "limit_surplus_price_units")?,
                distinct_owners: u16::try_from(small_field(score_value, "distinct_owners")?)
                    .map_err(|_| "distinct_owners out of range".to_string())?,
                churn: u64_field(score_value, "churn")?,
                digest: u128_field(score_value, "digest")?,
            },
            canonical_candidate_digest: u128_field(value, "canonical_candidate_digest")?,
        })
    }

    fn render_candidate(candidate: &v1::CandidateV1, outcomes: usize) -> Value {
        obj(vec![
            ("order_len", small(u64::from(candidate.order_len))),
            ("prices", prefix(&candidate.prices, outcomes)),
            ("virtual_split", dec(u128::from(candidate.virtual_split))),
            ("virtual_merge", dec(u128::from(candidate.virtual_merge))),
            (
                "fills",
                prefix(&candidate.fills, usize::from(candidate.order_len)),
            ),
            (
                "honored_aon_mask",
                dec(u128::from(candidate.honored_aon_mask)),
            ),
            ("claimed_score", render_score(&candidate.claimed_score)),
            (
                "canonical_candidate_digest",
                dec(candidate.canonical_candidate_digest),
            ),
        ])
    }

    fn render_summary(summary: &v1::SummaryV1) -> Value {
        let outcomes = usize::from(summary.outcome_count);
        obj(vec![
            ("outcome_count", small(u64::from(summary.outcome_count))),
            ("buy_flow", prefix(&summary.buy_flow, outcomes)),
            ("sell_flow", prefix(&summary.sell_flow, outcomes)),
            ("total_flow", prefix(&summary.total_flow, outcomes)),
            ("direct_flow", prefix(&summary.direct_flow, outcomes)),
            ("virtual_split", dec(u128::from(summary.virtual_split))),
            ("virtual_merge", dec(u128::from(summary.virtual_merge))),
            (
                "buyer_consideration_price_units",
                dec(summary.buyer_consideration_price_units),
            ),
            (
                "seller_credit_price_units",
                dec(summary.seller_credit_price_units),
            ),
            ("fee_price_units", dec(summary.fee_price_units)),
            ("debit_atoms", dec(summary.debit_atoms)),
            ("credit_atoms", dec(summary.credit_atoms)),
            (
                "distinct_participating_owners",
                small(u64::from(summary.distinct_participating_owners)),
            ),
            (
                "self_overlap_volume",
                dec(u128::from(summary.self_overlap_volume)),
            ),
            ("score", render_score(&summary.score)),
            ("candidate_digest", dec(summary.candidate_digest)),
        ])
    }

    fn render_witness(witness: &v1::PairingWitnessV1) -> Value {
        let slices: Vec<Value> = witness.slices[..usize::from(witness.len)]
            .iter()
            .map(|slice| {
                obj(vec![
                    ("outcome", small(u64::from(slice.outcome))),
                    ("quantity", dec(u128::from(slice.quantity))),
                    ("buy_ref", render_leg(slice.buy_ref)),
                    ("sell_ref", render_leg(slice.sell_ref)),
                ])
            })
            .collect();
        obj(vec![
            ("len", small(u64::from(witness.len))),
            ("slices", Value::Array(slices)),
        ])
    }
}

fn render_leg(leg: v1::LegRefV1) -> Value {
    match leg {
        v1::LegRefV1::Order(index) => obj(vec![
            ("kind", Value::Str("order".into())),
            ("index", small(u64::from(index))),
        ]),
        v1::LegRefV1::Split => obj(vec![("kind", Value::Str("virtual-split".into()))]),
        v1::LegRefV1::Merge => obj(vec![("kind", Value::Str("virtual-merge".into()))]),
    }
}

impl Executor for RelationExecutor {
    fn apply(&mut self, op: &str, args: &Value) -> Result<Observed, String> {
        let outcomes = usize::from(self.domain.outcome_count);
        match op {
            "canonical_candidate" => {
                let prices = self.read_prices(field(args, "prices")?)?;
                let split = u64_field(args, "virtual_split")?;
                let merge = u64_field(args, "virtual_merge")?;
                let imbalance = i64::try_from(split).map_err(|_| "virtual_split too large")?
                    - i64::try_from(merge).map_err(|_| "virtual_merge too large")?;
                let mask = u64_field(args, "honored_aon_mask")?;
                Ok(
                    match v1::canonical_candidate(
                        &self.domain,
                        &self.book,
                        &prices,
                        imbalance,
                        mask,
                    ) {
                        Ok(candidate) => Observed::Ok(Self::render_candidate(&candidate, outcomes)),
                        Err(error) => Observed::Error(relation_code(error)),
                    },
                )
            }
            "verify" | "verify_ignoring_claimed_aggregates" => {
                let candidate = self.read_candidate(field(args, "candidate")?)?;
                let result = if op == "verify" {
                    v1::verify(&self.domain, &self.book, &candidate, None)
                } else {
                    v1::verify_ignoring_claimed_aggregates(
                        &self.domain,
                        &self.book,
                        &candidate,
                        None,
                    )
                };
                Ok(match result {
                    Ok(summary) => Observed::Ok(Self::render_summary(&summary)),
                    Err(error) => Observed::Error(relation_code(error)),
                })
            }
            "canonical_pairing" => {
                let candidate = self.read_candidate(field(args, "candidate")?)?;
                Ok(
                    match v1::canonical_pairing(&self.domain, &self.book, &candidate) {
                        Ok(witness) => Observed::Ok(Self::render_witness(&witness)),
                        Err(error) => Observed::Error(relation_code(error)),
                    },
                )
            }
            "verify_pairing_witness" => {
                let candidate = self.read_candidate(field(args, "candidate")?)?;
                let witness = v1::canonical_pairing(&self.domain, &self.book, &candidate);
                Ok(match witness {
                    Ok(witness) => match v1::verify_pairing_witness(
                        &self.domain,
                        &self.book,
                        &candidate,
                        &witness,
                    ) {
                        Ok(()) => Observed::Ok(Self::render_witness(&witness)),
                        Err(error) => Observed::Error(relation_code(error)),
                    },
                    Err(error) => Observed::Error(relation_code(error)),
                })
            }
            "validate" => Ok(match self.book.validate(&self.domain) {
                Ok(()) => Observed::Ok(Value::Null),
                Err(error) => Observed::Error(relation_code(error)),
            }),
            other => Err(format!(
                "clutch-batch relation_v1 has no operation {other:?}"
            )),
        }
    }

    fn render_state(&self) -> Value {
        // The frozen domain and book are immutable across every operation.
        obj(vec![
            ("len", small(u64::from(self.book.len))),
            ("outcome_count", small(u64::from(self.domain.outcome_count))),
        ])
    }
}
