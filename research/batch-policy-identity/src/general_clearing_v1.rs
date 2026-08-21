//! Tier 2 general portfolio-clearing policy profile (T2-5).
//!
//! One pinned [`FrozenPolicyV1`] const — the general-clearing sibling of
//! [`crate::direct_window_v1::DIRECT_POLICY_V1`] — plus the cross-plane
//! verdict-identity gate the Tier 2 plan demands: the streaming verifier
//! ([`clutch_batch::relation_v1_stream::ClearWorkV1`]) driven under the
//! zero-sentinel `RelationDomainV1` must reach exactly the V0--V8 verdict of
//! [`crate::verify_submitted_candidate`] on the same coordinates.  The tests
//! live here, on the research crate's side of the dependency edge, because
//! this crate depends on `clutch-batch` and not the other way around.

use clutch_batch::relation_v1::{
    AllocationPolicyV1, AonPolicyV1, FeeBaseV1, FrozenPolicyV1, PairingWitnessPolicyV1,
    PortfolioLotPolicyV1, ResidualSettlementV1, RoundingBoundaryV1, ScorePolicyV1,
    SelfCrossPolicyV1, TransferPhaseV1,
};
use clutch_batch::DustPolicy;

/// **Tier 2 frozen policy profile v1 — FROZEN 2026-08-20.**
///
/// The general portfolio-clearing profile named by
/// `docs/design/TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md` (T2-5), frozen as
/// pinned by `docs/decisions/ADOPTED_2026-08-20.md` item 1
/// (`REPORT_general-clearing-policy-freeze`).  Every selector below is now a
/// freeze, not a proposal: a future profile is a **sibling const with a new
/// digest**, never an edit here, so the freeze forecloses no dynamics.
/// Pinning `fee_base: None` at 0 basis points deliberately does **not** preempt
/// the fee-base fork — that is item 9's separate decision, whose shape-only
/// sibling is [`GENERAL_CLEARING_FEE_SHAPE_V1`] below, and fees stay forced
/// zero at every Tier 2 gate regardless.
///
/// Plan-pinned selectors:
///
/// * `fee_base: None` (0 bps) — the plan's out-of-scope rule keeps fees zero.
/// * `pairing_witness: ExplicitSlices` — the candidate feed persists the
///   slices; T2-6c streams them through `push_slice`.
/// * `portfolio_lots: StrictWholeOrder` — the only implemented variant;
///   `MarginalProRataLots` is registered but refused by
///   `FrozenPolicyV1::validate`.
/// * `self_cross: RefuseOverlap` — two order passes, not three.
/// * `dust: AssignCanonical` — the plan requires ONE pinned choice.  Canonical
///   largest-remainder is taken from the relation's own code and tests, not
///   preference: both relation test suites freeze it in their base policies
///   (`relation_v1_tests.rs`, `relation_v1_stream_tests.rs`); the domain's
///   `remainder_seed` exists solely as the largest-remainder tie-break seed
///   (`relation_v1.rs`, `better_remainder` / the stream's `PoolRowV1.rank`) and
///   would be a dead field under `Reject`; and under `DustPolicy::Reject` a
///   marginal pro-rata pool with any leftover atom has **no valid candidate at
///   all** (`ErrorV1::DustRejected` from the canonical constructor) — a
///   liveness hazard generic on many-order books, exercised by
///   `general_clearing_dust_choice_keeps_remainder_books_clearable` below.
///   `DIRECT_POLICY_V1` pins `Reject`, but its two-order profile gives every
///   pro-rata pool at most one member, whose floor equals the target exactly,
///   so dust is structurally zero there and that precedent carries no force
///   for the general book.
///
/// Selectors the plan leaves open, taken from the shipped precedent
/// (`DIRECT_POLICY_V1`) except where the general book forces otherwise:
///
/// * `allocation: PricePriorityMarginalProRata` — the relation's canonical
///   allocation; every base policy in tree pins it.
/// * `aon: RefuseAdmission` — matches `DIRECT_POLICY_V1` and both relation
///   test bases; keeps `honored_aon_mask == 0` for the first general profile.
/// * `rounding: TerminalOwnerFloor` — `None` (exact-or-refuse) refuses any
///   candidate whose per-owner cash conversion leaves a remainder
///   (`ErrorV1::RemainderRequired`, relation_v1.rs V6), which is generic on
///   general books for the same reason dust is; R-b is the frozen base of both
///   relation test suites.  (`DIRECT_POLICY_V1`'s `None` is again vacuous at
///   two orders and zero fees.)
/// * `residual_settlement: UniqueSliceReceipts` — T2-8 creates one receipt PDA
///   per `(candidate, slice_index)`; matches `DIRECT_POLICY_V1`.
/// * `transfer_phase: ActiveOrResolved` — matches `DIRECT_POLICY_V1`.
/// * `score: LexicographicDispersionV1` — the only registered score family.
pub const GENERAL_CLEARING_POLICY_V1: FrozenPolicyV1 = FrozenPolicyV1 {
    allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
    self_cross: SelfCrossPolicyV1::RefuseOverlap,
    aon: AonPolicyV1::RefuseAdmission,
    rounding: RoundingBoundaryV1::TerminalOwnerFloor,
    residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
    transfer_phase: TransferPhaseV1::ActiveOrResolved,
    portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
    pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
    dust: DustPolicy::AssignCanonical,
    score: ScorePolicyV1::LexicographicDispersionV1,
    fee_base: FeeBaseV1::None,
};

/// **The fee-bearing SIBLING of [`GENERAL_CLEARING_POLICY_V1`] — SHAPE
/// ADOPTED 2026-08-20, both rates UNDECIDED and pinned to an explicit zero,
/// and unused by every settling path.**
///
/// The composite fee base selected 2026-08-20 (`ADOPTED_2026-08-20.md` item 9
/// on `REPORT_fee-base-selection`): `kappa*G(a,p) + kappa'*R(a)` — atomic
/// simplex dispersion with a price-free quotient-norm floor.  Every selector
/// besides `fee_base` is byte-identical to the frozen zero-fee profile, so
/// the two digests differ in exactly the fee member.
///
/// **Both rates are UNDECIDED and pinned zero here.**  Any nonzero rate is a
/// **new sibling const with a new digest behind its own ember decision**,
/// plus the relation-side composite arithmetic (unimplemented; the relation
/// refuses nonzero composite rates as `PolicyVariantUnimplemented`) and B2's
/// frozen bounds.  Nothing about this const relaxes any of the five
/// `max_fee_atoms == 0` gates: the GENERAL plane keeps clearing under the
/// frozen zero-fee const, and a fee-bearing epoch admission additionally
/// requires the Realm's revenue-policy record to name a real treasury and a
/// treasury-owned Position to exist (`REVENUE_POLICY_V1.md` §5, §8) — a
/// boundary that refuses structurally while the treasury pubkey stays
/// deferred (B4a).
pub const GENERAL_CLEARING_FEE_SHAPE_V1: FrozenPolicyV1 = FrozenPolicyV1 {
    allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
    self_cross: SelfCrossPolicyV1::RefuseOverlap,
    aon: AonPolicyV1::RefuseAdmission,
    rounding: RoundingBoundaryV1::TerminalOwnerFloor,
    residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
    transfer_phase: TransferPhaseV1::ActiveOrResolved,
    portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
    pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
    dust: DustPolicy::AssignCanonical,
    score: ScorePolicyV1::LexicographicDispersionV1,
    fee_base: FeeBaseV1::CompositeDispersionFloor {
        dispersion_bps: 0,
        floor_range_bps: 0,
    },
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direct_window_v1::DIRECT_POLICY_V1;
    use crate::hasher;
    use crate::{
        batch_policy_digest, canonical_batch_policy_bytes, complete_submitted_candidate,
        decode_batch_policy, full_relation_candidate_digest, sha256, verify_submitted_candidate,
        FullCandidateFeedBindingV1, FullRelationDomainV1, FullScoreV1, FullSubmittedCandidateV1,
        Identity32V1, PolicyIdentityErrorV1, BATCH_POLICY_DIGEST_DOMAIN,
    };
    use clutch_batch::relation_v1::{
        canonical_candidate, canonical_pairing, BookV1, CandidateV1, ErrorV1, OrderV1,
        PairingWitnessV1, PortfolioOrderV1, RelationDomainV1, ScoreV1, SingleEggOrderV1,
        SummaryV1, MAX_OUTCOMES, PRICE_SCALE, RELATION_VERSION_V1,
    };
    use clutch_batch::relation_v1_stream::{ClearWorkV1, FeedStatusV1, StreamCandidateV1};
    use clutch_batch::{PartialPolicy, Side};
    use core::cmp::Ordering;

    extern crate std;
    use std::boxed::Box;

    const SCALE: u64 = PRICE_SCALE;

    fn id(seed: u8) -> Identity32V1 {
        let mut bytes = [seed; 32];
        bytes[0] = seed.wrapping_add(0x40);
        Identity32V1(bytes)
    }

    fn full_domain_with(policy: FrozenPolicyV1, outcomes: u8, owners: u16) -> FullRelationDomainV1 {
        FullRelationDomainV1 {
            relation_version: RELATION_VERSION_V1,
            market_id: id(0x11),
            book_id: id(0x22),
            epoch_id: id(0x33),
            policy_id: batch_policy_digest(&policy).unwrap(),
            order_set_id: id(0x55),
            epoch_index: 7,
            outcome_count: outcomes,
            owner_count: owners,
            price_scale: SCALE,
            remainder_seed: 0x00C0_FFEE,
            policy,
        }
    }

    /// The T2-5 domain construction, verbatim: four u64 identity tags as zero
    /// sentinels, everything economic carried at full fidelity.
    fn zero_sentinel_domain(full: &FullRelationDomainV1) -> RelationDomainV1 {
        RelationDomainV1 {
            relation_version: full.relation_version,
            market_id: 0,
            book_id: 0,
            epoch: full.epoch_index,
            policy_id: 0,
            order_set_id: 0,
            outcome_count: full.outcome_count,
            owner_count: full.owner_count,
            price_scale: full.price_scale,
            remainder_seed: full.remainder_seed,
            policy: full.policy,
        }
    }

    fn single(id: u64, owner: u16, outcome: u8, side: Side, quantity: u64, limit: u64) -> OrderV1 {
        OrderV1::SingleEgg(SingleEggOrderV1 {
            canonical_order_id: id,
            owner,
            outcome,
            side,
            quantity,
            limit_price: limit,
            minimum_fill: 1,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        })
    }

    fn portfolio(
        id: u64,
        owner: u16,
        side: Side,
        coefficients: &[u64],
        lots: u64,
        limit_per_lot: u64,
    ) -> OrderV1 {
        let mut vector = [0u64; MAX_OUTCOMES];
        vector[..coefficients.len()].copy_from_slice(coefficients);
        OrderV1::Portfolio(PortfolioOrderV1 {
            canonical_order_id: id,
            owner,
            side,
            coefficients: vector,
            active_len: coefficients.len() as u8,
            lots,
            limit_collateral_per_lot: limit_per_lot,
            minimum_fill_lots: 1,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        })
    }

    fn book_of(orders: &[OrderV1]) -> BookV1 {
        let mut book = BookV1::empty();
        book.orders[..orders.len()].copy_from_slice(orders);
        book.len = orders.len() as u8;
        book
    }

    fn prices(values: &[u64]) -> [u64; MAX_OUTCOMES] {
        let mut vector = [0u64; MAX_OUTCOMES];
        vector[..values.len()].copy_from_slice(values);
        vector
    }

    /// Drive one whole feed under `strict_claims: false`, exactly the plan's
    /// on-chain shape: the claimed `ScoreV1` and `u128` digest are never
    /// consulted, so the header carries explicit zeros for both.
    fn drive_stream(
        work: &mut ClearWorkV1,
        domain: &RelationDomainV1,
        book: &BookV1,
        candidate: &CandidateV1,
        pairing: Option<&PairingWitnessV1>,
    ) -> Result<SummaryV1, ErrorV1> {
        let header = StreamCandidateV1 {
            order_len: candidate.order_len,
            prices: candidate.prices,
            virtual_split: candidate.virtual_split,
            virtual_merge: candidate.virtual_merge,
            honored_aon_mask: candidate.honored_aon_mask,
            claimed_score: ScoreV1::ZERO,
            canonical_candidate_digest: 0,
            declared_slices: pairing.map(|witness| witness.len),
        };
        work.begin(domain, &header, false).unwrap();
        loop {
            match work.status() {
                FeedStatusV1::NeedOrders { .. } => {
                    let mut j = 0usize;
                    while j < book.len as usize {
                        if work.status() == FeedStatusV1::Complete {
                            break;
                        }
                        work.push_order(&book.orders[j], candidate.fills[j]).unwrap();
                        j += 1;
                    }
                    if work.status() != FeedStatusV1::Complete {
                        work.end_pass().unwrap();
                    }
                }
                FeedStatusV1::NeedSlices => {
                    let witness = pairing.expect("slices demanded without a witness");
                    let mut k = 0usize;
                    while k < witness.len as usize {
                        work.push_slice(&witness.slices[k]).unwrap();
                        k += 1;
                    }
                    work.end_pass().unwrap();
                }
                FeedStatusV1::Complete => break,
            }
        }
        work.verdict()
            .expect("complete feed must have a verdict")
            .copied()
    }

    /// The T2-5 gate for one accepted coordinate set: the streaming verdict
    /// under the zero-sentinel domain equals `verify_submitted_candidate`'s
    /// V0--V8 verdict; the authoritative score is `FullScoreV1` over the
    /// streamed `SummaryV1`'s recomputed components, never the claimed `u128`.
    fn assert_verdict_identity(
        work: &mut ClearWorkV1,
        full: &FullRelationDomainV1,
        book: &BookV1,
        legacy: &CandidateV1,
    ) {
        let zero = zero_sentinel_domain(full);
        let witness = canonical_pairing(&zero, book, legacy).unwrap();

        let raw = FullSubmittedCandidateV1::from_relation_candidate(full, legacy).unwrap();
        let (candidate, feed) =
            complete_submitted_candidate(full, book, &raw, Some(&witness)).unwrap();
        let verified =
            verify_submitted_candidate(full, book, &candidate, &feed, Some(&witness)).unwrap();

        let streamed = drive_stream(work, &zero, book, legacy, Some(&witness)).unwrap();

        // V0--V8 economics identity.  Only the two obsolete u64-domain digest
        // fields may differ: the streamed summary computes them over the
        // zero-sentinel tags, the full verifier returns them as explicit zero
        // sentinels.  Nothing else may diverge.
        let mut economics = streamed;
        economics.score.digest = 0;
        economics.candidate_digest = 0;
        assert_eq!(economics, verified.economics);

        // Score identity: components recomputed from the streamed summary plus
        // the full-width relation-candidate digest, ordered by
        // `FullScoreV1::total_order`.
        let streamed_score = FullScoreV1 {
            weighted_direct_volume: streamed.score.weighted_direct_volume,
            limit_surplus_price_units: streamed.score.limit_surplus_price_units,
            distinct_owners: streamed.score.distinct_owners,
            churn: streamed.score.churn,
            digest: full_relation_candidate_digest(full, &candidate, Some(&witness)).unwrap(),
        };
        assert_eq!(streamed_score, verified.score);
        assert_eq!(streamed_score.total_order(&verified.score), Ordering::Equal);
    }

    #[test]
    fn general_clearing_policy_identity_value_is_pinned() {
        let policy = GENERAL_CLEARING_POLICY_V1;
        let bytes = canonical_batch_policy_bytes(&policy).unwrap();
        assert_eq!(
            bytes,
            [
                0x44, 0x43, 0x42, 0x41, 0x54, 0x50, 0x31, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x01, 0x03, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
            ],
            "the general-clearing policy artifact bytes moved"
        );
        assert_eq!(decode_batch_policy(&bytes), Ok(policy));
        // SHA-256("dragons-clutch/batch-policy/v1\0" || those 64 bytes), taken
        // from a third SHA-256 implementation rather than from either path here.
        assert_eq!(
            batch_policy_digest(&policy).unwrap().0,
            [
                0x7a, 0x9e, 0xa8, 0x0b, 0x81, 0x9f, 0x85, 0x3d, 0x95, 0x23, 0xa5, 0xe0, 0xed, 0x0b,
                0xb8, 0xe5, 0xab, 0x4e, 0x16, 0x7a, 0xb0, 0xc2, 0x24, 0x53, 0x16, 0x77, 0x59, 0x55,
                0xc7, 0xa2, 0x06, 0x5b
            ],
            "the general-clearing policy identity moved"
        );
        assert_eq!(
            batch_policy_digest(&policy).unwrap(),
            sha256::<hasher::Native<{ BATCH_POLICY_DIGEST_DOMAIN.len() + 64 }>>(
                BATCH_POLICY_DIGEST_DOMAIN,
                &[&bytes],
            ),
            "the on-chain hasher disagrees with the pinned value"
        );
        // Two distinct frozen profiles must never share an identity.
        assert_ne!(
            batch_policy_digest(&policy).unwrap(),
            batch_policy_digest(&DIRECT_POLICY_V1).unwrap()
        );
    }

    #[test]
    fn every_selector_mutation_moves_the_general_clearing_identity() {
        // Every registered alternative of every selector family, plus both fee
        // boundaries.  The score family has exactly one registered member, so
        // it has no alternative to list; the codec-level byte-mutation sweep in
        // the crate root already covers its wire byte.
        let variants = [
            FrozenPolicyV1 {
                allocation: AllocationPolicyV1::FullProRata,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                self_cross: SelfCrossPolicyV1::NetAtAdmission,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                self_cross: SelfCrossPolicyV1::AllowGateAtPairing,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                aon: AonPolicyV1::WitnessedHonoredMask,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                aon: AonPolicyV1::FullSizeCounting,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                rounding: RoundingBoundaryV1::None,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                rounding: RoundingBoundaryV1::ReceiptFloor,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                residual_settlement: ResidualSettlementV1::FullPairOnly,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                residual_settlement: ResidualSettlementV1::CumulativePairCanonical,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                residual_settlement: ResidualSettlementV1::CumulativePairFree,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                transfer_phase: TransferPhaseV1::ActiveOnly,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                portfolio_lots: PortfolioLotPolicyV1::MarginalProRataLots,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                dust: DustPolicy::Reject,
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                fee_base: FeeBaseV1::FlatNotional { bps: 0 },
                ..GENERAL_CLEARING_POLICY_V1
            },
            FrozenPolicyV1 {
                fee_base: FeeBaseV1::FlatNotional { bps: 10_000 },
                ..GENERAL_CLEARING_POLICY_V1
            },
            // The composite fee-base SHAPE (zero rates) — exactly the
            // fee-bearing sibling const, which must never share the frozen
            // zero-fee identity.
            FrozenPolicyV1 {
                fee_base: FeeBaseV1::CompositeDispersionFloor {
                    dispersion_bps: 0,
                    floor_range_bps: 0,
                },
                ..GENERAL_CLEARING_POLICY_V1
            },
        ];
        let pinned = batch_policy_digest(&GENERAL_CLEARING_POLICY_V1).unwrap();
        for policy in variants {
            assert_ne!(policy, GENERAL_CLEARING_POLICY_V1);
            assert_ne!(
                batch_policy_digest(&policy).unwrap(),
                pinned,
                "a selector mutation left the pinned identity unchanged: {policy:?}"
            );
        }
    }

    /// The fee-bearing sibling's identity, pinned byte-for-byte and by an
    /// independently computed SHA-256.  The artifact differs from the frozen
    /// zero-fee profile in exactly the fee discriminant (byte 22 = 0x02, the
    /// composite tag, both rate words zero), so the frozen digest is
    /// untouched by this const's existence.
    #[test]
    fn general_clearing_fee_shape_identity_is_pinned() {
        let policy = GENERAL_CLEARING_FEE_SHAPE_V1;
        let bytes = canonical_batch_policy_bytes(&policy).unwrap();
        let mut expected = canonical_batch_policy_bytes(&GENERAL_CLEARING_POLICY_V1).unwrap();
        expected[22] = 0x02;
        assert_eq!(bytes, expected, "the fee-shape artifact bytes moved");
        assert_eq!(decode_batch_policy(&bytes), Ok(policy));
        // SHA-256("dragons-clutch/batch-policy/v1\0" || those 64 bytes), taken
        // from a third SHA-256 implementation rather than from either path here.
        assert_eq!(
            batch_policy_digest(&policy).unwrap().0,
            [
                0xac, 0xf9, 0x74, 0x7c, 0xf8, 0x45, 0x52, 0xea, 0x3c, 0x17, 0x7a, 0x16, 0x71, 0x03,
                0x33, 0xc9, 0x2f, 0x01, 0x83, 0xd9, 0x46, 0xa1, 0x21, 0x96, 0xf0, 0xd8, 0x6f, 0xc3,
                0x71, 0x34, 0xeb, 0x06
            ],
            "the fee-shape identity moved"
        );
        assert_eq!(
            batch_policy_digest(&policy).unwrap(),
            sha256::<hasher::Native<{ BATCH_POLICY_DIGEST_DOMAIN.len() + 64 }>>(
                BATCH_POLICY_DIGEST_DOMAIN,
                &[&bytes],
            ),
            "the on-chain hasher disagrees with the pinned value"
        );
        // The sibling never shares an identity with either frozen profile.
        assert_ne!(
            batch_policy_digest(&policy).unwrap(),
            batch_policy_digest(&GENERAL_CLEARING_POLICY_V1).unwrap()
        );
        assert_ne!(
            batch_policy_digest(&policy).unwrap(),
            batch_policy_digest(&DIRECT_POLICY_V1).unwrap()
        );
    }

    /// The domain validator admits the fee shape as a valid member — and
    /// refuses every nonzero rate pair: rates are undecided, and any nonzero
    /// rate is a new const + digest + ember decision plus the relation-side
    /// composite arithmetic, which deliberately does not exist yet.
    #[test]
    fn the_fee_shape_is_domain_valid_and_nonzero_rates_refuse() {
        let full = full_domain_with(GENERAL_CLEARING_FEE_SHAPE_V1, 2, 2);
        assert_eq!(zero_sentinel_domain(&full).validate(), Ok(()));
        assert_eq!(GENERAL_CLEARING_FEE_SHAPE_V1.validate(), Ok(()));
        for (dispersion_bps, floor_range_bps) in [(1u32, 0u32), (0, 1), (40, 10), (10_000, 10_000)]
        {
            let rated = FrozenPolicyV1 {
                fee_base: clutch_batch::relation_v1::FeeBaseV1::CompositeDispersionFloor {
                    dispersion_bps,
                    floor_range_bps,
                },
                ..GENERAL_CLEARING_FEE_SHAPE_V1
            };
            assert_eq!(
                rated.validate(),
                Err(ErrorV1::PolicyVariantUnimplemented),
                "a nonzero composite rate must refuse validation"
            );
            // A nonzero rate has no canonical artifact bytes at all.
            assert!(canonical_batch_policy_bytes(&rated).is_err());
        }
    }

    /// REVENUE_POLICY_V1 §10 falsifiers 1/5/8 at zero rates, host plane: on a
    /// plain cross, on a wash-shaped two-owner round trip, and on a candidate
    /// clearing at a ZERO price coordinate, the fee-shape sibling's streamed
    /// verdict equals the frozen zero-fee verdict in every field except the
    /// two obsolete legacy digests, with the fee terms exactly zero.  The
    /// relation's own V7/V8 closes enforce conservation with that zero fee
    /// term on both drives, so a fee term appearing from nowhere — or a
    /// missing one — refuses rather than passes.
    #[test]
    fn the_fee_shape_at_zero_rates_is_economically_identical_to_zero_fee() {
        let mut work = Box::new(ClearWorkV1::new());

        // (a) a plain cross; (b) a wash-shaped round trip: the same two
        // owners take both sides in opposite directions across two outcomes;
        // (c) a zero-price clearing coordinate (the §10.5 channel fixture:
        // price 0 is an admissible coordinate, and at zero rates the
        // composite charges exactly zero on it — the runtime disposition is
        // "zeroes consistently", proved rather than assumed).
        let cross = book_of(&[
            single(1, 0, 0, Side::Buy, 4, SCALE),
            single(2, 1, 0, Side::Sell, 4, 0),
        ]);
        let wash = book_of(&[
            single(1, 0, 0, Side::Buy, 4, SCALE),
            single(2, 1, 0, Side::Sell, 4, 0),
            single(3, 1, 1, Side::Buy, 4, SCALE),
            single(4, 0, 1, Side::Sell, 4, 0),
        ]);
        let zero_price = book_of(&[
            single(1, 0, 0, Side::Buy, 4, SCALE / 2),
            single(2, 1, 0, Side::Sell, 4, 0),
        ]);
        let cases: [(&BookV1, [u64; MAX_OUTCOMES], u16); 3] = [
            (&cross, prices(&[SCALE / 2, SCALE / 2]), 2),
            (&wash, prices(&[SCALE / 2, SCALE / 2]), 2),
            (&zero_price, prices(&[0, SCALE]), 2),
        ];
        for (book, vector, owners) in cases {
            let zero_fee = zero_sentinel_domain(&full_domain_with(
                GENERAL_CLEARING_POLICY_V1,
                2,
                owners,
            ));
            let fee_shape = zero_sentinel_domain(&full_domain_with(
                GENERAL_CLEARING_FEE_SHAPE_V1,
                2,
                owners,
            ));
            let candidate = canonical_candidate(&zero_fee, book, &vector, 0, 0).unwrap();
            assert!(
                candidate.fills.iter().any(|fill| *fill != 0),
                "a vacuously empty candidate would prove nothing"
            );
            let witness = canonical_pairing(&zero_fee, book, &candidate).unwrap();
            let under_zero =
                drive_stream(&mut work, &zero_fee, book, &candidate, Some(&witness)).unwrap();
            let under_shape =
                drive_stream(&mut work, &fee_shape, book, &candidate, Some(&witness)).unwrap();
            // The zero fee term, stated: nothing was charged and nothing was
            // carried under the fee-bearing shape at zero rates.
            assert_eq!(under_shape.fee_price_units, 0);
            assert_eq!(under_shape.fee_carry_bps_units, 0);
            // Bit-identical economics: only the legacy digest fields (which
            // fold the policy tag) may differ between the two policy consts.
            let mut scrubbed_zero = under_zero;
            scrubbed_zero.score.digest = 0;
            scrubbed_zero.candidate_digest = 0;
            let mut scrubbed_shape = under_shape;
            scrubbed_shape.score.digest = 0;
            scrubbed_shape.candidate_digest = 0;
            assert_eq!(scrubbed_zero, scrubbed_shape);
        }

        // The full-width verifier reaches the identical verdict under the
        // fee-shape const — the whole T2-5 identity gate holds for the
        // sibling, so admitting the shape perturbs no pipeline.
        let candidate = canonical_candidate(
            &zero_sentinel_domain(&full_domain_with(GENERAL_CLEARING_FEE_SHAPE_V1, 2, 2)),
            &cross,
            &prices(&[SCALE / 2, SCALE / 2]),
            0,
            0,
        )
        .unwrap();
        assert_verdict_identity(
            &mut work,
            &full_domain_with(GENERAL_CLEARING_FEE_SHAPE_V1, 2, 2),
            &cross,
            &candidate,
        );
    }

    /// The dust-choice justification, executable.  A three-order marginal
    /// pro-rata book whose largest-remainder floors leave one atom clears under
    /// the pinned profile and has NO valid candidate at all under
    /// `DustPolicy::Reject` — the general book's liveness hazard the pinned
    /// `AssignCanonical` avoids.
    #[test]
    fn general_clearing_dust_choice_keeps_remainder_books_clearable() {
        let book = book_of(&[
            single(1, 0, 0, Side::Buy, 4, SCALE / 2),
            single(2, 1, 0, Side::Buy, 3, SCALE / 2),
            single(3, 2, 0, Side::Sell, 5, 0),
        ]);
        let vector = prices(&[SCALE / 2, SCALE / 2]);

        let full = full_domain_with(GENERAL_CLEARING_POLICY_V1, 2, 3);
        let zero = zero_sentinel_domain(&full);
        let candidate = canonical_candidate(&zero, &book, &vector, 0, 0).unwrap();
        assert_eq!(candidate.fills[0] + candidate.fills[1], 5);
        let mut work = Box::new(ClearWorkV1::new());
        assert_verdict_identity(&mut work, &full, &book, &candidate);

        let rejecting = full_domain_with(
            FrozenPolicyV1 {
                dust: DustPolicy::Reject,
                ..GENERAL_CLEARING_POLICY_V1
            },
            2,
            3,
        );
        assert_eq!(
            canonical_candidate(&zero_sentinel_domain(&rejecting), &book, &vector, 0, 0),
            Err(ErrorV1::DustRejected)
        );
    }

    /// The T2-5 gate: streaming verdict under the zero-sentinel domain equals
    /// `verify_submitted_candidate`'s V0--V8 verdict on the same coordinates —
    /// on a plain cross, on a marginal pro-rata book with assigned dust, on a
    /// portfolio book, and on a refused forgery.
    #[test]
    fn streaming_zero_sentinel_verdict_matches_the_full_width_verifier() {
        let mut work = Box::new(ClearWorkV1::new());

        // A plain two-owner cross.
        let cross = book_of(&[
            single(1, 0, 0, Side::Buy, 4, SCALE),
            single(2, 1, 0, Side::Sell, 4, 0),
        ]);
        let cross_prices = prices(&[SCALE / 2, SCALE / 2]);
        let full = full_domain_with(GENERAL_CLEARING_POLICY_V1, 2, 2);
        let zero = zero_sentinel_domain(&full);
        let candidate = canonical_candidate(&zero, &cross, &cross_prices, 0, 0).unwrap();
        assert_verdict_identity(&mut work, &full, &cross, &candidate);

        // A portfolio order clearing against singles — the coordinate shape
        // Tier 2 exists to clear.
        let portfolio_book = book_of(&[
            portfolio(1, 0, Side::Buy, &[1, 1, 1], 2, SCALE),
            single(2, 1, 0, Side::Sell, 2, 0),
            single(3, 1, 1, Side::Sell, 2, 0),
            single(4, 2, 2, Side::Sell, 2, 0),
        ]);
        let portfolio_prices = prices(&[SCALE / 4, SCALE / 4, SCALE / 2]);
        let portfolio_full = full_domain_with(GENERAL_CLEARING_POLICY_V1, 3, 3);
        let portfolio_zero = zero_sentinel_domain(&portfolio_full);
        let portfolio_candidate =
            canonical_candidate(&portfolio_zero, &portfolio_book, &portfolio_prices, 0, 0)
                .unwrap();
        assert!(portfolio_candidate.fills.iter().any(|fill| *fill != 0));
        assert_verdict_identity(&mut work, &portfolio_full, &portfolio_book, &portfolio_candidate);

        // Refusal identity: a forged fill refuses both paths with the same
        // relation error.
        let witness = canonical_pairing(&zero, &cross, &candidate).unwrap();
        let mut forged = candidate;
        forged.fills[0] = forged.fills[0].wrapping_add(1);
        let streamed = drive_stream(&mut work, &zero, &cross, &forged, Some(&witness));
        let relation_error = streamed.unwrap_err();
        let raw = FullSubmittedCandidateV1::from_relation_candidate(&full, &forged).unwrap();
        let feed = FullCandidateFeedBindingV1 {
            candidate_id: raw.candidate_id,
            epoch_id: full.epoch_id,
            market_id: full.market_id,
            order_set_id: full.order_set_id,
            claimed_score: raw.claimed_score,
        };
        assert_eq!(
            verify_submitted_candidate(&full, &cross, &raw, &feed, Some(&witness)),
            Err(PolicyIdentityErrorV1::Relation(relation_error))
        );
    }

    /// The region-borrowing digest is the buffered digest, byte for byte, on
    /// the same coordinates — with and without a witness — and its portable
    /// fold equals the syscall wrapper's fold over the same slice sequence.
    /// This is what lets `CompleteClearWork` recompute the full-width tie
    /// identity by borrowing the verified feed account's stored regions
    /// instead of assembling a multi-kilobyte preimage on an SBF frame.
    #[test]
    fn the_region_digest_is_the_buffered_digest() {
        use crate::{
            full_relation_candidate_digest_from_regions, region_sha256_native,
            FULL_RELATION_CANDIDATE_DIGEST_DOMAIN,
        };
        use clutch_batch::relation_v1::LegRefV1;
        use clutch_batch::MAX_ORDERS;

        let book = book_of(&[
            single(1, 0, 0, Side::Buy, 4, SCALE),
            single(2, 1, 0, Side::Sell, 4, 0),
        ]);
        let vector = prices(&[SCALE / 2, SCALE / 2]);
        let full = full_domain_with(GENERAL_CLEARING_POLICY_V1, 2, 2);
        let zero = zero_sentinel_domain(&full);
        let candidate = canonical_candidate(&zero, &book, &vector, 0, 0).unwrap();
        let witness = canonical_pairing(&zero, &book, &candidate).unwrap();
        assert!(witness.len > 0);
        let raw = FullSubmittedCandidateV1::from_relation_candidate(&full, &candidate).unwrap();
        let (completed, _) =
            complete_submitted_candidate(&full, &book, &raw, Some(&witness)).unwrap();

        // The regions exactly as the candidate-feed account stores them.
        let mut fills = [0u8; MAX_ORDERS * 8];
        let mut i = 0usize;
        while i < MAX_ORDERS {
            fills[i * 8..(i + 1) * 8].copy_from_slice(&completed.fills[i].to_le_bytes());
            i += 1;
        }
        let mut slices = std::vec::Vec::new();
        let mut k = 0usize;
        while k < witness.len as usize {
            let slice = witness.slices[k];
            for leg in [slice.buy_ref, slice.sell_ref] {
                match leg {
                    LegRefV1::Order(index) => slices.extend_from_slice(&[0, index]),
                    LegRefV1::Split => slices.extend_from_slice(&[1, 0]),
                    LegRefV1::Merge => slices.extend_from_slice(&[2, 0]),
                }
            }
            slices.push(slice.outcome);
            slices.extend_from_slice(&slice.quantity.to_le_bytes());
            k += 1;
        }
        let domain_digest = full.digest().unwrap();

        // With the witness: regions == buffered.
        assert_eq!(
            full_relation_candidate_digest_from_regions(
                domain_digest,
                completed.candidate_id,
                &fills,
                completed.honored_aon_mask,
                Some((witness.len, &slices)),
            )
            .unwrap(),
            full_relation_candidate_digest(&full, &completed, Some(&witness)).unwrap()
        );
        // Without: same statement over the no-witness discriminant.
        assert_eq!(
            full_relation_candidate_digest_from_regions(
                domain_digest,
                completed.candidate_id,
                &fills,
                completed.honored_aon_mask,
                None,
            )
            .unwrap(),
            full_relation_candidate_digest(&full, &completed, None).unwrap()
        );
        // The syscall wrapper's fold over the identical slice sequence.
        let mask_bytes = completed.honored_aon_mask.to_le_bytes();
        let length_bytes = witness.len.to_le_bytes();
        assert_eq!(
            region_sha256_native(&[
                FULL_RELATION_CANDIDATE_DIGEST_DOMAIN,
                &domain_digest.0,
                &completed.candidate_id.0,
                &fills,
                &mask_bytes,
                &[1u8],
                &length_bytes,
                &slices,
            ]),
            full_relation_candidate_digest(&full, &completed, Some(&witness)).unwrap()
        );
        // Region-length lies refuse rather than commit to a different string.
        assert_eq!(
            full_relation_candidate_digest_from_regions(
                domain_digest,
                completed.candidate_id,
                &fills[..8],
                completed.honored_aon_mask,
                None,
            ),
            Err(PolicyIdentityErrorV1::InvalidDomain)
        );
        assert_eq!(
            full_relation_candidate_digest_from_regions(
                domain_digest,
                completed.candidate_id,
                &fills,
                completed.honored_aon_mask,
                Some((witness.len, &slices[..slices.len() - 1])),
            ),
            Err(PolicyIdentityErrorV1::InvalidDomain)
        );
    }

    /// The zero-sentinel soundness statement, executable.  (a) The four u64
    /// identity tags feed only the obsolete legacy digest: two arithmetic
    /// domains differing ONLY in those tags produce summaries identical in
    /// every field except the two legacy digest fields.  (b) Identity
    /// authority is full-width: flipping one byte of one 32-byte identity
    /// leaves the zero-sentinel streaming verdict bit-identical while moving
    /// `FullRelationDomainV1::digest`, the full relation-candidate digest, and
    /// the `FullScoreV1::total_order` tie.
    #[test]
    fn zero_sentinel_tags_bind_nothing_and_full_width_identity_binds_everything() {
        let book = book_of(&[
            single(1, 0, 0, Side::Buy, 4, SCALE),
            single(2, 1, 0, Side::Sell, 4, 0),
        ]);
        let vector = prices(&[SCALE / 2, SCALE / 2]);
        let full = full_domain_with(GENERAL_CLEARING_POLICY_V1, 2, 2);
        let zero = zero_sentinel_domain(&full);
        let candidate = canonical_candidate(&zero, &book, &vector, 0, 0).unwrap();
        let witness = canonical_pairing(&zero, &book, &candidate).unwrap();
        let mut work = Box::new(ClearWorkV1::new());

        // (a) Junk nonzero u64 tags change nothing but the legacy digests.
        let mut tagged = zero;
        tagged.market_id = 0xDEAD;
        tagged.book_id = 0xBEEF;
        tagged.policy_id = 77;
        tagged.order_set_id = 88;
        let under_zero = drive_stream(&mut work, &zero, &book, &candidate, Some(&witness)).unwrap();
        let under_tags =
            drive_stream(&mut work, &tagged, &book, &candidate, Some(&witness)).unwrap();
        assert_ne!(under_zero.score.digest, under_tags.score.digest);
        assert_ne!(under_zero.candidate_digest, under_tags.candidate_digest);
        let mut scrubbed_zero = under_zero;
        scrubbed_zero.score.digest = 0;
        scrubbed_zero.candidate_digest = 0;
        let mut scrubbed_tags = under_tags;
        scrubbed_tags.score.digest = 0;
        scrubbed_tags.candidate_digest = 0;
        assert_eq!(scrubbed_zero, scrubbed_tags);

        // (b) A one-byte market-identity flip is invisible to the zero-sentinel
        // stream (the derived arithmetic domain is unchanged) but moves every
        // full-width identity the selection layer actually compares.
        let mut other = full;
        other.market_id.0[13] ^= 0x80;
        assert_eq!(zero_sentinel_domain(&other), zero);
        assert_ne!(other.digest().unwrap(), full.digest().unwrap());

        let raw = FullSubmittedCandidateV1::from_relation_candidate(&full, &candidate).unwrap();
        let (completed, feed) =
            complete_submitted_candidate(&full, &book, &raw, Some(&witness)).unwrap();
        let verified =
            verify_submitted_candidate(&full, &book, &completed, &feed, Some(&witness)).unwrap();
        let other_raw =
            FullSubmittedCandidateV1::from_relation_candidate(&other, &candidate).unwrap();
        let (other_completed, other_feed) =
            complete_submitted_candidate(&other, &book, &other_raw, Some(&witness)).unwrap();
        let other_verified =
            verify_submitted_candidate(&other, &book, &other_completed, &other_feed, Some(&witness))
                .unwrap();
        assert_ne!(other_verified.score.digest, verified.score.digest);
        assert_ne!(
            other_verified.score.total_order(&verified.score),
            Ordering::Equal
        );
        // The economic verdict itself is identity-free on both planes.
        assert_eq!(other_verified.economics, verified.economics);
    }
}
