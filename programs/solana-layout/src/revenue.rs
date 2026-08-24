//! The revenue plane's immutable per-Realm policy record and counted
//! per-Market treasury-service ledger.
//!
//! `docs/design/REVENUE_POLICY_V1.md` §3, under the adopted decisions of
//! `docs/decisions/ADOPTED_2026-08-20.md` items 6 and 8:
//!
//! | account | tag | bytes | what it holds |
//! | --- | ---: | ---: | --- |
//! | [`RevenuePolicyRecordV1`] | 27/v1 | 156 | historical deferred-treasury pin; decode-only for successor admission |
//! | [`RevenuePolicyRecordV2`] | 27/v2 | 160 | one successor Realm's immutable fee rates, beneficiary, Market-scoped Position lifecycle, and exact deletable-rent owner |
//! | [`TreasuryServiceLedgerV1`] | 0xbb/v1 | 268 | one Market's exhaustive admitted/settled fee-bearing Epoch counts and deletable-rent owner |
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
use clutch_batch_policy_identity::revenue_policy_v2::{
    canonical_revenue_policy_v2_bytes, decode_revenue_policy_v2, RevenuePolicyV2,
    TreasuryPositionDerivationPolicyV2, REVENUE_POLICY_V2_BYTES,
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

/// Exact initialize payload width: Profile 32, Realm nonce 8, max outcomes 1,
/// Profile version 1, and the canonical 80-byte RevenuePolicyV2 preimage.
pub const INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES: usize =
    32 + 8 + 1 + 1 + REVENUE_POLICY_V2_BYTES;
/// Exact record-close payload width: Realm identity only.
pub const CLOSE_REVENUE_POLICY_RECORD_V2_PAYLOAD_BYTES: usize = 32;

/// Treasury-service-ledger account discriminator.
pub const TREASURY_SERVICE_LEDGER_V1_TAG: u8 = 0xbb;
/// Treasury-service-ledger account version.
pub const TREASURY_SERVICE_LEDGER_V1_VERSION: u8 = 1;
/// Exact fixed ledger length.
pub const TREASURY_SERVICE_LEDGER_V1_BYTES: usize =
    2 + (6 * 32) + (3 * 8) + 48 + 1 + 1;
const _: () = assert!(TREASURY_SERVICE_LEDGER_V1_BYTES == 268);
const _: () = assert!(
    TREASURY_SERVICE_LEDGER_V1_TAG
        == crate::registry::TREASURY_SERVICE_LEDGER_V1_ACCOUNT_TAG
);
const _: () = assert!(
    TREASURY_SERVICE_LEDGER_V1_VERSION
        == crate::registry::TREASURY_SERVICE_LEDGER_V1_ACCOUNT_VERSION
);
const _: () = assert!(
    TREASURY_SERVICE_LEDGER_V1_BYTES
        == crate::registry::TREASURY_SERVICE_LEDGER_V1_ACCOUNT_BYTES
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

/// Exact payload for atomic Realm plus RevenuePolicyRecordV2 founding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitializeFeeBearingRealmV2Payload {
    /// Canonical Profile identity recomputed from the collateral policy.
    pub profile: Hash32,
    /// Realm nonce.
    pub realm_nonce: u64,
    /// Exact current Realm outcome width.
    pub max_outcomes: u8,
    /// Exact current Profile schema.
    pub profile_version: u8,
    /// Complete immutable policy preimage, including founder-selected
    /// treasury owner.
    pub policy: RevenuePolicyV2,
}

impl InitializeFeeBearingRealmV2Payload {
    /// Validate the payload independent of account metadata.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.profile)?;
        if usize::from(self.max_outcomes) != super::MAX_OUTCOMES {
            return Err(CodecError::InvalidCount);
        }
        if self.profile_version != super::PROFILE_SCHEMA_V2 {
            return Err(CodecError::InvalidEnum);
        }
        self.policy
            .validate()
            .map_err(|_| CodecError::MismatchedBinding)
    }

    /// Encode exactly [`INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES`].
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let policy_bytes = canonical_revenue_policy_v2_bytes(&self.policy)
            .map_err(|_| CodecError::MismatchedBinding)?;
        let mut writer = Writer::new(out);
        writer.hash(self.profile)?;
        writer.u64(self.realm_nonce)?;
        writer.u8(self.max_outcomes)?;
        writer.u8(self.profile_version)?;
        writer.bytes(&policy_bytes)?;
        if writer.at != INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(writer.at)
    }

    /// Decode one exact hostile payload.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES {
            return Err(CodecError::Truncated);
        }
        if input.len() > INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES {
            return Err(CodecError::TrailingBytes);
        }
        let mut reader = Reader::at(input, 0);
        let value = Self {
            profile: reader.hash()?,
            realm_nonce: reader.u64()?,
            max_outcomes: reader.u8()?,
            profile_version: reader.u8()?,
            policy: decode_revenue_policy_v2(&reader.bytes::<REVENUE_POLICY_V2_BYTES>()?)
                .map_err(|_| CodecError::MismatchedBinding)?,
        };
        reader.done()?;
        value.validate()?;
        Ok(value)
    }
}

/// Exact payload for V2 record close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseRevenuePolicyRecordV2Payload {
    /// Realm whose absent account permits the record close.
    pub realm: Hash32,
}

impl CloseRevenuePolicyRecordV2Payload {
    /// Encode exactly [`CLOSE_REVENUE_POLICY_RECORD_V2_PAYLOAD_BYTES`].
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        check_hash(self.realm)?;
        if out.len() < CLOSE_REVENUE_POLICY_RECORD_V2_PAYLOAD_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        out[..32].copy_from_slice(&self.realm.bytes());
        Ok(32)
    }

    /// Decode one exact hostile payload.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < CLOSE_REVENUE_POLICY_RECORD_V2_PAYLOAD_BYTES {
            return Err(CodecError::Truncated);
        }
        if input.len() > CLOSE_REVENUE_POLICY_RECORD_V2_PAYLOAD_BYTES {
            return Err(CodecError::TrailingBytes);
        }
        let value = Self {
            realm: Hash32::from_bytes(input.try_into().map_err(|_| CodecError::Truncated)?),
        };
        check_hash(value.realm)?;
        Ok(value)
    }
}

/// Per-Market aggregate preventing an ordinary treasury Position from closing
/// while any authenticated fee-bearing epoch remains unsettled.
///
/// Epoch identity/replay remains owned by the counted General root.  This
/// account owns only the exhaustive aggregate: each root may increment and
/// decrement once under a private adapter capability, and close requires the
/// two monotone counts equal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreasuryServiceLedgerV1 {
    /// Realm whose immutable revenue authority selected the beneficiary.
    pub realm: Hash32,
    /// Physical RevenuePolicyRecordV2 account.
    pub revenue_policy_record_account: Hash32,
    /// Semantic RevenuePolicyRecordV2 identity (rent-independent).
    pub revenue_policy_record_v2_id: Hash32,
    /// Full MarketInstanceV2 identity.
    pub market_instance_v2_id: Hash32,
    /// Immutable treasury beneficiary.
    pub treasury_owner: Hash32,
    /// Exact ordinary Market-scoped Position account guarded by this ledger.
    pub treasury_position_account: Hash32,
    /// PositionV3 generation proven at Market founding. Later treasury
    /// mutations advance the live Position generation without rewriting this
    /// immutable founding provenance.
    pub treasury_position_founding_generation: u64,
    /// Fee-bearing epoch services begun.
    pub admitted_epoch_count: u64,
    /// Fee-bearing epoch services fully settled.
    pub settled_epoch_count: u64,
    /// Exact ledger-rent payer and sole principal recipient.
    pub rent_payer: Hash32,
    /// Exact refundable rent principal.
    pub refundable_rent_principal: u64,
    /// Initial hostile prefund; never refundable to the payer.
    pub donation_floor: u64,
    /// Stored ledger PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; zero in V1.
    pub flags: u8,
}

impl TreasuryServiceLedgerV1 {
    /// Validate the exhaustive aggregate and immutable identities.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.realm,
            self.revenue_policy_record_account,
            self.revenue_policy_record_v2_id,
            self.market_instance_v2_id,
            self.treasury_owner,
            self.treasury_position_account,
            self.rent_payer,
        ] {
            check_hash(identity)?;
        }
        if self.treasury_position_founding_generation == 0 || self.refundable_rent_principal == 0 {
            return Err(CodecError::InvalidCount);
        }
        if self.settled_epoch_count > self.admitted_epoch_count {
            return Err(CodecError::AggregateClosureMismatch);
        }
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        self.refundable_rent_principal
            .checked_add(self.donation_floor)
            .ok_or(CodecError::ArithmeticOverflow)?;
        Ok(())
    }

    /// Increment exactly one privately authenticated epoch admission.
    pub fn admit_epoch(mut self) -> Result<Self> {
        self.validate()?;
        self.admitted_epoch_count = self
            .admitted_epoch_count
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?;
        Ok(self)
    }

    /// Increment exactly one privately authenticated terminal epoch service.
    pub fn settle_epoch(mut self) -> Result<Self> {
        self.validate()?;
        if self.settled_epoch_count == self.admitted_epoch_count {
            return Err(CodecError::AggregateClosureMismatch);
        }
        self.settled_epoch_count = self
            .settled_epoch_count
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?;
        Ok(self)
    }

    /// Whether every admitted service has settled and rent may proceed to the
    /// separately authenticated Market/Position close boundary.
    pub fn is_economically_closeable(&self) -> bool {
        self.validate().is_ok() && self.admitted_epoch_count == self.settled_epoch_count
    }

    /// Encode exactly [`TREASURY_SERVICE_LEDGER_V1_BYTES`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < TREASURY_SERVICE_LEDGER_V1_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut writer = Writer::new(out);
        put_header(
            &mut writer,
            TREASURY_SERVICE_LEDGER_V1_TAG,
            TREASURY_SERVICE_LEDGER_V1_VERSION,
        )?;
        for identity in [
            self.realm,
            self.revenue_policy_record_account,
            self.revenue_policy_record_v2_id,
            self.market_instance_v2_id,
            self.treasury_owner,
            self.treasury_position_account,
        ] {
            writer.hash(identity)?;
        }
        writer.u64(self.treasury_position_founding_generation)?;
        writer.u64(self.admitted_epoch_count)?;
        writer.u64(self.settled_epoch_count)?;
        writer.hash(self.rent_payer)?;
        writer.u64(self.refundable_rent_principal)?;
        writer.u64(self.donation_floor)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        if writer.at != TREASURY_SERVICE_LEDGER_V1_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(writer.at)
    }

    /// Decode exactly one hostile ledger image.
    pub fn decode(input: &[u8]) -> Result<Self> {
        check_header(
            input,
            TREASURY_SERVICE_LEDGER_V1_TAG,
            TREASURY_SERVICE_LEDGER_V1_VERSION,
            TREASURY_SERVICE_LEDGER_V1_BYTES,
        )?;
        let mut reader = Reader::at(input, 2);
        let value = Self {
            realm: reader.hash()?,
            revenue_policy_record_account: reader.hash()?,
            revenue_policy_record_v2_id: reader.hash()?,
            market_instance_v2_id: reader.hash()?,
            treasury_owner: reader.hash()?,
            treasury_position_account: reader.hash()?,
            treasury_position_founding_generation: reader.u64()?,
            admitted_epoch_count: reader.u64()?,
            settled_epoch_count: reader.u64()?,
            rent_payer: reader.hash()?,
            refundable_rent_principal: reader.u64()?,
            donation_floor: reader.u64()?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.done()?;
        value.validate()?;
        Ok(value)
    }
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

    fn service_ledger() -> TreasuryServiceLedgerV1 {
        TreasuryServiceLedgerV1 {
            realm: Hash32::from_bytes([1; 32]),
            revenue_policy_record_account: Hash32::from_bytes([2; 32]),
            revenue_policy_record_v2_id: Hash32::from_bytes([3; 32]),
            market_instance_v2_id: Hash32::from_bytes([4; 32]),
            treasury_owner: Hash32::from_bytes([5; 32]),
            treasury_position_account: Hash32::from_bytes([6; 32]),
            treasury_position_founding_generation: 1,
            admitted_epoch_count: 0,
            settled_epoch_count: 0,
            rent_payer: Hash32::from_bytes([7; 32]),
            refundable_rent_principal: 3_000_000,
            donation_floor: 19,
            stored_bump: 252,
            flags: 0,
        }
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
    fn founding_payload_roundtrips_without_a_parallel_treasury_field() {
        let value = InitializeFeeBearingRealmV2Payload {
            profile: Hash32::from_bytes([9; 32]),
            realm_nonce: 41,
            max_outcomes: u8::try_from(super::super::MAX_OUTCOMES).expect("bounded outcomes"),
            profile_version: super::super::PROFILE_SCHEMA_V2,
            policy: RevenuePolicyV2::successor_development([8; 32]),
        };
        let mut bytes = [0u8; INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES];
        assert_eq!(
            value.encode(&mut bytes),
            Ok(INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES)
        );
        assert_eq!(InitializeFeeBearingRealmV2Payload::decode(&bytes), Ok(value));
        assert!(InitializeFeeBearingRealmV2Payload::decode(&bytes[..121]).is_err());
        let mut long = [0u8; INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES + 1];
        long[..INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES].copy_from_slice(&bytes);
        assert!(InitializeFeeBearingRealmV2Payload::decode(&long).is_err());
        for index in [42usize, 43, 44, 121] {
            let mut hostile = bytes;
            hostile[index] ^= 0xff;
            assert!(InitializeFeeBearingRealmV2Payload::decode(&hostile).is_err());
        }
    }

    #[test]
    fn service_ledger_is_counted_exact_and_hostile_width_safe() {
        let value = service_ledger();
        assert!(value.is_economically_closeable());
        let begun = value.admit_epoch().unwrap();
        assert!(!begun.is_economically_closeable());
        let settled = begun.settle_epoch().unwrap();
        assert!(settled.is_economically_closeable());
        assert!(settled.settle_epoch().is_err());

        let mut bytes = [0u8; TREASURY_SERVICE_LEDGER_V1_BYTES];
        settled.encode(&mut bytes).unwrap();
        assert_eq!(TreasuryServiceLedgerV1::decode(&bytes), Ok(settled));
        assert!(TreasuryServiceLedgerV1::decode(&bytes[..267]).is_err());
        let mut long = [0u8; TREASURY_SERVICE_LEDGER_V1_BYTES + 1];
        long[..TREASURY_SERVICE_LEDGER_V1_BYTES].copy_from_slice(&bytes);
        assert!(TreasuryServiceLedgerV1::decode(&long).is_err());

        for hostile in [
            TreasuryServiceLedgerV1 {
                settled_epoch_count: 2,
                admitted_epoch_count: 1,
                ..value
            },
            TreasuryServiceLedgerV1 {
                treasury_position_founding_generation: 0,
                ..value
            },
            TreasuryServiceLedgerV1 {
                refundable_rent_principal: 0,
                ..value
            },
            TreasuryServiceLedgerV1 { flags: 1, ..value },
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
