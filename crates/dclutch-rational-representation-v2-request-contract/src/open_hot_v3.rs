//! Parent-free Hot V3 request for open Rational representation actions.

use crate::{
    Error, RepresentationActionV2, RepresentationRequestV2, Result, array_at, byte_at,
    generated::{
        ACTION_DENOMINATE, ACTION_ISSUE_STRUCTURED, ACTION_RECONSTITUTE, ACTION_UNWRAP_STRUCTURED,
        CALLER_ROLE_TRADING, PHYSICAL_ABI_VERSION_V2, REQUEST_ACTION_OFFSET,
        REQUEST_ASSET_COUNT_OFFSET, REQUEST_CALLER_ROLE_OFFSET, REQUEST_MAGIC_OFFSET,
        REQUEST_MAGIC_V2, REQUEST_PARENT_CONTEXT_OFFSET, REQUEST_VERSION_OFFSET,
    },
    is_zero, put, put_byte, require_zero, u16_at, u32_at,
};

/// Schema preimage for the variable-width open Rational Hot request.
pub const OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/rational-representation-open-hot-request-v3";
/// SHA-256 of [`OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_PREIMAGE_V3`].
pub const OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3: [u8; 32] = [
    0x24, 0x6d, 0x5c, 0x88, 0xd4, 0x18, 0x63, 0x85, 0x5d, 0xec, 0xe2, 0xa4, 0xbb, 0x87, 0x4e, 0xac,
    0x87, 0x33, 0xc7, 0x81, 0x06, 0x0c, 0x1b, 0xb3, 0x48, 0x8c, 0xb4, 0xde, 0xaf, 0x6a, 0xdb, 0x06,
];
/// Exact Hot family magic. The remaining layout is the canonical request wire.
pub const OPEN_REPRESENTATION_HOT_MAGIC_V3: [u8; 8] = *b"DCRROH03";
/// Current Hot family version.
pub const OPEN_REPRESENTATION_HOT_VERSION_V3: u16 = 3;

/// Borrowed wallet-facing request for one nonterminal Rational action.
///
/// Every byte except magic, version, and the zero parent coordinate is the
/// canonical representation child. Hot hashes these exact bytes and supplies
/// that digest as the child parent context, avoiding a self-digest fixed point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenRepresentationHotRequestV3<'a> {
    bytes: &'a [u8],
}

impl<'a> OpenRepresentationHotRequestV3<'a> {
    /// Hostile-decode using caller-owned scratch of exactly the same width.
    pub fn decode_with_scratch(input: &'a [u8], scratch: &mut [u8]) -> Result<Self> {
        if input.len() != scratch.len() {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(input, REQUEST_MAGIC_OFFSET)? != OPEN_REPRESENTATION_HOT_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, REQUEST_VERSION_OFFSET)? != OPEN_REPRESENTATION_HOT_VERSION_V3 {
            return Err(Error::UnsupportedVersion);
        }
        if byte_at(input, REQUEST_CALLER_ROLE_OFFSET)? != CALLER_ROLE_TRADING {
            return Err(Error::InvalidActionShape);
        }
        require_zero(input, REQUEST_PARENT_CONTEXT_OFFSET, 32)?;
        let action = decode_open_action(byte_at(input, REQUEST_ACTION_OFFSET)?)?;
        scratch.copy_from_slice(input);
        put(scratch, REQUEST_MAGIC_OFFSET, &REQUEST_MAGIC_V2)?;
        put(
            scratch,
            REQUEST_VERSION_OFFSET,
            &PHYSICAL_ABI_VERSION_V2.to_le_bytes(),
        )?;
        put(scratch, REQUEST_PARENT_CONTEXT_OFFSET, &[1_u8; 32])?;
        let child = RepresentationRequestV2::decode(scratch)?;
        if child.header().action != action {
            return Err(Error::InvalidActionShape);
        }
        Ok(Self { bytes: input })
    }

    /// Project an already decoded canonical child into parent-free family form.
    pub fn from_child_into<'b>(
        child: RepresentationRequestV2<'_>,
        output: &'b mut [u8],
    ) -> Result<OpenRepresentationHotRequestV3<'b>> {
        decode_open_action(child.header().action as u8)?;
        child.encode_into(output)?;
        put(
            output,
            REQUEST_MAGIC_OFFSET,
            &OPEN_REPRESENTATION_HOT_MAGIC_V3,
        )?;
        put(
            output,
            REQUEST_VERSION_OFFSET,
            &OPEN_REPRESENTATION_HOT_VERSION_V3.to_le_bytes(),
        )?;
        output
            .get_mut(REQUEST_PARENT_CONTEXT_OFFSET..REQUEST_PARENT_CONTEXT_OFFSET + 32)
            .ok_or(Error::InvalidLength)?
            .fill(0);
        Ok(OpenRepresentationHotRequestV3 { bytes: output })
    }

    /// Specialize into the exact canonical Claims child under one family digest.
    pub fn specialize_child_into<'b>(
        self,
        family_digest: [u8; 32],
        output: &'b mut [u8],
    ) -> Result<RepresentationRequestV2<'b>> {
        if is_zero(family_digest) {
            return Err(Error::ZeroIdentity);
        }
        if output.len() != self.bytes.len() {
            return Err(Error::InvalidLength);
        }
        output.copy_from_slice(self.bytes);
        put(output, REQUEST_MAGIC_OFFSET, &REQUEST_MAGIC_V2)?;
        put(
            output,
            REQUEST_VERSION_OFFSET,
            &PHYSICAL_ABI_VERSION_V2.to_le_bytes(),
        )?;
        put(output, REQUEST_PARENT_CONTEXT_OFFSET, &family_digest)?;
        put_byte(output, REQUEST_CALLER_ROLE_OFFSET, CALLER_ROLE_TRADING)?;
        RepresentationRequestV2::decode(output)
    }

    /// Selected open action.
    pub fn action(self) -> Result<RepresentationActionV2> {
        decode_open_action(byte_at(self.bytes, REQUEST_ACTION_OFFSET)?)
    }

    /// Exact number of canonical asset rows.
    pub fn asset_count(self) -> Result<u32> {
        u32_at(self.bytes, REQUEST_ASSET_COUNT_OFFSET)
    }

    /// Whether this request spans the complete Product outcome set.
    pub fn is_structured(self) -> Result<bool> {
        Ok(matches!(
            self.action()?,
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
        ))
    }

    /// Exact bytes whose SHA-256 becomes the child parent context.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

fn decode_open_action(value: u8) -> Result<RepresentationActionV2> {
    match value {
        ACTION_DENOMINATE => Ok(RepresentationActionV2::Denominate),
        ACTION_RECONSTITUTE => Ok(RepresentationActionV2::Reconstitute),
        ACTION_ISSUE_STRUCTURED => Ok(RepresentationActionV2::IssueStructured),
        ACTION_UNWRAP_STRUCTURED => Ok(RepresentationActionV2::UnwrapStructured),
        _ => Err(Error::InvalidActionShape),
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use crate::{
        ABSENT_REVISION, ASSET_BYTES_V2, AssetV2, CallerRoleV2, RepresentationRequestHeaderV2,
    };
    use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn request_bytes(action: RepresentationActionV2) -> alloc::vec::Vec<u8> {
        let structured = matches!(
            action,
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
        );
        let count = if structured { 2 } else { 1 };
        let mut rows = alloc::vec![0_u8; count * ASSET_BYTES_V2];
        for index in 0..count {
            AssetV2 {
                shard_mint: id(20 + u8::try_from(index).expect("small index")),
                actor_shard_account: id(30 + u8::try_from(index).expect("small index")),
                structured_custody_account: id(40 + u8::try_from(index).expect("small index")),
                claims_custody_owner: id(50 + u8::try_from(index).expect("small index")),
                coefficient: 10,
                expected_shard_supply: 100,
                expected_actor_shards: 50,
                expected_structured_shards: if structured { 20 } else { 0 },
            }
            .encode_into(
                rows.get_mut(index * ASSET_BYTES_V2..(index + 1) * ASSET_BYTES_V2)
                    .expect("row"),
            )
            .expect("asset");
        }
        let child = RepresentationRequestV2::new(
            RepresentationRequestHeaderV2 {
                action,
                caller_role: CallerRoleV2::Trading,
                release_set: id(1),
                market: id(2),
                graph_id: id(3),
                descriptor_id: id(4),
                parent_context: id(5),
                actor: id(6),
                receipt_mint: id(7),
                receipt_account: if structured { id(8) } else { [0; 32] },
                representation_authority: id(9),
                token_program: TOKEN_2022_PROGRAM_ID,
                realm: [0; 32],
                collateral_recipient: [0; 32],
                expected_representation_revision: 3,
                expected_claims_market_revision: if structured { ABSENT_REVISION } else { 4 },
                expected_actor_position_revision: if structured { ABSENT_REVISION } else { 5 },
                expected_custody_position_revision: if structured { ABSENT_REVISION } else { 6 },
                expected_custody_replay_revision: ABSENT_REVISION,
                generation: 7,
                quantity: 2,
                denominator: 10,
                expected_receipt_supply: 8,
                outcome_count: 2,
                selected_outcome: if structured { u32::MAX } else { 1 },
                asset_count: u32::try_from(count).expect("small count"),
            },
            &rows,
        )
        .expect("child");
        let mut bytes = alloc::vec![0_u8; crate::REQUEST_HEADER_BYTES_V2 + rows.len()];
        child.encode_into(&mut bytes).expect("request bytes");
        bytes
    }

    #[test]
    fn all_open_actions_round_trip_through_one_parent_free_family() {
        for action in [
            RepresentationActionV2::Denominate,
            RepresentationActionV2::Reconstitute,
            RepresentationActionV2::IssueStructured,
            RepresentationActionV2::UnwrapStructured,
        ] {
            let child_bytes = request_bytes(action);
            let child = RepresentationRequestV2::decode(&child_bytes).expect("child");
            let mut family_bytes = alloc::vec![0_u8; child_bytes.len()];
            let width = family_bytes.len();
            let family = OpenRepresentationHotRequestV3::from_child_into(child, &mut family_bytes)
                .expect("family");
            let mut scratch = alloc::vec![0_u8; width];
            let decoded = OpenRepresentationHotRequestV3::decode_with_scratch(
                family.as_bytes(),
                &mut scratch,
            )
            .expect("family decode");
            assert_eq!(decoded.action(), Ok(action));
            assert_eq!(decoded.is_structured(), Ok(!action.selected_outcome()));
            let mut specialized = alloc::vec![0_u8; width];
            let exact = decoded
                .specialize_child_into(id(99), &mut specialized)
                .expect("specialized child");
            assert_eq!(exact.header().parent_context, id(99));
            assert_eq!(exact.header().action, action);
            assert_eq!(exact.asset_bytes(), child.asset_bytes());
        }
    }

    #[test]
    fn terminal_parent_and_same_width_substitutions_refuse() {
        let child_bytes = request_bytes(RepresentationActionV2::Denominate);
        let child = RepresentationRequestV2::decode(&child_bytes).expect("child");
        let mut family_bytes = alloc::vec![0_u8; child_bytes.len()];
        OpenRepresentationHotRequestV3::from_child_into(child, &mut family_bytes).expect("family");
        let mut scratch = alloc::vec![0_u8; family_bytes.len()];
        for offset in [REQUEST_MAGIC_OFFSET, REQUEST_PARENT_CONTEXT_OFFSET] {
            let mut hostile = family_bytes.clone();
            *hostile.get_mut(offset).expect("hostile byte") ^= 1;
            assert!(
                OpenRepresentationHotRequestV3::decode_with_scratch(&hostile, &mut scratch)
                    .is_err()
            );
        }
        let mut terminal = family_bytes;
        *terminal.get_mut(REQUEST_ACTION_OFFSET).expect("action") =
            crate::generated::ACTION_REDEEM_TERMINAL;
        assert!(
            OpenRepresentationHotRequestV3::decode_with_scratch(&terminal, &mut scratch).is_err()
        );
    }
}
