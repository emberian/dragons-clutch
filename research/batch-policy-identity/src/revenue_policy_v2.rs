//! Immutable fee-bearing revenue policy for successor Realms.
//!
//! V1 remains the historical zero-fee/deferred-treasury shape.  V2 is a new
//! transcript: it binds both composite-fee rates, the split, the exact maker
//! evidence class, and the ordinary treasury owner selected at Realm birth.
//! Absence of a V2 Realm record still means zero fee; there is no retrofit or
//! treasury-rotation interpretation of these bytes.

use crate::hasher::Chosen;
use crate::revenue_policy_v1::{
    REVENUE_NEUTRAL_SINK_BYTES_V1, REVENUE_TREASURY_UNSET_V1,
};
use crate::{sha256, Identity32V1};

/// Canonical V2 revenue-policy width.
pub const REVENUE_POLICY_V2_BYTES: usize = 80;
/// ASCII `DCREVP2` followed by one zero byte.
pub const REVENUE_POLICY_V2_MAGIC: [u8; 8] = *b"DCREVP2\0";
/// Canonical V2 schema.
pub const REVENUE_POLICY_SCHEMA_V2: u16 = 2;
/// SHA-256 transcript domain for V2 policy identities.
pub const REVENUE_POLICY_V2_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/revenue-policy/v2\0";
/// SHA-256 transcript domain for the selected treasury-Position lifecycle.
pub const TREASURY_POSITION_DERIVATION_V2_DIGEST_DOMAIN: &[u8] =
    b"dragons-clutch/treasury-position-derivation/v2\0";
/// SHA-256 transcript domain for one Realm's semantic revenue-record ID.
pub const REVENUE_POLICY_RECORD_V2_ID_DOMAIN: &[u8] =
    b"dragons-clutch/revenue-policy-record/v2\0";
/// Basis-point denominator used by both V2 composite-fee rates.
pub const REVENUE_POLICY_V2_BPS_DENOMINATOR: u32 = 10_000;
/// Unified successor development dispersion rate: 40 basis points.
pub const SUCCESSOR_DEV_DISPERSION_BPS: u32 = 40;
/// Unified successor development range-floor rate: 10 basis points.
pub const SUCCESSOR_DEV_FLOOR_RANGE_BPS: u32 = 10;
/// Unified successor development maker split numerator.
pub const SUCCESSOR_DEV_MAKER_REBATE_NUM: u32 = 60;
/// Unified successor development executor split numerator.
pub const SUCCESSOR_DEV_EXECUTOR_NUM: u32 = 0;
/// Unified successor development treasury split numerator.
pub const SUCCESSOR_DEV_TREASURY_NUM: u32 = 40;
/// Unified successor development split denominator.
pub const SUCCESSOR_DEV_SPLIT_DEN: u32 = 100;

const REVENUE_POLICY_V2_FLAGS: u16 = 0;
const REVENUE_POLICY_V2_RESERVED_BYTES: usize = 8;
const REVENUE_POLICY_V2_PREIMAGE: usize =
    REVENUE_POLICY_V2_DIGEST_DOMAIN.len() + REVENUE_POLICY_V2_BYTES;
const TREASURY_POSITION_DERIVATION_V2_PREIMAGE: usize =
    TREASURY_POSITION_DERIVATION_V2_DIGEST_DOMAIN.len() + 1;
const REVENUE_POLICY_RECORD_V2_ID_PREIMAGE: usize =
    REVENUE_POLICY_RECORD_V2_ID_DOMAIN.len() + 32 + 32 + 32 + 32;
const _: () = assert!(REVENUE_POLICY_V2_BYTES == 8 + 2 + 2 + 32 + 8 + 16 + 4 + 8);

/// Destination for split-rounding residual atoms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevenueResidualV2 {
    /// Assign every residual atom to the ordinary treasury Position.
    Treasury,
}

impl RevenueResidualV2 {
    const fn byte(self) -> u8 {
        match self {
            Self::Treasury => 0,
        }
    }

    fn decode(byte: u8) -> Result<Self, RevenuePolicyErrorV2> {
        match byte {
            0 => Ok(Self::Treasury),
            _ => Err(RevenuePolicyErrorV2::InvalidEnum),
        }
    }
}

/// Semantic authority for maker-rebate weights.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MakerWeightAuthorityV2 {
    /// One private settlement traversal certifies, per owner, the exact sum of
    /// composite fee numerators over that owner's resting fills.  No public
    /// row may supply or override a weight.
    CertifiedOwnerNettedCompositeNumerator,
}

impl MakerWeightAuthorityV2 {
    const fn byte(self) -> u8 {
        match self {
            Self::CertifiedOwnerNettedCompositeNumerator => 0,
        }
    }

    fn decode(byte: u8) -> Result<Self, RevenuePolicyErrorV2> {
        match byte {
            0 => Ok(Self::CertifiedOwnerNettedCompositeNumerator),
            _ => Err(RevenuePolicyErrorV2::InvalidEnum),
        }
    }
}

/// Plane-L disposition selected by this atom-denominated policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LamportSinkV2 {
    /// No lamport revenue or liveness funding exists in this policy.
    None,
}

impl LamportSinkV2 {
    const fn byte(self) -> u8 {
        match self {
            Self::None => 0,
        }
    }

    fn decode(byte: u8) -> Result<Self, RevenuePolicyErrorV2> {
        match byte {
            0 => Ok(Self::None),
            _ => Err(RevenuePolicyErrorV2::InvalidEnum),
        }
    }
}

/// Exact lifecycle required for a V2 policy's treasury custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreasuryPositionDerivationPolicyV2 {
    /// For each full-width MarketInstanceV2, Product/General founding derives
    /// and separately rent-funds an ordinary `PositionV3` owned and
    /// controlled by the Realm's immutable treasury owner, its mandatory
    /// purpose-owned GEN1 ReplayV3, plus a counted service ledger that
    /// prevents closure while any fee-bearing epoch is outstanding.  The
    /// Position/Replay purpose binding is the canonical General MarketRuntime,
    /// not the MarketInstance.  None is a Realm account or Product ScheduleV3
    /// funding slot.
    PerMarketOrdinaryGeneralPositionV3WithCountedServiceLedgerV1,
}

impl TreasuryPositionDerivationPolicyV2 {
    /// Canonical transcript byte for this closed-set member.
    pub const fn byte(self) -> u8 {
        match self {
            Self::PerMarketOrdinaryGeneralPositionV3WithCountedServiceLedgerV1 => 0,
        }
    }

    /// Decode one canonical transcript byte.
    pub fn decode(byte: u8) -> Result<Self, RevenuePolicyErrorV2> {
        match byte {
            0 => Ok(Self::PerMarketOrdinaryGeneralPositionV3WithCountedServiceLedgerV1),
            _ => Err(RevenuePolicyErrorV2::InvalidEnum),
        }
    }
}

/// One immutable, registered V2 revenue policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevenuePolicyV2 {
    /// Must equal [`REVENUE_POLICY_SCHEMA_V2`].
    pub version: u16,
    /// Nonzero, non-sink owner of each Market's ordinary treasury Position.
    pub treasury_owner: [u8; 32],
    /// Composite-dispersion rate in basis points.
    pub dispersion_bps: u32,
    /// Composite range-floor rate in basis points.
    pub floor_range_bps: u32,
    /// Maker-rebate share numerator.
    pub maker_rebate_num: u32,
    /// Executor share numerator.  V2 requires zero because it owns no
    /// authenticated executor identity.
    pub executor_num: u32,
    /// Treasury share numerator.
    pub treasury_num: u32,
    /// Exact split denominator.
    pub split_den: u32,
    /// Residual-atom disposition.
    pub residual: RevenueResidualV2,
    /// Sole admissible maker-weight evidence class.
    pub maker_weight_authority: MakerWeightAuthorityV2,
    /// Lamport plane remains absent and unit-disjoint.
    pub lamport_sink: LamportSinkV2,
    /// Market-scoped ordinary-Position derivation and counted-close policy.
    pub treasury_position_derivation: TreasuryPositionDerivationPolicyV2,
}

/// Refusal from a V2 policy validator or codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevenuePolicyErrorV2 {
    /// Unknown schema version.
    WrongVersion,
    /// A composite-fee rate is outside the basis-point domain.
    RateOutOfRange,
    /// Both composite-fee rates are zero; absence, not a zero-valued record,
    /// is the zero-fee representation.
    ZeroRate,
    /// The split denominator is zero.
    ZeroDenominator,
    /// Share numerators do not sum exactly to the denominator.
    SplitSumMismatch,
    /// V2 has no authenticated executor identity and therefore refuses a
    /// nonzero executor share.
    ExecutorIdentityAbsent,
    /// Treasury owner is the all-zero key.
    UnownedTreasury,
    /// Treasury owner is the V1 deferred sentinel.
    DeferredTreasury,
    /// Treasury owner is the neutral sink.
    SinkTreasury,
    /// Wrong transcript magic.
    WrongMagic,
    /// Input is shorter than one canonical transcript.
    Truncated,
    /// Input is longer than one canonical transcript.
    TrailingBytes,
    /// Reserved bytes or re-encoding are noncanonical.
    NonCanonicalPadding,
    /// An enum byte has no registered meaning.
    InvalidEnum,
    /// Checked arithmetic failed.
    Arithmetic,
}

/// Exact terminal split of collected collateral atoms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevenueSplitV2 {
    /// Maker-rebate atoms, rounded down.
    pub maker_rebate_atoms: u64,
    /// Executor atoms; always zero in V2.
    pub executor_atoms: u64,
    /// Treasury atoms, including every residual atom.
    pub treasury_atoms: u64,
}

impl RevenuePolicyV2 {
    /// Construct the unified development policy around a Realm founder's
    /// immutable treasury owner.  The owner is data, never a program constant.
    pub const fn successor_development(treasury_owner: [u8; 32]) -> Self {
        Self {
            version: REVENUE_POLICY_SCHEMA_V2,
            treasury_owner,
            dispersion_bps: SUCCESSOR_DEV_DISPERSION_BPS,
            floor_range_bps: SUCCESSOR_DEV_FLOOR_RANGE_BPS,
            maker_rebate_num: SUCCESSOR_DEV_MAKER_REBATE_NUM,
            executor_num: SUCCESSOR_DEV_EXECUTOR_NUM,
            treasury_num: SUCCESSOR_DEV_TREASURY_NUM,
            split_den: SUCCESSOR_DEV_SPLIT_DEN,
            residual: RevenueResidualV2::Treasury,
            maker_weight_authority:
                MakerWeightAuthorityV2::CertifiedOwnerNettedCompositeNumerator,
            lamport_sink: LamportSinkV2::None,
            treasury_position_derivation:
                TreasuryPositionDerivationPolicyV2::PerMarketOrdinaryGeneralPositionV3WithCountedServiceLedgerV1,
        }
    }

    /// Validate the generic immutable V2 envelope.
    pub fn validate(&self) -> Result<(), RevenuePolicyErrorV2> {
        if self.version != REVENUE_POLICY_SCHEMA_V2 {
            return Err(RevenuePolicyErrorV2::WrongVersion);
        }
        if self.treasury_owner == [0; 32] {
            return Err(RevenuePolicyErrorV2::UnownedTreasury);
        }
        if self.treasury_owner == REVENUE_TREASURY_UNSET_V1 {
            return Err(RevenuePolicyErrorV2::DeferredTreasury);
        }
        if self.treasury_owner == REVENUE_NEUTRAL_SINK_BYTES_V1 {
            return Err(RevenuePolicyErrorV2::SinkTreasury);
        }
        if self.dispersion_bps > REVENUE_POLICY_V2_BPS_DENOMINATOR
            || self.floor_range_bps > REVENUE_POLICY_V2_BPS_DENOMINATOR
        {
            return Err(RevenuePolicyErrorV2::RateOutOfRange);
        }
        if self.dispersion_bps == 0 && self.floor_range_bps == 0 {
            return Err(RevenuePolicyErrorV2::ZeroRate);
        }
        if self.split_den == 0 {
            return Err(RevenuePolicyErrorV2::ZeroDenominator);
        }
        let split_sum = u64::from(self.maker_rebate_num)
            .checked_add(u64::from(self.executor_num))
            .and_then(|sum| sum.checked_add(u64::from(self.treasury_num)))
            .ok_or(RevenuePolicyErrorV2::Arithmetic)?;
        if split_sum != u64::from(self.split_den) {
            return Err(RevenuePolicyErrorV2::SplitSumMismatch);
        }
        if self.executor_num != 0 {
            return Err(RevenuePolicyErrorV2::ExecutorIdentityAbsent);
        }
        Ok(())
    }

    /// Whether this generic V2 member is the exact policy selected by the
    /// unified successor development profile.
    pub fn is_successor_development_profile(&self) -> bool {
        self.validate().is_ok()
            && self.dispersion_bps == SUCCESSOR_DEV_DISPERSION_BPS
            && self.floor_range_bps == SUCCESSOR_DEV_FLOOR_RANGE_BPS
            && self.maker_rebate_num == SUCCESSOR_DEV_MAKER_REBATE_NUM
            && self.executor_num == SUCCESSOR_DEV_EXECUTOR_NUM
            && self.treasury_num == SUCCESSOR_DEV_TREASURY_NUM
            && self.split_den == SUCCESSOR_DEV_SPLIT_DEN
            && self.residual == RevenueResidualV2::Treasury
            && self.maker_weight_authority
                == MakerWeightAuthorityV2::CertifiedOwnerNettedCompositeNumerator
            && self.lamport_sink == LamportSinkV2::None
            && self.treasury_position_derivation
                == TreasuryPositionDerivationPolicyV2::PerMarketOrdinaryGeneralPositionV3WithCountedServiceLedgerV1
    }

    /// Split terminal collected fee atoms.  Maker and executor shares round
    /// down once; the treasury receives the exact remainder.
    pub fn allocate_split(&self, fee_atoms: u64) -> Result<RevenueSplitV2, RevenuePolicyErrorV2> {
        self.validate()?;
        let denominator = u128::from(self.split_den);
        let maker = u128::from(fee_atoms)
            .checked_mul(u128::from(self.maker_rebate_num))
            .ok_or(RevenuePolicyErrorV2::Arithmetic)?
            / denominator;
        let executor = u128::from(fee_atoms)
            .checked_mul(u128::from(self.executor_num))
            .ok_or(RevenuePolicyErrorV2::Arithmetic)?
            / denominator;
        let treasury = u128::from(fee_atoms)
            .checked_sub(maker)
            .and_then(|remainder| remainder.checked_sub(executor))
            .ok_or(RevenuePolicyErrorV2::Arithmetic)?;
        Ok(RevenueSplitV2 {
            maker_rebate_atoms: u64::try_from(maker)
                .map_err(|_| RevenuePolicyErrorV2::Arithmetic)?,
            executor_atoms: u64::try_from(executor)
                .map_err(|_| RevenuePolicyErrorV2::Arithmetic)?,
            treasury_atoms: u64::try_from(treasury)
                .map_err(|_| RevenuePolicyErrorV2::Arithmetic)?,
        })
    }
}

/// Encode exactly one validated V2 policy transcript.
pub fn encode_revenue_policy_v2(
    policy: &RevenuePolicyV2,
    out: &mut [u8],
) -> Result<usize, RevenuePolicyErrorV2> {
    policy.validate()?;
    if out.len() < REVENUE_POLICY_V2_BYTES {
        return Err(RevenuePolicyErrorV2::Truncated);
    }
    let mut at = 0usize;
    out[at..at + 8].copy_from_slice(&REVENUE_POLICY_V2_MAGIC);
    at += 8;
    out[at..at + 2].copy_from_slice(&REVENUE_POLICY_SCHEMA_V2.to_le_bytes());
    at += 2;
    out[at..at + 2].copy_from_slice(&REVENUE_POLICY_V2_FLAGS.to_le_bytes());
    at += 2;
    out[at..at + 32].copy_from_slice(&policy.treasury_owner);
    at += 32;
    for value in [
        policy.dispersion_bps,
        policy.floor_range_bps,
        policy.maker_rebate_num,
        policy.executor_num,
        policy.treasury_num,
        policy.split_den,
    ] {
        out[at..at + 4].copy_from_slice(&value.to_le_bytes());
        at += 4;
    }
    out[at] = policy.residual.byte();
    at += 1;
    out[at] = policy.maker_weight_authority.byte();
    at += 1;
    out[at] = policy.lamport_sink.byte();
    at += 1;
    out[at] = policy.treasury_position_derivation.byte();
    at += 1;
    out[at..at + REVENUE_POLICY_V2_RESERVED_BYTES].fill(0);
    at += REVENUE_POLICY_V2_RESERVED_BYTES;
    if at != REVENUE_POLICY_V2_BYTES {
        return Err(RevenuePolicyErrorV2::NonCanonicalPadding);
    }
    Ok(at)
}

/// Return the exact canonical transcript bytes.
pub fn canonical_revenue_policy_v2_bytes(
    policy: &RevenuePolicyV2,
) -> Result<[u8; REVENUE_POLICY_V2_BYTES], RevenuePolicyErrorV2> {
    let mut out = [0; REVENUE_POLICY_V2_BYTES];
    encode_revenue_policy_v2(policy, &mut out)?;
    Ok(out)
}

/// Decode exactly one canonical V2 transcript.
pub fn decode_revenue_policy_v2(input: &[u8]) -> Result<RevenuePolicyV2, RevenuePolicyErrorV2> {
    if input.len() < REVENUE_POLICY_V2_BYTES {
        return Err(RevenuePolicyErrorV2::Truncated);
    }
    if input.len() > REVENUE_POLICY_V2_BYTES {
        return Err(RevenuePolicyErrorV2::TrailingBytes);
    }
    if input[..8] != REVENUE_POLICY_V2_MAGIC {
        return Err(RevenuePolicyErrorV2::WrongMagic);
    }
    if u16::from_le_bytes([input[8], input[9]]) != REVENUE_POLICY_SCHEMA_V2 {
        return Err(RevenuePolicyErrorV2::WrongVersion);
    }
    if u16::from_le_bytes([input[10], input[11]]) != REVENUE_POLICY_V2_FLAGS {
        return Err(RevenuePolicyErrorV2::NonCanonicalPadding);
    }
    let mut treasury_owner = [0u8; 32];
    treasury_owner.copy_from_slice(&input[12..44]);
    let mut values = [0u32; 6];
    for (index, value) in values.iter_mut().enumerate() {
        let start = 44 + index * 4;
        *value = u32::from_le_bytes([
            input[start],
            input[start + 1],
            input[start + 2],
            input[start + 3],
        ]);
    }
    let policy = RevenuePolicyV2 {
        version: REVENUE_POLICY_SCHEMA_V2,
        treasury_owner,
        dispersion_bps: values[0],
        floor_range_bps: values[1],
        maker_rebate_num: values[2],
        executor_num: values[3],
        treasury_num: values[4],
        split_den: values[5],
        residual: RevenueResidualV2::decode(input[68])?,
        maker_weight_authority: MakerWeightAuthorityV2::decode(input[69])?,
        lamport_sink: LamportSinkV2::decode(input[70])?,
        treasury_position_derivation: TreasuryPositionDerivationPolicyV2::decode(input[71])?,
    };
    if input[72..80] != [0; REVENUE_POLICY_V2_RESERVED_BYTES] {
        return Err(RevenuePolicyErrorV2::NonCanonicalPadding);
    }
    if canonical_revenue_policy_v2_bytes(&policy)? != input {
        return Err(RevenuePolicyErrorV2::NonCanonicalPadding);
    }
    Ok(policy)
}

/// Compute the typed identity of one treasury-Position lifecycle selector.
pub fn treasury_position_derivation_policy_v2_id(
    policy: TreasuryPositionDerivationPolicyV2,
) -> Identity32V1 {
    sha256::<Chosen<TREASURY_POSITION_DERIVATION_V2_PREIMAGE>>(
        TREASURY_POSITION_DERIVATION_V2_DIGEST_DOMAIN,
        &[&[policy.byte()]],
    )
}

/// Compute the semantic ID of a Realm's immutable V2 record.  Physical rent,
/// bump, and account address are deliberately outside this ID; the immutable
/// economic owner and lifecycle selector are inside it.
pub fn revenue_policy_record_v2_id(
    realm: [u8; 32],
    policy: &RevenuePolicyV2,
) -> Result<Identity32V1, RevenuePolicyErrorV2> {
    let policy_digest = revenue_policy_v2_digest(policy)?;
    let derivation_id =
        treasury_position_derivation_policy_v2_id(policy.treasury_position_derivation);
    Ok(sha256::<Chosen<REVENUE_POLICY_RECORD_V2_ID_PREIMAGE>>(
        REVENUE_POLICY_RECORD_V2_ID_DOMAIN,
        &[
            &realm,
            &policy_digest.0,
            &policy.treasury_owner,
            &derivation_id.0,
        ],
    ))
}

/// Compute the immutable V2 revenue-policy identity.
pub fn revenue_policy_v2_digest(
    policy: &RevenuePolicyV2,
) -> Result<Identity32V1, RevenuePolicyErrorV2> {
    let bytes = canonical_revenue_policy_v2_bytes(policy)?;
    Ok(sha256::<Chosen<REVENUE_POLICY_V2_PREIMAGE>>(
        REVENUE_POLICY_V2_DIGEST_DOMAIN,
        &[&bytes],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> RevenuePolicyV2 {
        RevenuePolicyV2::successor_development([7; 32])
    }

    #[test]
    fn successor_policy_roundtrips_and_splits_exactly() {
        let policy = policy();
        assert!(policy.is_successor_development_profile());
        let bytes = canonical_revenue_policy_v2_bytes(&policy).unwrap();
        assert_eq!(decode_revenue_policy_v2(&bytes), Ok(policy));
        let split = policy.allocate_split(101).unwrap();
        assert_eq!(split.maker_rebate_atoms, 60);
        assert_eq!(split.executor_atoms, 0);
        assert_eq!(split.treasury_atoms, 41);
    }

    #[test]
    fn identity_binds_every_economic_and_owner_field() {
        let policy = policy();
        let digest = revenue_policy_v2_digest(&policy).unwrap();
        for changed in [
            RevenuePolicyV2 { treasury_owner: [8; 32], ..policy },
            RevenuePolicyV2 { dispersion_bps: 41, ..policy },
            RevenuePolicyV2 { floor_range_bps: 11, ..policy },
            RevenuePolicyV2 { maker_rebate_num: 59, treasury_num: 41, ..policy },
        ] {
            assert_ne!(revenue_policy_v2_digest(&changed).unwrap(), digest);
        }
        assert_ne!(
            revenue_policy_record_v2_id([1; 32], &policy).unwrap(),
            revenue_policy_record_v2_id([2; 32], &policy).unwrap()
        );
    }

    #[test]
    fn invalid_treasury_rate_split_and_executor_refuse() {
        let valid = policy();
        let cases = [
            (RevenuePolicyV2 { treasury_owner: [0; 32], ..valid }, RevenuePolicyErrorV2::UnownedTreasury),
            (RevenuePolicyV2 { treasury_owner: REVENUE_TREASURY_UNSET_V1, ..valid }, RevenuePolicyErrorV2::DeferredTreasury),
            (RevenuePolicyV2 { treasury_owner: REVENUE_NEUTRAL_SINK_BYTES_V1, ..valid }, RevenuePolicyErrorV2::SinkTreasury),
            (RevenuePolicyV2 { dispersion_bps: 10_001, ..valid }, RevenuePolicyErrorV2::RateOutOfRange),
            (RevenuePolicyV2 { dispersion_bps: 0, floor_range_bps: 0, ..valid }, RevenuePolicyErrorV2::ZeroRate),
            (RevenuePolicyV2 { split_den: 0, ..valid }, RevenuePolicyErrorV2::ZeroDenominator),
            (RevenuePolicyV2 { maker_rebate_num: 61, ..valid }, RevenuePolicyErrorV2::SplitSumMismatch),
            (RevenuePolicyV2 { maker_rebate_num: 59, executor_num: 1, ..valid }, RevenuePolicyErrorV2::ExecutorIdentityAbsent),
        ];
        for (candidate, expected) in cases {
            assert_eq!(candidate.validate(), Err(expected));
        }
        assert!(RevenuePolicyV2 {
            maker_rebate_num: 90,
            treasury_num: 10,
            ..valid
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn codec_refuses_width_enums_and_every_reserved_byte() {
        let bytes = canonical_revenue_policy_v2_bytes(&policy()).unwrap();
        assert_eq!(decode_revenue_policy_v2(&bytes[..79]), Err(RevenuePolicyErrorV2::Truncated));
        let mut long = [0u8; 81];
        long[..80].copy_from_slice(&bytes);
        assert_eq!(decode_revenue_policy_v2(&long), Err(RevenuePolicyErrorV2::TrailingBytes));
        for index in [10usize, 11, 68, 69, 70, 71, 72, 79] {
            let mut hostile = bytes;
            hostile[index] = 0xff;
            assert!(decode_revenue_policy_v2(&hostile).is_err());
        }
    }
}
