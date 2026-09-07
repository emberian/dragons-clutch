//! Chain-derived compilation of a CHILD market's records: a product `A × B`
//! or a conditional `B | A = a` over two parents that already exist.
//!
//! The semantics are `DClutchSemantics.ConditionalMarketV1`; the wire is
//! `ParentReferenceV1Abi`; the Rust twin of both is
//! `dclutch_source::parent_reference_v1`, which this module calls for every
//! number (`ChildShapeV1`) rather than restating one. What this module adds is
//! the RECORDS: the `ParentReferenceV1` bytes, the consecutive-cut
//! `ResultDomainV2`, the founder's portfolio over the child's cells, the
//! refunding categorical `ProductBasisV3` at `payoutScale = n`, and the two
//! window facts the child's Source graph is built from. It never signs,
//! submits, or reads a chain: the parents' facts arrive as an input the driver
//! read off their Core Markets and result domains, and Core's founding proves
//! them again (`programs/dclutch-core-sbf/src/parents_v1.rs`).
//!
//! # `WindowBeforeParents`, and where it is enforced
//!
//! Neither the parents' windows nor the child's own are in Core's Found
//! frame, so the design note's founding conjunct `settle_by ≥ deadline(P) +
//! margin` (§2.3) is this compiler's, by [`require_settle_by_after_parents_v1`],
//! and the settle route binds `settle_by` to the child's window end. A child
//! founded past this conjunct by another compiler is a founder's mis-sizing:
//! it walks to its failure coordinate at `settle_by` and refunds, and the
//! founder bond prices exactly that (`BUILD_PRODUCT-SHAPES.md` §3, rulings 5
//! and 7).

use dclutch_product::ContentId;
use dclutch_product::payoff::runtime_v3::{
    BasisInputV3, BasisKindV3, basis_record_bytes_v3, compile_basis_v3,
};
use dclutch_source::parent_reference_v1::{
    ChildShapeV1, PARENT_REFERENCE_BYTES_V1, ParentRefV1, ParentReferenceKindV1, ParentReferenceV1,
    ParentRefusalV1,
};
use solana_program::pubkey::Pubkey;

use crate::{
    CompiledProductRecordsV2, Error, ProductCompilationInputV2, Result, compile_product_records_v2,
};

/// What a driver read off one parent before compiling the child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParentFactsV1 {
    /// The parent's Core Market account.
    pub market: [u8; 32],
    /// `CoreState.identity.generation`.
    pub generation: u64,
    /// `CoreState.identity.product_record`.
    pub product_record_digest: [u8; 32],
    /// The parent's `ResultDomainV2::region_count`.
    pub ordinary_count: u32,
    /// The parent's last deadline in Unix seconds: its last funded rung's
    /// deadline (`Ladder.deadline?`) or, with no policy, its window's
    /// `end + max_age`. The design note's `deadline(P)`.
    pub deadline_unix_seconds: i64,
}

impl ParentFactsV1 {
    fn reference(self) -> ParentRefV1 {
        ParentRefV1 {
            market: self.market,
            generation: self.generation,
            product_record_digest: self.product_record_digest,
            ordinary_count: self.ordinary_count,
        }
    }
}

/// Which child, in the vocabulary a founder uses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildQuestionV1 {
    /// "The joint outcome of `A` and `B`", `A` major.
    ProductOf,
    /// "`B`, given that `A` resolves to `condition`."
    ConditionalOn {
        /// An ordinary outcome of `A`.
        condition: u32,
    },
}

/// The founder's own portfolio over the child's cells. Like
/// `MarketQuestionV1`, the shape fixes the payoff so a founder cannot build a
/// child whose recipe ignores its own cells.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildPortfolioV1 {
    /// One claim of every cell in row `a` of a product child: "`A = a`,
    /// whatever `B`" — the one bundle shape the joint clearing admits as an
    /// interval (`MECHANISM_JOINT_CLEARING_2026_09_04.md` §1.4).
    RowBundle {
        /// The row.
        row: u32,
        /// Quantity of each cell.
        quantity: u64,
    },
    /// One claim of every branch cell of a conditional child: "`A =
    /// condition`", whatever `B`.
    BranchBundle {
        /// Quantity of each branch cell.
        quantity: u64,
    },
    /// One cell.
    Cell {
        /// The cell.
        cell: u32,
        /// Quantity.
        quantity: u64,
    },
}

/// Identities a child still has to be told. Coordinate domain and result unit
/// are the child's own — "the joint index of `A` and `B`" — and are derived
/// by the driver from the parents' identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildIdentitiesV1 {
    /// Stable Product semantic identity.
    pub product_id: ContentId,
    /// The joint-index coordinate domain.
    pub coordinate_domain_id: ContentId,
    /// The joint-index result unit.
    pub result_unit_id: ContentId,
    /// Exact native claim basis.
    pub claim_basis_id: ContentId,
    /// Product-selected liability basis.
    pub liability_basis_id: ContentId,
    /// Product-selected representation semantic release.
    pub representation_release_id: ContentId,
    /// Product-selected coordinate mapping semantic release.
    pub mapping_release_id: ContentId,
    /// The categorical evaluator release the basis names.
    pub evaluator_release_id: ContentId,
    /// Positive common exact portfolio denominator.
    pub portfolio_denominator: u64,
}

/// One child compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildCompilationInputV1 {
    /// Which child.
    pub question: ChildQuestionV1,
    /// The major axis; the condition's parent.
    pub parent_a: ParentFactsV1,
    /// The minor axis; the branch's parent.
    pub parent_b: ParentFactsV1,
    /// The child's deadline, Unix seconds; `windowEnd = acceptThrough`.
    pub settle_by: u64,
    /// The margin past every needed parent's deadline that covers that
    /// parent's own walk (`MAINNET_STATE_RELAY.md` §12.7: one transition with
    /// no policy). Also the child's window `max_age`.
    pub margin_seconds: u32,
    /// The founder's portfolio.
    pub portfolio: ChildPortfolioV1,
    /// The identities.
    pub identities: ChildIdentitiesV1,
}

/// Everything the driver needs to publish and found the child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledChildRecordsV1 {
    /// The reference as a value.
    pub reference: ParentReferenceV1,
    /// The reference's exact bytes: the record the child's spec names.
    pub reference_bytes: [u8; PARENT_REFERENCE_BYTES_V1],
    /// The admitted shape.
    pub shape: ChildShapeV1,
    /// Ordinary cells `n`.
    pub ordinary_count: u32,
    /// The consecutive cuts `[1, …, n − 1]`.
    pub cuts: Vec<i128>,
    /// The founder's coefficients over `n + 1` outcomes; the last is zero.
    pub coefficients: Vec<u64>,
    /// The three Product records' coordinates.
    pub product: CompiledProductRecordsV2,
    /// The window the Source graph carries: `(start, end, max_age)`.
    pub window: ChildWindowV1,
}

/// The child's terminal window. `start` is the founding instant the driver
/// supplies; the parents' terminals whenever they happened are admissible
/// evidence, including before the child was founded, because the observation
/// is a certificate and not a publication (design §2.3).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildWindowV1 {
    /// `start_unix_seconds`.
    pub start_unix_seconds: i64,
    /// `end_unix_seconds == settle_by`.
    pub end_unix_seconds: i64,
    /// `max_age_seconds == margin`.
    pub max_age_seconds: u32,
}

/// The exact `ProductBasisV3` width for a child: categorical, `n + 1` wide.
pub fn child_basis_output_bytes_v1(shape: ChildShapeV1) -> Result<usize> {
    let width =
        usize::try_from(shape.width().map_err(child_refusal)?).map_err(|_| Error::WidthMismatch)?;
    basis_record_bytes_v3(BasisKindV3::CategoricalQ1, width, 0, 0).map_err(Error::ProductBasis)
}

/// `WindowBeforeParents`: `settle_by ≥ deadline(A) + margin` always, and
/// `settle_by ≥ deadline(B) + margin` for a product child and for a
/// conditional child on its branch — a conditional child settles off its
/// branch without `B` (`off_condition_ignores_B`), but its branch cannot know
/// in advance that it will not be taken, so `B`'s deadline binds it too.
pub fn require_settle_by_after_parents_v1(
    question: ChildQuestionV1,
    parent_a: ParentFactsV1,
    parent_b: ParentFactsV1,
    settle_by: u64,
    margin_seconds: u32,
) -> Result<()> {
    let settle_by = i64::try_from(settle_by).map_err(|_| Error::FoundingBand)?;
    let margin = i64::from(margin_seconds);
    let bound = |deadline: i64| deadline.checked_add(margin).ok_or(Error::FoundingBand);
    // Both parents bind both kinds. A conditional child settles off its
    // condition without reading `B` at all (`off_condition_ignores_B`), but at
    // founding it cannot know that its condition will not be taken, so `B`'s
    // deadline is as binding for it as for a product child. `question` is
    // taken so a future kind that genuinely needed one parent could say so
    // here rather than at a call site.
    let _ = question;
    if settle_by < bound(parent_a.deadline_unix_seconds)?
        || settle_by < bound(parent_b.deadline_unix_seconds)?
    {
        return Err(Error::FoundingBand);
    }
    Ok(())
}

fn child_refusal(refusal: ParentRefusalV1) -> Error {
    match refusal {
        ParentRefusalV1::WidthOverflow => Error::WidthMismatch,
        _ => Error::InvalidRecord,
    }
}

/// Compile one child: the reference, the domain over consecutive cuts, the
/// founder's portfolio, the refunding basis, and the window facts.
///
/// All four caller buffers commit together only after the reference's
/// founding conjuncts, the window conjunct, and every record succeed.
pub fn compile_child_product_records_v1(
    registry_program: Pubkey,
    input: ChildCompilationInputV1,
    founding_unix_seconds: i64,
    product_output: &mut [u8],
    domain_output: &mut [u8],
    portfolio_output: &mut [u8],
    basis_output: &mut [u8],
) -> Result<CompiledChildRecordsV1> {
    let (kind, condition) = match input.question {
        ChildQuestionV1::ProductOf => (ParentReferenceKindV1::Product, 0),
        ChildQuestionV1::ConditionalOn { condition } => {
            (ParentReferenceKindV1::Conditional, condition)
        }
    };
    let reference = ParentReferenceV1 {
        kind,
        parent_a: input.parent_a.reference(),
        parent_b: input.parent_b.reference(),
        condition,
        settle_by: input.settle_by,
    };
    let shape = reference.admit_founding().map_err(child_refusal)?;
    require_settle_by_after_parents_v1(
        input.question,
        input.parent_a,
        input.parent_b,
        input.settle_by,
        input.margin_seconds,
    )?;
    let reference_bytes = reference.to_bytes().map_err(Error::Source)?;

    let ordinary_count = shape.ordinary_count().map_err(child_refusal)?;
    let cut_count = shape.cut_count().map_err(child_refusal)?;
    let mut cuts =
        Vec::with_capacity(usize::try_from(cut_count).map_err(|_| Error::WidthMismatch)?);
    for index in 0..cut_count {
        cuts.push(shape.cut(index).map_err(child_refusal)?);
    }
    let coefficients = child_coefficients_v1(shape, input.portfolio)?;

    let identities = input.identities;
    let product = compile_product_records_v2(
        registry_program,
        ProductCompilationInputV2 {
            product_id: identities.product_id,
            coordinate_domain_id: identities.coordinate_domain_id,
            result_unit_id: identities.result_unit_id,
            claim_basis_id: identities.claim_basis_id,
            liability_basis_id: identities.liability_basis_id,
            representation_release_id: identities.representation_release_id,
            mapping_release_id: identities.mapping_release_id,
            cut_denominator: 1,
            cuts: &cuts,
            portfolio_denominator: identities.portfolio_denominator,
            coefficients: &coefficients,
        },
        product_output,
        domain_output,
        portfolio_output,
    )?;
    let width = shape.width().map_err(child_refusal)?;
    if product.outcome_count != width {
        return Err(Error::WidthMismatch);
    }

    if basis_output.len() != child_basis_output_bytes_v1(shape)? {
        return Err(Error::OutputLength);
    }
    compile_basis_v3(
        BasisInputV3 {
            kind: BasisKindV3::CategoricalQ1,
            product_id: identities.product_id.to_bytes(),
            result_domain_id: product.receipt.result_domain.content_digest.to_bytes(),
            coordinate_domain_id: identities.coordinate_domain_id.to_bytes(),
            result_unit_id: identities.result_unit_id.to_bytes(),
            evaluator_release_id: identities.evaluator_release_id.to_bytes(),
            basis_width: width,
            // REFUNDING: `payoutScale = ordinaryCount`, the shape decision 0025
            // admits and `categoricalRefundsOnFailure` proves; the child's
            // outage arm is the escrow's walk reused verbatim.
            payout_scale: shape.payout_scale().map_err(child_refusal)?,
            knot_denominator: 1,
            knots: &[],
            terms: &[],
            failure_payouts: &[],
            price_gate_certificate_digest: [0_u8; 32],
        },
        basis_output,
    )
    .map_err(Error::ProductBasis)?;

    let end_unix_seconds = i64::try_from(input.settle_by).map_err(|_| Error::FoundingBand)?;
    if founding_unix_seconds <= 0 || founding_unix_seconds > end_unix_seconds {
        return Err(Error::FoundingBand);
    }
    Ok(CompiledChildRecordsV1 {
        reference,
        reference_bytes,
        shape,
        ordinary_count,
        cuts,
        coefficients,
        product,
        window: ChildWindowV1 {
            start_unix_seconds: founding_unix_seconds,
            end_unix_seconds,
            max_age_seconds: input.margin_seconds,
        },
    })
}

/// The founder's coefficients over the child's `n + 1` outcomes.
fn child_coefficients_v1(shape: ChildShapeV1, portfolio: ChildPortfolioV1) -> Result<Vec<u64>> {
    let ordinary = usize::try_from(shape.ordinary_count().map_err(child_refusal)?)
        .map_err(|_| Error::WidthMismatch)?;
    let mut coefficients = vec![0_u64; ordinary + 1];
    match (shape, portfolio) {
        (
            ChildShapeV1::Product { rows, columns },
            ChildPortfolioV1::RowBundle { row, quantity },
        ) => {
            if row >= rows || quantity == 0 {
                return Err(Error::InvalidRecord);
            }
            let first = usize::try_from(row.checked_mul(columns).ok_or(Error::WidthMismatch)?)
                .map_err(|_| Error::WidthMismatch)?;
            let width = usize::try_from(columns).map_err(|_| Error::WidthMismatch)?;
            for coefficient in coefficients
                .get_mut(first..first + width)
                .ok_or(Error::WidthMismatch)?
            {
                *coefficient = quantity;
            }
        }
        (ChildShapeV1::Conditional { branch, .. }, ChildPortfolioV1::BranchBundle { quantity }) => {
            if quantity == 0 {
                return Err(Error::InvalidRecord);
            }
            let width = usize::try_from(branch).map_err(|_| Error::WidthMismatch)?;
            for coefficient in coefficients.get_mut(..width).ok_or(Error::WidthMismatch)? {
                *coefficient = quantity;
            }
        }
        (_, ChildPortfolioV1::Cell { cell, quantity }) => {
            let index = usize::try_from(cell).map_err(|_| Error::WidthMismatch)?;
            if index >= ordinary || quantity == 0 {
                return Err(Error::InvalidRecord);
            }
            *coefficients.get_mut(index).ok_or(Error::WidthMismatch)? = quantity;
        }
        // A row bundle over a conditional child, or a branch bundle over a
        // product child, is a shape the other kind does not have.
        _ => return Err(Error::InvalidRecord),
    }
    Ok(coefficients)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent(tag: u8, ordinary_count: u32, deadline: i64) -> ParentFactsV1 {
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
            deadline_unix_seconds: deadline,
        }
    }

    #[test]
    fn settle_by_must_clear_both_parents_plus_margin() {
        let a = parent(0xA, 2, 1_000);
        let b = parent(0xB, 3, 2_000);
        assert_eq!(
            require_settle_by_after_parents_v1(ChildQuestionV1::ProductOf, a, b, 2_600, 600),
            Ok(())
        );
        assert_eq!(
            require_settle_by_after_parents_v1(ChildQuestionV1::ProductOf, a, b, 2_599, 600),
            Err(Error::FoundingBand)
        );
        // The later parent is the binding one for a conditional child too: at
        // founding it cannot know its condition will not be taken.
        assert_eq!(
            require_settle_by_after_parents_v1(
                ChildQuestionV1::ConditionalOn { condition: 0 },
                a,
                b,
                2_599,
                600
            ),
            Err(Error::FoundingBand)
        );
        assert_eq!(
            require_settle_by_after_parents_v1(
                ChildQuestionV1::ConditionalOn { condition: 0 },
                a,
                b,
                2_600,
                600
            ),
            Ok(())
        );
    }

    /// The flagship shape: `2 × 3` cells, width 7, cuts `[1..5]`, the row
    /// bundle of "activated" is cells 0..3.
    #[test]
    fn the_flagship_product_row_bundle() {
        let shape = ChildShapeV1::Product {
            rows: 2,
            columns: 3,
        };
        let coefficients = child_coefficients_v1(
            shape,
            ChildPortfolioV1::RowBundle {
                row: 0,
                quantity: 5,
            },
        )
        .expect("row bundle");
        assert_eq!(coefficients, vec![5, 5, 5, 0, 0, 0, 0]);
        // A branch bundle is a shape a product child does not have.
        assert_eq!(
            child_coefficients_v1(shape, ChildPortfolioV1::BranchBundle { quantity: 1 }),
            Err(Error::InvalidRecord)
        );
    }

    #[test]
    fn the_flagship_conditional_branch_bundle() {
        let shape = ChildShapeV1::Conditional {
            rows: 2,
            condition: 0,
            branch: 3,
        };
        let coefficients =
            child_coefficients_v1(shape, ChildPortfolioV1::BranchBundle { quantity: 2 })
                .expect("branch bundle");
        assert_eq!(coefficients, vec![2, 2, 2, 0, 0]);
    }
}
