//! `clutch-kernel` (surface S1).

use std::collections::BTreeMap;

use clutch_kernel::{
    Amount, Error, MarketState, PayoutSet, PayoutVector, Phase, Position, TransferPhasePolicy,
    MAX_OUTCOMES, MAX_PAYOUTS,
};

use super::*;
use crate::json::Value;
use crate::taxonomy::{Observed, Refusal};

/// S1's variant to taxonomy-code map, per `VECTOR_SPINE_PROPOSAL.md` §2.4.
pub fn code(error: Error) -> Refusal {
    let (code, variant) = match error {
        Error::InvalidOutcomeCount => (2041, "InvalidOutcomeCount"),
        Error::InvalidPayoutCount => (2042, "InvalidPayoutCount"),
        // R1: this variant names two facts, so it maps to the coarse 2060.
        Error::InvalidPayoutIndex => (2060, "InvalidPayoutIndex"),
        Error::InvalidDenominator => (2043, "InvalidDenominator"),
        Error::InvalidPayoutWeights => (2044, "InvalidPayoutWeights"),
        Error::ZeroQuantity => (2045, "ZeroQuantity"),
        Error::ArithmeticOverflow => (1001, "ArithmeticOverflow"),
        Error::ArithmeticUnderflow => (1002, "ArithmeticUnderflow"),
        Error::InsufficientBalance => (5001, "InsufficientBalance"),
        Error::InsufficientCollateral => (5002, "InsufficientCollateral"),
        Error::NotActive => (3001, "NotActive"),
        Error::AlreadyResolved => (3002, "AlreadyResolved"),
        Error::NotResolved => (3003, "NotResolved"),
        Error::InvariantViolation => (5003, "InvariantViolation"),
        Error::RemainderRequired => (1004, "RemainderRequired"),
    };
    Refusal::new(code, "kernel", variant)
}

pub struct KernelExecutor {
    market: MarketState,
    positions: BTreeMap<String, Position>,
}

fn read_payout_vector(value: &Value, outcomes: usize) -> Result<PayoutVector, String> {
    let denominator = u64_field(value, "denominator")?;
    let weights: [u64; MAX_OUTCOMES] = read_prefix(field(value, "weights")?, outcomes)?;
    Ok(PayoutVector::new(denominator, weights))
}

fn read_payouts(value: &Value) -> Result<PayoutSet, String> {
    let count = small_field(value, "count")? as u8;
    let outcomes = small_field(value, "outcomes")? as u8;
    let items = field(value, "vectors")?.as_array()?;
    if items.len() != usize::from(count) {
        return Err(format!(
            "ARR-1: payouts.vectors has {} entries, count declares {count}",
            items.len()
        ));
    }
    let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
    for (index, item) in items.iter().enumerate() {
        vectors[index] = read_payout_vector(item, usize::from(outcomes))?;
    }
    Ok(PayoutSet::new(count, outcomes, vectors))
}

fn read_position(value: &Value, outcomes: usize) -> Result<Position, String> {
    Ok(Position {
        internal: read_prefix(field(value, "internal")?, outcomes)?,
        external: read_prefix(field(value, "external")?, outcomes)?,
    })
}

fn render_payouts(payouts: &PayoutSet) -> Value {
    let outcomes = usize::from(payouts.outcomes);
    let vectors: Vec<Value> = payouts.vectors[..usize::from(payouts.count)]
        .iter()
        .map(|vector| {
            obj(vec![
                ("denominator", dec(u128::from(vector.denominator))),
                ("weights", prefix(&vector.weights, outcomes)),
            ])
        })
        .collect();
    obj(vec![
        ("count", small(u64::from(payouts.count))),
        ("outcomes", small(u64::from(payouts.outcomes))),
        ("vectors", Value::Array(vectors)),
    ])
}

fn render_position(position: &Position, outcomes: usize) -> Value {
    obj(vec![
        ("internal", prefix(&position.internal, outcomes)),
        ("external", prefix(&position.external, outcomes)),
    ])
}

impl KernelExecutor {
    pub fn open(constructed_by: &str, value: &Value) -> Result<Self, String> {
        let outcomes = small_field(value, "outcomes")? as u8;
        let active = usize::from(outcomes);
        let payouts = read_payouts(field(value, "payouts")?)?;
        let collateral: Amount = u64_field(value, "collateral")?;
        let phase = match str_field(value, "phase")? {
            "active" => Phase::Active,
            "resolved" => Phase::Resolved,
            other => return Err(format!("ENUM-1: unknown kernel phase {other:?}")),
        };
        let resolved_payout = small_field(value, "resolved_payout")? as u8;
        let total_supply: [u64; MAX_OUTCOMES] = read_prefix(field(value, "total_supply")?, active)?;

        let market = match constructed_by {
            // A constructor-built state must be reachable through `new`, and
            // the vector's own declared fields must be what `new` produced.
            "constructor" => {
                let mut market = MarketState::new(outcomes, payouts, collateral)
                    .map_err(|error| format!("initial_state is not constructible: {error:?}"))?;
                if total_supply != [0; MAX_OUTCOMES]
                    || phase != Phase::Active
                    || resolved_payout != 0
                {
                    return Err(
                        "constructed_by is \"constructor\" but the state is not one MarketState::new returns"
                            .into(),
                    );
                }
                market.total_supply = total_supply;
                market
            }
            // §3.3: raw-fields is declared, never inferred.
            "raw-fields" | "operation-sequence" => MarketState {
                outcomes,
                phase,
                resolved_payout,
                collateral,
                total_supply,
                payouts,
            },
            other => return Err(format!("unknown constructed_by {other:?}")),
        };

        let mut positions = BTreeMap::new();
        positions.insert(
            "position".to_string(),
            read_position(field(value, "position")?, active)?,
        );
        if let Some(extra) = value.get("positions") {
            for (name, entry) in extra.as_object()? {
                if name == "position" {
                    return Err("positions may not redeclare \"position\"".into());
                }
                positions.insert(name.clone(), read_position(entry, active)?);
            }
        }
        Ok(Self { market, positions })
    }

    fn take(&mut self, name: &str) -> Result<Position, String> {
        self.positions
            .get(name)
            .copied()
            .ok_or_else(|| format!("no position named {name:?}"))
    }

    fn put(&mut self, name: &str, position: Position) {
        self.positions.insert(name.to_string(), position);
    }

    fn which<'a>(&self, args: &'a Value, key: &str) -> Result<&'a str, String> {
        match args.get(key) {
            Some(value) => value.as_str(),
            None => Ok("position"),
        }
    }
}

impl Executor for KernelExecutor {
    fn apply(&mut self, op: &str, args: &Value) -> Result<Observed, String> {
        let name = self.which(args, "position")?.to_string();
        macro_rules! run {
            ($body:expr) => {{
                let mut position = self.take(&name)?;
                let result = $body(&mut self.market, &mut position);
                match result {
                    Ok(value) => {
                        self.put(&name, position);
                        Ok(Observed::Ok(value))
                    }
                    Err(error) => {
                        // The kernel's landed contract is that a refused
                        // transition writes nothing, so the local copy is
                        // written back either way and the post-state check
                        // observes whether that contract held.
                        self.put(&name, position);
                        Ok(Observed::Error(code(error)))
                    }
                }
            }};
        }

        match op {
            "split" => {
                let quantity = u64_field(args, "quantity")?;
                run!(|market: &mut MarketState, position: &mut Position| market
                    .split(position, quantity)
                    .map(|()| Value::Null))
            }
            "merge" => {
                let quantity = u64_field(args, "quantity")?;
                run!(|market: &mut MarketState, position: &mut Position| market
                    .merge(position, quantity)
                    .map(|()| Value::Null))
            }
            "materialize" => {
                let outcome = small_field(args, "outcome")? as u8;
                let quantity = u64_field(args, "quantity")?;
                run!(|market: &mut MarketState, position: &mut Position| market
                    .materialize(position, outcome, quantity)
                    .map(|()| Value::Null))
            }
            "dematerialize" => {
                let outcome = small_field(args, "outcome")? as u8;
                let quantity = u64_field(args, "quantity")?;
                run!(|market: &mut MarketState, position: &mut Position| market
                    .dematerialize(position, outcome, quantity)
                    .map(|()| Value::Null))
            }
            "redeem_internal" => {
                let outcome = small_field(args, "outcome")? as u8;
                let quantity = u64_field(args, "quantity")?;
                run!(|market: &mut MarketState, position: &mut Position| market
                    .redeem_internal(position, outcome, quantity)
                    .map(|payout| obj(vec![("payout", dec(u128::from(payout)))])))
            }
            "redeem_external" => {
                let outcome = small_field(args, "outcome")? as u8;
                let quantity = u64_field(args, "quantity")?;
                run!(|market: &mut MarketState, position: &mut Position| market
                    .redeem_external(position, outcome, quantity)
                    .map(|payout| obj(vec![("payout", dec(u128::from(payout)))])))
            }
            "redeem_complete_set" => {
                let quantity = u64_field(args, "quantity")?;
                run!(|market: &mut MarketState, position: &mut Position| market
                    .redeem_complete_set(position, quantity)
                    .map(|payout| obj(vec![("payout", dec(u128::from(payout)))])))
            }
            "resolve" => {
                let index = small_field(args, "payout_index")? as u8;
                match self.market.resolve(index) {
                    Ok(()) => Ok(Observed::Ok(Value::Null)),
                    Err(error) => Ok(Observed::Error(code(error))),
                }
            }
            "transfer_internal" => {
                let from_name = str_field(args, "from")?.to_string();
                let to_name = str_field(args, "to")?.to_string();
                if from_name == to_name {
                    return Err("transfer_internal cannot name one position twice".into());
                }
                let outcome = small_field(args, "outcome")? as u8;
                let quantity = u64_field(args, "quantity")?;
                let policy = match str_field(args, "phase_policy")? {
                    "active-only" => TransferPhasePolicy::ActiveOnly,
                    "active-or-resolved" => TransferPhasePolicy::ActiveOrResolved,
                    other => {
                        return Err(format!("ENUM-1: unknown transfer phase policy {other:?}"))
                    }
                };
                let mut from = self.take(&from_name)?;
                let mut to = self.take(&to_name)?;
                let result = self
                    .market
                    .transfer_internal(&mut from, &mut to, outcome, quantity, policy);
                self.put(&from_name, from);
                self.put(&to_name, to);
                match result {
                    Ok(()) => Ok(Observed::Ok(Value::Null)),
                    Err(error) => Ok(Observed::Error(code(error))),
                }
            }
            "check_invariants" => match self.market.check_invariants() {
                Ok(()) => Ok(Observed::Ok(Value::Null)),
                Err(error) => Ok(Observed::Error(code(error))),
            },
            "required_collateral" => match self.market.required_collateral() {
                Ok(required) => Ok(Observed::Ok(obj(vec![(
                    "required",
                    dec(u128::from(required)),
                )]))),
                Err(error) => Ok(Observed::Error(code(error))),
            },
            other => Err(format!("clutch-kernel has no operation {other:?}")),
        }
    }

    fn render_state(&self) -> Value {
        let active = usize::from(self.market.outcomes);
        let mut pairs = vec![
            ("outcomes", small(u64::from(self.market.outcomes))),
            (
                "phase",
                Value::Str(
                    match self.market.phase {
                        Phase::Active => "active",
                        Phase::Resolved => "resolved",
                    }
                    .to_string(),
                ),
            ),
            (
                "resolved_payout",
                small(u64::from(self.market.resolved_payout)),
            ),
            ("collateral", dec(u128::from(self.market.collateral))),
            ("total_supply", prefix(&self.market.total_supply, active)),
            ("payouts", render_payouts(&self.market.payouts)),
            (
                "position",
                render_position(&self.positions["position"], active),
            ),
        ];
        let extra: BTreeMap<String, Value> = self
            .positions
            .iter()
            .filter(|(name, _)| name.as_str() != "position")
            .map(|(name, position)| (name.clone(), render_position(position, active)))
            .collect();
        if !extra.is_empty() {
            pairs.push(("positions", Value::Object(extra)));
        }
        obj(pairs)
    }
}
