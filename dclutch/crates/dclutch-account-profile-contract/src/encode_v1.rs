//! Safe, allocation-free `AccountProfileV1` artifact encoder.
//!
//! Typed inputs retain privilege, permission, alias and operation-tag authority
//! in this semantic-owner crate. Without them an artifact author has to restate
//! `ACCOUNT_PROFILE_*_OFFSET`, `ACCOUNT_RULE_*_OFFSET`,
//! `ACCOUNT_OPERATION_*_OFFSET` and the seven `OP_*` -- every one of which is
//! crate-private -- and the wire format acquires a second authority that
//! drifts. `v2::encode` is what this generation should have had and did not.
//!
//! The encoder builds into caller scratch, hostile-decodes the complete
//! candidate through [`AccountProfileV1::decode_selected`] -- which is the same
//! walk the activation seam performs, alias anchoring, effect-authority
//! anchoring and duplicate-projection checks included -- and copies to `output`
//! only after it accepts. A refusal leaves `output` byte-for-byte unchanged.

use crate::generated::{
    ACCOUNT_OPERATION_ACCOUNT_OFFSET, ACCOUNT_OPERATION_DATA_OFFSET,
    ACCOUNT_OPERATION_OPCODE_OFFSET, ACCOUNT_OPERATION_REGISTER_OFFSET,
    ACCOUNT_PROFILE_ACCOUNT_COUNT_OFFSET, ACCOUNT_PROFILE_ARTIFACT_OFFSET,
    ACCOUNT_PROFILE_IDENTITY_COUNT_OFFSET, ACCOUNT_PROFILE_MAGIC_OFFSET,
    ACCOUNT_PROFILE_OPERATION_COUNT_OFFSET, ACCOUNT_PROFILE_SCALAR_COUNT_OFFSET,
    ACCOUNT_PROFILE_VERSION_OFFSET, ACCOUNT_RULE_ALIAS_OF_OFFSET, ACCOUNT_RULE_DATA_LENGTH_OFFSET,
    ACCOUNT_RULE_EFFECT_PERMISSIONS_OFFSET, ACCOUNT_RULE_PRIVILEGES_OFFSET,
    OP_PROJECT_DATA_IDENTITY, OP_PROJECT_DATA_U64, OP_PROJECT_KEY, OP_PROJECT_LAMPORTS,
    OP_PROJECT_OWNER, OP_REQUIRE_KEY_EQ_IDENTITY, OP_REQUIRE_OWNER_EQ_IDENTITY,
};
use crate::{
    ACCOUNT_PROFILE_ARTIFACT_PROFILE_V1, ACCOUNT_PROFILE_HEADER_BYTES_V1, ACCOUNT_PROFILE_MAGIC_V1,
    ACCOUNT_PROFILE_OPERATION_BYTES_V1, ACCOUNT_PROFILE_RULE_BYTES_V1,
    ACCOUNT_PROFILE_SCHEMA_VERSION_V1, AccountProfileV1, EFFECT_PERMISSION_CREDIT_LAMPORTS,
    EFFECT_PERMISSION_DEBIT_LAMPORTS, EFFECT_PERMISSION_WRITE_DATA, Error, Result,
};

/// Placeholder content identity used only to satisfy the decode-time join.
///
/// [`AccountProfileV1::decode_selected`] joins a descriptor selection to an
/// adapter-authenticated record digest. That join is the composing adapter's
/// business, not the encoder's: the encoder is proving the BYTES are canonical.
/// One nonzero placeholder on both sides satisfies the join exactly and leaves
/// every structural check unchanged.
const ENCODER_CONTENT_IDENTITY_V1: [u8; 32] = [0xff; 32];

/// Exact `TransitionVM` V2 register-bank widths the encoded profile declares.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterGeometryV1 {
    /// Exact scalar-bank width every projection must supply.
    pub scalars: u16,
    /// Exact identity-bank width every projection must supply.
    pub identities: u16,
}

/// Exact signer/writable/executable privilege tuple one account must present.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountPrivilegesV1 {
    signer: bool,
    writable: bool,
    executable: bool,
}

impl AccountPrivilegesV1 {
    /// Construct one exact privilege tuple.
    #[must_use]
    pub const fn new(signer: bool, writable: bool, executable: bool) -> Self {
        Self {
            signer,
            writable,
            executable,
        }
    }

    const fn bits(self) -> u8 {
        (if self.signer { 1 } else { 0 })
            | (if self.writable { 2 } else { 0 })
            | (if self.executable { 4 } else { 0 })
    }
}

/// Exact effect-kernel authority granted to one account coordinate.
///
/// Any authority at all requires a runtime-writable account, and debit or
/// data-write authority additionally requires the profile to anchor that
/// account's owner. Both are decoder rules, so the encoder cannot emit a rule
/// that violates them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountEffectPermissionsV1 {
    debit_lamports: bool,
    credit_lamports: bool,
    write_data: bool,
}

impl AccountEffectPermissionsV1 {
    /// Construct one exact effect-permission tuple.
    #[must_use]
    pub const fn new(debit_lamports: bool, credit_lamports: bool, write_data: bool) -> Self {
        Self {
            debit_lamports,
            credit_lamports,
            write_data,
        }
    }

    /// Grant no effect authority.
    #[must_use]
    pub const fn none() -> Self {
        Self::new(false, false, false)
    }

    const fn bits(self) -> u8 {
        (if self.debit_lamports {
            EFFECT_PERMISSION_DEBIT_LAMPORTS
        } else {
            0
        }) | (if self.credit_lamports {
            EFFECT_PERMISSION_CREDIT_LAMPORTS
        } else {
            0
        }) | (if self.write_data {
            EFFECT_PERMISSION_WRITE_DATA
        } else {
            0
        })
    }
}

/// Canonical alias relation for one account rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountAliasInputV1 {
    /// This coordinate is its own representative.
    SelfRepresentative,
    /// This coordinate is a second logical name for one strictly earlier
    /// self-representative coordinate, whose privileges, effect permissions and
    /// data width it must match exactly.
    Representative(u16),
}

/// One account rule: what the adapter must present at this coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountRuleInputV1 {
    /// Exact runtime privileges.
    pub privileges: AccountPrivilegesV1,
    /// Exact effect-kernel authority.
    pub effect_permissions: AccountEffectPermissionsV1,
    /// Canonical alias relation.
    pub alias: AccountAliasInputV1,
    /// Exact required account-data width.
    pub data_length: u32,
}

/// One account relation or register projection.
///
/// A relation reads the immutable input bank and admits an account; a
/// projection writes the candidate bank. Profile bytes carry no identity
/// literals, so every expected value is itself a register coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountOperationInputV1 {
    /// Require this account's key to equal an input identity register.
    RequireKey {
        /// Account coordinate.
        account: u16,
        /// Immutable identity coordinate carrying the expected key.
        expected: u16,
    },
    /// Require this account's owner to equal an input identity register.
    RequireOwner {
        /// Account coordinate.
        account: u16,
        /// Immutable identity coordinate carrying the expected owner.
        expected: u16,
    },
    /// Project this account's key into an identity register.
    ProjectKey {
        /// Account coordinate.
        account: u16,
        /// Destination identity coordinate.
        destination: u16,
    },
    /// Project this account's owner into an identity register.
    ProjectOwner {
        /// Account coordinate.
        account: u16,
        /// Destination identity coordinate.
        destination: u16,
    },
    /// Project this account's lamport balance into a scalar register.
    ProjectLamports {
        /// Account coordinate.
        account: u16,
        /// Destination scalar coordinate.
        destination: u16,
    },
    /// Project an exact little-endian data `u64` into a scalar register.
    ProjectDataU64 {
        /// Account coordinate.
        account: u16,
        /// Byte offset within the account's declared data width.
        data_offset: u32,
        /// Destination scalar coordinate.
        destination: u16,
    },
    /// Project an exact thirty-two-byte data field into an identity register.
    ProjectDataIdentity {
        /// Account coordinate.
        account: u16,
        /// Byte offset within the account's declared data width.
        data_offset: u32,
        /// Destination identity coordinate.
        destination: u16,
    },
}

impl AccountOperationInputV1 {
    const fn encoded(self) -> (u8, u16, u16, u32) {
        match self {
            Self::RequireKey { account, expected } => {
                (OP_REQUIRE_KEY_EQ_IDENTITY, account, expected, 0)
            }
            Self::RequireOwner { account, expected } => {
                (OP_REQUIRE_OWNER_EQ_IDENTITY, account, expected, 0)
            }
            Self::ProjectKey {
                account,
                destination,
            } => (OP_PROJECT_KEY, account, destination, 0),
            Self::ProjectOwner {
                account,
                destination,
            } => (OP_PROJECT_OWNER, account, destination, 0),
            Self::ProjectLamports {
                account,
                destination,
            } => (OP_PROJECT_LAMPORTS, account, destination, 0),
            Self::ProjectDataU64 {
                account,
                data_offset,
                destination,
            } => (OP_PROJECT_DATA_U64, account, destination, data_offset),
            Self::ProjectDataIdentity {
                account,
                data_offset,
                destination,
            } => (OP_PROJECT_DATA_IDENTITY, account, destination, data_offset),
        }
    }
}

/// Exact encoded width of a profile with `accounts` rules and `operations` operations.
///
/// # Errors
///
/// Refuses a width that does not fit `usize`.
pub const fn account_profile_v1_bytes(accounts: usize, operations: usize) -> Result<usize> {
    let Some(rules) = accounts.checked_mul(ACCOUNT_PROFILE_RULE_BYTES_V1) else {
        return Err(Error::InvalidLength);
    };
    let Some(body) = operations.checked_mul(ACCOUNT_PROFILE_OPERATION_BYTES_V1) else {
        return Err(Error::InvalidLength);
    };
    let Some(prefix) = ACCOUNT_PROFILE_HEADER_BYTES_V1.checked_add(rules) else {
        return Err(Error::InvalidLength);
    };
    match prefix.checked_add(body) {
        Some(total) => Ok(total),
        None => Err(Error::InvalidLength),
    }
}

/// Encode one complete `AccountProfileV1` into caller-owned buffers atomically.
///
/// `scratch` and `output` must both be exactly [`account_profile_v1_bytes`]
/// wide. The candidate is assembled in `scratch`, hostile-decoded in full, and
/// copied to `output` only on success: on any refusal `output` is unchanged.
///
/// # Errors
///
/// Refuses buffer widths that differ from the exact encoded width, and every
/// refusal [`AccountProfileV1::decode_selected`] itself raises against the
/// candidate -- a forward or noncanonical alias, an alias whose privileges or
/// width differ from its representative, effect authority on a readonly
/// account, debit or data-write authority whose owner is not anchored, a
/// self-representative account no relation anchors, two projections into one
/// register, a projection that overwrites an authority register, a data field
/// outside its account's declared width, and out-of-bank coordinates.
pub fn encode_account_profile_v1_atomic(
    rules: &[AccountRuleInputV1],
    operations: &[AccountOperationInputV1],
    registers: RegisterGeometryV1,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let account_count = u16::try_from(rules.len()).map_err(|_| Error::InvalidLength)?;
    let operation_count = u16::try_from(operations.len()).map_err(|_| Error::InvalidLength)?;
    let expected = account_profile_v1_bytes(rules.len(), operations.len())?;
    if scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    write(
        scratch,
        ACCOUNT_PROFILE_MAGIC_OFFSET,
        &ACCOUNT_PROFILE_MAGIC_V1,
    )?;
    for (offset, value) in [
        (
            ACCOUNT_PROFILE_VERSION_OFFSET,
            ACCOUNT_PROFILE_SCHEMA_VERSION_V1,
        ),
        (
            ACCOUNT_PROFILE_ARTIFACT_OFFSET,
            ACCOUNT_PROFILE_ARTIFACT_PROFILE_V1,
        ),
        (ACCOUNT_PROFILE_ACCOUNT_COUNT_OFFSET, account_count),
        (ACCOUNT_PROFILE_OPERATION_COUNT_OFFSET, operation_count),
        (ACCOUNT_PROFILE_SCALAR_COUNT_OFFSET, registers.scalars),
        (ACCOUNT_PROFILE_IDENTITY_COUNT_OFFSET, registers.identities),
    ] {
        write(scratch, offset, &value.to_le_bytes())?;
    }
    let mut cursor = ACCOUNT_PROFILE_HEADER_BYTES_V1;
    for (index, rule) in rules.iter().enumerate() {
        let alias_of = match rule.alias {
            AccountAliasInputV1::SelfRepresentative => {
                u16::try_from(index).map_err(|_| Error::InvalidAlias)?
            }
            AccountAliasInputV1::Representative(representative) => representative,
        };
        write_byte(
            scratch,
            add(cursor, ACCOUNT_RULE_PRIVILEGES_OFFSET)?,
            rule.privileges.bits(),
        )?;
        write_byte(
            scratch,
            add(cursor, ACCOUNT_RULE_EFFECT_PERMISSIONS_OFFSET)?,
            rule.effect_permissions.bits(),
        )?;
        write(
            scratch,
            add(cursor, ACCOUNT_RULE_ALIAS_OF_OFFSET)?,
            &alias_of.to_le_bytes(),
        )?;
        write(
            scratch,
            add(cursor, ACCOUNT_RULE_DATA_LENGTH_OFFSET)?,
            &rule.data_length.to_le_bytes(),
        )?;
        cursor = add(cursor, ACCOUNT_PROFILE_RULE_BYTES_V1)?;
    }
    for operation in operations {
        let (opcode, account, register, data_offset) = operation.encoded();
        write_byte(
            scratch,
            add(cursor, ACCOUNT_OPERATION_OPCODE_OFFSET)?,
            opcode,
        )?;
        write(
            scratch,
            add(cursor, ACCOUNT_OPERATION_ACCOUNT_OFFSET)?,
            &account.to_le_bytes(),
        )?;
        write(
            scratch,
            add(cursor, ACCOUNT_OPERATION_REGISTER_OFFSET)?,
            &register.to_le_bytes(),
        )?;
        write(
            scratch,
            add(cursor, ACCOUNT_OPERATION_DATA_OFFSET)?,
            &data_offset.to_le_bytes(),
        )?;
        cursor = add(cursor, ACCOUNT_PROFILE_OPERATION_BYTES_V1)?;
    }
    if cursor != expected {
        return Err(Error::InvalidLength);
    }
    AccountProfileV1::decode_selected(
        ENCODER_CONTENT_IDENTITY_V1,
        ENCODER_CONTENT_IDENTITY_V1,
        scratch,
    )?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<()> {
    let end = add(offset, bytes.len())?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

fn write_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;
    use crate::generated::{AGREEMENT_PROFILE_V1, ALIAS_AGREEMENT_PROFILE_V1};

    fn encode(
        rules: &[AccountRuleInputV1],
        operations: &[AccountOperationInputV1],
        registers: RegisterGeometryV1,
    ) -> Result<vec::Vec<u8>> {
        let width = account_profile_v1_bytes(rules.len(), operations.len())?;
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0xcd_u8; width];
        match encode_account_profile_v1_atomic(
            rules,
            operations,
            registers,
            &mut scratch,
            &mut output,
        ) {
            Ok(()) => Ok(output),
            Err(error) => {
                assert!(
                    output.iter().all(|byte| *byte == 0xcd),
                    "a refused encode left bytes in output"
                );
                Err(error)
            }
        }
    }

    fn readonly(data_length: u32) -> AccountRuleInputV1 {
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, false, false),
            effect_permissions: AccountEffectPermissionsV1::none(),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length,
        }
    }

    /// The public encoder reproduces the Lean-emitted agreement profile.
    ///
    /// `AGREEMENT_PROFILE_V1` is emitted by
    /// `formal/dclutch-semantics/EmitAccountProfileAbiRust.lean` and is
    /// therefore an authority this crate does not own. An encoder that agrees
    /// with it on every one of the three hundred and thirty-six bytes is a
    /// projection of the wire format rather than a second statement of it.
    #[test]
    fn the_public_encoder_reproduces_the_emitted_agreement_profile() {
        let rules = [
            readonly(64),
            readonly(232),
            AccountRuleInputV1 {
                privileges: AccountPrivilegesV1::new(false, true, false),
                effect_permissions: AccountEffectPermissionsV1::new(true, false, false),
                alias: AccountAliasInputV1::SelfRepresentative,
                data_length: 0,
            },
            readonly(40),
        ];
        let operations = [
            AccountOperationInputV1::RequireOwner {
                account: 0,
                expected: 8,
            },
            AccountOperationInputV1::RequireOwner {
                account: 1,
                expected: 8,
            },
            AccountOperationInputV1::RequireKey {
                account: 0,
                expected: 9,
            },
            AccountOperationInputV1::RequireKey {
                account: 1,
                expected: 10,
            },
            AccountOperationInputV1::RequireKey {
                account: 2,
                expected: 6,
            },
            AccountOperationInputV1::RequireKey {
                account: 3,
                expected: 11,
            },
            AccountOperationInputV1::RequireOwner {
                account: 2,
                expected: 7,
            },
            AccountOperationInputV1::ProjectKey {
                account: 2,
                destination: 12,
            },
            AccountOperationInputV1::ProjectOwner {
                account: 2,
                destination: 13,
            },
            AccountOperationInputV1::ProjectLamports {
                account: 2,
                destination: 14,
            },
            AccountOperationInputV1::ProjectDataU64 {
                account: 1,
                data_offset: 112,
                destination: 15,
            },
            AccountOperationInputV1::ProjectDataU64 {
                account: 1,
                data_offset: 120,
                destination: 16,
            },
            AccountOperationInputV1::ProjectDataU64 {
                account: 3,
                data_offset: 0,
                destination: 17,
            },
            AccountOperationInputV1::ProjectDataIdentity {
                account: 1,
                data_offset: 16,
                destination: 14,
            },
            AccountOperationInputV1::ProjectDataIdentity {
                account: 1,
                data_offset: 80,
                destination: 15,
            },
        ];
        let encoded = encode(
            &rules,
            &operations,
            RegisterGeometryV1 {
                scalars: 20,
                identities: 16,
            },
        )
        .expect("agreement profile");
        assert_eq!(encoded.as_slice(), AGREEMENT_PROFILE_V1.as_slice());
    }

    /// The alias relation round-trips through the emitted alias profile too.
    #[test]
    fn the_public_encoder_reproduces_the_emitted_alias_profile() {
        let encoded = encode(
            &[
                readonly(8),
                AccountRuleInputV1 {
                    alias: AccountAliasInputV1::Representative(0),
                    ..readonly(8)
                },
            ],
            &[
                AccountOperationInputV1::RequireKey {
                    account: 0,
                    expected: 0,
                },
                AccountOperationInputV1::ProjectLamports {
                    account: 1,
                    destination: 0,
                },
            ],
            RegisterGeometryV1 {
                scalars: 1,
                identities: 1,
            },
        )
        .expect("alias agreement profile");
        assert_eq!(encoded.as_slice(), ALIAS_AGREEMENT_PROFILE_V1.as_slice());
    }

    /// The encoder is total: it refuses whatever the decoder refuses, and a
    /// refusal never leaves partial bytes in `output`.
    ///
    /// `encode` asserts the untouched-output property on every refusal, so each
    /// case below carries it as well as its named error.
    #[test]
    fn the_public_encoder_refuses_what_the_decoder_refuses() {
        let one_scalar = RegisterGeometryV1 {
            scalars: 1,
            identities: 1,
        };
        let anchor = AccountOperationInputV1::RequireKey {
            account: 0,
            expected: 0,
        };
        let project = AccountOperationInputV1::ProjectLamports {
            account: 0,
            destination: 0,
        };

        // A self-representative account no relation anchors.
        assert_eq!(
            encode(&[readonly(0), readonly(0)], &[anchor, project], one_scalar),
            Err(Error::UnanchoredAccount)
        );
        // An alias that names a later coordinate.
        assert_eq!(
            encode(
                &[
                    AccountRuleInputV1 {
                        alias: AccountAliasInputV1::Representative(1),
                        ..readonly(0)
                    },
                    readonly(0),
                ],
                &[anchor, project],
                one_scalar,
            ),
            Err(Error::InvalidAlias)
        );
        // An alias whose declared width differs from its representative's.
        assert_eq!(
            encode(
                &[
                    readonly(8),
                    AccountRuleInputV1 {
                        alias: AccountAliasInputV1::Representative(0),
                        ..readonly(16)
                    },
                ],
                &[anchor, project],
                one_scalar,
            ),
            Err(Error::InvalidAlias)
        );
        // Effect authority on a runtime-readonly account.
        assert_eq!(
            encode(
                &[AccountRuleInputV1 {
                    effect_permissions: AccountEffectPermissionsV1::new(false, true, false),
                    ..readonly(0)
                }],
                &[anchor, project],
                one_scalar,
            ),
            Err(Error::EffectRequiresWritable)
        );
        // Debit authority whose owner relation the profile never anchors.
        assert_eq!(
            encode(
                &[AccountRuleInputV1 {
                    privileges: AccountPrivilegesV1::new(false, true, false),
                    effect_permissions: AccountEffectPermissionsV1::new(true, false, false),
                    ..readonly(0)
                }],
                &[anchor, project],
                one_scalar,
            ),
            Err(Error::EffectOwnerUnanchored)
        );
        // Two projections into one register.
        assert_eq!(
            encode(&[readonly(0)], &[anchor, project, project], one_scalar),
            Err(Error::DuplicateProjection)
        );
        // A projection that overwrites the register a relation reads.
        assert_eq!(
            encode(
                &[readonly(0)],
                &[
                    anchor,
                    AccountOperationInputV1::ProjectKey {
                        account: 0,
                        destination: 0,
                    },
                ],
                one_scalar,
            ),
            Err(Error::AuthorityOverwrite)
        );
        // A data field outside the account's declared width.
        assert_eq!(
            encode(
                &[readonly(8)],
                &[
                    anchor,
                    AccountOperationInputV1::ProjectDataU64 {
                        account: 0,
                        data_offset: 4,
                        destination: 0,
                    },
                ],
                one_scalar,
            ),
            Err(Error::DataFieldOutOfBounds)
        );
        // Out-of-suffix account and out-of-bank register coordinates.
        assert_eq!(
            encode(
                &[readonly(0)],
                &[
                    anchor,
                    AccountOperationInputV1::ProjectLamports {
                        account: 1,
                        destination: 0,
                    },
                ],
                one_scalar,
            ),
            Err(Error::InvalidAccountIndex)
        );
        assert_eq!(
            encode(
                &[readonly(0)],
                &[
                    anchor,
                    AccountOperationInputV1::ProjectLamports {
                        account: 0,
                        destination: 1,
                    },
                ],
                one_scalar,
            ),
            Err(Error::InvalidRegister)
        );
        // A profile with no account, no operation, no projection, or no bank.
        assert_eq!(encode(&[], &[anchor], one_scalar), Err(Error::EmptyProfile));
        assert_eq!(
            encode(&[readonly(0)], &[], one_scalar),
            Err(Error::EmptyProfile)
        );
        assert_eq!(
            encode(&[readonly(0)], &[anchor], one_scalar),
            Err(Error::EmptyProfile)
        );
        assert_eq!(
            encode(
                &[readonly(0)],
                &[anchor, project],
                RegisterGeometryV1 {
                    scalars: 0,
                    identities: 0,
                },
            ),
            Err(Error::EmptyProfile)
        );
    }

    /// Buffers that are not the exact encoded width refuse without writing.
    #[test]
    fn off_width_buffers_refuse_atomically() {
        let rules = [readonly(0)];
        let operations = [
            AccountOperationInputV1::RequireKey {
                account: 0,
                expected: 0,
            },
            AccountOperationInputV1::ProjectLamports {
                account: 0,
                destination: 0,
            },
        ];
        let registers = RegisterGeometryV1 {
            scalars: 1,
            identities: 1,
        };
        let width = account_profile_v1_bytes(rules.len(), operations.len()).expect("width");
        for (scratch_width, output_width) in [(width - 1, width), (width, width + 1)] {
            let mut scratch = vec![0_u8; scratch_width];
            let mut output = vec![0xcd_u8; output_width];
            assert_eq!(
                encode_account_profile_v1_atomic(
                    &rules,
                    &operations,
                    registers,
                    &mut scratch,
                    &mut output,
                ),
                Err(Error::InvalidLength)
            );
            assert!(output.iter().all(|byte| *byte == 0xcd));
        }
    }
}
