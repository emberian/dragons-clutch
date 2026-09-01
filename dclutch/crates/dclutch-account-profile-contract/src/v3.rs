//! Explicit funding-owned account lifecycle declarations over AccountProfile V2.
//!
//! The embedded V2 profile remains the sole owner of accounts, aliases,
//! privileges, projections, and exact live data widths. This successor refines
//! a bounded set of fixed `LifecycleBound` coordinates as funding-owned. A
//! selected Effect successor must join every declaration before the runtime may
//! create or close one of those accounts. StateLifecyclePolicy never acquires
//! authority over a refined coordinate.

use dclutch_capability_seal_contract::{SealedArtifactV1, SealedRoleV1};

use crate::v2::{AccountPrestateV2, AccountProfileV2, AliasKindV2, Error as ErrorV2};

/// Distinct successor magic.
pub const MAGIC_V3: [u8; 8] = *b"DCLTAP03";
/// Successor wire version.
pub const VERSION_V3: u16 = 3;
/// Finalized-record schema label.
pub const SCHEMA_RELEASE_PREIMAGE_V3: &[u8] = b"dclutch/schema/account-profile-v3-funding-bound-v1";
/// SHA-256 of [`SCHEMA_RELEASE_PREIMAGE_V3`].
pub const SCHEMA_RELEASE_ID_V3: [u8; 32] = [
    0x1c, 0x8c, 0x62, 0x8c, 0xd1, 0xc5, 0x81, 0xb4, 0xe4, 0xd8, 0xc7, 0x84, 0x6b, 0x58, 0xdd, 0x7e,
    0xe2, 0x8d, 0x5d, 0x2e, 0x8d, 0x5f, 0x55, 0x48, 0x7c, 0x64, 0xbf, 0xf9, 0x22, 0xe1, 0xf8, 0xa4,
];
/// Exact successor header width.
pub const HEADER_BYTES_V3: usize = 24;
/// Exact width of one funding-bound declaration.
pub const FUNDING_BOUND_BYTES_V3: usize = 8;

/// Funding actions an exact coordinate may admit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingActionMaskV3(u8);

impl FundingActionMaskV3 {
    /// Permit funding-owned creation only.
    pub const CREATE: Self = Self(1);
    /// Permit funding-owned closure only.
    pub const CLOSE: Self = Self(2);
    /// Permit both funding-owned operations.
    pub const CREATE_AND_CLOSE: Self = Self(3);

    /// Canonical bit representation.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Whether creation is selected.
    pub const fn permits_create(self) -> bool {
        self.0 & Self::CREATE.0 != 0
    }

    /// Whether closure is selected.
    pub const fn permits_close(self) -> bool {
        self.0 & Self::CLOSE.0 != 0
    }

    fn decode(value: u8) -> Result<Self, ErrorV3> {
        if value == 0 || value & !Self::CREATE_AND_CLOSE.0 != 0 {
            Err(ErrorV3::FundingTable)
        } else {
            Ok(Self(value))
        }
    }
}

/// One fixed, self-representative account whose lifecycle belongs to funding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingBoundV3 {
    coordinate: u16,
    actions: FundingActionMaskV3,
    live_bytes: u32,
}

impl FundingBoundV3 {
    /// Construct one funding-bound declaration.
    pub const fn new(coordinate: u16, actions: FundingActionMaskV3, live_bytes: u32) -> Self {
        Self {
            coordinate,
            actions,
            live_bytes,
        }
    }

    /// Fixed logical account coordinate.
    pub const fn coordinate(self) -> u16 {
        self.coordinate
    }

    /// Exact admitted funding actions.
    pub const fn actions(self) -> FundingActionMaskV3 {
        self.actions
    }

    /// Exact nonzero live data width.
    pub const fn live_bytes(self) -> u32 {
        self.live_bytes
    }

    fn decode(bytes: &[u8], offset: usize) -> Result<Self, ErrorV3> {
        Ok(Self {
            coordinate: read_u16(bytes, offset)?,
            actions: FundingActionMaskV3::decode(read_u8(bytes, offset + 2)?)?,
            live_bytes: read_u32(bytes, offset + 4)?,
        })
        .and_then(|value| {
            if read_u8(bytes, offset + 3)? != 0 {
                Err(ErrorV3::Wire)
            } else {
                Ok(value)
            }
        })
    }
}

/// Stable hostile-decode or cross-profile refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorV3 {
    /// Magic, version, reserved bytes, or exact width differed.
    Wire,
    /// Embedded V2 account profile refused.
    BaseProfile,
    /// Funding declarations were unordered, duplicated, or malformed.
    FundingTable,
    /// A declaration did not refine one exact fixed LifecycleBound rule.
    ProfileMismatch,
    /// Checked width arithmetic overflowed.
    Arithmetic,
}

impl From<ErrorV2> for ErrorV3 {
    fn from(_: ErrorV2) -> Self {
        Self::BaseProfile
    }
}

/// Result alias for funding-bound profiles.
pub type ResultV3<T> = core::result::Result<T, ErrorV3>;

/// Borrowed exact V2 profile plus its funding-owned refinement table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountProfileV3<'a> {
    bytes: &'a [u8],
    base: AccountProfileV2<'a>,
    funding_count: u16,
}

impl<'a> AccountProfileV3<'a> {
    /// Hostile-decode the complete successor and embedded V2 profile.
    pub fn decode(bytes: &'a [u8]) -> ResultV3<Self> {
        let value = Self::decode_shape(bytes)?;
        value.validate_funding_table()?;
        Ok(value)
    }

    /// Decode bytes already authenticated by one current Trading seal.
    pub fn from_sealed(bytes: &'a [u8], sealed: SealedArtifactV1<'_>) -> ResultV3<Self> {
        sealed
            .require(SealedRoleV1::AccountProfile, bytes)
            .map_err(|_| ErrorV3::Wire)?;
        Self::decode(bytes)
    }

    fn decode_shape(bytes: &'a [u8]) -> ResultV3<Self> {
        if bytes.len() < HEADER_BYTES_V3
            || bytes.get(..8) != Some(MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != VERSION_V3
            || read_u16(bytes, 12)? != 0
            || slice(bytes, 18, 6)?.iter().any(|byte| *byte != 0)
        {
            return Err(ErrorV3::Wire);
        }
        let funding_count = read_u16(bytes, 10)?;
        let base_bytes = usize::try_from(read_u32(bytes, 14)?).map_err(|_| ErrorV3::Wire)?;
        let base_start = HEADER_BYTES_V3
            .checked_add(
                usize::from(funding_count)
                    .checked_mul(FUNDING_BOUND_BYTES_V3)
                    .ok_or(ErrorV3::Arithmetic)?,
            )
            .ok_or(ErrorV3::Arithmetic)?;
        let base_end = base_start
            .checked_add(base_bytes)
            .ok_or(ErrorV3::Arithmetic)?;
        if base_bytes == 0 || base_end != bytes.len() {
            return Err(ErrorV3::Wire);
        }
        let base = AccountProfileV2::decode(slice(bytes, base_start, base_bytes)?)?;
        Ok(Self {
            bytes,
            base,
            funding_count,
        })
    }

    /// Complete canonical successor bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Embedded exact V2 account profile.
    pub const fn base(self) -> AccountProfileV2<'a> {
        self.base
    }

    /// Number of ordered funding-bound coordinates.
    pub const fn funding_bound_count(self) -> u16 {
        self.funding_count
    }

    /// Decode one ordered funding-bound declaration.
    pub fn funding_bound(self, index: u16) -> ResultV3<FundingBoundV3> {
        if index >= self.funding_count {
            return Err(ErrorV3::FundingTable);
        }
        let offset = HEADER_BYTES_V3
            .checked_add(
                usize::from(index)
                    .checked_mul(FUNDING_BOUND_BYTES_V3)
                    .ok_or(ErrorV3::Arithmetic)?,
            )
            .ok_or(ErrorV3::Arithmetic)?;
        FundingBoundV3::decode(self.bytes, offset)
    }

    /// Find the declaration for one fixed coordinate.
    pub fn funding_bound_for(self, coordinate: u16) -> ResultV3<Option<FundingBoundV3>> {
        let mut index = 0_u16;
        while index < self.funding_count {
            let bound = self.funding_bound(index)?;
            if bound.coordinate == coordinate {
                return Ok(Some(bound));
            }
            if bound.coordinate > coordinate {
                return Ok(None);
            }
            index = index.checked_add(1).ok_or(ErrorV3::Arithmetic)?;
        }
        Ok(None)
    }

    fn validate_funding_table(self) -> ResultV3<()> {
        let mut prior = None;
        let mut index = 0_u16;
        while index < self.funding_count {
            let bound = self.funding_bound(index)?;
            if bound.live_bytes == 0
                || bound.coordinate >= self.base.fixed_account_count()
                || prior.is_some_and(|prior| prior >= bound.coordinate)
            {
                return Err(ErrorV3::FundingTable);
            }
            let rule = self.base.rule(false, bound.coordinate)?;
            if rule.prestate() != AccountPrestateV2::LifecycleBound
                || rule.alias_kind() != AliasKindV2::SelfCoordinate
                || rule.alias_index() != 0
                || rule.data_length() != bound.live_bytes
                || rule.data_item_stride() != 0
            {
                return Err(ErrorV3::ProfileMismatch);
            }
            prior = Some(bound.coordinate);
            index = index.checked_add(1).ok_or(ErrorV3::Arithmetic)?;
        }
        Ok(())
    }
}

/// Encode one funding-bound successor atomically around exact V2 bytes.
pub fn encode_account_profile_v3_atomic(
    base_profile: &[u8],
    funding: &[FundingBoundV3],
    scratch: &mut [u8],
    output: &mut [u8],
) -> ResultV3<()> {
    AccountProfileV2::decode(base_profile)?;
    let expected = HEADER_BYTES_V3
        .checked_add(
            funding
                .len()
                .checked_mul(FUNDING_BOUND_BYTES_V3)
                .ok_or(ErrorV3::Arithmetic)?,
        )
        .and_then(|value| value.checked_add(base_profile.len()))
        .ok_or(ErrorV3::Arithmetic)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(ErrorV3::Wire);
    }
    scratch.fill(0);
    put(scratch, 0, &MAGIC_V3)?;
    put(scratch, 8, &VERSION_V3.to_le_bytes())?;
    put(
        scratch,
        10,
        &u16::try_from(funding.len())
            .map_err(|_| ErrorV3::Arithmetic)?
            .to_le_bytes(),
    )?;
    put(
        scratch,
        14,
        &u32::try_from(base_profile.len())
            .map_err(|_| ErrorV3::Arithmetic)?
            .to_le_bytes(),
    )?;
    for (index, bound) in funding.iter().copied().enumerate() {
        let offset = HEADER_BYTES_V3
            .checked_add(
                index
                    .checked_mul(FUNDING_BOUND_BYTES_V3)
                    .ok_or(ErrorV3::Arithmetic)?,
            )
            .ok_or(ErrorV3::Arithmetic)?;
        put(scratch, offset, &bound.coordinate.to_le_bytes())?;
        put(scratch, offset + 2, &[bound.actions.bits()])?;
        put(scratch, offset + 4, &bound.live_bytes.to_le_bytes())?;
    }
    let base_start = HEADER_BYTES_V3
        .checked_add(
            funding
                .len()
                .checked_mul(FUNDING_BOUND_BYTES_V3)
                .ok_or(ErrorV3::Arithmetic)?,
        )
        .ok_or(ErrorV3::Arithmetic)?;
    put(scratch, base_start, base_profile)?;
    AccountProfileV3::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn read_u8(bytes: &[u8], offset: usize) -> ResultV3<u8> {
    bytes.get(offset).copied().ok_or(ErrorV3::Wire)
}

fn read_u16(bytes: &[u8], offset: usize) -> ResultV3<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| ErrorV3::Wire)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> ResultV3<u32> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?
            .try_into()
            .map_err(|_| ErrorV3::Wire)?,
    ))
}

fn slice(bytes: &[u8], offset: usize, len: usize) -> ResultV3<&[u8]> {
    let end = offset.checked_add(len).ok_or(ErrorV3::Arithmetic)?;
    bytes.get(offset..end).ok_or(ErrorV3::Wire)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> ResultV3<()> {
    let end = offset.checked_add(value.len()).ok_or(ErrorV3::Arithmetic)?;
    output
        .get_mut(offset..end)
        .ok_or(ErrorV3::Wire)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::vec;

    use crate::v2::encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountProfileArtifactV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, RegisterGeometryV2, ScalarCoordinateV2,
        encode_account_profile_v2_atomic, encode_account_profile_with_lifecycle_v2_atomic,
    };
    use crate::v2::{HEADER_BYTES, OPERATION_BYTES, RULE_BYTES, TrustedEnvironmentV2};

    use super::*;

    fn base_profile(lifecycle_bound: bool, data_bytes: u32) -> alloc::vec::Vec<u8> {
        let rules = [
            AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, true, false),
                effect_permissions: if lifecycle_bound {
                    AccountEffectPermissionsV2::new(true, true, true)
                } else {
                    AccountEffectPermissionsV2::new(false, true, false)
                },
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: data_bytes,
                data_item_stride: 0,
            },
            AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, false, false),
                effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
        ];
        let operations = [AccountOperationInputV2::ProjectLamports {
            account: AccountCoordinateV2::fixed(1),
            destination: ScalarCoordinateV2::common(0),
        }];
        let registers = RegisterGeometryV2 {
            common_scalars: 1,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        };
        let mut scratch = vec![0_u8; HEADER_BYTES + 2 * RULE_BYTES + OPERATION_BYTES];
        let mut output = vec![0_u8; scratch.len()];
        if lifecycle_bound {
            encode_account_profile_with_lifecycle_v2_atomic(
                TrustedEnvironmentV2::None,
                &[
                    AccountRuleWithPrestateInputV2 {
                        rule: rules[0],
                        prestate: AccountPrestateV2::LifecycleBound,
                    },
                    AccountRuleWithPrestateInputV2 {
                        rule: rules[1],
                        prestate: AccountPrestateV2::Exact,
                    },
                ],
                &[],
                &operations,
                &[],
                registers,
                &mut scratch,
                &mut output,
            )
            .expect("lifecycle-bound base profile");
        } else {
            encode_account_profile_v2_atomic(
                AccountProfileArtifactV2::RuntimeTail,
                &rules,
                &[],
                &operations,
                &[],
                registers,
                &mut scratch,
                &mut output,
            )
            .expect("exact base profile");
        }
        output
    }

    #[test]
    fn exact_funding_refinement_round_trips() {
        let base = base_profile(true, 64);
        let bound = [FundingBoundV3::new(
            0,
            FundingActionMaskV3::CREATE_AND_CLOSE,
            64,
        )];
        let width = HEADER_BYTES_V3 + FUNDING_BOUND_BYTES_V3 + base.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_account_profile_v3_atomic(&base, &bound, &mut scratch, &mut output)
            .expect("successor profile");
        let decoded = AccountProfileV3::decode(&output).expect("decode successor");
        assert_eq!(decoded.base().bytes(), base);
        assert_eq!(decoded.funding_bound(0), Ok(bound[0]));
    }

    #[test]
    fn exact_base_profile_cannot_be_substituted_or_misclassified() {
        for (lifecycle_bound, live_bytes) in [(false, 64), (true, 63)] {
            let base = base_profile(lifecycle_bound, 64);
            let bound = [FundingBoundV3::new(
                0,
                FundingActionMaskV3::CREATE,
                live_bytes,
            )];
            let width = HEADER_BYTES_V3 + FUNDING_BOUND_BYTES_V3 + base.len();
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0_u8; width];
            assert_eq!(
                encode_account_profile_v3_atomic(&base, &bound, &mut scratch, &mut output),
                Err(ErrorV3::ProfileMismatch)
            );
            assert!(output.iter().all(|byte| *byte == 0));
        }
    }

    #[test]
    fn funding_table_is_strictly_ordered_and_reserved_bytes_are_zero() {
        let base = base_profile(true, 64);
        let duplicate = [
            FundingBoundV3::new(0, FundingActionMaskV3::CREATE, 64),
            FundingBoundV3::new(0, FundingActionMaskV3::CLOSE, 64),
        ];
        let width = HEADER_BYTES_V3 + 2 * FUNDING_BOUND_BYTES_V3 + base.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        assert_eq!(
            encode_account_profile_v3_atomic(&base, &duplicate, &mut scratch, &mut output),
            Err(ErrorV3::FundingTable)
        );
        let exact = [duplicate[0]];
        let width = HEADER_BYTES_V3 + FUNDING_BOUND_BYTES_V3 + base.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_account_profile_v3_atomic(&base, &exact, &mut scratch, &mut output)
            .expect("exact profile");
        output[HEADER_BYTES_V3 + 3] = 1;
        assert_eq!(AccountProfileV3::decode(&output), Err(ErrorV3::Wire));
    }

    #[test]
    fn exact_empty_refinement_round_trips_without_phantom_coverage() {
        let base = base_profile(true, 64);
        let width = HEADER_BYTES_V3 + base.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_account_profile_v3_atomic(&base, &[], &mut scratch, &mut output)
            .expect("canonical empty successor profile");
        let decoded = AccountProfileV3::decode(&output).expect("empty refinement decodes");
        assert_eq!(decoded.base().bytes(), base);
        assert_eq!(decoded.funding_bound_count(), 0);
        assert_eq!(decoded.funding_bound_for(0), Ok(None));
        assert_eq!(decoded.funding_bound(0), Err(ErrorV3::FundingTable));
    }

    #[test]
    fn noncanonical_empty_refinements_refuse_offsets_reserved_and_trailing_bytes() {
        let base = base_profile(true, 64);
        let width = HEADER_BYTES_V3 + base.len();
        let mut scratch = vec![0_u8; width];
        let mut exact = vec![0_u8; width];
        encode_account_profile_v3_atomic(&base, &[], &mut scratch, &mut exact)
            .expect("canonical empty successor profile");

        let mut reserved = exact.clone();
        reserved[12] = 1;
        assert_eq!(AccountProfileV3::decode(&reserved), Err(ErrorV3::Wire));

        let mut shifted_base = exact.clone();
        shifted_base[14..18].copy_from_slice(
            &u32::try_from(base.len() - 1)
                .expect("bounded base")
                .to_le_bytes(),
        );
        assert_eq!(AccountProfileV3::decode(&shifted_base), Err(ErrorV3::Wire));

        let mut trailing = exact;
        trailing.push(0);
        assert_eq!(AccountProfileV3::decode(&trailing), Err(ErrorV3::Wire));
    }
}
