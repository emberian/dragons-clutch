//! The revenue plane's one account family: the per-Realm policy record.
//!
//! `docs/design/REVENUE_POLICY_V1.md` §3, under the adopted decisions of
//! `docs/decisions/ADOPTED_2026-08-20.md` items 6 and 8:
//!
//! | account | tag | bytes | what it holds |
//! | --- | ---: | ---: | --- |
//! | [`RevenuePolicyRecordV1`] | 27/v1 | 156 | historical deferred-treasury pin; decode-only for successor admission |
//! | [`RevenuePolicyRecordV2`] | 27/v2 | 160 | one successor Realm's immutable fee rates, beneficiary, Market-scoped Position lifecycle, and exact deletable-rent owner |
//!
//! **The record's absence IS the zero-take state (D4).**  Existing Realms are
//! zero-take forever by construction: no retrofit instruction exists, because
//! any retrofit authority is exactly the "silently redirect" surface
//! `docs/DEPLOYMENT_REVENUE_BOUNDARY.md` forbids.  A record exists only for a
//! Realm that elected the policy **in the same transition that created the
//! Realm**, and no instruction mutates one after creation — the
//! no-silent-redirect falsifier (§10.7) is a property of the instruction
//! set's shape, re-checked by the SVM tests.
//!
//! **The TerminalIdentityV1 header rides from day one** (B4f): bytes 98..154
//! are the exact 56-byte header layout of the R4 terminal design
//! (`research/terminal-identity-v1`: payer, payer_principal, donation_floor,
//! generation — no magic, no padding), embedded rather than sibling-shaped,
//! so the Realm-lifetime classification can tighten later without an ABI
//! change.  Close stays *admissible* (principal to the stored payer, surplus
//! burned) and is gated on the Realm account's absence — the Realm row is
//! PERMANENT_INFRA with no close route, so the record is permanent in
//! practice and `terminal_profile.py` says exactly that.
//!
//! No vault family exists here and none may be added: B4c froze every Plane-L
//! charge at zero and **no vault is built**.

use super::{
    check_hash, check_header, put_header, CodecError, Hash32, Reader, RealmHash, Result, Writer,
};

/// Account discriminator of the per-Realm revenue-policy record.
pub const REVENUE_POLICY_RECORD_TAG: u8 = 27;
/// First revenue-policy-record schema.
pub const REVENUE_POLICY_RECORD_VERSION: u8 = 1;
/// Exact fixed length of one revenue-policy record.
///
/// Header 2, realm 32, policy digest 32, treasury 32, TerminalIdentityV1
/// header 56 (payer 32 + principal 8 + donation floor 8 + generation 8),
/// stored bump 1, flags 1.
pub const REVENUE_POLICY_RECORD_BYTES: usize = 2 + 32 + 32 + 32 + 56 + 1 + 1;

/// Fresh fee-bearing revenue-policy-record schema.  V1 bytes are never
/// interpreted through this version.
pub const REVENUE_POLICY_RECORD_VERSION_V2: u8 = 2;
/// Exact V2 record length: header 2, three identities 96, lifecycle selector
/// plus reserved bytes 4, payer/principal/donation/generation 56, bump 1,
/// flags 1.
pub const REVENUE_POLICY_RECORD_BYTES_V2: usize = 2 + 32 + 32 + 32 + 4 + 56 + 1 + 1;
const _: () = assert!(REVENUE_POLICY_RECORD_BYTES_V2 == 160);
const _: () = assert!(
    REVENUE_POLICY_RECORD_TAG
        == crate::registry::REVENUE_POLICY_RECORD_V2_ACCOUNT_TAG
);
const _: () = assert!(
    REVENUE_POLICY_RECORD_VERSION_V2
        == crate::registry::REVENUE_POLICY_RECORD_V2_ACCOUNT_VERSION
);
const _: () = assert!(
    REVENUE_POLICY_RECORD_BYTES_V2
        == crate::registry::REVENUE_POLICY_RECORD_V2_ACCOUNT_BYTES
);

/// Byte offset of the embedded 56-byte TerminalIdentityV1 header.
pub const REVENUE_POLICY_RECORD_TERMINAL_AT: usize = 2 + 32 + 32 + 32;

/// One Realm's immutable revenue-policy pin (design §3).
///
/// `policy_digest` is the identity of the frozen `RevenuePolicyV1` const the
/// Realm elected at birth; `treasury` is that const's recipient, copied out
/// for account-list identity checks (in V1 always the structural UNSET
/// sentinel — the B4a deferral — which the fee-bearing admission seam
/// refuses).  The terminal fields are the embedded header: the exact funding
/// wallet, its exact recorded outlay for this record, the monotone donation
/// floor observed at creation, and the close/reopen generation (counted from
/// 1; the record never reopens, so it stays 1 for life).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevenuePolicyRecordV1 {
    /// Realm this record binds, forever.
    pub realm: RealmHash,
    /// Digest of the frozen revenue-policy const pinned at Realm creation.
    pub policy_digest: Hash32,
    /// The policy's recipient identity, copied out of the const.
    pub treasury: Hash32,
    /// TerminalIdentityV1: exact funding wallet, sole principal recipient.
    pub terminal_payer: Hash32,
    /// TerminalIdentityV1: exact lamports debited after prefund
    /// normalization.
    pub terminal_payer_principal: u64,
    /// TerminalIdentityV1: monotone donation lower bound at creation.
    pub terminal_donation_floor: u64,
    /// TerminalIdentityV1: close/reopen era, counted from 1.
    pub terminal_generation: u64,
    /// Stored PDA bump, opaque to this crate.
    pub stored_bump: u8,
    /// Reserved flags; zero in V1.
    pub flags: u8,
}

impl RevenuePolicyRecordV1 {
    /// Refuse every shape the layout can express but the design forbids:
    /// zero identities, a zero payer, a zero recorded principal, a
    /// generation outside the from-1 convention, and reserved flags.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.realm)?;
        check_hash(self.policy_digest)?;
        check_hash(self.treasury)?;
        check_hash(self.terminal_payer)?;
        if self.terminal_payer_principal == 0 {
            return Err(CodecError::InvalidCount);
        }
        if self.terminal_generation != 1 {
            return Err(CodecError::InvalidCount);
        }
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }

    /// Whether this record names `owner` as the Realm's revenue treasury.
    ///
    /// The B4b mid-epoch-close grief rider's one predicate: a Position the
    /// record points at is *serving* every fee-bearing epoch of that Realm,
    /// and a close route must consult
    /// `clutch_liveness::TreasuryServiceLedger` before retiring it.  V1 pins
    /// the treasury at the structural UNSET sentinel, so this is false for
    /// every Position that can exist today — which is the fact the rider's
    /// falsifier pins, not an accident it relies on.
    pub fn names_treasury(&self, owner: Hash32) -> bool {
        self.treasury == owner
    }

    /// Encode exactly [`REVENUE_POLICY_RECORD_BYTES`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < REVENUE_POLICY_RECORD_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(
            &mut w,
            REVENUE_POLICY_RECORD_TAG,
            REVENUE_POLICY_RECORD_VERSION,
        )?;
        w.hash(self.realm)?;
        w.hash(self.policy_digest)?;
        w.hash(self.treasury)?;
        w.hash(self.terminal_payer)?;
        w.u64(self.terminal_payer_principal)?;
        w.u64(self.terminal_donation_floor)?;
        w.u64(self.terminal_generation)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        if w.at != REVENUE_POLICY_RECORD_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(w.at)
    }

    /// Parse exactly [`REVENUE_POLICY_RECORD_BYTES`] hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        check_header(
            input,
            REVENUE_POLICY_RECORD_TAG,
            REVENUE_POLICY_RECORD_VERSION,
            REVENUE_POLICY_RECORD_BYTES,
        )?;
        let mut r = Reader::at(input, 2);
        let value = Self {
            realm: r.hash()?,
            policy_digest: r.hash()?,
            treasury: r.hash()?,
            terminal_payer: r.hash()?,
            terminal_payer_principal: r.u64()?,
            terminal_donation_floor: r.u64()?,
            terminal_generation: r.u64()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        value.validate()?;
        Ok(value)
    }
}

/// One successor Realm's immutable fee-bearing authority.
///
/// The record is created atomically with the Realm and never updated.  The
/// policy digest binds rates and split; copied-out owner and lifecycle facts
/// make downstream account-list checks local while the adapter must still
/// reauthenticate their equality to the policy preimage.  The lifecycle is
/// Market-scoped: this account never pretends that a Position address can be
/// known before a MarketInstance exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevenuePolicyRecordV2 {
    /// Realm created atomically with this record.
    pub realm: RealmHash,
    /// Immutable [`clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2`]
    /// digest.
    pub policy_digest: Hash32,
    /// Immutable owner of every derived Market treasury Position.
    pub treasury_owner: Hash32,
    /// Exact per-Market ordinary-Position and counted-service-ledger policy.
    pub treasury_position_derivation:
        clutch_batch_policy_identity::revenue_policy_v2::TreasuryPositionDerivationPolicyV2,
    /// Exact creator payer and sole refundable-principal recipient.
    pub terminal_payer: Hash32,
    /// Lamports actually debited from `terminal_payer` after hostile prefund
    /// normalization.
    pub terminal_payer_principal: u64,
    /// Initial hostile prefund, never refundable to the creator.
    pub terminal_donation_floor: u64,
    /// Close/reopen generation.  Immutable Realm founding admits exactly 1;
    /// no reopen route exists.
    pub terminal_generation: u64,
    /// Stored record PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; zero in V2.
    pub flags: u8,
}

impl RevenuePolicyRecordV2 {
    /// Validate every local layout invariant.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.realm)?;
        check_hash(self.policy_digest)?;
        check_hash(self.treasury_owner)?;
        check_hash(self.terminal_payer)?;
        if self.terminal_payer_principal == 0 || self.terminal_generation != 1 {
            return Err(CodecError::InvalidCount);
        }
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }

    /// Authenticate copied-out owner, lifecycle, and digest against the exact
    /// policy preimage.  This is required in addition to decoding the record.
    pub fn binds_policy(
        &self,
        policy: &clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2,
    ) -> Result<()> {
        self.validate()?;
        let digest =
            clutch_batch_policy_identity::revenue_policy_v2::revenue_policy_v2_digest(policy)
                .map_err(|_| CodecError::MismatchedBinding)?;
        if self.policy_digest != Hash32::from_bytes(digest.0)
            || self.treasury_owner != Hash32::from_bytes(policy.treasury_owner)
            || self.treasury_position_derivation != policy.treasury_position_derivation
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Whether this record names `owner` as its immutable treasury owner.
    pub fn names_treasury(&self, owner: Hash32) -> bool {
        self.treasury_owner == owner
    }

    /// Encode exactly [`REVENUE_POLICY_RECORD_BYTES_V2`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < REVENUE_POLICY_RECORD_BYTES_V2 {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(
            &mut w,
            REVENUE_POLICY_RECORD_TAG,
            REVENUE_POLICY_RECORD_VERSION_V2,
        )?;
        w.hash(self.realm)?;
        w.hash(self.policy_digest)?;
        w.hash(self.treasury_owner)?;
        w.u8(self.treasury_position_derivation.byte())?;
        w.u8(0)?;
        w.u8(0)?;
        w.u8(0)?;
        w.hash(self.terminal_payer)?;
        w.u64(self.terminal_payer_principal)?;
        w.u64(self.terminal_donation_floor)?;
        w.u64(self.terminal_generation)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        if w.at != REVENUE_POLICY_RECORD_BYTES_V2 {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(w.at)
    }

    /// Decode exactly one hostile V2 record image.
    pub fn decode(input: &[u8]) -> Result<Self> {
        check_header(
            input,
            REVENUE_POLICY_RECORD_TAG,
            REVENUE_POLICY_RECORD_VERSION_V2,
            REVENUE_POLICY_RECORD_BYTES_V2,
        )?;
        let mut r = Reader::at(input, 2);
        let realm = r.hash()?;
        let policy_digest = r.hash()?;
        let treasury_owner = r.hash()?;
        let treasury_position_derivation =
            clutch_batch_policy_identity::revenue_policy_v2::TreasuryPositionDerivationPolicyV2::decode(
                r.u8()?,
            )
            .map_err(|_| CodecError::InvalidEnum)?;
        if r.u8()? != 0 || r.u8()? != 0 || r.u8()? != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        let value = Self {
            realm,
            policy_digest,
            treasury_owner,
            treasury_position_derivation,
            terminal_payer: r.hash()?,
            terminal_payer_principal: r.u64()?,
            terminal_donation_floor: r.u64()?,
            terminal_generation: r.u64()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> RevenuePolicyRecordV1 {
        RevenuePolicyRecordV1 {
            realm: Hash32::from_bytes([1; 32]),
            policy_digest: Hash32::from_bytes([2; 32]),
            treasury: Hash32::from_bytes(
                clutch_batch_policy_identity::revenue_policy_v1::REVENUE_TREASURY_UNSET_V1,
            ),
            terminal_payer: Hash32::from_bytes([4; 32]),
            terminal_payer_principal: 1_976_640,
            terminal_donation_floor: 0,
            terminal_generation: 1,
            stored_bump: 254,
            flags: 0,
        }
    }

    fn record_v2() -> (
        RevenuePolicyRecordV2,
        clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2,
    ) {
        use clutch_batch_policy_identity::revenue_policy_v2::{
            revenue_policy_v2_digest, RevenuePolicyV2,
        };
        let policy = RevenuePolicyV2::successor_development([3; 32]);
        let digest = revenue_policy_v2_digest(&policy).unwrap();
        (
            RevenuePolicyRecordV2 {
                realm: Hash32::from_bytes([1; 32]),
                policy_digest: Hash32::from_bytes(digest.0),
                treasury_owner: Hash32::from_bytes(policy.treasury_owner),
                treasury_position_derivation: policy.treasury_position_derivation,
                terminal_payer: Hash32::from_bytes([4; 32]),
                terminal_payer_principal: 2_000_000,
                terminal_donation_floor: 17,
                terminal_generation: 1,
                stored_bump: 253,
                flags: 0,
            },
            policy,
        )
    }

    #[test]
    fn v2_roundtrips_and_binds_the_exact_policy() {
        let (record, policy) = record_v2();
        let mut bytes = [0u8; REVENUE_POLICY_RECORD_BYTES_V2];
        assert_eq!(record.encode(&mut bytes), Ok(REVENUE_POLICY_RECORD_BYTES_V2));
        assert_eq!(RevenuePolicyRecordV2::decode(&bytes), Ok(record));
        assert_eq!(record.binds_policy(&policy), Ok(()));

        let different =
            clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2::successor_development(
                [9; 32],
            );
        assert_eq!(record.binds_policy(&different), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn v2_refuses_width_alias_padding_and_rent_owner_mutations() {
        let (record, _) = record_v2();
        let mut bytes = [0u8; REVENUE_POLICY_RECORD_BYTES_V2];
        record.encode(&mut bytes).unwrap();
        assert!(RevenuePolicyRecordV1::decode(&bytes).is_err());
        assert!(RevenuePolicyRecordV2::decode(&bytes[..159]).is_err());
        let mut long = [0u8; REVENUE_POLICY_RECORD_BYTES_V2 + 1];
        long[..REVENUE_POLICY_RECORD_BYTES_V2].copy_from_slice(&bytes);
        assert!(RevenuePolicyRecordV2::decode(&long).is_err());
        for index in [98usize, 99, 100, 101] {
            let mut hostile = bytes;
            hostile[index] = 0xff;
            assert!(RevenuePolicyRecordV2::decode(&hostile).is_err());
        }
        for hostile in [
            RevenuePolicyRecordV2 { treasury_owner: Hash32::ZERO, ..record },
            RevenuePolicyRecordV2 { terminal_payer: Hash32::ZERO, ..record },
            RevenuePolicyRecordV2 { terminal_payer_principal: 0, ..record },
            RevenuePolicyRecordV2 { terminal_generation: 2, ..record },
            RevenuePolicyRecordV2 { flags: 1, ..record },
        ] {
            assert!(hostile.validate().is_err());
        }
    }

    #[test]
    fn record_round_trips_and_refuses_every_forbidden_shape() {
        let value = record();
        let mut bytes = [0u8; REVENUE_POLICY_RECORD_BYTES];
        assert_eq!(value.encode(&mut bytes), Ok(REVENUE_POLICY_RECORD_BYTES));
        assert_eq!(RevenuePolicyRecordV1::decode(&bytes), Ok(value));

        // Truncation and trailing bytes both refuse.
        assert!(RevenuePolicyRecordV1::decode(&bytes[..REVENUE_POLICY_RECORD_BYTES - 1]).is_err());
        let mut long = [0u8; REVENUE_POLICY_RECORD_BYTES + 1];
        long[..REVENUE_POLICY_RECORD_BYTES].copy_from_slice(&bytes);
        assert!(RevenuePolicyRecordV1::decode(&long).is_err());

        // Forbidden shapes refuse before any byte is trusted.
        for broken in [
            RevenuePolicyRecordV1 {
                realm: Hash32::ZERO,
                ..value
            },
            RevenuePolicyRecordV1 {
                policy_digest: Hash32::ZERO,
                ..value
            },
            RevenuePolicyRecordV1 {
                treasury: Hash32::ZERO,
                ..value
            },
            RevenuePolicyRecordV1 {
                terminal_payer: Hash32::ZERO,
                ..value
            },
            RevenuePolicyRecordV1 {
                terminal_payer_principal: 0,
                ..value
            },
            RevenuePolicyRecordV1 {
                terminal_generation: 0,
                ..value
            },
            RevenuePolicyRecordV1 {
                terminal_generation: 2,
                ..value
            },
            RevenuePolicyRecordV1 { flags: 1, ..value },
        ] {
            assert!(broken.validate().is_err());
            let mut out = [0u8; REVENUE_POLICY_RECORD_BYTES];
            assert!(broken.encode(&mut out).is_err());
        }
    }

    /// The embedded terminal header is byte-identical to the R4 design's
    /// 56-byte TerminalIdentityV1 layout: same offsets, same encoding, no
    /// magic, nothing behind it.  This is what makes "header from day one"
    /// a checked fact instead of a naming convention.
    #[test]
    fn the_embedded_terminal_header_matches_the_r4_layout() {
        let value = record();
        let mut bytes = [0u8; REVENUE_POLICY_RECORD_BYTES];
        value.encode(&mut bytes).unwrap();
        let at = REVENUE_POLICY_RECORD_TERMINAL_AT;
        assert_eq!(&bytes[at..at + 32], &value.terminal_payer.bytes());
        assert_eq!(
            bytes[at + 32..at + 40],
            value.terminal_payer_principal.to_le_bytes()
        );
        assert_eq!(
            bytes[at + 40..at + 48],
            value.terminal_donation_floor.to_le_bytes()
        );
        assert_eq!(
            bytes[at + 48..at + 56],
            value.terminal_generation.to_le_bytes()
        );
        // The header span ends exactly where the bump begins.
        assert_eq!(at + 56, REVENUE_POLICY_RECORD_BYTES - 2);
    }
}
