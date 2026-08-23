//! Checked ranking fold over valid submitted RelationV2 candidates.
//!
//! The fold owns one exact domain, book, and price precondition. Every incoming
//! witness is reverified against those same immutable values before its
//! domain-bound ScoreV2-Q certificate may enter the ranking. This selects the
//! best valid submitted candidate encountered; it is not an optimality search.

use crate::relation_v2::{
    verify_economic_candidate_v2, EconomicBookV2, EconomicCandidateV2, EconomicDomainV2,
    EconomicErrorV2, PricePreconditionV2, VerifiedEconomicsV2,
};
use crate::score_v2::{BestSubmittedScoreV2, SelectionUpdateV2};

/// Fixed-domain fold retaining one best valid submitted RelationV2 candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BestValidSubmittedCandidateV2 {
    domain: EconomicDomainV2,
    book: EconomicBookV2,
    price: PricePreconditionV2,
    best_candidate: EconomicCandidateV2,
    best_economics: VerifiedEconomicsV2,
    score_selection: BestSubmittedScoreV2,
}

impl BestValidSubmittedCandidateV2 {
    /// Verify the first submission and begin one immutable-domain ranking fold.
    pub fn begin(
        domain: EconomicDomainV2,
        book: EconomicBookV2,
        price: PricePreconditionV2,
        first_candidate: EconomicCandidateV2,
    ) -> Result<Self, EconomicErrorV2> {
        let best_economics =
            verify_economic_candidate_v2(&domain, &book, &price, &first_candidate)?;
        let score_selection = BestSubmittedScoreV2::begin(best_economics.score);
        Ok(Self {
            domain,
            book,
            price,
            best_candidate: first_candidate,
            best_economics,
            score_selection,
        })
    }

    /// Reverify and rank one more submitted candidate.
    ///
    /// A refused witness leaves the fold unchanged. An exactly equal score
    /// retains the earlier submission. The incoming candidate replaces the
    /// retained candidate only when its checked ScoreV2-Q certificate is
    /// strictly preferred in the same immutable domain.
    pub fn submit(
        &mut self,
        candidate: EconomicCandidateV2,
    ) -> Result<SelectionUpdateV2, EconomicErrorV2> {
        let economics =
            verify_economic_candidate_v2(&self.domain, &self.book, &self.price, &candidate)?;
        let update = self
            .score_selection
            .consider(economics.score)
            .map_err(EconomicErrorV2::Score)?;
        if update == SelectionUpdateV2::ReplacedBest {
            self.best_candidate = candidate;
            self.best_economics = economics;
        }
        Ok(update)
    }

    /// Exact immutable economic domain shared by every admitted submission.
    pub const fn domain(&self) -> &EconomicDomainV2 {
        &self.domain
    }

    /// Exact immutable owner-blind order book shared by every submission.
    pub const fn book(&self) -> &EconomicBookV2 {
        &self.book
    }

    /// Exact immutable price precondition shared by every submission.
    pub const fn price(&self) -> &PricePreconditionV2 {
        &self.price
    }

    /// Retained best valid submitted candidate witness.
    pub const fn best_candidate(&self) -> &EconomicCandidateV2 {
        &self.best_candidate
    }

    /// Independently verified economics and checked ScoreV2-Q certificate.
    pub const fn best_economics(&self) -> &VerifiedEconomicsV2 {
        &self.best_economics
    }

    /// Number of valid submitted candidates admitted to the fold.
    pub const fn valid_submission_count(&self) -> u64 {
        self.score_selection.checked_submission_count()
    }
}
