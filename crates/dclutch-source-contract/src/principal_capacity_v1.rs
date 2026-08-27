//! The Mango-lesson founding bound: principal admitted against a venue floor.
//!
//! `docs/research/CHAIN_STATE_SOURCES_2026_08.md` §6.5 proposes one founding
//! admission predicate for a Market whose resolution family reads third-party
//! on-chain state:
//!
//! ```text
//! total_principal <= κ · manipulation_cost_lower_bound
//! ```
//!
//! §5.5's Mango lesson is that the violated invariant was the *ratio* of
//! position size to venue depth, so this is the right shape of predicate even
//! before κ has a measured value.
//!
//! # Where it lives, and why nothing parallel was minted
//!
//! κ takes two `u32` coordinates out of [`super::SourceCapacityProfileV1`]'s
//! existing reserved tail. That record is already the Source's capacity
//! envelope, already carries the `Measured`/`Provisional` distinction, already
//! names its lifting plan in `envelope_basis_id`, and is already selected by
//! `SourceSpecV1::capacity_profile_id`. A provisional κ therefore inherits the
//! lifting-plan mechanism the crate already had rather than growing a second
//! one, and the profile's width does not move.
//!
//! The *floor* is a different kind of fact — it is a venue derivation, not a
//! policy — so it is its own immutable record, [`ManipulationFloorV1`]. It
//! exists to make one specific substitution refuse: a floor derived for a
//! deeper venue, a different Source, or a different collateral unit decodes
//! perfectly and is still refused, because the record binds all three and the
//! admission compares all three.
//!
//! # κ's conservative default and its lifting plan
//!
//! [`CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1`] / [`CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1`]
//! is one quarter, and it is conservative in a stateable way: an attacker who
//! forces the observed outcome captures at most the whole Hoard, so κ = 1 is
//! break-even against a perfectly extracting attacker and κ = 1/4 keeps a
//! fourfold margin against one. This is **provisional**. The lifting plan named
//! by [`PRINCIPAL_CAPACITY_LIFTING_PLAN_ID_V1`] is: measure the fraction an
//! attacker can actually realise on a given venue, then state a per-venue κ
//! with a `Measured` envelope.
//!
//! # Arithmetic
//!
//! The predicate is cross-multiplied, so no division and no float appears:
//! `principal · denominator <= numerator · floor`. The right-hand side is a
//! `u32` times a `u64` and cannot overflow `u128`; the left-hand side can, and
//! is refused when it does. That refusal is *exact* rather than conservative —
//! `overflow_is_exact` in
//! `formal/dclutch-semantics/DClutchSemantics/SourcePrincipalCapacityV1.lean`
//! shows the right-hand side stays below `2^96`, so a left-hand side that does
//! not fit `u128` is genuinely larger.

use super::{ContentId, Error, Result, SourceSpecV1, header, one, put, read_array, zero};

pub use super::generated_principal_capacity_v1::{
    BONDING_CURVE_FLOOR_DERIVATION_ID_V1, BONDING_CURVE_FLOOR_DERIVATION_PREIMAGE_V1,
    BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1, CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1,
    CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
    MANIPULATION_FLOOR_SCHEMA_RELEASE_PREIMAGE_V1, MANIPULATION_FLOOR_V1_BYTES,
    MANIPULATION_FLOOR_V1_MAGIC, MANIPULATION_FLOOR_V1_MAGIC_OFFSET,
    MANIPULATION_FLOOR_V1_SCHEMA_VERSION, MANIPULATION_FLOOR_V1_VERSION_OFFSET,
    PRINCIPAL_ADMISSION_CASES_V1, PRINCIPAL_CAPACITY_LIFTING_PLAN_ID_V1,
    PRINCIPAL_CAPACITY_LIFTING_PLAN_PREIMAGE_V1, SOURCE_CAPACITY_PRINCIPAL_DENOMINATOR_OFFSET_V1,
    SOURCE_CAPACITY_PRINCIPAL_NUMERATOR_OFFSET_V1,
    SOURCE_CAPACITY_PRINCIPAL_TAIL_RESERVED_BYTES_V1,
    SOURCE_CAPACITY_PRINCIPAL_TAIL_RESERVED_OFFSET_V1,
};

use super::generated_principal_capacity_v1::{
    MANIPULATION_FLOOR_V1_ADAPTER_CONFIG_OFFSET, MANIPULATION_FLOOR_V1_BASIS_OFFSET,
    MANIPULATION_FLOOR_V1_COLLATERAL_UNIT_OFFSET, MANIPULATION_FLOOR_V1_CURVE_DERIVED_TAG,
    MANIPULATION_FLOOR_V1_DERIVATION_RELEASE_OFFSET, MANIPULATION_FLOOR_V1_FLOOR_ATOMS_OFFSET,
    MANIPULATION_FLOOR_V1_OBSERVED_DEPTH_TAG, MANIPULATION_FLOOR_V1_RESERVED_BYTES,
    MANIPULATION_FLOOR_V1_RESERVED_OFFSET, MANIPULATION_FLOOR_V1_SOURCE_SPEC_OFFSET,
    MANIPULATION_FLOOR_V1_TAIL_RESERVED_BYTES, MANIPULATION_FLOOR_V1_TAIL_RESERVED_OFFSET,
};

/// One Lean-emitted founding-admission case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrincipalAdmissionCaseV1 {
    /// κ numerator.
    pub numerator: u32,
    /// κ denominator; zero means the capacity was not stated as a rational.
    pub denominator: u32,
    /// Venue manipulation floor, in the Market's collateral atoms.
    pub floor_atoms: u64,
    /// Total Hoard principal being founded.
    pub principal_atoms: u128,
    /// Whether §6.5 admits this founding.
    pub admitted: bool,
}

/// How a venue's manipulation floor was arrived at.
///
/// The two differ in whether the number falls when liquidity thins. A
/// curve-derived floor (§5.4) is fixed by the venue's own published parameters
/// and does not; an observed-depth floor (§5.2) does, which is exactly why §6.5
/// requires an observation-time refusal in addition to this founding bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ManipulationFloorBasis {
    /// Fixed by the venue's own published curve parameters.
    CurveDerived = MANIPULATION_FLOOR_V1_CURVE_DERIVED_TAG,
    /// Read from the venue's reserves at founding.
    ObservedDepth = MANIPULATION_FLOOR_V1_OBSERVED_DEPTH_TAG,
}

impl ManipulationFloorBasis {
    fn decode(byte: u8) -> Result<Self> {
        match byte {
            MANIPULATION_FLOOR_V1_CURVE_DERIVED_TAG => Ok(Self::CurveDerived),
            MANIPULATION_FLOOR_V1_OBSERVED_DEPTH_TAG => Ok(Self::ObservedDepth),
            _ => Err(Error::UnknownManipulationFloorBasis),
        }
    }

    const fn byte(self) -> u8 {
        self as u8
    }
}

/// A venue's derived cost floor for forcing the observation a Market resolves on.
///
/// The record carries no Market, no generation and no principal: it is the same
/// immutable derivation for every Market founded against that Source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManipulationFloorV1 {
    basis: ManipulationFloorBasis,
    source_spec_id: ContentId,
    adapter_config_id: ContentId,
    collateral_unit_id: ContentId,
    derivation_release_id: ContentId,
    floor_atoms: u64,
}

impl ManipulationFloorV1 {
    /// Construct one venue floor.
    ///
    /// A floor of zero is representable and means "found nothing against this
    /// Source"; the identities are not, because every binding this record
    /// exists to check would then be vacuous, and `ContentId` already refuses
    /// the all-zero sentinel.
    pub const fn new(
        basis: ManipulationFloorBasis,
        source_spec_id: ContentId,
        adapter_config_id: ContentId,
        collateral_unit_id: ContentId,
        derivation_release_id: ContentId,
        floor_atoms: u64,
    ) -> Self {
        Self {
            basis,
            source_spec_id,
            adapter_config_id,
            collateral_unit_id,
            derivation_release_id,
            floor_atoms,
        }
    }

    /// Hostile-decode one exact canonical floor preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(
            bytes,
            MANIPULATION_FLOOR_V1_BYTES,
            MANIPULATION_FLOOR_V1_MAGIC,
        )?;
        zero(
            bytes,
            MANIPULATION_FLOOR_V1_RESERVED_OFFSET,
            MANIPULATION_FLOOR_V1_RESERVED_BYTES,
        )?;
        zero(
            bytes,
            MANIPULATION_FLOOR_V1_TAIL_RESERVED_OFFSET,
            MANIPULATION_FLOOR_V1_TAIL_RESERVED_BYTES,
        )?;
        Ok(Self::new(
            ManipulationFloorBasis::decode(one(bytes, MANIPULATION_FLOOR_V1_BASIS_OFFSET)?)?,
            super::content(bytes, MANIPULATION_FLOOR_V1_SOURCE_SPEC_OFFSET)?,
            super::content(bytes, MANIPULATION_FLOOR_V1_ADAPTER_CONFIG_OFFSET)?,
            super::content(bytes, MANIPULATION_FLOOR_V1_COLLATERAL_UNIT_OFFSET)?,
            super::content(bytes, MANIPULATION_FLOOR_V1_DERIVATION_RELEASE_OFFSET)?,
            u64::from_le_bytes(read_array(bytes, MANIPULATION_FLOOR_V1_FLOOR_ATOMS_OFFSET)?),
        ))
    }

    /// Encode exact canonical floor bytes.
    pub fn to_bytes(self) -> [u8; MANIPULATION_FLOOR_V1_BYTES] {
        let mut out = super::base::<MANIPULATION_FLOOR_V1_BYTES>(MANIPULATION_FLOOR_V1_MAGIC);
        put(
            &mut out,
            MANIPULATION_FLOOR_V1_BASIS_OFFSET,
            &[self.basis.byte()],
        );
        put(
            &mut out,
            MANIPULATION_FLOOR_V1_SOURCE_SPEC_OFFSET,
            self.source_spec_id.as_bytes(),
        );
        put(
            &mut out,
            MANIPULATION_FLOOR_V1_ADAPTER_CONFIG_OFFSET,
            self.adapter_config_id.as_bytes(),
        );
        put(
            &mut out,
            MANIPULATION_FLOOR_V1_COLLATERAL_UNIT_OFFSET,
            self.collateral_unit_id.as_bytes(),
        );
        put(
            &mut out,
            MANIPULATION_FLOOR_V1_DERIVATION_RELEASE_OFFSET,
            self.derivation_release_id.as_bytes(),
        );
        put(
            &mut out,
            MANIPULATION_FLOOR_V1_FLOOR_ATOMS_OFFSET,
            &self.floor_atoms.to_le_bytes(),
        );
        out
    }

    /// Bind this floor to the authenticated Source and the Market's collateral.
    ///
    /// This is the check that refuses a *substituted* floor: one derived for a
    /// deeper venue, for a different Source, or in a different unit. All three
    /// identities are compared, and the venue identity compared is the
    /// `adapter_config_id` the Source itself names, so the floor cannot select
    /// its own venue.
    pub fn validate_binding(
        self,
        source_spec_id: ContentId,
        source: SourceSpecV1,
        market_collateral_unit_id: ContentId,
    ) -> Result<()> {
        if self.source_spec_id != source_spec_id
            || self.adapter_config_id != source.adapter_config_id()
            || self.collateral_unit_id != market_collateral_unit_id
        {
            return Err(Error::LinkageMismatch);
        }
        Ok(())
    }

    /// Return the derived floor, in the Market's collateral atoms.
    pub const fn floor_atoms(self) -> u64 {
        self.floor_atoms
    }

    /// Return how the floor was derived.
    pub const fn basis(self) -> ManipulationFloorBasis {
        self.basis
    }

    /// Return the Source this floor was derived for.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// Return the pinned venue configuration this floor was derived from.
    pub const fn adapter_config_id(self) -> ContentId {
        self.adapter_config_id
    }

    /// Return the collateral unit `floor_atoms` is denominated in.
    pub const fn collateral_unit_id(self) -> ContentId {
        self.collateral_unit_id
    }

    /// Return the release that names the derivation producing `floor_atoms`.
    pub const fn derivation_release_id(self) -> ContentId {
        self.derivation_release_id
    }
}

/// How a decoded capacity profile reads its κ tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrincipalCapacityV1 {
    /// This Source states no principal bound at all.
    ///
    /// Every capacity profile written before κ existed reads this way, because
    /// the tail was reserved zero. It is refused at admission rather than at
    /// decode, so an old record stays decodable and founds nothing.
    Unstated,
    /// A stated κ. The numerator may be zero; the denominator may not.
    Bounded {
        /// κ numerator.
        numerator: u32,
        /// κ denominator, nonzero.
        denominator: u32,
    },
}

impl PrincipalCapacityV1 {
    /// Read one κ tail. A numerator without a denominator is not a rational.
    pub const fn read(numerator: u32, denominator: u32) -> Result<Self> {
        if denominator == 0 {
            if numerator == 0 {
                Ok(Self::Unstated)
            } else {
                Err(Error::NonCanonicalCapacity)
            }
        } else {
            Ok(Self::Bounded {
                numerator,
                denominator,
            })
        }
    }

    /// The conservative default for a chain-state Source: κ = 1/4.
    pub const DEFAULT_CHAIN_STATE: Self = Self::Bounded {
        numerator: CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1,
        denominator: CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1,
    };

    /// Decide §6.5's founding predicate against one venue floor.
    ///
    /// Refuses an unstated capacity, a principal of zero, a stated bound of
    /// zero, and any principal above the bound. The overflow refusal is exact.
    pub fn admit(self, floor_atoms: u64, total_principal_atoms: u128) -> Result<()> {
        let Self::Bounded {
            numerator,
            denominator,
        } = self
        else {
            return Err(Error::PrincipalCapacityUnstated);
        };
        if denominator == 0 {
            return Err(Error::NonCanonicalCapacity);
        }
        if total_principal_atoms == 0 {
            return Err(Error::ZeroCapacity);
        }
        let bound = u128::from(numerator)
            .checked_mul(u128::from(floor_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        if bound == 0 {
            return Err(Error::PrincipalExceedsCapacity);
        }
        let scaled = total_principal_atoms
            .checked_mul(u128::from(denominator))
            .ok_or(Error::PrincipalExceedsCapacity)?;
        if scaled > bound {
            return Err(Error::PrincipalExceedsCapacity);
        }
        Ok(())
    }
}

/// The whole chain-state founding admission, as one decision.
///
/// Read the Source's κ, bind the venue floor to the authenticated Source and
/// the Market's collateral unit, then apply §6.5. This is the call a founding
/// route for a chain-state Source makes; a Source that never stated κ refuses
/// here rather than founding unbounded.
pub fn admit_founding_principal(
    capacity: PrincipalCapacityV1,
    floor: ManipulationFloorV1,
    source_spec_id: ContentId,
    source: SourceSpecV1,
    market_collateral_unit_id: ContentId,
    total_principal_atoms: u128,
) -> Result<()> {
    floor.validate_binding(source_spec_id, source, market_collateral_unit_id)?;
    capacity.admit(floor.floor_atoms(), total_principal_atoms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapacityEnvelope, SOURCE_CAPACITY_PROFILE_BYTES, SourceAccessProfile,
        SourceCapacityProfileV1, generated_principal_capacity_v1::MANIPULATION_FLOOR_V1_EXAMPLE,
        generated_principal_capacity_v1::MANIPULATION_FLOOR_V1_REFUSAL_CORPUS,
    };

    fn id(seed: u8) -> ContentId {
        ContentId::new([seed; 32]).expect("nonzero")
    }

    fn source() -> SourceSpecV1 {
        SourceSpecV1::new(
            id(1),
            id(2),
            id(3),
            SourceAccessProfile::RelayedObservationRecord,
            id(4),
            id(5),
        )
    }

    fn floor(atoms: u64) -> ManipulationFloorV1 {
        ManipulationFloorV1::new(
            ManipulationFloorBasis::CurveDerived,
            id(9),
            id(4),
            id(7),
            ContentId::new(BONDING_CURVE_FLOOR_DERIVATION_ID_V1).expect("derivation"),
            atoms,
        )
    }

    /// The Lean `Refusal.tag` mapping, in the order the decoder tests them.
    /// A refusal outside this closed set is not a floor refusal at all, and is
    /// reported as a tag the corpus can never carry.
    fn error_tag(error: Error) -> Option<u8> {
        match error {
            Error::InvalidLength => Some(0),
            Error::InvalidMagic => Some(1),
            Error::UnsupportedSchema => Some(2),
            Error::NonCanonicalReservedBytes => Some(3),
            Error::UnknownManipulationFloorBasis => Some(4),
            Error::ZeroContentId => Some(5),
            _ => None,
        }
    }

    #[test]
    fn the_lean_admission_corpus_decides_every_case_the_same_way() {
        for case in PRINCIPAL_ADMISSION_CASES_V1 {
            let observed = PrincipalCapacityV1::read(case.numerator, case.denominator)
                .and_then(|capacity| capacity.admit(case.floor_atoms, case.principal_atoms));
            assert_eq!(
                observed.is_ok(),
                case.admitted,
                "Lean and Rust disagreed on {case:?}"
            );
        }
    }

    #[test]
    fn the_lean_floor_refusal_corpus_refuses_for_the_same_named_reason() {
        for (bytes, expected) in MANIPULATION_FLOOR_V1_REFUSAL_CORPUS {
            let error = ManipulationFloorV1::decode(bytes).expect_err("hostile floor");
            assert_eq!(
                error_tag(error),
                Some(expected),
                "floor refused for the wrong reason: {error:?}"
            );
        }
        assert_eq!(
            ManipulationFloorV1::decode(&MANIPULATION_FLOOR_V1_EXAMPLE)
                .expect("Lean-admitted floor")
                .floor_atoms(),
            BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1
        );
    }

    #[test]
    fn exact_round_trip_over_both_bases() {
        for basis in [
            ManipulationFloorBasis::CurveDerived,
            ManipulationFloorBasis::ObservedDepth,
        ] {
            let value = ManipulationFloorV1::new(basis, id(9), id(4), id(7), id(8), 42);
            let bytes = value.to_bytes();
            assert_eq!(bytes.len(), MANIPULATION_FLOOR_V1_BYTES);
            assert_eq!(ManipulationFloorV1::decode(&bytes), Ok(value));
        }
    }

    #[test]
    fn a_principal_over_the_bound_refuses_at_founding() {
        let capacity = PrincipalCapacityV1::DEFAULT_CHAIN_STATE;
        let venue = floor(BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1);
        // 18.618074 SOL of unrecoverable loss, quartered.
        assert_eq!(
            admit_founding_principal(capacity, venue, id(9), source(), id(7), 4_654_518_500),
            Ok(())
        );
        assert_eq!(
            admit_founding_principal(capacity, venue, id(9), source(), id(7), 4_654_518_501),
            Err(Error::PrincipalExceedsCapacity)
        );
        // Sizing against the 85 SOL nominal curve cost instead of the floor
        // over-admits by roughly 4.6x -- §5.4's first correction, made to bite.
        assert_eq!(
            admit_founding_principal(
                capacity,
                floor(85_005_359_000),
                id(9),
                source(),
                id(7),
                21_251_339_750
            ),
            Ok(())
        );
        assert_eq!(
            admit_founding_principal(capacity, venue, id(9), source(), id(7), 21_251_339_750),
            Err(Error::PrincipalExceedsCapacity)
        );
    }

    #[test]
    fn a_bound_of_zero_refuses_everything() {
        let venue = floor(BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1);
        let zero_kappa = PrincipalCapacityV1::Bounded {
            numerator: 0,
            denominator: 4,
        };
        for principal in [1u128, 1_000, u128::MAX] {
            assert_eq!(
                zero_kappa.admit(venue.floor_atoms(), principal),
                Err(Error::PrincipalExceedsCapacity)
            );
            assert_eq!(
                PrincipalCapacityV1::DEFAULT_CHAIN_STATE.admit(0, principal),
                Err(Error::PrincipalExceedsCapacity)
            );
        }
        // And a Source that never stated κ founds nothing at all.
        assert_eq!(
            PrincipalCapacityV1::read(0, 0),
            Ok(PrincipalCapacityV1::Unstated)
        );
        assert_eq!(
            PrincipalCapacityV1::Unstated.admit(venue.floor_atoms(), 1),
            Err(Error::PrincipalCapacityUnstated)
        );
        assert_eq!(
            PrincipalCapacityV1::read(1, 0),
            Err(Error::NonCanonicalCapacity)
        );
    }

    #[test]
    fn a_substituted_venue_floor_refuses_even_though_it_decodes() {
        let capacity = PrincipalCapacityV1::DEFAULT_CHAIN_STATE;
        let principal = 1_000u128;
        // A floor derived for a different, deeper venue: same shape, same
        // Source, same unit, and an adapter configuration the Source does not
        // name. It would admit 250x more principal if it were accepted.
        let deeper = ManipulationFloorV1::new(
            ManipulationFloorBasis::CurveDerived,
            id(9),
            id(44),
            id(7),
            id(8),
            u64::MAX,
        );
        assert!(ManipulationFloorV1::decode(&deeper.to_bytes()).is_ok());
        assert_eq!(
            admit_founding_principal(capacity, deeper, id(9), source(), id(7), principal),
            Err(Error::LinkageMismatch)
        );
        // A floor derived for a different Source on the same venue.
        let other_source = ManipulationFloorV1::new(
            ManipulationFloorBasis::CurveDerived,
            id(99),
            id(4),
            id(7),
            id(8),
            u64::MAX,
        );
        assert_eq!(
            admit_founding_principal(capacity, other_source, id(9), source(), id(7), principal),
            Err(Error::LinkageMismatch)
        );
        // A floor denominated in a unit the Market does not hold. Without this
        // the comparison would be atoms of one asset against atoms of another.
        let other_unit = ManipulationFloorV1::new(
            ManipulationFloorBasis::CurveDerived,
            id(9),
            id(4),
            id(77),
            id(8),
            u64::MAX,
        );
        assert_eq!(
            admit_founding_principal(capacity, other_unit, id(9), source(), id(7), principal),
            Err(Error::LinkageMismatch)
        );
        // The correctly bound floor with the same enormous number is admitted:
        // the refusals above are about binding, not about the value.
        let bound = ManipulationFloorV1::new(
            ManipulationFloorBasis::CurveDerived,
            id(9),
            id(4),
            id(7),
            id(8),
            u64::MAX,
        );
        assert_eq!(
            admit_founding_principal(capacity, bound, id(9), source(), id(7), principal),
            Ok(())
        );
    }

    #[test]
    fn the_overflow_refusal_never_admits_and_never_wraps() {
        let capacity = PrincipalCapacityV1::Bounded {
            numerator: u32::MAX,
            denominator: u32::MAX,
        };
        assert_eq!(
            capacity.admit(u64::MAX, u128::MAX),
            Err(Error::PrincipalExceedsCapacity)
        );
        // The widest admissible founding: κ = 1 against the widest floor.
        assert_eq!(
            PrincipalCapacityV1::Bounded {
                numerator: 1,
                denominator: 1,
            }
            .admit(u64::MAX, u128::from(u64::MAX)),
            Ok(())
        );
        assert_eq!(
            PrincipalCapacityV1::Bounded {
                numerator: 1,
                denominator: 1,
            }
            .admit(u64::MAX, u128::from(u64::MAX) + 1),
            Err(Error::PrincipalExceedsCapacity)
        );
    }

    #[test]
    fn kappa_rides_the_capacity_profiles_former_reserved_tail_without_moving_it() {
        let unstated = SourceCapacityProfileV1::new(
            CapacityEnvelope::Provisional,
            8,
            2,
            id(1),
            ContentId::new(PRINCIPAL_CAPACITY_LIFTING_PLAN_ID_V1).expect("plan"),
            4096,
            4,
        )
        .expect("profile");
        assert_eq!(
            unstated.principal_capacity(),
            Ok(PrincipalCapacityV1::Unstated)
        );
        let bounded = unstated
            .bounding_principal(
                CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1,
                CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1,
            )
            .expect("kappa");
        let bytes = bounded.to_bytes();
        assert_eq!(bytes.len(), SOURCE_CAPACITY_PROFILE_BYTES);
        assert_eq!(
            bytes.get(
                SOURCE_CAPACITY_PRINCIPAL_NUMERATOR_OFFSET_V1
                    ..SOURCE_CAPACITY_PRINCIPAL_NUMERATOR_OFFSET_V1 + 4
            ),
            Some(
                CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1
                    .to_le_bytes()
                    .as_slice()
            )
        );
        assert_eq!(SourceCapacityProfileV1::decode(&bytes), Ok(bounded));
        assert_eq!(
            bounded.principal_capacity(),
            Ok(PrincipalCapacityV1::DEFAULT_CHAIN_STATE)
        );
        // The surviving reserved span is still enforced, and a numerator
        // without a denominator is still not a rational.
        for offset in SOURCE_CAPACITY_PRINCIPAL_TAIL_RESERVED_OFFSET_V1
            ..SOURCE_CAPACITY_PRINCIPAL_TAIL_RESERVED_OFFSET_V1
                + SOURCE_CAPACITY_PRINCIPAL_TAIL_RESERVED_BYTES_V1
        {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("reserved byte") = 1;
            assert_eq!(
                SourceCapacityProfileV1::decode(&hostile),
                Err(Error::NonCanonicalReservedBytes)
            );
        }
        let mut hostile = bytes;
        hostile
            .get_mut(
                SOURCE_CAPACITY_PRINCIPAL_DENOMINATOR_OFFSET_V1
                    ..SOURCE_CAPACITY_PRINCIPAL_DENOMINATOR_OFFSET_V1 + 4,
            )
            .expect("denominator")
            .fill(0);
        assert_eq!(
            SourceCapacityProfileV1::decode(&hostile),
            Err(Error::NonCanonicalCapacity)
        );
        assert_eq!(
            unstated.bounding_principal(1, 0),
            Err(Error::NonCanonicalCapacity)
        );
    }
}
