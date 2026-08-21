//! RevenuePolicyV1 — the frozen revenue-policy const family (B4a–B4f).
//!
//! The policy object of `docs/design/REVENUE_POLICY_V1.md` §3, authored
//! beside the [`crate::general_clearing_v1`] const family and digested by the
//! same machinery, executing the decisions of
//! `docs/decisions/ADOPTED_2026-08-20.md` items 6 and 8:
//!
//! * **B4a** — custody requirements adopted, **treasury pubkey DEFERRED** to
//!   the first fee-bearing Realm.  The deferral is structural, not prose:
//!   [`REVENUE_POLICY_V1`] pins [`REVENUE_TREASURY_UNSET_V1`], a
//!   distinguished sentinel that [`treasury_admits_fee_bearing`] refuses, so
//!   fee-bearing epoch admission fails closed at the treasury byte until a
//!   const naming a real key exists — a new sibling const with a new digest,
//!   behind ember's reserved decision.
//! * **B4b** — Plane C's destination is an ordinary treasury-owned Position
//!   (D6); nothing here allocates any pot family.
//! * **B4c** — Plane L charges are a permanent zero as frozen policy; **no
//!   vault is built**, and [`LamportSinkV1::None`] documents the reserved
//!   member.
//! * **B4e** — the V1 split vector is **60 / 0 / 40 over 100** with
//!   [`StandingMakerV1::AllRestingMakers`] and residual atoms to the
//!   treasury; the published envelope (executor ≤ 15%, treasury ≥ 25%,
//!   `docs/ECONOMICS.md`) is a [`RevenuePolicyV1::validate`] refusal, not
//!   prose.
//! * **D3/D4** — a policy is a frozen const plus digest pinned per Realm at
//!   creation; existing Realms are zero-take forever, and **the absence of a
//!   per-Realm record IS the zero-take state** — no retrofit instruction
//!   exists or may exist.
//!
//! Nothing in this module charges anything: both fee rates are zero
//! elsewhere ([`crate::general_clearing_v1::GENERAL_CLEARING_FEE_SHAPE_V1`]),
//! every `max_fee_atoms == 0` gate stands, and the split arithmetic below is
//! exercised at fee zero until a rate decision exists.

use crate::hasher::Chosen;
use crate::{sha256, Identity32V1};

/// Exact size of a canonical revenue-policy artifact.
pub const REVENUE_POLICY_BYTES: usize = 64;
/// Revenue-policy magic: ASCII `DCREVP1` followed by one zero byte.
pub const REVENUE_POLICY_MAGIC: [u8; 8] = *b"DCREVP1\0";
/// The only canonical revenue-policy schema understood here.
pub const REVENUE_POLICY_SCHEMA_V1: u16 = 1;
/// Domain separator for the immutable revenue-policy identity.
pub const REVENUE_POLICY_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/revenue-policy/v1\0";

const REVENUE_POLICY_FLAGS_V1: u16 = 0;
const REVENUE_POLICY_PREIMAGE: usize = REVENUE_POLICY_DIGEST_DOMAIN.len() + REVENUE_POLICY_BYTES;
const _: () = assert!(REVENUE_POLICY_BYTES == 8 + 2 + 2 + 32 + 16 + 3 + 1);

/// **The B4a deferral, made structural.**  The distinguished 32-byte sentinel
/// standing where the treasury pubkey will one day be: printable ASCII, not a
/// key anyone can sign for, and refused by [`treasury_admits_fee_bearing`],
/// so a policy pinned to it can never admit a fee-bearing epoch.  Binding a
/// real key is a **new frozen const with a new digest** — reserved to ember.
pub const REVENUE_TREASURY_UNSET_V1: [u8; 32] = *b"REVENUE-TREASURY-UNSET-SENTINEL1";
const _: () = assert!(REVENUE_TREASURY_UNSET_V1.len() == 32);

/// The frozen program-wide neutral sink (the canonical Solana incinerator,
/// `1nc1nerator11111111111111111111111111111111`), restated here as raw bytes
/// because this crate carries no Solana dependency.  The SVM fixture pins
/// byte equality against `solana_sdk_ids::incinerator::ID`; a treasury equal
/// to it is the §1(D1) misclassification (an owed compartment with a burn
/// address for an owner) and refuses validation.
pub const REVENUE_NEUTRAL_SINK_BYTES_V1: [u8; 32] = [
    0x00, 0x33, 0x90, 0x72, 0x8d, 0x34, 0x11, 0x60, 0x79, 0xbd, 0xc9, 0x11, 0xbf, 0xff, 0x00,
    0xdb, 0xd4, 0x4d, 0x2e, 0xcd, 0xcc, 0xf7, 0x9c, 0xa6, 0xe1, 0x00, 0x38, 0xe1, 0x00, 0x00,
    0x00, 0x00,
];

/// Frozen residual-atom rule for split remainders.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevenueResidualV1 {
    /// Residual atoms land in the treasury share (V1).
    Treasury,
}

/// Standing-maker predicate selecting who earns the maker rebate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandingMakerV1 {
    /// The trivially-true predicate: every resting maker qualifies (V1; a
    /// stricter predicate is a sibling const with evidence, never a
    /// mutation — `REPORT_revenue-policy-v1` B4e).
    AllRestingMakers,
}

/// Plane-L lamport disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LamportSinkV1 {
    /// No lamport sink exists: all five ResolutionWork charges are a
    /// **permanent zero as frozen policy** (B4c) and no vault is built.  The
    /// member is reserved; L1 (a per-Realm vault) is the ratified
    /// disposition of record should a future *optional* service flow ever
    /// carry a nonzero charge — as a new const, never an amendment.
    None,
}

/// The frozen revenue-policy shape (design §3), pinned per Realm at creation
/// by digest and immutable forever after (D3).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevenuePolicyV1 {
    /// Must equal [`REVENUE_POLICY_SCHEMA_V1`].
    pub version: u32,
    /// Sole authenticated revenue recipient — or the
    /// [`REVENUE_TREASURY_UNSET_V1`] sentinel while the key stays deferred.
    pub treasury: [u8; 32],
    /// Maker-rebate share numerator over `split_den`.  V1: 60.
    pub maker_rebate_num: u32,
    /// Executor share numerator over `split_den`.  V1: 0 (deferred, D9 — no
    /// authenticated executor identity exists in the atom plane).
    pub executor_num: u32,
    /// Treasury share numerator over `split_den`.  V1: 40.
    pub treasury_num: u32,
    /// Share denominator.  V1: 100.
    pub split_den: u32,
    /// Frozen residual-atom rule.  V1: treasury.
    pub residual: RevenueResidualV1,
    /// Standing-maker predicate.  V1: all resting makers.
    pub standing_maker: StandingMakerV1,
    /// Plane-L lamport disposition.  V1: none (B4c permanent zero).
    pub lamport_sink: LamportSinkV1,
}

/// A refusal from the revenue-policy validator or codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevenuePolicyErrorV1 {
    /// Unknown schema version.
    WrongVersion,
    /// The split denominator is zero.
    ZeroDenominator,
    /// The three share numerators do not sum to the denominator exactly.
    SplitSumMismatch,
    /// The executor share exceeds the published 15% envelope ceiling.
    ExecutorAboveEnvelope,
    /// The treasury share is below the published 25% envelope floor.
    TreasuryBelowEnvelope,
    /// The treasury is the all-zero identity: an owed compartment with no
    /// owner is the §1(D1) misclassification.
    UnownedTreasury,
    /// The treasury is the frozen neutral sink: revenue must never ride the
    /// incinerator (D1 — revenue is owed, never surplus).
    SinkTreasury,
    /// Wrong artifact magic.
    WrongMagic,
    /// Input shorter than one canonical artifact.
    Truncated,
    /// Input longer than one canonical artifact.
    TrailingBytes,
    /// A reserved or padding byte is nonzero, or re-encoding disagrees.
    NonCanonicalPadding,
    /// An enum byte holds no registered member.
    InvalidEnum,
    /// Split arithmetic left the checked domain.
    Arithmetic,
}

/// **The one pinned V1 member of the closed set.**  60/0/40 over 100,
/// all-resting-makers, residual to treasury, no lamport sink, treasury
/// DEFERRED at the structural sentinel.  Every existing Realm is zero-take
/// forever by the *absence* of a record (D4); a new Realm electing this
/// policy at birth still cannot admit a fee-bearing epoch until a sibling
/// const binds a real treasury key.
pub const REVENUE_POLICY_V1: RevenuePolicyV1 = RevenuePolicyV1 {
    version: REVENUE_POLICY_SCHEMA_V1 as u32,
    treasury: REVENUE_TREASURY_UNSET_V1,
    maker_rebate_num: 60,
    executor_num: 0,
    treasury_num: 40,
    split_den: 100,
    residual: RevenueResidualV1::Treasury,
    standing_maker: StandingMakerV1::AllRestingMakers,
    lamport_sink: LamportSinkV1::None,
};

/// Whether a recorded treasury identity may admit a fee-bearing epoch.
///
/// False for the [`REVENUE_TREASURY_UNSET_V1`] deferral sentinel, the
/// all-zero identity, and the frozen neutral sink.  This is the predicate
/// the program's admission seam consults, kept here so the sentinel has
/// exactly one authority.
pub fn treasury_admits_fee_bearing(treasury: &[u8; 32]) -> bool {
    *treasury != REVENUE_TREASURY_UNSET_V1
        && *treasury != [0u8; 32]
        && *treasury != REVENUE_NEUTRAL_SINK_BYTES_V1
}

/// One exact split of a settled fee under the frozen vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevenueSplitV1 {
    /// Maker-rebate atoms (rounds down).
    pub maker_rebate_atoms: u64,
    /// Executor atoms (rounds down; V1 always zero).
    pub executor_atoms: u64,
    /// Treasury atoms: the remainder, residual included (V1 residual rule).
    pub treasury_atoms: u64,
}

impl RevenuePolicyV1 {
    /// Refuse every shape the struct can express but the design forbids.
    ///
    /// The published envelope (`docs/ECONOMICS.md`: executor ≤ 15%, treasury
    /// ≥ 25%) becomes structure here: a const violating it does not digest.
    /// The [`REVENUE_TREASURY_UNSET_V1`] sentinel deliberately *passes* —
    /// deferral is a valid frozen state; **admission** is what it refuses.
    pub fn validate(&self) -> Result<(), RevenuePolicyErrorV1> {
        if self.version != REVENUE_POLICY_SCHEMA_V1 as u32 {
            return Err(RevenuePolicyErrorV1::WrongVersion);
        }
        if self.split_den == 0 {
            return Err(RevenuePolicyErrorV1::ZeroDenominator);
        }
        let sum = (self.maker_rebate_num as u64)
            .checked_add(self.executor_num as u64)
            .and_then(|part| part.checked_add(self.treasury_num as u64))
            .ok_or(RevenuePolicyErrorV1::Arithmetic)?;
        if sum != self.split_den as u64 {
            return Err(RevenuePolicyErrorV1::SplitSumMismatch);
        }
        if self.executor_num as u64 * 100 > 15 * self.split_den as u64 {
            return Err(RevenuePolicyErrorV1::ExecutorAboveEnvelope);
        }
        if (self.treasury_num as u64) * 100 < 25 * self.split_den as u64 {
            return Err(RevenuePolicyErrorV1::TreasuryBelowEnvelope);
        }
        if self.treasury == [0u8; 32] {
            return Err(RevenuePolicyErrorV1::UnownedTreasury);
        }
        if self.treasury == REVENUE_NEUTRAL_SINK_BYTES_V1 {
            return Err(RevenuePolicyErrorV1::SinkTreasury);
        }
        Ok(())
    }

    /// Allocate one settled fee (already-terminal atoms) under the frozen
    /// vector: **rebates round down, the executor share rounds down, and the
    /// treasury takes the exact remainder** — the residual rule of
    /// `docs/ECONOMICS.md` with [`RevenueResidualV1::Treasury`].  Exact by
    /// construction: the three parts always sum to `fee_atoms`, so no atom
    /// is created, destroyed, or silently redirected by the split.
    pub fn allocate_split(&self, fee_atoms: u64) -> Result<RevenueSplitV1, RevenuePolicyErrorV1> {
        self.validate()?;
        let den = self.split_den as u128;
        let maker = (fee_atoms as u128 * self.maker_rebate_num as u128) / den;
        let executor = (fee_atoms as u128 * self.executor_num as u128) / den;
        let treasury = (fee_atoms as u128)
            .checked_sub(maker)
            .and_then(|rest| rest.checked_sub(executor))
            .ok_or(RevenuePolicyErrorV1::Arithmetic)?;
        // Each floor share is at most the fee, so u64 holds every part.
        let RevenueResidualV1::Treasury = self.residual;
        Ok(RevenueSplitV1 {
            maker_rebate_atoms: maker as u64,
            executor_atoms: executor as u64,
            treasury_atoms: treasury as u64,
        })
    }
}

/// Encode one validated revenue policy into exactly
/// [`REVENUE_POLICY_BYTES`] canonical bytes.
pub fn encode_revenue_policy(
    policy: &RevenuePolicyV1,
    out: &mut [u8],
) -> Result<usize, RevenuePolicyErrorV1> {
    policy.validate()?;
    if out.len() < REVENUE_POLICY_BYTES {
        return Err(RevenuePolicyErrorV1::Truncated);
    }
    let mut at = 0usize;
    out[at..at + 8].copy_from_slice(&REVENUE_POLICY_MAGIC);
    at += 8;
    out[at..at + 2].copy_from_slice(&REVENUE_POLICY_SCHEMA_V1.to_le_bytes());
    at += 2;
    out[at..at + 2].copy_from_slice(&REVENUE_POLICY_FLAGS_V1.to_le_bytes());
    at += 2;
    out[at..at + 32].copy_from_slice(&policy.treasury);
    at += 32;
    for word in [
        policy.maker_rebate_num,
        policy.executor_num,
        policy.treasury_num,
        policy.split_den,
    ] {
        out[at..at + 4].copy_from_slice(&word.to_le_bytes());
        at += 4;
    }
    out[at] = match policy.residual {
        RevenueResidualV1::Treasury => 0,
    };
    at += 1;
    out[at] = match policy.standing_maker {
        StandingMakerV1::AllRestingMakers => 0,
    };
    at += 1;
    out[at] = match policy.lamport_sink {
        LamportSinkV1::None => 0,
    };
    at += 1;
    out[at] = 0;
    at += 1;
    if at != REVENUE_POLICY_BYTES {
        return Err(RevenuePolicyErrorV1::NonCanonicalPadding);
    }
    Ok(at)
}

/// Return the exact canonical revenue-policy byte image.
pub fn canonical_revenue_policy_bytes(
    policy: &RevenuePolicyV1,
) -> Result<[u8; REVENUE_POLICY_BYTES], RevenuePolicyErrorV1> {
    let mut out = [0; REVENUE_POLICY_BYTES];
    encode_revenue_policy(policy, &mut out)?;
    Ok(out)
}

/// Decode exactly one canonical revenue-policy artifact.
pub fn decode_revenue_policy(input: &[u8]) -> Result<RevenuePolicyV1, RevenuePolicyErrorV1> {
    if input.len() < REVENUE_POLICY_BYTES {
        return Err(RevenuePolicyErrorV1::Truncated);
    }
    if input.len() > REVENUE_POLICY_BYTES {
        return Err(RevenuePolicyErrorV1::TrailingBytes);
    }
    if input[..8] != REVENUE_POLICY_MAGIC {
        return Err(RevenuePolicyErrorV1::WrongMagic);
    }
    if u16::from_le_bytes([input[8], input[9]]) != REVENUE_POLICY_SCHEMA_V1 {
        return Err(RevenuePolicyErrorV1::WrongVersion);
    }
    if u16::from_le_bytes([input[10], input[11]]) != REVENUE_POLICY_FLAGS_V1 {
        return Err(RevenuePolicyErrorV1::InvalidEnum);
    }
    let mut treasury = [0u8; 32];
    treasury.copy_from_slice(&input[12..44]);
    let mut words = [0u32; 4];
    for (index, word) in words.iter_mut().enumerate() {
        let start = 44 + index * 4;
        *word = u32::from_le_bytes([
            input[start],
            input[start + 1],
            input[start + 2],
            input[start + 3],
        ]);
    }
    let residual = match input[60] {
        0 => RevenueResidualV1::Treasury,
        _ => return Err(RevenuePolicyErrorV1::InvalidEnum),
    };
    let standing_maker = match input[61] {
        0 => StandingMakerV1::AllRestingMakers,
        _ => return Err(RevenuePolicyErrorV1::InvalidEnum),
    };
    let lamport_sink = match input[62] {
        0 => LamportSinkV1::None,
        _ => return Err(RevenuePolicyErrorV1::InvalidEnum),
    };
    if input[63] != 0 {
        return Err(RevenuePolicyErrorV1::NonCanonicalPadding);
    }
    let value = RevenuePolicyV1 {
        version: REVENUE_POLICY_SCHEMA_V1 as u32,
        treasury,
        maker_rebate_num: words[0],
        executor_num: words[1],
        treasury_num: words[2],
        split_den: words[3],
        residual,
        standing_maker,
        lamport_sink,
    };
    // Re-encoding is the final canonicality oracle (validation included).
    if canonical_revenue_policy_bytes(&value)? != input {
        return Err(RevenuePolicyErrorV1::NonCanonicalPadding);
    }
    Ok(value)
}

/// Compute the canonical immutable identity of one registered revenue policy.
pub fn revenue_policy_digest(
    policy: &RevenuePolicyV1,
) -> Result<Identity32V1, RevenuePolicyErrorV1> {
    let bytes = canonical_revenue_policy_bytes(policy)?;
    Ok(sha256::<Chosen<REVENUE_POLICY_PREIMAGE>>(
        REVENUE_POLICY_DIGEST_DOMAIN,
        &[&bytes],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hasher;

    #[test]
    fn revenue_policy_identity_value_is_pinned() {
        let policy = REVENUE_POLICY_V1;
        assert_eq!(policy.validate(), Ok(()));
        let bytes = canonical_revenue_policy_bytes(&policy).unwrap();
        assert_eq!(
            bytes,
            [
                0x44, 0x43, 0x52, 0x45, 0x56, 0x50, 0x31, 0x00, 0x01, 0x00, 0x00, 0x00, 0x52,
                0x45, 0x56, 0x45, 0x4e, 0x55, 0x45, 0x2d, 0x54, 0x52, 0x45, 0x41, 0x53, 0x55,
                0x52, 0x59, 0x2d, 0x55, 0x4e, 0x53, 0x45, 0x54, 0x2d, 0x53, 0x45, 0x4e, 0x54,
                0x49, 0x4e, 0x45, 0x4c, 0x31, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x28, 0x00, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
            ],
            "the revenue-policy artifact bytes moved"
        );
        assert_eq!(decode_revenue_policy(&bytes), Ok(policy));
        // SHA-256("dragons-clutch/revenue-policy/v1\0" || those 64 bytes),
        // taken from a third SHA-256 implementation.
        assert_eq!(
            revenue_policy_digest(&policy).unwrap().0,
            [
                0xdc, 0x0e, 0x99, 0xc6, 0xc5, 0x5a, 0x60, 0xae, 0x7d, 0x40, 0x65, 0x95, 0xc4,
                0x36, 0x03, 0x64, 0x2e, 0x86, 0xc3, 0x80, 0xc8, 0xa8, 0x83, 0x53, 0x7c, 0x20,
                0xc8, 0xc7, 0x89, 0xe8, 0xa6, 0x9b
            ],
            "the revenue-policy identity moved"
        );
        assert_eq!(
            revenue_policy_digest(&policy).unwrap(),
            sha256::<hasher::Native<{ REVENUE_POLICY_PREIMAGE }>>(
                REVENUE_POLICY_DIGEST_DOMAIN,
                &[&bytes],
            ),
            "the on-chain hasher disagrees with the pinned value"
        );
    }

    /// Every byte mutation of the canonical artifact refuses or moves both
    /// semantics and digest — the same fail-closed sweep the batch-policy
    /// codec carries.
    #[test]
    fn every_revenue_policy_byte_mutation_refuses_or_changes_semantics_and_digest() {
        let policy = REVENUE_POLICY_V1;
        let bytes = canonical_revenue_policy_bytes(&policy).unwrap();
        let digest = revenue_policy_digest(&policy).unwrap();
        for offset in 0..REVENUE_POLICY_BYTES {
            let mut mutated = bytes;
            mutated[offset] ^= 0x80;
            if let Ok(other) = decode_revenue_policy(&mutated) {
                assert_ne!(other, policy, "byte {offset} changed no semantics");
                assert_ne!(
                    revenue_policy_digest(&other).unwrap(),
                    digest,
                    "byte {offset} changed no digest"
                );
            }
        }
    }

    /// §10.6's red half: out-of-envelope consts refuse to digest.  The
    /// published envelope and the §3 identity refusals are structure, so a
    /// silently redirected or misclassified const cannot even acquire an
    /// identity to be pinned under.
    #[test]
    fn out_of_envelope_and_misclassified_consts_refuse_to_digest() {
        let cases: [(RevenuePolicyV1, RevenuePolicyErrorV1); 6] = [
            (
                RevenuePolicyV1 {
                    maker_rebate_num: 50,
                    executor_num: 20,
                    treasury_num: 30,
                    ..REVENUE_POLICY_V1
                },
                RevenuePolicyErrorV1::ExecutorAboveEnvelope,
            ),
            (
                RevenuePolicyV1 {
                    maker_rebate_num: 80,
                    executor_num: 0,
                    treasury_num: 20,
                    ..REVENUE_POLICY_V1
                },
                RevenuePolicyErrorV1::TreasuryBelowEnvelope,
            ),
            (
                RevenuePolicyV1 {
                    maker_rebate_num: 61,
                    ..REVENUE_POLICY_V1
                },
                RevenuePolicyErrorV1::SplitSumMismatch,
            ),
            (
                RevenuePolicyV1 {
                    split_den: 0,
                    maker_rebate_num: 0,
                    executor_num: 0,
                    treasury_num: 0,
                    ..REVENUE_POLICY_V1
                },
                RevenuePolicyErrorV1::ZeroDenominator,
            ),
            (
                RevenuePolicyV1 {
                    treasury: [0u8; 32],
                    ..REVENUE_POLICY_V1
                },
                RevenuePolicyErrorV1::UnownedTreasury,
            ),
            (
                RevenuePolicyV1 {
                    treasury: REVENUE_NEUTRAL_SINK_BYTES_V1,
                    ..REVENUE_POLICY_V1
                },
                RevenuePolicyErrorV1::SinkTreasury,
            ),
        ];
        for (policy, refusal) in cases {
            assert_eq!(policy.validate(), Err(refusal));
            assert_eq!(revenue_policy_digest(&policy), Err(refusal));
        }
    }

    /// B4a's structural deferral: the sentinel validates as a frozen state
    /// and refuses fee-bearing admission; so do the zero identity and the
    /// sink, defensively.
    #[test]
    fn the_unset_sentinel_refuses_fee_bearing_admission() {
        assert!(!treasury_admits_fee_bearing(&REVENUE_TREASURY_UNSET_V1));
        assert!(!treasury_admits_fee_bearing(&[0u8; 32]));
        assert!(!treasury_admits_fee_bearing(&REVENUE_NEUTRAL_SINK_BYTES_V1));
        assert!(treasury_admits_fee_bearing(&[7u8; 32]));
        assert_eq!(REVENUE_POLICY_V1.treasury, REVENUE_TREASURY_UNSET_V1);
    }

    /// §10.6 split exactness — at zero (the only reachable fee today: an
    /// exact zero split with no residual leak) and across the small-fee
    /// range, where the frozen rounding rules are asserted as structure:
    /// rebates and executor floor, treasury takes the exact remainder, and
    /// the three parts always sum to the fee.
    #[test]
    fn split_exactness_at_sixty_zero_forty_including_zero() {
        assert_eq!(
            REVENUE_POLICY_V1.allocate_split(0),
            Ok(RevenueSplitV1 {
                maker_rebate_atoms: 0,
                executor_atoms: 0,
                treasury_atoms: 0,
            })
        );
        for fee in 0..=1_000u64 {
            let split = REVENUE_POLICY_V1.allocate_split(fee).unwrap();
            assert_eq!(split.maker_rebate_atoms, fee * 60 / 100, "rebate floors");
            assert_eq!(split.executor_atoms, 0, "V1 executor share is zero");
            assert_eq!(
                split.maker_rebate_atoms + split.executor_atoms + split.treasury_atoms,
                fee,
                "no atom is created or destroyed by the split"
            );
            // Residual atoms ride the treasury share: the treasury never
            // receives less than its floor share.
            assert!(split.treasury_atoms >= fee * 40 / 100);
        }
    }
}
