//! The opponent: a fixed-belief automaton, and nothing more than that.
//!
//! It is **not** a model, an agent, a strategy, or an AI.  It holds one
//! integer vector that never changes, it compares that vector to a published
//! reference, and it rests one order per knot where the two disagree.  Both
//! numbers are shown on the Book screen, so a person can predict every order
//! it will ever place before it places one.  The bench says this in those
//! words; model theater would make the demonstration worthless, because the
//! whole thing being demonstrated is that a *disagreement between two stated
//! beliefs* is what clears.
//!
//! The quoting rule is the disagreement exhibit's book-former
//! (`svm-tests/tests/disagreement_exhibit.rs::book_plan`): on each knot the
//! side whose value is higher rests a buy at its own value and the side whose
//! value is lower rests a sell at its own value, `z` Eggs a quote.  The
//! exhibit compares two models to each other; at session open there is no
//! human belief yet, so the automaton compares itself to the flat prior — a
//! stated, fixed reference, published beside the belief.

use crate::quantize::{belief_on_ladder, PRICE_SCALE};
use serde_json::{json, Value};

/// The exhibit's Model E belief, in price units on the eight knots.
pub const MODEL_E: [u64; 8] = [0, 127, 2_662, 5_945, 1_266, 0, 0, 0];
/// Eggs per single-Egg quote — the book-former's `z`.
pub const QUOTE_SIZE: u64 = 500;

/// One resting order the automaton wants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Quote {
    pub outcome: u8,
    /// Zero buy, one sell, as the frozen record spells it.
    pub side: u8,
    pub quantity: u64,
    pub limit: u64,
}

pub struct Bot {
    /// The published belief.  Exact, fixed, never updated.
    pub belief: [u64; 8],
    /// The belief put on the frozen limit ladder — what it can actually quote.
    pub quoted: Vec<u64>,
    /// The reference it takes sides against at open: the flat prior.
    pub reference: [u64; 8],
    pub size: u64,
    ladder_step: u64,
}

impl Bot {
    /// The automaton this bench ships: Model E on the session's ladder.
    #[must_use]
    pub fn model_e(ladder_step: u64) -> Self {
        let flat = PRICE_SCALE / 8;
        let quoted = belief_on_ladder(&MODEL_E, ladder_step).unwrap_or_else(|| vec![0; 8]);
        Self {
            belief: MODEL_E,
            quoted,
            reference: [flat; 8],
            size: QUOTE_SIZE,
            ladder_step,
        }
    }

    /// The opening book: one quote per knot where the belief and the flat
    /// prior disagree, at the automaton's own ladder value.
    #[must_use]
    pub fn opening_quotes(&self) -> Vec<Quote> {
        (0..8_u8)
            .filter_map(|outcome| {
                let index = usize::from(outcome);
                let mine = self.belief[index];
                let theirs = self.reference[index];
                let side = match mine.cmp(&theirs) {
                    std::cmp::Ordering::Greater => 0,
                    std::cmp::Ordering::Less => 1,
                    std::cmp::Ordering::Equal => return None,
                };
                Some(Quote {
                    outcome,
                    side,
                    quantity: self.size,
                    limit: self.quoted[index],
                })
            })
            .collect()
    }

    /// The response rule: an order that crosses the automaton's own value is
    /// answered on the other side, at that same value.
    ///
    /// This is what "willing to trade at my stated value" means as a rule
    /// rather than as a slogan, and it is the only thing that ever adds an
    /// order after the opening book.  It never quotes a side its belief
    /// contradicts: it sells only at or above its value and buys only at or
    /// below it, which is the same inequality the opening rule uses.
    #[must_use]
    pub fn response_to(&self, order: Quote, resting: &[Quote]) -> Option<Quote> {
        let index = usize::from(order.outcome);
        let value = *self.quoted.get(index)?;
        let side = match order.side {
            // A buy at or above my value: I will sell into it at my value.
            0 if order.limit >= value => 1,
            // A sell at or below my value: I will buy it at my value.
            1 if order.limit <= value => 0,
            _ => return None,
        };
        if resting
            .iter()
            .any(|quote| quote.outcome == order.outcome && quote.side == side)
        {
            return None;
        }
        Some(Quote {
            outcome: order.outcome,
            side,
            quantity: self.size,
            limit: value,
        })
    }

    /// The automaton as the Book screen shows it: what it believes, what it
    /// can quote, and the rule it quotes by — said plainly.
    #[must_use]
    pub fn disclosure(&self) -> Value {
        json!({
            "kind": "fixed-belief automaton",
            "not": "a model, a strategy, or an AI",
            "belief": self.belief,
            "quoted_belief": self.quoted,
            "ladder_step": self.ladder_step,
            "reference": self.reference,
            "quote_size": self.size,
            "opening_rule": "one quote per knot where the belief differs from the flat prior: \
                             buy at my value where I am higher, sell at my value where I am lower",
            "response_rule": "an order that crosses my value is answered on the other side at my value",
            "belief_source": "the disagreement exhibit's Model E \
                              (svm-tests/tests/disagreement_exhibit.rs)",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_published_belief_is_a_price_vector() {
        assert_eq!(MODEL_E.iter().sum::<u64>(), PRICE_SCALE);
    }

    #[test]
    fn the_opening_book_takes_one_side_per_disagreeing_knot() {
        let bot = Bot::model_e(200);
        let quotes = bot.opening_quotes();
        // Every knot disagrees with the flat prior: three above, five below.
        assert_eq!(quotes.len(), 8);
        assert_eq!(quotes.iter().filter(|quote| quote.side == 0).count(), 3);
        assert_eq!(quotes.iter().filter(|quote| quote.side == 1).count(), 5);
        let buys: Vec<u8> = quotes
            .iter()
            .filter(|quote| quote.side == 0)
            .map(|quote| quote.outcome)
            .collect();
        assert_eq!(buys, vec![2, 3, 4]);
    }

    #[test]
    fn every_quoted_limit_is_on_the_ladder() {
        let bot = Bot::model_e(200);
        for quote in bot.opening_quotes() {
            assert_eq!(quote.limit % 200, 0);
        }
    }

    #[test]
    fn a_crossing_order_is_answered_and_a_non_crossing_one_is_not() {
        let bot = Bot::model_e(200);
        // The automaton's ladder value at knot 3 is 6000.
        let crossing = Quote {
            outcome: 3,
            side: 1,
            quantity: 500,
            limit: 5_800,
        };
        let answer = bot
            .response_to(crossing, &[])
            .expect("a crossing sell is answered");
        assert_eq!(answer.side, 0);
        assert_eq!(answer.limit, 6_000);
        let far = Quote {
            outcome: 3,
            side: 1,
            quantity: 500,
            limit: 6_200,
        };
        assert!(bot.response_to(far, &[]).is_none());
    }

    #[test]
    fn the_automaton_does_not_stack_a_second_quote_on_one_side() {
        let bot = Bot::model_e(200);
        let resting = bot.opening_quotes();
        let crossing = Quote {
            outcome: 3,
            side: 1,
            quantity: 500,
            limit: 5_800,
        };
        // It already rests a buy at knot 3 from the opening book.
        assert!(bot.response_to(crossing, &resting).is_none());
    }

    #[test]
    fn the_disclosure_never_calls_it_a_model() {
        let text = Bot::model_e(200).disclosure().to_string();
        assert!(text.contains("fixed-belief automaton"));
        assert!(!text.contains("\"model\""));
    }
}
