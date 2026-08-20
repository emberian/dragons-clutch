#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Canonical batch-policy bytes and the full-width relation identity bridge.
//!
//! The original host relation predates the Solana account plane.  Its
//! [`RelationDomainV1`] therefore carries five `u64` tags where persisted state
//! carries five independent 32-byte identities.  No injective mapping from a
//! 256-bit set into a 64-bit set exists.  This module does not invent one.
//! Instead it:
//!
//! * gives every [`FrozenPolicyV1`] selector and parameter one exact 64-byte
//!   canonical preimage and a domain-separated SHA-256 identity;
//! * carries market, book, epoch, policy, and order-set identities at their full
//!   32-byte width in [`FullRelationDomainV1`];
//! * verifies the unchanged V0--V8 economic relation through an explicitly
//!   identity-free arithmetic projection; and
//! * replaces the old non-cryptographic `u128` V9 tie digest with a full-width
//!   relation-candidate digest that commits to the full domain, fills, and the
//!   policy-selected pairing witness.
//!
//! A success from [`verify_submitted_candidate`] means **valid submitted
//! candidate** only.  Candidate-window closure, comparison over the complete
//! submitted set, and the once-only `SELECTED` transition remain a separate
//! authority.  Nothing in this module can select a candidate or mutate an
//! epoch.

use core::cmp::Ordering;

use crate::hasher::{Chosen, Sha256Like};

use clutch_batch::relation_v1::{
    self, AllocationPolicyV1, AonPolicyV1, BookV1, CandidateV1, ErrorV1, FeeBaseV1, FrozenPolicyV1,
    LegRefV1, PairingWitnessPolicyV1, PairingWitnessV1, PortfolioLotPolicyV1, RelationDomainV1,
    ResidualSettlementV1, RoundingBoundaryV1, ScorePolicyV1, SelfCrossPolicyV1, SummaryV1,
    TransferPhaseV1, FEE_BPS_DENOMINATOR, MAX_OUTCOMES, MAX_PRICE_SCALE, RELATION_VERSION_V1,
};
use clutch_batch::{DustPolicy, MAX_ORDERS};

/// SHA-256 in the shape each target can afford.
///
/// Every identity in this crate is SHA-256 over one canonical, domain-separated
/// preimage.  Off-chain that preimage is folded by the portable `sha2`
/// implementation, unchanged.  Under `cfg(target_os = "solana")` `sha2` is not
/// in the dependency graph at all: its fully unrolled `compress256` was 53,952
/// bytes of the deployable program ELF — its single largest symbol — and every
/// 64-byte block it folded was compute the runtime charged for, while that same
/// runtime already exposes SHA-256 as one syscall.
///
/// The syscall wrapper consumes a whole preimage at once and has no incremental
/// form, so [`Native`] buffers the identical `update` sequence into a frame of
/// exactly the width its call site declares and hashes it in one call.  The two
/// forms therefore commit to the same byte string and produce the same value;
/// only the cost differs.  Every hashing function below is written once, generic
/// over [`Sha256Like`], and instantiated at both — which is what lets the tests
/// require byte-identical output rather than assume it.
pub(crate) mod hasher {
    /// The incremental surface every identity in this crate is written against.
    ///
    /// This is deliberately not `sha2::Digest`: the on-chain form is not a
    /// compression function at all, it is a preimage buffer in front of a
    /// syscall, and only these three operations have a meaning on both.
    pub(crate) trait Sha256Like {
        /// Start an empty preimage.
        fn new() -> Self;
        /// Append bytes to the preimage.
        fn update(&mut self, data: impl AsRef<[u8]>);
        /// Consume the preimage and return its SHA-256.
        fn finalize(self) -> [u8; 32];
    }

    /// Portable `sha2` fold.  `N` is the declared preimage width, which this
    /// form does not need and carries only so both share one signature.
    #[cfg(not(target_os = "solana"))]
    #[derive(Clone, Debug, Default)]
    pub(crate) struct Portable<const N: usize>(sha2::Sha256);

    #[cfg(not(target_os = "solana"))]
    impl<const N: usize> Sha256Like for Portable<N> {
        fn new() -> Self {
            Self(<sha2::Sha256 as sha2::Digest>::new())
        }
        fn update(&mut self, data: impl AsRef<[u8]>) {
            sha2::Digest::update(&mut self.0, data);
        }
        fn finalize(self) -> [u8; 32] {
            sha2::Digest::finalize(self.0).into()
        }
    }

    /// Solana's native SHA-256 syscall over a buffered preimage of exactly `N`
    /// bytes.
    ///
    /// Built on SBF and in host unit tests.  The host build resolves the
    /// wrapper's `sha2` backend, which is what lets a test hash the same
    /// sequence both ways in one process and compare the bytes.
    #[cfg(any(target_os = "solana", test))]
    #[derive(Clone, Debug)]
    pub(crate) struct Native<const N: usize> {
        preimage: [u8; N],
        at: usize,
    }

    #[cfg(any(target_os = "solana", test))]
    impl<const N: usize> Sha256Like for Native<N> {
        fn new() -> Self {
            Self {
                preimage: [0; N],
                at: 0,
            }
        }
        /// A call site whose declared `N` is smaller than the preimage it
        /// actually writes halts here on the slice bound.  That is deliberate:
        /// a short buffer must never silently commit to a truncated preimage,
        /// because a truncated preimage is a different — and forgeable —
        /// identity.  The equivalence tests write every site at its widest
        /// shape, so an undersized `N` fails on the host before it can ship.
        fn update(&mut self, data: impl AsRef<[u8]>) {
            let data = data.as_ref();
            let end = self.at + data.len();
            self.preimage[self.at..end].copy_from_slice(data);
            self.at = end;
        }
        fn finalize(self) -> [u8; 32] {
            solana_sha256_hasher::hashv(&[&self.preimage[..self.at]]).to_bytes()
        }
    }

    /// The form this target actually ships.
    #[cfg(not(target_os = "solana"))]
    pub(crate) type Chosen<const N: usize> = Portable<N>;
    /// The form this target actually ships.
    #[cfg(target_os = "solana")]
    pub(crate) type Chosen<const N: usize> = Native<N>;
}

/// Staged, bounded, donation-safe lifecycle model for the direct profile.
/// It allocates no live Solana tags or instructions.
pub mod direct_lifecycle_v3;
/// Fixed-capacity, full-width candidate-window model for the bounded direct
/// selection profile.  It is an offline account-body and transition model;
/// it does not allocate live Solana tags or instructions.
pub mod direct_window_v1;
/// The PROPOSED Tier 2 general portfolio-clearing policy profile and the
/// streaming/full-width verdict-identity gate (T2-5).  It allocates no live
/// Solana tags, accounts, or instructions.
pub mod general_clearing_v1;

/// Exact size of a canonical batch-policy artifact.
pub const BATCH_POLICY_BYTES: usize = 64;
/// Exact size of the canonical full-width relation-domain preimage.
pub const FULL_RELATION_DOMAIN_BYTES: usize = 284;
/// Batch-policy magic: ASCII `DCBATP1` followed by one zero byte.
pub const BATCH_POLICY_MAGIC: [u8; 8] = *b"DCBATP1\0";
/// Full relation-domain magic: ASCII `DCBRDV1` followed by one zero byte.
pub const FULL_RELATION_DOMAIN_MAGIC: [u8; 8] = *b"DCBRDV1\0";
/// The only canonical batch-policy schema understood here.
pub const BATCH_POLICY_SCHEMA_V1: u16 = 1;
/// The only canonical full-domain schema understood here.
pub const FULL_RELATION_DOMAIN_SCHEMA_V1: u16 = 1;
/// Reserved bytes at the tail of the policy artifact.
pub const BATCH_POLICY_RESERVED_BYTES: usize = 36;
/// Reserved bytes at the tail of the full-domain preimage.
pub const FULL_RELATION_DOMAIN_RESERVED_BYTES: usize = 16;
/// Domain separator for the immutable batch-policy identity.
pub const BATCH_POLICY_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/batch-policy/v1\0";
/// Domain separator for the full-width relation-domain identity.
pub const FULL_RELATION_DOMAIN_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/full-relation-domain/v1\0";
/// Domain separator for the full-width relation-candidate tie identity.
pub const FULL_RELATION_CANDIDATE_DIGEST_DOMAIN: &[u8] =
    b"dragons-clutch/full-relation-candidate/v1\0";
/// Existing account-plane candidate identity domain.
pub const ACCOUNT_CANDIDATE_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/candidate/v1";

const POLICY_FLAGS_V1: u16 = 0;
const FULL_DOMAIN_FLAGS_V1: u16 = 0;

const _: () = assert!(BATCH_POLICY_BYTES == 8 + 2 + 2 + 10 + 1 + 1 + 4 + 36);
const _: () = assert!(
    FULL_RELATION_DOMAIN_BYTES == 8 + 2 + 2 + 4 + (5 * 32) + 8 + 1 + 1 + 2 + 8 + 8 + 64 + 16
);

/// A full-width identity.  The byte ordering is also the canonical ascending
/// digest ordering used as the last score tie-break.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Identity32V1(pub [u8; 32]);

impl Identity32V1 {
    /// The all-zero sentinel.  Frozen domain identities refuse it.
    pub const ZERO: Self = Self([0; 32]);

    /// Whether all 32 bytes are zero.
    pub const fn is_zero(self) -> bool {
        let mut i = 0usize;
        while i < self.0.len() {
            if self.0[i] != 0 {
                return false;
            }
            i += 1;
        }
        true
    }
}

/// Refusals owned by the policy/identity seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyIdentityErrorV1 {
    /// The byte slice is shorter than the one exact codec length.
    Truncated,
    /// The byte slice is longer than the one exact codec length.
    TrailingBytes,
    /// The fixed magic/tag is not the V1 magic.
    WrongMagic,
    /// A schema or relation version is not the implemented version.
    WrongVersion,
    /// A flag, selector, or fee discriminant is unknown.
    InvalidEnum,
    /// Padding, a reserved byte, or an inactive parameter is nonzero.
    NonCanonicalPadding,
    /// A required persisted identity is the zero sentinel.
    ZeroIdentity,
    /// The policy bytes and policy identity, or candidate/feed bindings, differ.
    MismatchedBinding,
    /// A domain shape or bound is invalid.
    InvalidDomain,
    /// The submitted candidate's account-plane identity is not canonical.
    CandidateIdentityMismatch,
    /// A claimed full-width score differs from the recomputed score.
    ClaimedScoreMismatch,
    /// The underlying V0--V8 relation refused.
    Relation(ErrorV1),
}

impl From<ErrorV1> for PolicyIdentityErrorV1 {
    fn from(value: ErrorV1) -> Self {
        Self::Relation(value)
    }
}

/// Exact preimage width of the batch-policy identity: domain plus one
/// [`BATCH_POLICY_BYTES`] canonical artifact.
const BATCH_POLICY_PREIMAGE: usize = BATCH_POLICY_DIGEST_DOMAIN.len() + BATCH_POLICY_BYTES;
/// Exact preimage width of the full-relation-domain identity.
const FULL_RELATION_DOMAIN_PREIMAGE: usize =
    FULL_RELATION_DOMAIN_DIGEST_DOMAIN.len() + FULL_RELATION_DOMAIN_BYTES;
/// Exact preimage width of the account-plane candidate identity: domain, two
/// 32-byte parents, the order length and outcome count, the full simplex, and
/// the virtual pair and honoured-AON mask.
const ACCOUNT_CANDIDATE_PREIMAGE: usize =
    ACCOUNT_CANDIDATE_DIGEST_DOMAIN.len() + 32 + 32 + 1 + 1 + (MAX_OUTCOMES * 8) + 8 + 8 + 8;
/// Widest preimage of the full relation-candidate identity: domain, the domain
/// digest, the candidate identity, every fill, the honoured-AON mask, the
/// witness discriminant, and — when a witness is present — its length and every
/// slice at [`relation_v1::MAX_SLICES`].
///
/// This one is several kilobytes because an explicit pairing witness is
/// unbounded in practice, and it is why [`full_relation_candidate_digest`] is
/// off-chain code.  The bound is the *type's*, not any caller's, so it cannot be
/// tightened without turning a long witness into a refusal — which would be a
/// semantic change, not an optimisation.  The direct profile carries no witness
/// at all and folds the fixed 615-byte
/// [`direct_window_v1`] preimage instead, which is the shape the program
/// actually reaches.
const FULL_RELATION_CANDIDATE_PREIMAGE: usize = FULL_RELATION_CANDIDATE_DIGEST_DOMAIN.len()
    + 32
    + 32
    + (MAX_ORDERS * 8)
    + 8
    + 1
    + 2
    + (relation_v1::MAX_SLICES * (2 + 2 + 1 + 8));

fn sha256<H: Sha256Like>(domain: &[u8], parts: &[&[u8]]) -> Identity32V1 {
    let mut h = H::new();
    h.update(domain);
    let mut i = 0usize;
    while i < parts.len() {
        h.update(parts[i]);
        i += 1;
    }
    Identity32V1(h.finalize())
}

fn allocation_byte(value: AllocationPolicyV1) -> u8 {
    match value {
        AllocationPolicyV1::PricePriorityMarginalProRata => 0,
        AllocationPolicyV1::FullProRata => 1,
    }
}

fn allocation_from(byte: u8) -> Result<AllocationPolicyV1, PolicyIdentityErrorV1> {
    match byte {
        0 => Ok(AllocationPolicyV1::PricePriorityMarginalProRata),
        1 => Ok(AllocationPolicyV1::FullProRata),
        _ => Err(PolicyIdentityErrorV1::InvalidEnum),
    }
}

fn self_cross_byte(value: SelfCrossPolicyV1) -> u8 {
    match value {
        SelfCrossPolicyV1::RefuseOverlap => 0,
        SelfCrossPolicyV1::NetAtAdmission => 1,
        SelfCrossPolicyV1::AllowGateAtPairing => 2,
    }
}

fn self_cross_from(byte: u8) -> Result<SelfCrossPolicyV1, PolicyIdentityErrorV1> {
    match byte {
        0 => Ok(SelfCrossPolicyV1::RefuseOverlap),
        1 => Ok(SelfCrossPolicyV1::NetAtAdmission),
        2 => Ok(SelfCrossPolicyV1::AllowGateAtPairing),
        _ => Err(PolicyIdentityErrorV1::InvalidEnum),
    }
}

fn aon_byte(value: AonPolicyV1) -> u8 {
    match value {
        AonPolicyV1::RefuseAdmission => 0,
        AonPolicyV1::WitnessedHonoredMask => 1,
        AonPolicyV1::FullSizeCounting => 2,
    }
}

fn aon_from(byte: u8) -> Result<AonPolicyV1, PolicyIdentityErrorV1> {
    match byte {
        0 => Ok(AonPolicyV1::RefuseAdmission),
        1 => Ok(AonPolicyV1::WitnessedHonoredMask),
        2 => Ok(AonPolicyV1::FullSizeCounting),
        _ => Err(PolicyIdentityErrorV1::InvalidEnum),
    }
}

fn rounding_byte(value: RoundingBoundaryV1) -> u8 {
    match value {
        RoundingBoundaryV1::None => 0,
        RoundingBoundaryV1::TerminalOwnerFloor => 1,
        RoundingBoundaryV1::ReceiptFloor => 2,
    }
}

fn rounding_from(byte: u8) -> Result<RoundingBoundaryV1, PolicyIdentityErrorV1> {
    match byte {
        0 => Ok(RoundingBoundaryV1::None),
        1 => Ok(RoundingBoundaryV1::TerminalOwnerFloor),
        2 => Ok(RoundingBoundaryV1::ReceiptFloor),
        _ => Err(PolicyIdentityErrorV1::InvalidEnum),
    }
}

fn residual_byte(value: ResidualSettlementV1) -> u8 {
    match value {
        ResidualSettlementV1::FullPairOnly => 0,
        ResidualSettlementV1::CumulativePairCanonical => 1,
        ResidualSettlementV1::CumulativePairFree => 2,
        ResidualSettlementV1::UniqueSliceReceipts => 3,
    }
}

fn residual_from(byte: u8) -> Result<ResidualSettlementV1, PolicyIdentityErrorV1> {
    match byte {
        0 => Ok(ResidualSettlementV1::FullPairOnly),
        1 => Ok(ResidualSettlementV1::CumulativePairCanonical),
        2 => Ok(ResidualSettlementV1::CumulativePairFree),
        3 => Ok(ResidualSettlementV1::UniqueSliceReceipts),
        _ => Err(PolicyIdentityErrorV1::InvalidEnum),
    }
}

fn transfer_byte(value: TransferPhaseV1) -> u8 {
    match value {
        TransferPhaseV1::ActiveOnly => 0,
        TransferPhaseV1::ActiveOrResolved => 1,
    }
}

fn transfer_from(byte: u8) -> Result<TransferPhaseV1, PolicyIdentityErrorV1> {
    match byte {
        0 => Ok(TransferPhaseV1::ActiveOnly),
        1 => Ok(TransferPhaseV1::ActiveOrResolved),
        _ => Err(PolicyIdentityErrorV1::InvalidEnum),
    }
}

fn portfolio_byte(value: PortfolioLotPolicyV1) -> u8 {
    match value {
        PortfolioLotPolicyV1::StrictWholeOrder => 0,
        PortfolioLotPolicyV1::MarginalProRataLots => 1,
    }
}

fn portfolio_from(byte: u8) -> Result<PortfolioLotPolicyV1, PolicyIdentityErrorV1> {
    match byte {
        0 => Ok(PortfolioLotPolicyV1::StrictWholeOrder),
        1 => Ok(PortfolioLotPolicyV1::MarginalProRataLots),
        _ => Err(PolicyIdentityErrorV1::InvalidEnum),
    }
}

fn pairing_byte(value: PairingWitnessPolicyV1) -> u8 {
    match value {
        PairingWitnessPolicyV1::RecomputedConstructor => 0,
        PairingWitnessPolicyV1::ExplicitSlices => 1,
    }
}

fn pairing_from(byte: u8) -> Result<PairingWitnessPolicyV1, PolicyIdentityErrorV1> {
    match byte {
        0 => Ok(PairingWitnessPolicyV1::RecomputedConstructor),
        1 => Ok(PairingWitnessPolicyV1::ExplicitSlices),
        _ => Err(PolicyIdentityErrorV1::InvalidEnum),
    }
}

fn dust_byte(value: DustPolicy) -> u8 {
    match value {
        DustPolicy::AssignCanonical => 0,
        DustPolicy::Reject => 1,
    }
}

fn dust_from(byte: u8) -> Result<DustPolicy, PolicyIdentityErrorV1> {
    match byte {
        0 => Ok(DustPolicy::AssignCanonical),
        1 => Ok(DustPolicy::Reject),
        _ => Err(PolicyIdentityErrorV1::InvalidEnum),
    }
}

fn score_byte(value: ScorePolicyV1) -> u8 {
    match value {
        ScorePolicyV1::LexicographicDispersionV1 => 0,
    }
}

fn score_from(byte: u8) -> Result<ScorePolicyV1, PolicyIdentityErrorV1> {
    match byte {
        0 => Ok(ScorePolicyV1::LexicographicDispersionV1),
        _ => Err(PolicyIdentityErrorV1::InvalidEnum),
    }
}

fn validate_registered_policy(policy: &FrozenPolicyV1) -> Result<(), PolicyIdentityErrorV1> {
    if let FeeBaseV1::FlatNotional { bps } = policy.fee_base {
        if bps as u64 > FEE_BPS_DENOMINATOR {
            return Err(PolicyIdentityErrorV1::InvalidEnum);
        }
    }
    Ok(())
}

/// Encode every registered policy family and its fee parameter into exactly
/// [`BATCH_POLICY_BYTES`] bytes.  Registered-but-unimplemented variants still
/// have identities; [`FullRelationDomainV1::validate`] separately refuses them
/// for relation execution.
pub fn encode_batch_policy(
    policy: &FrozenPolicyV1,
    out: &mut [u8],
) -> Result<usize, PolicyIdentityErrorV1> {
    validate_registered_policy(policy)?;
    if out.len() < BATCH_POLICY_BYTES {
        return Err(PolicyIdentityErrorV1::Truncated);
    }
    let mut at = 0usize;
    out[at..at + 8].copy_from_slice(&BATCH_POLICY_MAGIC);
    at += 8;
    out[at..at + 2].copy_from_slice(&BATCH_POLICY_SCHEMA_V1.to_le_bytes());
    at += 2;
    out[at..at + 2].copy_from_slice(&POLICY_FLAGS_V1.to_le_bytes());
    at += 2;
    let selectors = [
        allocation_byte(policy.allocation),
        self_cross_byte(policy.self_cross),
        aon_byte(policy.aon),
        rounding_byte(policy.rounding),
        residual_byte(policy.residual_settlement),
        transfer_byte(policy.transfer_phase),
        portfolio_byte(policy.portfolio_lots),
        pairing_byte(policy.pairing_witness),
        dust_byte(policy.dust),
        score_byte(policy.score),
    ];
    out[at..at + selectors.len()].copy_from_slice(&selectors);
    at += selectors.len();
    let (fee_tag, fee_bps) = match policy.fee_base {
        FeeBaseV1::None => (0u8, 0u32),
        FeeBaseV1::FlatNotional { bps } => (1u8, bps),
    };
    out[at] = fee_tag;
    at += 1;
    out[at] = 0;
    at += 1;
    out[at..at + 4].copy_from_slice(&fee_bps.to_le_bytes());
    at += 4;
    out[at..at + BATCH_POLICY_RESERVED_BYTES].fill(0);
    at += BATCH_POLICY_RESERVED_BYTES;
    if at != BATCH_POLICY_BYTES {
        return Err(PolicyIdentityErrorV1::InvalidDomain);
    }
    Ok(at)
}

/// Return the exact canonical policy byte image.
pub fn canonical_batch_policy_bytes(
    policy: &FrozenPolicyV1,
) -> Result<[u8; BATCH_POLICY_BYTES], PolicyIdentityErrorV1> {
    let mut out = [0; BATCH_POLICY_BYTES];
    encode_batch_policy(policy, &mut out)?;
    Ok(out)
}

/// Decode exactly one canonical batch-policy artifact.
pub fn decode_batch_policy(input: &[u8]) -> Result<FrozenPolicyV1, PolicyIdentityErrorV1> {
    if input.len() < BATCH_POLICY_BYTES {
        return Err(PolicyIdentityErrorV1::Truncated);
    }
    if input.len() > BATCH_POLICY_BYTES {
        return Err(PolicyIdentityErrorV1::TrailingBytes);
    }
    if input[..8] != BATCH_POLICY_MAGIC {
        return Err(PolicyIdentityErrorV1::WrongMagic);
    }
    let schema = u16::from_le_bytes([input[8], input[9]]);
    if schema != BATCH_POLICY_SCHEMA_V1 {
        return Err(PolicyIdentityErrorV1::WrongVersion);
    }
    if u16::from_le_bytes([input[10], input[11]]) != POLICY_FLAGS_V1 {
        return Err(PolicyIdentityErrorV1::InvalidEnum);
    }
    if input[23] != 0
        || input[BATCH_POLICY_BYTES - BATCH_POLICY_RESERVED_BYTES..]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(PolicyIdentityErrorV1::NonCanonicalPadding);
    }
    let fee_bps = u32::from_le_bytes([input[24], input[25], input[26], input[27]]);
    let fee_base = match input[22] {
        0 if fee_bps == 0 => FeeBaseV1::None,
        0 => return Err(PolicyIdentityErrorV1::NonCanonicalPadding),
        1 if fee_bps as u64 <= FEE_BPS_DENOMINATOR => FeeBaseV1::FlatNotional { bps: fee_bps },
        1 => return Err(PolicyIdentityErrorV1::InvalidEnum),
        _ => return Err(PolicyIdentityErrorV1::InvalidEnum),
    };
    let value = FrozenPolicyV1 {
        allocation: allocation_from(input[12])?,
        self_cross: self_cross_from(input[13])?,
        aon: aon_from(input[14])?,
        rounding: rounding_from(input[15])?,
        residual_settlement: residual_from(input[16])?,
        transfer_phase: transfer_from(input[17])?,
        portfolio_lots: portfolio_from(input[18])?,
        pairing_witness: pairing_from(input[19])?,
        dust: dust_from(input[20])?,
        score: score_from(input[21])?,
        fee_base,
    };
    // Re-encoding is the final canonicality oracle.
    if canonical_batch_policy_bytes(&value)? != input {
        return Err(PolicyIdentityErrorV1::NonCanonicalPadding);
    }
    Ok(value)
}

/// Compute the canonical immutable identity of one registered policy.
pub fn batch_policy_digest(policy: &FrozenPolicyV1) -> Result<Identity32V1, PolicyIdentityErrorV1> {
    let bytes = canonical_batch_policy_bytes(policy)?;
    Ok(sha256::<Chosen<BATCH_POLICY_PREIMAGE>>(
        BATCH_POLICY_DIGEST_DOMAIN,
        &[&bytes],
    ))
}

/// A relation domain with no lossy identity projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullRelationDomainV1 {
    /// Must equal [`RELATION_VERSION_V1`].
    pub relation_version: u32,
    /// Full account-plane Market identity.
    pub market_id: Identity32V1,
    /// Full frozen Book identity.
    pub book_id: Identity32V1,
    /// Full canonical Epoch identity.
    pub epoch_id: Identity32V1,
    /// Full immutable BatchPolicy identity.
    pub policy_id: Identity32V1,
    /// Full frozen order-set identity.
    pub order_set_id: Identity32V1,
    /// Epoch index used by order expiry admission.
    pub epoch_index: u64,
    /// Active outcomes.
    pub outcome_count: u8,
    /// Distinct owner-tag cardinality.
    pub owner_count: u16,
    /// Exact simplex scale.
    pub price_scale: u64,
    /// Frozen largest-remainder permutation seed.
    pub remainder_seed: u64,
    /// Exact policy preimage, checked against `policy_id`.
    pub policy: FrozenPolicyV1,
}

impl FullRelationDomainV1 {
    /// Validate every domain bound and recompute the policy commitment.
    pub fn validate(&self) -> Result<(), PolicyIdentityErrorV1> {
        if self.relation_version != RELATION_VERSION_V1 {
            return Err(PolicyIdentityErrorV1::WrongVersion);
        }
        for identity in [
            self.market_id,
            self.book_id,
            self.epoch_id,
            self.policy_id,
            self.order_set_id,
        ] {
            if identity.is_zero() {
                return Err(PolicyIdentityErrorV1::ZeroIdentity);
            }
        }
        if self.outcome_count < 2
            || self.outcome_count as usize > MAX_OUTCOMES
            || self.owner_count == 0
            || self.price_scale == 0
            || self.price_scale > MAX_PRICE_SCALE
        {
            return Err(PolicyIdentityErrorV1::InvalidDomain);
        }
        if batch_policy_digest(&self.policy)? != self.policy_id {
            return Err(PolicyIdentityErrorV1::MismatchedBinding);
        }
        self.policy.validate()?;
        Ok(())
    }

    /// Encode the exact full-domain digest preimage.  Every byte of every
    /// persisted identity is copied; there is no integer projection.
    pub fn canonical_bytes(
        &self,
    ) -> Result<[u8; FULL_RELATION_DOMAIN_BYTES], PolicyIdentityErrorV1> {
        self.validate()?;
        let policy = canonical_batch_policy_bytes(&self.policy)?;
        let mut out = [0; FULL_RELATION_DOMAIN_BYTES];
        let mut at = 0usize;
        out[at..at + 8].copy_from_slice(&FULL_RELATION_DOMAIN_MAGIC);
        at += 8;
        out[at..at + 2].copy_from_slice(&FULL_RELATION_DOMAIN_SCHEMA_V1.to_le_bytes());
        at += 2;
        out[at..at + 2].copy_from_slice(&FULL_DOMAIN_FLAGS_V1.to_le_bytes());
        at += 2;
        out[at..at + 4].copy_from_slice(&self.relation_version.to_le_bytes());
        at += 4;
        for identity in [
            self.market_id,
            self.book_id,
            self.epoch_id,
            self.policy_id,
            self.order_set_id,
        ] {
            out[at..at + 32].copy_from_slice(&identity.0);
            at += 32;
        }
        out[at..at + 8].copy_from_slice(&self.epoch_index.to_le_bytes());
        at += 8;
        out[at] = self.outcome_count;
        at += 1;
        out[at] = 0;
        at += 1;
        out[at..at + 2].copy_from_slice(&self.owner_count.to_le_bytes());
        at += 2;
        out[at..at + 8].copy_from_slice(&self.price_scale.to_le_bytes());
        at += 8;
        out[at..at + 8].copy_from_slice(&self.remainder_seed.to_le_bytes());
        at += 8;
        out[at..at + BATCH_POLICY_BYTES].copy_from_slice(&policy);
        at += BATCH_POLICY_BYTES;
        out[at..at + FULL_RELATION_DOMAIN_RESERVED_BYTES].fill(0);
        at += FULL_RELATION_DOMAIN_RESERVED_BYTES;
        if at != FULL_RELATION_DOMAIN_BYTES {
            return Err(PolicyIdentityErrorV1::InvalidDomain);
        }
        Ok(out)
    }

    /// SHA-256 identity over the exact full-width domain preimage.
    pub fn digest(&self) -> Result<Identity32V1, PolicyIdentityErrorV1> {
        let bytes = self.canonical_bytes()?;
        Ok(sha256::<Chosen<FULL_RELATION_DOMAIN_PREIMAGE>>(
            FULL_RELATION_DOMAIN_DIGEST_DOMAIN,
            &[&bytes],
        ))
    }

    /// Identity-free arithmetic projection used only to reuse V0--V8.
    ///
    /// The four zero tags below are not representations of the full
    /// identities.  `RelationDomainV1` reads them only in its obsolete V9
    /// digest; [`verify_submitted_candidate`] discards that digest before it
    /// returns and installs no truncated value in its place.
    fn arithmetic_domain(&self) -> RelationDomainV1 {
        RelationDomainV1 {
            relation_version: self.relation_version,
            market_id: 0,
            book_id: 0,
            epoch: self.epoch_index,
            policy_id: 0,
            order_set_id: 0,
            outcome_count: self.outcome_count,
            owner_count: self.owner_count,
            price_scale: self.price_scale,
            remainder_seed: self.remainder_seed,
            policy: self.policy,
        }
    }
}

/// Full-width V9 score.  The first four fields have the exact directions of
/// [`relation_v1::ScoreV1`]; the final tie-break is the complete SHA-256
/// relation-candidate identity, with the lexicographically smaller digest
/// preferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullScoreV1 {
    /// Score component 1.
    pub weighted_direct_volume: i128,
    /// Score component 3.
    pub limit_surplus_price_units: u128,
    /// Score component 4.
    pub distinct_owners: u16,
    /// Score component 5; lower is better.
    pub churn: u64,
    /// Full-width final tie identity; lexicographically lower is better.
    pub digest: Identity32V1,
}

impl FullScoreV1 {
    /// Canonical all-zero unclaimed score.
    pub const ZERO: Self = Self {
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        distinct_owners: 0,
        churn: 0,
        digest: Identity32V1::ZERO,
    };

    /// Frozen total ordering used only after a complete submitted set is closed.
    pub fn total_order(&self, other: &Self) -> Ordering {
        match self
            .weighted_direct_volume
            .cmp(&other.weighted_direct_volume)
        {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        match self
            .limit_surplus_price_units
            .cmp(&other.limit_surplus_price_units)
        {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        match self.distinct_owners.cmp(&other.distinct_owners) {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        match other.churn.cmp(&self.churn) {
            Ordering::Equal => {}
            unequal => return unequal,
        }
        // Smaller digest wins, matching ScoreV1's final direction.
        other.digest.cmp(&self.digest)
    }

    /// Whether this score outranks another score after window closure.
    pub fn is_better_than(&self, other: &Self) -> bool {
        self.total_order(other) == Ordering::Greater
    }
}

/// A candidate witness plus its full-width account and relation claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullSubmittedCandidateV1 {
    /// Canonical account-plane Candidate identity.
    pub candidate_id: Identity32V1,
    /// Orders this candidate binds.
    pub order_len: u8,
    /// Full fixed-width price vector.
    pub prices: [u64; MAX_OUTCOMES],
    /// Virtual complete-set split quantity.
    pub virtual_split: u64,
    /// Virtual complete-set merge quantity.
    pub virtual_merge: u64,
    /// Canonical derived fills.
    pub fills: [u64; MAX_ORDERS],
    /// Witnessed honored-AON mask.
    pub honored_aon_mask: u64,
    /// Full-width claimed score.
    pub claimed_score: FullScoreV1,
}

impl FullSubmittedCandidateV1 {
    /// Lift an existing host candidate's economic coordinates without copying
    /// its obsolete `u128` identity claims.  The resulting score is explicitly
    /// unclaimed until [`complete_submitted_candidate`] recomputes it.
    pub fn from_relation_candidate(
        domain: &FullRelationDomainV1,
        candidate: &CandidateV1,
    ) -> Result<Self, PolicyIdentityErrorV1> {
        let mut value = Self {
            candidate_id: Identity32V1::ZERO,
            order_len: candidate.order_len,
            prices: candidate.prices,
            virtual_split: candidate.virtual_split,
            virtual_merge: candidate.virtual_merge,
            fills: candidate.fills,
            honored_aon_mask: candidate.honored_aon_mask,
            claimed_score: FullScoreV1::ZERO,
        };
        value.candidate_id = value.recomputed_account_candidate_id(domain);
        Ok(value)
    }

    /// Recompute the existing account-plane Candidate identity exactly: epoch,
    /// market, order length, outcome width, all prices, virtual pair, and mask.
    pub fn recomputed_account_candidate_id(&self, domain: &FullRelationDomainV1) -> Identity32V1 {
        self.account_candidate_id_with::<Chosen<ACCOUNT_CANDIDATE_PREIMAGE>>(domain)
    }

    fn account_candidate_id_with<H: Sha256Like>(
        &self,
        domain: &FullRelationDomainV1,
    ) -> Identity32V1 {
        let mut h = H::new();
        h.update(ACCOUNT_CANDIDATE_DIGEST_DOMAIN);
        h.update(domain.epoch_id.0);
        h.update(domain.market_id.0);
        h.update([self.order_len]);
        h.update([domain.outcome_count]);
        let mut i = 0usize;
        while i < MAX_OUTCOMES {
            h.update(self.prices[i].to_le_bytes());
            i += 1;
        }
        h.update(self.virtual_split.to_le_bytes());
        h.update(self.virtual_merge.to_le_bytes());
        h.update(self.honored_aon_mask.to_le_bytes());
        Identity32V1(h.finalize())
    }

    fn as_unclaimed_relation_candidate(&self) -> CandidateV1 {
        CandidateV1 {
            order_len: self.order_len,
            prices: self.prices,
            virtual_split: self.virtual_split,
            virtual_merge: self.virtual_merge,
            fills: self.fills,
            honored_aon_mask: self.honored_aon_mask,
            claimed_score: relation_v1::ScoreV1::ZERO,
            canonical_candidate_digest: 0,
        }
    }
}

/// CandidateFeed fields which must agree with the candidate and frozen domain.
/// This is a semantic input, not a live account ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullCandidateFeedBindingV1 {
    /// Candidate identity repeated by the feed.
    pub candidate_id: Identity32V1,
    /// Epoch identity repeated by the feed.
    pub epoch_id: Identity32V1,
    /// Market identity repeated by the feed.
    pub market_id: Identity32V1,
    /// Frozen order-set identity the fill vector was computed against.
    pub order_set_id: Identity32V1,
    /// Full-width claimed score repeated by the feed.
    pub claimed_score: FullScoreV1,
}

impl FullCandidateFeedBindingV1 {
    fn validate_against(
        &self,
        domain: &FullRelationDomainV1,
        candidate: &FullSubmittedCandidateV1,
    ) -> Result<(), PolicyIdentityErrorV1> {
        if self.candidate_id != candidate.candidate_id
            || self.epoch_id != domain.epoch_id
            || self.market_id != domain.market_id
            || self.order_set_id != domain.order_set_id
            || self.claimed_score != candidate.claimed_score
        {
            return Err(PolicyIdentityErrorV1::MismatchedBinding);
        }
        Ok(())
    }
}

fn feed_leg<H: Sha256Like>(h: &mut H, leg: LegRefV1) {
    match leg {
        LegRefV1::Order(index) => {
            h.update([0, index]);
        }
        LegRefV1::Split => h.update([1, 0]),
        LegRefV1::Merge => h.update([2, 0]),
    }
}

/// Full relation-candidate identity.  It commits to the full domain, canonical
/// account candidate, every fill, and (when selected by policy) every explicit
/// pairing slice.  The policy itself is already included byte-for-byte in the
/// full-domain digest.
///
/// This is off-chain code.  Its widest preimage is
/// [`FULL_RELATION_CANDIDATE_PREIMAGE`] bytes, so the on-chain form does not fit
/// a 4 KiB SBF frame and the SBF backend says so; nothing on the program's reach
/// graph calls it, and link-time optimisation drops it from the deployable ELF.
/// The bounded direct profile's own fixed-width fold is what ships.
pub fn full_relation_candidate_digest(
    domain: &FullRelationDomainV1,
    candidate: &FullSubmittedCandidateV1,
    pairing: Option<&PairingWitnessV1>,
) -> Result<Identity32V1, PolicyIdentityErrorV1> {
    full_relation_candidate_digest_with::<Chosen<FULL_RELATION_CANDIDATE_PREIMAGE>>(
        domain, candidate, pairing,
    )
}

/// [`full_relation_candidate_digest`], recomputed from stored feed regions —
/// the on-chain form.
///
/// The candidate-feed account already stores the digest's fill and witness
/// segments byte for byte: all [`MAX_ORDERS`] fills as little-endian `u64`s
/// with canonical zero padding (exactly what the digest folds), and each
/// declared slice as `buy(kind, index), sell(kind, index), outcome, quantity
/// LE` — the exact sequence [`feed_leg`] and the quantity fold produce.  So
/// the whole preimage can be committed by *borrowing* those regions plus a
/// few small locals, and no frame ever holds the multi-kilobyte preimage the
/// buffered [`full_relation_candidate_digest`] needs (the reason that form is
/// off-chain only).
///
/// `fill_region` must be exactly `MAX_ORDERS * 8` bytes; a declared witness
/// travels as `(declared_len, region)` with the region exactly thirteen bytes
/// per declared slice.  The caller owns the regions' authenticity — on-chain they
/// are borrowed from a `verify_candidate_feed`-verified account, whose codec
/// enforces the canonical padding and virtual-leg zero bytes this preimage
/// equality rests on.  The host and syscall forms commit to the same byte
/// string; the equality test in `general_clearing_v1` compares all three
/// paths on the same coordinates.
pub fn full_relation_candidate_digest_from_regions(
    domain_digest: Identity32V1,
    candidate_id: Identity32V1,
    fill_region: &[u8],
    honored_aon_mask: u64,
    witness_region: Option<(u16, &[u8])>,
) -> Result<Identity32V1, PolicyIdentityErrorV1> {
    if fill_region.len() != MAX_ORDERS * 8 {
        return Err(PolicyIdentityErrorV1::InvalidDomain);
    }
    let mask_bytes = honored_aon_mask.to_le_bytes();
    let (flag, length_bytes, slices): (&[u8], [u8; 2], &[u8]) = match witness_region {
        None => (&[0u8], [0; 2], &[]),
        Some((declared, region)) => {
            if region.len() != declared as usize * 13 {
                return Err(PolicyIdentityErrorV1::InvalidDomain);
            }
            (&[1u8], declared.to_le_bytes(), region)
        }
    };
    let parts: [&[u8]; 8] = [
        FULL_RELATION_CANDIDATE_DIGEST_DOMAIN,
        &domain_digest.0,
        &candidate_id.0,
        fill_region,
        &mask_bytes,
        flag,
        &length_bytes[..if witness_region.is_some() { 2 } else { 0 }],
        slices,
    ];
    Ok(region_sha256(&parts))
}

/// SHA-256 over a borrowed slice sequence: the syscall on-chain, the portable
/// implementation off-chain.  Both commit to the concatenation, which is what
/// lets the host equality test require byte-identical output.
#[cfg(target_os = "solana")]
fn region_sha256(parts: &[&[u8]]) -> Identity32V1 {
    Identity32V1(solana_sha256_hasher::hashv(parts).to_bytes())
}

#[cfg(not(target_os = "solana"))]
fn region_sha256(parts: &[&[u8]]) -> Identity32V1 {
    let mut h = <sha2::Sha256 as sha2::Digest>::new();
    for part in parts {
        sha2::Digest::update(&mut h, part);
    }
    Identity32V1(sha2::Digest::finalize(h).into())
}

/// The syscall wrapper's own fold over the same sequence, compiled for host
/// tests only, so one process can require the two paths byte-identical.
#[cfg(test)]
pub(crate) fn region_sha256_native(parts: &[&[u8]]) -> Identity32V1 {
    Identity32V1(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn full_relation_candidate_digest_with<H: Sha256Like>(
    domain: &FullRelationDomainV1,
    candidate: &FullSubmittedCandidateV1,
    pairing: Option<&PairingWitnessV1>,
) -> Result<Identity32V1, PolicyIdentityErrorV1> {
    let domain_digest = domain.digest()?;
    let mut h = H::new();
    h.update(FULL_RELATION_CANDIDATE_DIGEST_DOMAIN);
    h.update(domain_digest.0);
    h.update(candidate.candidate_id.0);
    let mut i = 0usize;
    while i < MAX_ORDERS {
        h.update(candidate.fills[i].to_le_bytes());
        i += 1;
    }
    h.update(candidate.honored_aon_mask.to_le_bytes());
    match pairing {
        None => h.update([0]),
        Some(witness) => {
            h.update([1]);
            h.update(witness.len.to_le_bytes());
            let mut k = 0usize;
            while k < witness.len as usize {
                let slice = witness.slices[k];
                feed_leg(&mut h, slice.buy_ref);
                feed_leg(&mut h, slice.sell_ref);
                h.update([slice.outcome]);
                h.update(slice.quantity.to_le_bytes());
                k += 1;
            }
        }
    }
    Ok(Identity32V1(h.finalize()))
}

fn recompute(
    domain: &FullRelationDomainV1,
    book: &BookV1,
    candidate: &FullSubmittedCandidateV1,
    pairing: Option<&PairingWitnessV1>,
) -> Result<(SummaryV1, FullScoreV1), PolicyIdentityErrorV1> {
    domain.validate()?;
    if candidate.candidate_id != candidate.recomputed_account_candidate_id(domain) {
        return Err(PolicyIdentityErrorV1::CandidateIdentityMismatch);
    }
    let arithmetic = domain.arithmetic_domain();
    let inner = candidate.as_unclaimed_relation_candidate();
    let mut summary =
        relation_v1::verify_ignoring_claimed_aggregates(&arithmetic, book, &inner, pairing)?;
    let score = FullScoreV1 {
        weighted_direct_volume: summary.score.weighted_direct_volume,
        limit_surplus_price_units: summary.score.limit_surplus_price_units,
        distinct_owners: summary.score.distinct_owners,
        churn: summary.score.churn,
        digest: full_relation_candidate_digest(domain, candidate, pairing)?,
    };
    // These two fields belong to the legacy u64-domain identity.  Returning
    // them would create a second, lossy semantic truth, so the economic summary
    // carries explicit zero sentinels and the full score above is authoritative.
    summary.score.digest = 0;
    summary.candidate_digest = 0;
    Ok((summary, score))
}

/// Verification result for one valid submitted candidate.  The type name is
/// intentionally not `SelectedCandidate`: this result has no candidate-window
/// closure evidence and grants no settlement authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedSubmittedCandidateV1 {
    /// Canonical account-plane identity of this submission.
    pub candidate_id: Identity32V1,
    /// Recomputed full-width score.
    pub score: FullScoreV1,
    /// Recomputed V0--V8 economics.  Its two obsolete legacy digest fields are
    /// zero sentinels; `score.digest` above is the only tie identity.
    pub economics: SummaryV1,
}

/// Complete the canonical full score for a structurally unclaimed candidate
/// and produce the matching feed binding.  This does not select it.
pub fn complete_submitted_candidate(
    domain: &FullRelationDomainV1,
    book: &BookV1,
    candidate: &FullSubmittedCandidateV1,
    pairing: Option<&PairingWitnessV1>,
) -> Result<(FullSubmittedCandidateV1, FullCandidateFeedBindingV1), PolicyIdentityErrorV1> {
    let mut completed = *candidate;
    completed.candidate_id = completed.recomputed_account_candidate_id(domain);
    let (_, score) = recompute(domain, book, &completed, pairing)?;
    completed.claimed_score = score;
    let feed = FullCandidateFeedBindingV1 {
        candidate_id: completed.candidate_id,
        epoch_id: domain.epoch_id,
        market_id: domain.market_id,
        order_set_id: domain.order_set_id,
        claimed_score: score,
    };
    Ok((completed, feed))
}

/// Verify one submitted candidate and its feed against the full-width domain.
/// Every economic and score claim is recomputed.  Success is not selection.
pub fn verify_submitted_candidate(
    domain: &FullRelationDomainV1,
    book: &BookV1,
    candidate: &FullSubmittedCandidateV1,
    feed: &FullCandidateFeedBindingV1,
    pairing: Option<&PairingWitnessV1>,
) -> Result<VerifiedSubmittedCandidateV1, PolicyIdentityErrorV1> {
    domain.validate()?;
    feed.validate_against(domain, candidate)?;
    let (economics, score) = recompute(domain, book, candidate, pairing)?;
    if candidate.claimed_score != score || feed.claimed_score != score {
        return Err(PolicyIdentityErrorV1::ClaimedScoreMismatch);
    }
    Ok(VerifiedSubmittedCandidateV1 {
        candidate_id: candidate.candidate_id,
        score,
        economics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::relation_v1::{canonical_candidate, OrderV1, SingleEggOrderV1, PRICE_SCALE};
    use clutch_batch::{PartialPolicy, Side};

    fn id(seed: u8) -> Identity32V1 {
        let mut bytes = [seed; 32];
        bytes[0] = seed.wrapping_add(0x40);
        Identity32V1(bytes)
    }

    fn base_policy() -> FrozenPolicyV1 {
        FrozenPolicyV1 {
            allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
            self_cross: SelfCrossPolicyV1::AllowGateAtPairing,
            aon: AonPolicyV1::RefuseAdmission,
            rounding: RoundingBoundaryV1::TerminalOwnerFloor,
            residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
            transfer_phase: TransferPhaseV1::ActiveOrResolved,
            portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
            pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
            dust: DustPolicy::AssignCanonical,
            score: ScorePolicyV1::LexicographicDispersionV1,
            fee_base: FeeBaseV1::None,
        }
    }

    fn domain_with(policy: FrozenPolicyV1) -> FullRelationDomainV1 {
        FullRelationDomainV1 {
            relation_version: RELATION_VERSION_V1,
            market_id: id(1),
            book_id: id(2),
            epoch_id: id(3),
            policy_id: batch_policy_digest(&policy).unwrap(),
            order_set_id: id(5),
            epoch_index: 7,
            outcome_count: 2,
            owner_count: 2,
            price_scale: PRICE_SCALE,
            remainder_seed: 9,
            policy,
        }
    }

    fn arithmetic(domain: &FullRelationDomainV1) -> RelationDomainV1 {
        domain.arithmetic_domain()
    }

    fn single(id: u64, owner: u16, side: Side, limit: u64) -> OrderV1 {
        OrderV1::SingleEgg(SingleEggOrderV1 {
            canonical_order_id: id,
            owner,
            outcome: 0,
            side,
            quantity: 4,
            limit_price: limit,
            minimum_fill: 1,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        })
    }

    fn crossing_book() -> BookV1 {
        let mut book = BookV1::empty();
        book.orders[0] = single(1, 0, Side::Buy, PRICE_SCALE);
        book.orders[1] = single(2, 1, Side::Sell, 0);
        book.len = 2;
        book
    }

    #[test]
    fn policy_codec_round_trips_every_registered_selector_family() {
        let base = base_policy();
        let variants = [
            FrozenPolicyV1 {
                allocation: AllocationPolicyV1::FullProRata,
                ..base
            },
            FrozenPolicyV1 {
                self_cross: SelfCrossPolicyV1::RefuseOverlap,
                ..base
            },
            FrozenPolicyV1 {
                self_cross: SelfCrossPolicyV1::NetAtAdmission,
                ..base
            },
            FrozenPolicyV1 {
                aon: AonPolicyV1::WitnessedHonoredMask,
                ..base
            },
            FrozenPolicyV1 {
                aon: AonPolicyV1::FullSizeCounting,
                ..base
            },
            FrozenPolicyV1 {
                rounding: RoundingBoundaryV1::None,
                ..base
            },
            FrozenPolicyV1 {
                rounding: RoundingBoundaryV1::ReceiptFloor,
                ..base
            },
            FrozenPolicyV1 {
                residual_settlement: ResidualSettlementV1::FullPairOnly,
                ..base
            },
            FrozenPolicyV1 {
                residual_settlement: ResidualSettlementV1::CumulativePairCanonical,
                ..base
            },
            FrozenPolicyV1 {
                residual_settlement: ResidualSettlementV1::CumulativePairFree,
                ..base
            },
            FrozenPolicyV1 {
                transfer_phase: TransferPhaseV1::ActiveOnly,
                ..base
            },
            FrozenPolicyV1 {
                portfolio_lots: PortfolioLotPolicyV1::MarginalProRataLots,
                ..base
            },
            FrozenPolicyV1 {
                pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
                ..base
            },
            FrozenPolicyV1 {
                dust: DustPolicy::Reject,
                ..base
            },
            FrozenPolicyV1 {
                fee_base: FeeBaseV1::FlatNotional { bps: 37 },
                ..base
            },
        ];
        let baseline = batch_policy_digest(&base).unwrap();
        for policy in variants {
            let bytes = canonical_batch_policy_bytes(&policy).unwrap();
            assert_eq!(decode_batch_policy(&bytes), Ok(policy));
            assert_ne!(batch_policy_digest(&policy).unwrap(), baseline);
        }
        assert_eq!(
            decode_batch_policy(&canonical_batch_policy_bytes(&base).unwrap()),
            Ok(base)
        );
    }

    #[test]
    fn every_registered_policy_product_has_one_canonical_round_trip() {
        // All ten selector radices, in wire order.  The score family has one
        // registered member; keeping its radix here means a future member must
        // deliberately extend this exhaustive product.
        let radices = [2u8, 3, 3, 3, 4, 2, 2, 2, 2, 1];
        let selector_products = radices
            .iter()
            .fold(1usize, |product, radix| product * *radix as usize);
        let base = canonical_batch_policy_bytes(&base_policy()).unwrap();
        let mut admitted = 0usize;
        for code in 0..selector_products {
            let mut quotient = code;
            let mut selectors = [0u8; 10];
            for (selector, radix) in selectors.iter_mut().zip(radices) {
                *selector = (quotient % radix as usize) as u8;
                quotient /= radix as usize;
            }
            for fee_case in 0..3 {
                let mut bytes = base;
                bytes[12..22].copy_from_slice(&selectors);
                match fee_case {
                    0 => {
                        bytes[22] = 0;
                        bytes[24..28].copy_from_slice(&0u32.to_le_bytes());
                    }
                    1 => {
                        bytes[22] = 1;
                        bytes[24..28].copy_from_slice(&0u32.to_le_bytes());
                    }
                    _ => {
                        bytes[22] = 1;
                        bytes[24..28].copy_from_slice(&(FEE_BPS_DENOMINATOR as u32).to_le_bytes());
                    }
                }
                let decoded = decode_batch_policy(&bytes).unwrap();
                assert_eq!(canonical_batch_policy_bytes(&decoded).unwrap(), bytes);
                admitted += 1;
            }
        }
        assert_eq!(admitted, 10_368);
    }

    #[test]
    fn stream_checkpoint_policy_bytes_equal_the_artifact_selector_bytes() {
        // The Tier 2 checkpoint codec (`clutch_batch::relation_v1_stream`)
        // cannot depend on this crate — the dependency points the other way —
        // so it *restates* the selector byte values.  This is the cross-crate
        // equality gate the restatement is conditioned on: over every
        // registered selector product and the three fee shapes, the
        // checkpoint codec's 15 policy bytes must equal this artifact's
        // selector region (bytes 12..22), fee discriminant (byte 22), and fee
        // parameter (bytes 24..28) exactly, and its decoder must return the
        // identical `FrozenPolicyV1`.
        use clutch_batch::relation_v1_stream::{
            decode_policy_v1, encode_policy_v1, POLICY_ENCODED_BYTES,
        };
        assert_eq!(POLICY_ENCODED_BYTES, 15);
        let radices = [2u8, 3, 3, 3, 4, 2, 2, 2, 2, 1];
        let selector_products = radices
            .iter()
            .fold(1usize, |product, radix| product * *radix as usize);
        let base = canonical_batch_policy_bytes(&base_policy()).unwrap();
        let mut compared = 0usize;
        for code in 0..selector_products {
            let mut quotient = code;
            let mut selectors = [0u8; 10];
            for (selector, radix) in selectors.iter_mut().zip(radices) {
                *selector = (quotient % radix as usize) as u8;
                quotient /= radix as usize;
            }
            for fee_case in 0..3 {
                let mut bytes = base;
                bytes[12..22].copy_from_slice(&selectors);
                match fee_case {
                    0 => {
                        bytes[22] = 0;
                        bytes[24..28].copy_from_slice(&0u32.to_le_bytes());
                    }
                    1 => {
                        bytes[22] = 1;
                        bytes[24..28].copy_from_slice(&0u32.to_le_bytes());
                    }
                    _ => {
                        bytes[22] = 1;
                        bytes[24..28].copy_from_slice(&(FEE_BPS_DENOMINATOR as u32).to_le_bytes());
                    }
                }
                let policy = decode_batch_policy(&bytes).unwrap();
                let mut stream_bytes = [0u8; 15];
                encode_policy_v1(&policy, &mut stream_bytes).unwrap();
                assert_eq!(&stream_bytes[..10], &bytes[12..22], "selector bytes");
                assert_eq!(stream_bytes[10], bytes[22], "fee discriminant");
                assert_eq!(&stream_bytes[11..15], &bytes[24..28], "fee parameter");
                assert_eq!(decode_policy_v1(&stream_bytes), Ok(policy));
                compared += 1;
            }
        }
        assert_eq!(compared, 10_368);
    }

    #[test]
    fn policy_codec_refuses_unknowns_noncanonical_fee_and_reserved_mutations() {
        let bytes = canonical_batch_policy_bytes(&base_policy()).unwrap();
        assert_eq!(
            decode_batch_policy(&bytes[..63]),
            Err(PolicyIdentityErrorV1::Truncated)
        );
        let mut long = [0u8; 65];
        long[..64].copy_from_slice(&bytes);
        assert_eq!(
            decode_batch_policy(&long),
            Err(PolicyIdentityErrorV1::TrailingBytes)
        );
        for offset in 12..=22 {
            let mut bad = bytes;
            bad[offset] = 0xff;
            assert_eq!(
                decode_batch_policy(&bad),
                Err(PolicyIdentityErrorV1::InvalidEnum)
            );
        }
        let mut inactive_fee = bytes;
        inactive_fee[24] = 1;
        assert_eq!(
            decode_batch_policy(&inactive_fee),
            Err(PolicyIdentityErrorV1::NonCanonicalPadding)
        );
        let mut reserved = bytes;
        reserved[63] = 1;
        assert_eq!(
            decode_batch_policy(&reserved),
            Err(PolicyIdentityErrorV1::NonCanonicalPadding)
        );
        let mut flags = bytes;
        flags[10] = 1;
        assert_eq!(
            decode_batch_policy(&flags),
            Err(PolicyIdentityErrorV1::InvalidEnum)
        );
    }

    #[test]
    fn every_policy_byte_mutation_refuses_or_changes_semantics_and_digest() {
        let policy = base_policy();
        let bytes = canonical_batch_policy_bytes(&policy).unwrap();
        let digest = batch_policy_digest(&policy).unwrap();
        for offset in 0..BATCH_POLICY_BYTES {
            let mut mutated = bytes;
            mutated[offset] ^= 0x80;
            if let Ok(other) = decode_batch_policy(&mutated) {
                assert_ne!(other, policy, "byte {offset} changed no semantics");
                assert_ne!(
                    batch_policy_digest(&other).unwrap(),
                    digest,
                    "byte {offset} changed no digest"
                );
            }
        }
    }

    #[test]
    fn high_192_identity_bits_are_bound_and_never_projected_to_u64() {
        let domain = domain_with(base_policy());
        let baseline = domain.digest().unwrap();
        for field in 0..5 {
            for byte in 0..32 {
                let mut changed = domain;
                let target = match field {
                    0 => &mut changed.market_id,
                    1 => &mut changed.book_id,
                    2 => &mut changed.epoch_id,
                    3 => &mut changed.policy_id,
                    _ => &mut changed.order_set_id,
                };
                target.0[byte] ^= 0x80;
                if field == 3 {
                    assert_eq!(
                        changed.digest(),
                        Err(PolicyIdentityErrorV1::MismatchedBinding)
                    );
                } else {
                    assert_ne!(changed.digest().unwrap(), baseline);
                }
            }
        }
    }

    #[test]
    fn full_verifier_recomputes_claims_and_returns_submitted_not_selected() {
        let domain = domain_with(base_policy());
        let book = crossing_book();
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = PRICE_SCALE / 2;
        prices[1] = PRICE_SCALE / 2;
        let legacy = canonical_candidate(&arithmetic(&domain), &book, &prices, 0, 0).unwrap();
        let raw = FullSubmittedCandidateV1::from_relation_candidate(&domain, &legacy).unwrap();
        let (candidate, feed) = complete_submitted_candidate(&domain, &book, &raw, None).unwrap();
        let verified = verify_submitted_candidate(&domain, &book, &candidate, &feed, None).unwrap();
        assert_eq!(verified.candidate_id, candidate.candidate_id);
        assert_eq!(verified.score, candidate.claimed_score);
        assert_eq!(verified.economics.score.digest, 0);
        assert_eq!(verified.economics.candidate_digest, 0);

        let mut lied = candidate;
        lied.claimed_score.weighted_direct_volume += 1;
        let mut matching_feed = feed;
        matching_feed.claimed_score = lied.claimed_score;
        assert_eq!(
            verify_submitted_candidate(&domain, &book, &lied, &matching_feed, None),
            Err(PolicyIdentityErrorV1::ClaimedScoreMismatch)
        );
    }

    #[test]
    fn candidate_feed_and_policy_substitution_fail_closed() {
        let domain = domain_with(base_policy());
        let book = crossing_book();
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = PRICE_SCALE / 2;
        prices[1] = PRICE_SCALE / 2;
        let legacy = canonical_candidate(&arithmetic(&domain), &book, &prices, 0, 0).unwrap();
        let raw = FullSubmittedCandidateV1::from_relation_candidate(&domain, &legacy).unwrap();
        let (candidate, feed) = complete_submitted_candidate(&domain, &book, &raw, None).unwrap();
        let mut wrong_feed = feed;
        wrong_feed.order_set_id.0[17] ^= 1;
        assert_eq!(
            verify_submitted_candidate(&domain, &book, &candidate, &wrong_feed, None),
            Err(PolicyIdentityErrorV1::MismatchedBinding)
        );

        let mut substituted = domain;
        substituted.policy.rounding = RoundingBoundaryV1::ReceiptFloor;
        assert_eq!(
            verify_submitted_candidate(&substituted, &book, &candidate, &feed, None),
            Err(PolicyIdentityErrorV1::MismatchedBinding)
        );
    }

    #[test]
    fn full_digest_binds_explicit_witness_and_score_order_uses_all_256_bits() {
        let mut policy = base_policy();
        policy.pairing_witness = PairingWitnessPolicyV1::ExplicitSlices;
        let domain = domain_with(policy);
        let book = crossing_book();
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = PRICE_SCALE / 2;
        prices[1] = PRICE_SCALE / 2;
        let legacy = canonical_candidate(&arithmetic(&domain), &book, &prices, 0, 0).unwrap();
        let witness = relation_v1::canonical_pairing(&arithmetic(&domain), &book, &legacy).unwrap();
        let raw = FullSubmittedCandidateV1::from_relation_candidate(&domain, &legacy).unwrap();
        let without = full_relation_candidate_digest(&domain, &raw, None).unwrap();
        let with = full_relation_candidate_digest(&domain, &raw, Some(&witness)).unwrap();
        assert_ne!(without, with);
        let (candidate, feed) =
            complete_submitted_candidate(&domain, &book, &raw, Some(&witness)).unwrap();
        verify_submitted_candidate(&domain, &book, &candidate, &feed, Some(&witness)).unwrap();
        assert!(matches!(
            verify_submitted_candidate(&domain, &book, &candidate, &feed, None),
            Err(PolicyIdentityErrorV1::Relation(
                ErrorV1::PairingWitnessMissing
            ))
        ));

        let mut lower = candidate.claimed_score;
        let mut higher = lower;
        lower.digest.0[20] = 1;
        higher.digest.0[20] = 2;
        assert!(lower.is_better_than(&higher));
    }

    /* --- native-hasher equivalence ------------------------------------------
     *
     * On SBF every fold below buffers its preimage and hands it to the runtime
     * SHA-256 syscall instead of compiling a software compression function.
     * That is a size and compute change and must never be a value change: these
     * identities are PDA seeds, stored account commitments, and the final score
     * coordinate of candidate selection.
     *
     * Each test instantiates the *same* fold twice -- once at the portable
     * `sha2` state, once at the on-chain preimage buffer -- and requires
     * byte-identical output.  There is one body, so a preimage that drifts
     * drifts on both sides; what is being checked is that buffering the update
     * sequence and folding it incrementally commit to the same byte string, and
     * that every declared buffer width is wide enough for the widest shape its
     * call site can produce (a short buffer halts inside `Native::update`).
     * ---------------------------------------------------------------------- */

    /// The domain-separated helper and the account-plane candidate identity.
    #[test]
    fn native_hasher_matches_the_portable_hasher_on_policy_and_domain_identities() {
        let policy = base_policy();
        let bytes = canonical_batch_policy_bytes(&policy).unwrap();
        assert_eq!(
            batch_policy_digest(&policy).unwrap(),
            sha256::<hasher::Native<BATCH_POLICY_PREIMAGE>>(BATCH_POLICY_DIGEST_DOMAIN, &[&bytes]),
            "the batch-policy identity differs between the two hashers"
        );

        let domain = domain_with(policy);
        let domain_bytes = domain.canonical_bytes().unwrap();
        assert_eq!(
            domain.digest().unwrap(),
            sha256::<hasher::Native<FULL_RELATION_DOMAIN_PREIMAGE>>(
                FULL_RELATION_DOMAIN_DIGEST_DOMAIN,
                &[&domain_bytes],
            ),
            "the full-relation-domain identity differs between the two hashers"
        );

        // The helper is also exercised directly on the degenerate shapes: no
        // parts at all, and empty parts inside a non-empty list.
        assert_eq!(
            sha256::<Chosen<64>>(b"", &[]),
            sha256::<hasher::Native<64>>(b"", &[])
        );
        assert_eq!(
            sha256::<Chosen<64>>(b"domain", &[&[], &[1, 2, 3], &[]]),
            sha256::<hasher::Native<64>>(b"domain", &[&[], &[1, 2, 3], &[]])
        );
    }

    /// The candidate identity and the relation-candidate digest, at both the
    /// witness-free shape and the widest explicit-witness shape.
    #[test]
    fn native_hasher_matches_the_portable_hasher_on_candidate_identities() {
        let mut policy = base_policy();
        policy.pairing_witness = PairingWitnessPolicyV1::ExplicitSlices;
        let domain = domain_with(policy);
        let book = crossing_book();
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = PRICE_SCALE / 2;
        prices[1] = PRICE_SCALE / 2;
        let legacy = canonical_candidate(&arithmetic(&domain), &book, &prices, 0, 0).unwrap();
        let witness = relation_v1::canonical_pairing(&arithmetic(&domain), &book, &legacy).unwrap();
        let raw = FullSubmittedCandidateV1::from_relation_candidate(&domain, &legacy).unwrap();

        assert_eq!(
            raw.recomputed_account_candidate_id(&domain),
            raw.account_candidate_id_with::<hasher::Native<ACCOUNT_CANDIDATE_PREIMAGE>>(&domain),
            "the account-plane candidate identity differs between the two hashers"
        );

        assert_eq!(
            full_relation_candidate_digest(&domain, &raw, None).unwrap(),
            full_relation_candidate_digest_with::<
                hasher::Native<FULL_RELATION_CANDIDATE_PREIMAGE>,
            >(&domain, &raw, None)
            .unwrap(),
            "the witness-free relation-candidate digest differs between the two hashers"
        );
        assert_eq!(
            full_relation_candidate_digest(&domain, &raw, Some(&witness)).unwrap(),
            full_relation_candidate_digest_with::<
                hasher::Native<FULL_RELATION_CANDIDATE_PREIMAGE>,
            >(&domain, &raw, Some(&witness))
            .unwrap(),
            "the witnessed relation-candidate digest differs between the two hashers"
        );

        // The declared buffer must hold the widest witness the type admits, not
        // merely the small one this fixture produces.  Every slice is fed, so a
        // width computed from a smaller bound halts here.
        let mut widest = witness;
        widest.len = relation_v1::MAX_SLICES as u16;
        assert_eq!(
            full_relation_candidate_digest(&domain, &raw, Some(&widest)).unwrap(),
            full_relation_candidate_digest_with::<
                hasher::Native<FULL_RELATION_CANDIDATE_PREIMAGE>,
            >(&domain, &raw, Some(&widest))
            .unwrap(),
            "the widest admissible pairing witness overflows the declared preimage"
        );
    }

    /// The identity values themselves are frozen.  The expected bytes are plain
    /// SHA-256 over the documented preimage, computed independently of both
    /// implementations, so this pins the value rather than the agreement of two
    /// paths that could have moved together.
    #[test]
    fn canonical_policy_identity_value_is_frozen() {
        // `DIRECT_POLICY_V1` is the shipped policy: its digest is a live PDA
        // seed and the `policy_id` every candidate binds to.
        let policy = direct_window_v1::DIRECT_POLICY_V1;
        let bytes = canonical_batch_policy_bytes(&policy).unwrap();
        assert_eq!(
            bytes,
            [
                0x44, 0x43, 0x42, 0x41, 0x54, 0x50, 0x31, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x02,
                0x00, 0x00, 0x03, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
            ],
            "the canonical policy artifact bytes moved"
        );
        // SHA-256("dragons-clutch/batch-policy/v1\0" || those 64 bytes), taken
        // from a third SHA-256 implementation rather than from either path here.
        assert_eq!(
            batch_policy_digest(&policy).unwrap().0,
            [
                0xcc, 0xc7, 0x08, 0x3a, 0x80, 0x1f, 0x08, 0xd1, 0xf1, 0xd4, 0x27, 0x8c, 0x8a, 0x44,
                0xd2, 0x1d, 0x59, 0xc2, 0x10, 0x5b, 0xec, 0x54, 0x9d, 0x86, 0x98, 0x2b, 0x59, 0x02,
                0x59, 0x31, 0x81, 0x9b
            ],
            "the canonical batch-policy identity moved"
        );
        assert_eq!(
            batch_policy_digest(&policy).unwrap(),
            sha256::<hasher::Native<BATCH_POLICY_PREIMAGE>>(BATCH_POLICY_DIGEST_DOMAIN, &[&bytes]),
            "the on-chain hasher disagrees with the frozen value"
        );
    }
}
