//! Canonical fixed-data prestate predicates for AccountProfile V2 Profile 14.

use core::convert::TryInto;

use super::generated_profile14::{
    FIXED_DATA_PREDICATE_ACCOUNT_OFFSET_V2, FIXED_DATA_PREDICATE_ARTIFACT_PROFILE,
    FIXED_DATA_PREDICATE_BYTES, FIXED_DATA_PREDICATE_COUNT_OFFSET,
    FIXED_DATA_PREDICATE_DATA_OFFSET_V2, FIXED_DATA_PREDICATE_HEADER_BYTES,
    FIXED_DATA_PREDICATE_HEADER_RESERVED_OFFSET, FIXED_DATA_PREDICATE_OPCODE_OFFSET_V2,
    FIXED_DATA_PREDICATE_PAYLOAD_OFFSET_V2, FIXED_DATA_PREDICATE_REQUIRE_U8,
    FIXED_DATA_PREDICATE_REQUIRE_U16, FIXED_DATA_PREDICATE_REQUIRE_U32,
    FIXED_DATA_PREDICATE_REQUIRE_U64, FIXED_DATA_PREDICATE_REQUIRE_ZERO_RANGE,
    FIXED_DATA_PREDICATE_RESERVED_OFFSET_V2,
};
use super::{
    AccountObservationV1, AccountPrestateV2, AccountProfileV2, AliasKindV2,
    DYNAMIC_FIXED_SPAN_ENTRY_BYTES, Error, Result, add, byte, dynamic_runtime_coordinate_for_base,
    read_u16, read_u32,
};

/// Canonical fixed-data predicate operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixedDataPredicateKindV2 {
    /// Require one exact byte.
    RequireDataU8(u8),
    /// Require one exact little-endian `u16`.
    RequireDataU16(u16),
    /// Require one exact little-endian `u32`.
    RequireDataU32(u32),
    /// Require one exact little-endian `u64`.
    RequireDataU64(u64),
    /// Require one nonempty byte range to be entirely zero.
    RequireZeroRange(u32),
}

impl FixedDataPredicateKindV2 {
    const fn width(self) -> u32 {
        match self {
            Self::RequireDataU8(_) => 1,
            Self::RequireDataU16(_) => 2,
            Self::RequireDataU32(_) => 4,
            Self::RequireDataU64(_) => 8,
            Self::RequireZeroRange(width) => width,
        }
    }
}

/// One hostile-decoded, canonical Profile 14 prestate predicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedDataPredicateV2 {
    account: u16,
    data_offset: u32,
    kind: FixedDataPredicateKindV2,
}

impl FixedDataPredicateV2 {
    /// Fixed-rule account coordinate.
    pub const fn account(self) -> u16 {
        self.account
    }

    /// Exact account-data offset.
    pub const fn data_offset(self) -> u32 {
        self.data_offset
    }

    /// Typed predicate operation and expected value/range width.
    pub const fn kind(self) -> FixedDataPredicateKindV2 {
        self.kind
    }

    fn end(self) -> Result<u32> {
        self.data_offset
            .checked_add(self.kind.width())
            .ok_or(Error::InvalidFixedDataPredicate)
    }

    fn accepts(self, data: &[u8]) -> Result<bool> {
        let offset =
            usize::try_from(self.data_offset).map_err(|_| Error::InvalidFixedDataPredicate)?;
        let end = usize::try_from(self.end()?).map_err(|_| Error::InvalidFixedDataPredicate)?;
        let bytes = data
            .get(offset..end)
            .ok_or(Error::FixedDataPredicateMismatch)?;
        Ok(match self.kind {
            FixedDataPredicateKindV2::RequireDataU8(expected) => bytes == [expected],
            FixedDataPredicateKindV2::RequireDataU16(expected) => bytes == expected.to_le_bytes(),
            FixedDataPredicateKindV2::RequireDataU32(expected) => bytes == expected.to_le_bytes(),
            FixedDataPredicateKindV2::RequireDataU64(expected) => bytes == expected.to_le_bytes(),
            FixedDataPredicateKindV2::RequireZeroRange(_) => bytes.iter().all(|value| *value == 0),
        })
    }
}

pub(super) fn decode_predicate_count(bytes: &[u8], artifact_profile: u16) -> Result<u16> {
    if artifact_profile != FIXED_DATA_PREDICATE_ARTIFACT_PROFILE {
        return Ok(0);
    }
    if bytes
        .get(FIXED_DATA_PREDICATE_HEADER_RESERVED_OFFSET..FIXED_DATA_PREDICATE_HEADER_BYTES)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|value| *value != 0)
    {
        return Err(Error::InvalidFixedDataPredicate);
    }
    read_u16(bytes, FIXED_DATA_PREDICATE_COUNT_OFFSET)
}

impl AccountProfileV2<'_> {
    /// Number of canonical fixed-data predicates owned by Profile 14.
    pub const fn fixed_data_predicate_count(self) -> u16 {
        self.fixed_data_predicate_count
    }

    /// Decode one canonical fixed-data predicate.
    pub fn fixed_data_predicate(self, index: u16) -> Result<FixedDataPredicateV2> {
        if !self.uses_fixed_data_predicates() || index >= self.fixed_data_predicate_count {
            return Err(Error::InvalidFixedDataPredicate);
        }
        let offset = usize::from(self.dynamic_fixed_span_count)
            .checked_mul(DYNAMIC_FIXED_SPAN_ENTRY_BYTES)
            .and_then(|spans| FIXED_DATA_PREDICATE_HEADER_BYTES.checked_add(spans))
            .and_then(|base| {
                usize::from(index)
                    .checked_mul(FIXED_DATA_PREDICATE_BYTES)
                    .and_then(|body| base.checked_add(body))
            })
            .ok_or(Error::InvalidLength)?;
        if byte(
            self.bytes,
            add(offset, FIXED_DATA_PREDICATE_RESERVED_OFFSET_V2)?,
        )? != 0
        {
            return Err(Error::InvalidFixedDataPredicate);
        }
        let opcode = byte(
            self.bytes,
            add(offset, FIXED_DATA_PREDICATE_OPCODE_OFFSET_V2)?,
        )?;
        let account = read_u16(
            self.bytes,
            add(offset, FIXED_DATA_PREDICATE_ACCOUNT_OFFSET_V2)?,
        )?;
        let data_offset = read_u32(
            self.bytes,
            add(offset, FIXED_DATA_PREDICATE_DATA_OFFSET_V2)?,
        )?;
        let payload_offset = add(offset, FIXED_DATA_PREDICATE_PAYLOAD_OFFSET_V2)?;
        let payload = self
            .bytes
            .get(payload_offset..add(payload_offset, 8)?)
            .ok_or(Error::InvalidLength)?;
        let kind = match opcode {
            FIXED_DATA_PREDICATE_REQUIRE_U8 => {
                if payload
                    .get(1..)
                    .is_none_or(|inactive| inactive.iter().any(|value| *value != 0))
                {
                    return Err(Error::InvalidFixedDataPredicate);
                }
                FixedDataPredicateKindV2::RequireDataU8(
                    *payload.first().ok_or(Error::InvalidLength)?,
                )
            }
            FIXED_DATA_PREDICATE_REQUIRE_U16 => {
                if payload
                    .get(2..)
                    .is_none_or(|inactive| inactive.iter().any(|value| *value != 0))
                {
                    return Err(Error::InvalidFixedDataPredicate);
                }
                FixedDataPredicateKindV2::RequireDataU16(read_u16(payload, 0)?)
            }
            FIXED_DATA_PREDICATE_REQUIRE_U32 => {
                if payload
                    .get(4..)
                    .is_none_or(|inactive| inactive.iter().any(|value| *value != 0))
                {
                    return Err(Error::InvalidFixedDataPredicate);
                }
                FixedDataPredicateKindV2::RequireDataU32(read_u32(payload, 0)?)
            }
            FIXED_DATA_PREDICATE_REQUIRE_U64 => FixedDataPredicateKindV2::RequireDataU64(
                u64::from_le_bytes(payload.try_into().map_err(|_| Error::InvalidLength)?),
            ),
            FIXED_DATA_PREDICATE_REQUIRE_ZERO_RANGE => {
                if payload
                    .get(4..)
                    .is_none_or(|inactive| inactive.iter().any(|value| *value != 0))
                {
                    return Err(Error::InvalidFixedDataPredicate);
                }
                let width = read_u32(payload, 0)?;
                if width == 0 {
                    return Err(Error::InvalidFixedDataPredicate);
                }
                FixedDataPredicateKindV2::RequireZeroRange(width)
            }
            _ => return Err(Error::InvalidFixedDataPredicate),
        };
        Ok(FixedDataPredicateV2 {
            account,
            data_offset,
            kind,
        })
    }

    pub(super) fn validate_fixed_data_predicates(self) -> Result<()> {
        if !self.uses_fixed_data_predicates() {
            return if self.fixed_data_predicate_count == 0 {
                Ok(())
            } else {
                Err(Error::InvalidFixedDataPredicate)
            };
        }
        if self.fixed_data_predicate_count == 0 {
            return Err(Error::InvalidFixedDataPredicate);
        }
        let mut prior: Option<FixedDataPredicateV2> = None;
        let mut index = 0_u16;
        while index < self.fixed_data_predicate_count {
            let predicate = self.fixed_data_predicate(index)?;
            let rule = self.rule(false, predicate.account)?;
            if !matches!(
                rule.prestate,
                AccountPrestateV2::Exact | AccountPrestateV2::LifecycleBound
            ) || rule.alias_kind != AliasKindV2::SelfCoordinate
                || rule.alias_index != 0
                || rule.data_item_stride != 0
                || predicate.end()? > rule.data_length
                || prior.is_some_and(|previous| {
                    predicate.account < previous.account
                        || (predicate.account == previous.account
                            && (predicate.data_offset <= previous.data_offset
                                || predicate.data_offset < previous.end().unwrap_or(u32::MAX)))
                })
            {
                return Err(Error::InvalidFixedDataPredicate);
            }
            prior = Some(predicate);
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }
}

pub(super) fn validate_observations(
    profile: AccountProfileV2<'_>,
    span_counts: &[u32],
    accounts: &[AccountObservationV1<'_>],
) -> Result<()> {
    if !profile.uses_fixed_data_predicates() {
        return Ok(());
    }
    let mut index = 0_u16;
    while index < profile.fixed_data_predicate_count {
        let predicate = profile.fixed_data_predicate(index)?;
        let coordinate =
            dynamic_runtime_coordinate_for_base(profile, span_counts, predicate.account)?;
        let account = accounts
            .get(coordinate)
            .copied()
            .ok_or(Error::InvalidCoordinate)?;
        let rule = profile.rule(false, predicate.account)?;
        if account.data().is_empty() && rule.prestate == AccountPrestateV2::LifecycleBound {
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
            continue;
        }
        if !predicate.accepts(account.data())? {
            return Err(Error::FixedDataPredicateMismatch);
        }
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::{vec, vec::Vec};

    use super::*;
    use crate::v2::{
        DYNAMIC_FIXED_SPAN_ENTRY_BYTES, ProjectionRegistersV2,
        encode::{
            AccountAliasInputV2, AccountEffectPermissionsV2, AccountPrivilegesV2,
            AccountRuleInputV2, AccountRuleWithPrestateInputV2, DynamicFixedSpanInputV2,
            FixedDataPredicateInputV2, RegisterGeometryV2,
            encode_account_profile_with_fixed_data_predicates_v2_atomic,
        },
        project_dynamic_fixed_spans_atomic,
    };

    fn rule(
        writable: bool,
        alias: AccountAliasInputV2,
        prestate: AccountPrestateV2,
        data_length: u32,
    ) -> AccountRuleWithPrestateInputV2 {
        AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, writable, false),
                effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
                alias,
                data_length,
                data_item_stride: 0,
            },
            prestate,
        }
    }

    fn profile_bytes(
        predicates: &[FixedDataPredicateInputV2],
        rules: &[AccountRuleWithPrestateInputV2],
    ) -> core::result::Result<Vec<u8>, Error> {
        let spans = [DynamicFixedSpanInputV2 {
            insertion_coordinate: 2,
            count_scalar: 0,
            rule_start: 0,
            rule_stride: 1,
            minimum: 1,
            maximum: 2,
            step: 1,
        }];
        let span_rules = [rule(
            false,
            AccountAliasInputV2::SelfCoordinate,
            AccountPrestateV2::Exact,
            0,
        )];
        let width = FIXED_DATA_PREDICATE_HEADER_BYTES
            + DYNAMIC_FIXED_SPAN_ENTRY_BYTES
            + predicates.len() * FIXED_DATA_PREDICATE_BYTES
            + (rules.len() + span_rules.len()) * super::super::RULE_BYTES;
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0xa5_u8; width];
        encode_account_profile_with_fixed_data_predicates_v2_atomic(
            super::super::TrustedEnvironmentV2::None,
            super::super::TrustedIdentityEnvironmentV2::None,
            super::super::TrustedBuiltinIdentityV2::None,
            &spans,
            predicates,
            rules,
            &span_rules,
            &[],
            RegisterGeometryV2 {
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &mut scratch,
            &mut output,
        )?;
        Ok(output)
    }

    fn canonical_rules() -> [AccountRuleWithPrestateInputV2; 2] {
        [
            rule(
                true,
                AccountAliasInputV2::SelfCoordinate,
                AccountPrestateV2::Exact,
                16,
            ),
            rule(
                true,
                AccountAliasInputV2::Fixed(0),
                AccountPrestateV2::AuthenticatedRouteAlias,
                0,
            ),
        ]
    }

    #[test]
    fn profile14_preserves_dynamic_alias_geometry_and_checks_prestate_atomically() {
        let predicates = [
            FixedDataPredicateInputV2::RequireDataU64 {
                account: 0,
                data_offset: 0,
                value: 0x0032_5041_544c_4344,
            },
            FixedDataPredicateInputV2::RequireZeroRange {
                account: 0,
                data_offset: 8,
                length: 8,
            },
        ];
        let encoded = profile_bytes(&predicates, &canonical_rules()).expect("encode Profile14");
        let profile = AccountProfileV2::decode(&encoded).expect("decode Profile14");
        assert_eq!(
            profile.artifact_profile(),
            FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
        );
        assert!(profile.uses_dynamic_fixed_spans());
        assert!(profile.supports_route_alias_packing());
        assert!(profile.uses_fixed_data_predicates());
        assert_eq!(profile.fixed_data_predicate_count(), 2);
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(0, &[1]),
            Ok(3)
        );
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(0, &[1]),
            Ok(2)
        );
        assert_eq!(
            profile.physical_account_ordinal_with_dynamic_spans(0, &[1], 1),
            Ok(0)
        );
        assert!(
            profile
                .route_privileges_with_dynamic_spans(0, &[1], 0)
                .expect("route zero privileges")
                .writable()
        );
        assert!(
            profile
                .route_privileges_with_dynamic_spans(0, &[1], 1)
                .expect("route alias privileges")
                .writable()
        );
        assert!(
            profile
                .physical_account_geometry_with_dynamic_spans(0, &[1], 0)
                .expect("physical geometry")
                .privileges()
                .writable()
        );

        let mut data = [0_u8; 16];
        data[..8].copy_from_slice(&0x0032_5041_544c_4344_u64.to_le_bytes());
        let accounts = [
            AccountObservationV1::new([1; 32], [9; 32], 7, &data, false, true, false),
            AccountObservationV1::new([1; 32], [9; 32], 7, &data, false, true, false),
            AccountObservationV1::new([2; 32], [9; 32], 0, &[], false, false, false),
        ];
        let input_scalars = [1_u64];
        let input_identities = [[7_u8; 32]];
        let mut scratch_scalars = [91_u64];
        let mut scratch_identities = [[91_u8; 32]];
        let mut output_scalars = [92_u64];
        let mut output_identities = [[92_u8; 32]];
        project_dynamic_fixed_spans_atomic(
            profile,
            0,
            &[1],
            &accounts,
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("matching fixed prestate");
        assert_eq!(output_scalars, input_scalars);
        assert_eq!(output_identities, input_identities);

        let mut hostile_data = data;
        hostile_data[15] = 1;
        let hostile = [
            AccountObservationV1::new([1; 32], [9; 32], 7, &hostile_data, false, true, false),
            AccountObservationV1::new([1; 32], [9; 32], 7, &hostile_data, false, true, false),
            accounts[2],
        ];
        output_scalars = [0xaa];
        output_identities = [[0xbb; 32]];
        assert_eq!(
            project_dynamic_fixed_spans_atomic(
                profile,
                0,
                &[1],
                &hostile,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
            ),
            Err(Error::FixedDataPredicateMismatch)
        );
        assert_eq!(output_scalars, [0xaa]);
        assert_eq!(output_identities, [[0xbb; 32]]);
    }

    #[test]
    fn hostile_predicate_shapes_and_targets_refuse_without_output_mutation() {
        let rules = canonical_rules();
        for predicates in [
            vec![FixedDataPredicateInputV2::RequireZeroRange {
                account: 0,
                data_offset: 8,
                length: 0,
            }],
            vec![FixedDataPredicateInputV2::RequireDataU64 {
                account: 0,
                data_offset: 12,
                value: 1,
            }],
            vec![
                FixedDataPredicateInputV2::RequireDataU16 {
                    account: 0,
                    data_offset: 8,
                    value: 2,
                },
                FixedDataPredicateInputV2::RequireDataU8 {
                    account: 0,
                    data_offset: 8,
                    value: 2,
                },
            ],
        ] {
            assert_eq!(
                profile_bytes(&predicates, &rules),
                Err(Error::InvalidFixedDataPredicate)
            );
        }

        let aliased_target = [
            rules[0],
            rule(
                true,
                AccountAliasInputV2::Fixed(0),
                AccountPrestateV2::AuthenticatedRouteAlias,
                16,
            ),
        ];
        assert_eq!(
            profile_bytes(
                &[FixedDataPredicateInputV2::RequireDataU8 {
                    account: 1,
                    data_offset: 0,
                    value: 1,
                }],
                &aliased_target,
            ),
            Err(Error::InvalidRouteAlias)
        );
    }

    #[test]
    fn hostile_wire_reserved_inactive_and_duplicate_encodings_refuse() {
        let predicates = [FixedDataPredicateInputV2::RequireDataU8 {
            account: 0,
            data_offset: 0,
            value: 0x44,
        }];
        let encoded = profile_bytes(&predicates, &canonical_rules()).expect("encode Profile14");
        let predicate_offset = FIXED_DATA_PREDICATE_HEADER_BYTES + DYNAMIC_FIXED_SPAN_ENTRY_BYTES;
        for (offset, value) in [
            (FIXED_DATA_PREDICATE_HEADER_RESERVED_OFFSET, 1),
            (
                predicate_offset + FIXED_DATA_PREDICATE_RESERVED_OFFSET_V2,
                1,
            ),
            (
                predicate_offset + FIXED_DATA_PREDICATE_PAYLOAD_OFFSET_V2 + 1,
                1,
            ),
            (
                predicate_offset + FIXED_DATA_PREDICATE_OPCODE_OFFSET_V2,
                0xff,
            ),
        ] {
            let mut hostile = encoded.clone();
            *hostile.get_mut(offset).expect("hostile byte coordinate") = value;
            assert!(AccountProfileV2::decode(&hostile).is_err());
        }
    }

    #[test]
    fn lifecycle_predicates_admit_vacancy_and_become_mandatory_when_live() {
        let lifecycle = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, true, false),
                effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 16,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::LifecycleBound,
        };
        let alias = rule(
            true,
            AccountAliasInputV2::Fixed(0),
            AccountPrestateV2::AuthenticatedRouteAlias,
            0,
        );
        let encoded = profile_bytes(
            &[FixedDataPredicateInputV2::RequireDataU16 {
                account: 0,
                data_offset: 8,
                value: 2,
            }],
            &[lifecycle, alias],
        )
        .expect("encode lifecycle Profile14");
        let profile = AccountProfileV2::decode(&encoded).expect("decode lifecycle Profile14");
        let vacant = [
            AccountObservationV1::new([1; 32], [9; 32], 7, &[], false, true, false),
            AccountObservationV1::new([1; 32], [9; 32], 7, &[], false, true, false),
            AccountObservationV1::new([2; 32], [9; 32], 0, &[], false, false, false),
        ];
        let input_scalars = [1_u64];
        let input_identities = [[7_u8; 32]];
        let mut scratch_scalars = [0_u64];
        let mut scratch_identities = [[0_u8; 32]];
        let mut output_scalars = [0_u64];
        let mut output_identities = [[0_u8; 32]];
        project_dynamic_fixed_spans_atomic(
            profile,
            0,
            &[1],
            &vacant,
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("vacant lifecycle branch has no invented data prestate");

        let live = [0_u8; 16];
        let hostile_live = [
            AccountObservationV1::new([1; 32], [9; 32], 7, &live, false, true, false),
            AccountObservationV1::new([1; 32], [9; 32], 7, &live, false, true, false),
            vacant[2],
        ];
        assert_eq!(
            project_dynamic_fixed_spans_atomic(
                profile,
                0,
                &[1],
                &hostile_live,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
            ),
            Err(Error::FixedDataPredicateMismatch)
        );
    }
}
