//! Compact permanent replay anchor for a closed shared Market lifecycle.
//!
//! The mutable `0xaa/1` root is deliberately deletable after all family and
//! Series obligations are terminal. Its replacement `0xb0/1` receipt is the
//! sole permanent Market/generation replay owner: creation of another root for
//! the same coordinates must refuse whenever this content-authenticated anchor
//! exists. Solana framing, PDA authentication, rent, and writes remain in the
//! adapter; this module owns the exact semantic body only.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, ContentId, Error, FixedCodec, MarketInstanceTerminalProjectionV1,
    MarketInstanceV2Id, MarketLifecyclePhaseV1, MarketLifecycleReplayReceiptV1Id,
    MarketLifecycleRootV1, RegistryCapabilityProfileV3Id, RegistryProgramReleaseV1Id, Result,
};

const MARKET_LIFECYCLE_REPLAY_MAGIC_V1: [u8; 8] = *b"DCMLRPV1";
const MARKET_LIFECYCLE_REPLAY_VERSION_V1: u16 = 1;

/// Exact semantic-body width of the permanent `0xb0/1` replay anchor.
pub const MARKET_LIFECYCLE_REPLAY_RECEIPT_BYTES_V1: usize = 389;
/// Domain of the complete permanent Market lifecycle replay receipt.
pub const MARKET_LIFECYCLE_REPLAY_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-lifecycle-replay-receipt/v1";

/// Permanent terminal replacement for one exact mutable `0xaa/1` root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketLifecycleReplayReceiptV1 {
    /// Canonical physical `0xb0/1` replay account.
    pub replay_account_id: ContentId,
    /// Exact deleted `0xaa/1` account.
    pub root_account_id: ContentId,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Immutable shared Market binding identity.
    pub market_binding_id: ContentId,
    /// Exact terminal semantic state formerly persisted by `0xaa/1`.
    pub terminal_root_semantic_id: ContentId,
    /// Whole-Market terminal projection/receipt identity.
    pub whole_market_terminal_receipt_id: ContentId,
    /// Exact physical `0xaa` rent-refund/surplus-sink close plan.
    pub root_close_disposition_id: ContentId,
    /// Exact terminal FoundationVault donation-disposition receipt.
    pub foundation_vault_disposition_receipt_id: ContentId,
    /// Loader-authenticated central Registry release frozen by the Market.
    pub registry_release_id: RegistryProgramReleaseV1Id,
    /// Current V3 central capability profile frozen by the Market.
    pub capability_profile_id: RegistryCapabilityProfileV3Id,
    /// Nonzero shared Market generation.
    pub generation: u64,
    /// Last transition sequence of the terminal root.
    pub terminal_transition_sequence: u64,
    /// Total Series/ordinal links ever admitted.
    pub admitted_series_links: u32,
    /// Must equal admitted links at terminality.
    pub retired_series_links: u32,
    /// Must be zero at terminality.
    pub live_series_links: u32,
    /// Exact completed Foundation slot bitmap.
    pub foundation_initialized_bitmap: u64,
    /// Full founder-owned Foundation principal, never donation value.
    pub foundation_principal_total_lamports: u64,
    /// Exact separately itemized permanent `0xb0/1` rent principal.
    pub permanent_replay_principal_lamports: u64,
    /// Exact active outcome count which determines the complete slot bitmap.
    pub outcome_count: u8,
}

impl MarketLifecycleReplayReceiptV1 {
    /// Seal the compact permanent postimage from the exact terminal root and
    /// its freshly re-derived whole-Market projection.
    pub fn seal(
        replay_account_id: ContentId,
        root_account_id: ContentId,
        root: &MarketLifecycleRootV1,
        terminal: MarketInstanceTerminalProjectionV1,
        foundation_vault_disposition_receipt_id: ContentId,
        root_close_disposition_id: ContentId,
        permanent_replay_principal_lamports: u64,
    ) -> Result<Self> {
        replay_account_id.validate()?;
        root_account_id.validate()?;
        foundation_vault_disposition_receipt_id.validate()?;
        root_close_disposition_id.validate()?;
        if permanent_replay_principal_lamports == 0 {
            return Err(Error::InvalidParameter);
        }
        if root.phase() != MarketLifecyclePhaseV1::Terminal
            || root.terminal_projection()? != terminal
            || terminal.root_semantic_id() != root.semantic_id()?
            || terminal.market_instance_id() != root.binding().market_instance_id
            || terminal.generation() != root.binding().generation
            || terminal.final_transition_sequence() != root.transition_sequence()
        {
            return Err(Error::MismatchedArtifact);
        }
        let value = Self {
            replay_account_id,
            root_account_id,
            market_instance_id: root.binding().market_instance_id,
            market_binding_id: root.binding().id()?,
            terminal_root_semantic_id: terminal.root_semantic_id(),
            whole_market_terminal_receipt_id: terminal.id(),
            root_close_disposition_id,
            foundation_vault_disposition_receipt_id,
            registry_release_id: RegistryProgramReleaseV1Id::from_bytes(
                root.binding().registry_release_id.bytes(),
            ),
            capability_profile_id: RegistryCapabilityProfileV3Id::from_bytes(
                root.binding().capability_profile_id.bytes(),
            ),
            generation: root.binding().generation,
            terminal_transition_sequence: root.transition_sequence(),
            admitted_series_links: root.admitted_series_links(),
            retired_series_links: root.retired_series_links(),
            live_series_links: root.live_series_links(),
            foundation_initialized_bitmap: root.foundation().initialized_bitmap,
            foundation_principal_total_lamports: root.capital().principal_total_lamports,
            permanent_replay_principal_lamports,
            outcome_count: root.binding().outcome_count,
        };
        value.validate()?;
        Ok(value)
    }

    /// Domain-separated semantic identity of the exact permanent body.
    pub fn id(&self) -> Result<MarketLifecycleReplayReceiptV1Id> {
        let mut body = [0u8; MARKET_LIFECYCLE_REPLAY_RECEIPT_BYTES_V1];
        self.encode_into(&mut body)?;
        Ok(MarketLifecycleReplayReceiptV1Id::from_bytes(
            content_id(MARKET_LIFECYCLE_REPLAY_RECEIPT_DOMAIN_V1, &body).bytes(),
        ))
    }

    /// Validate the terminal count partition and every permanent identity.
    pub fn validate(&self) -> Result<()> {
        for id in [
            self.replay_account_id,
            self.root_account_id,
            self.market_instance_id.content_id(),
            self.market_binding_id,
            self.terminal_root_semantic_id,
            self.whole_market_terminal_receipt_id,
            self.root_close_disposition_id,
            self.foundation_vault_disposition_receipt_id,
            self.registry_release_id.content_id(),
            self.capability_profile_id.content_id(),
        ] {
            id.validate()?;
        }
        let expected_bitmap = expected_foundation_bitmap(self.outcome_count)?;
        if self.generation == 0
            || self.terminal_transition_sequence == 0
            || self.admitted_series_links == 0
            || self.live_series_links != 0
            || self.retired_series_links != self.admitted_series_links
            || self.foundation_initialized_bitmap != expected_bitmap
            || self.foundation_principal_total_lamports == 0
            || self.permanent_replay_principal_lamports == 0
            || self.replay_account_id == self.root_account_id
            || self.registry_release_id.content_id() == self.capability_profile_id.content_id()
        {
            return Err(Error::WorkStateMismatch);
        }
        Ok(())
    }
}

impl FixedCodec for MarketLifecycleReplayReceiptV1 {
    const ENCODED_LEN: usize = MARKET_LIFECYCLE_REPLAY_RECEIPT_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MARKET_LIFECYCLE_REPLAY_MAGIC_V1);
        writer.u16(MARKET_LIFECYCLE_REPLAY_VERSION_V1);
        writer.reserved(6);
        writer.id(self.replay_account_id);
        writer.id(self.root_account_id);
        writer.id(self.market_instance_id.content_id());
        writer.id(self.market_binding_id);
        writer.id(self.terminal_root_semantic_id);
        writer.id(self.whole_market_terminal_receipt_id);
        writer.id(self.root_close_disposition_id);
        writer.id(self.foundation_vault_disposition_receipt_id);
        writer.id(self.registry_release_id.content_id());
        writer.id(self.capability_profile_id.content_id());
        writer.u64(self.generation);
        writer.u64(self.terminal_transition_sequence);
        writer.u32(self.admitted_series_links);
        writer.u32(self.retired_series_links);
        writer.u32(self.live_series_links);
        writer.u64(self.foundation_initialized_bitmap);
        writer.u64(self.foundation_principal_total_lamports);
        writer.u64(self.permanent_replay_principal_lamports);
        writer.u8(self.outcome_count);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MARKET_LIFECYCLE_REPLAY_MAGIC_V1)?;
        if reader.u16() != MARKET_LIFECYCLE_REPLAY_VERSION_V1 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            replay_account_id: reader.id(),
            root_account_id: reader.id(),
            market_instance_id: MarketInstanceV2Id::from_bytes(reader.id().bytes()),
            market_binding_id: reader.id(),
            terminal_root_semantic_id: reader.id(),
            whole_market_terminal_receipt_id: reader.id(),
            root_close_disposition_id: reader.id(),
            foundation_vault_disposition_receipt_id: reader.id(),
            registry_release_id: RegistryProgramReleaseV1Id::from_bytes(reader.id().bytes()),
            capability_profile_id: RegistryCapabilityProfileV3Id::from_bytes(reader.id().bytes()),
            generation: reader.u64(),
            terminal_transition_sequence: reader.u64(),
            admitted_series_links: reader.u32(),
            retired_series_links: reader.u32(),
            live_series_links: reader.u32(),
            foundation_initialized_bitmap: reader.u64(),
            foundation_principal_total_lamports: reader.u64(),
            permanent_replay_principal_lamports: reader.u64(),
            outcome_count: reader.u8(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

fn expected_foundation_bitmap(outcome_count: u8) -> Result<u64> {
    let outcomes = u32::from(outcome_count);
    if outcomes == 0 || outcomes > 16 {
        return Err(Error::InvalidParameter);
    }
    let active_outcomes = 1_u64
        .checked_shl(outcomes)
        .ok_or(Error::ArithmeticOverflow)?
        .checked_sub(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let core = 1_u64
        .checked_shl(14)
        .ok_or(Error::ArithmeticOverflow)?
        .checked_sub(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let mints = active_outcomes
        .checked_shl(14)
        .ok_or(Error::ArithmeticOverflow)?;
    let custody = active_outcomes
        .checked_shl(30)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(core | mints | custody)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value() -> MarketLifecycleReplayReceiptV1 {
        MarketLifecycleReplayReceiptV1 {
            replay_account_id: ContentId::from_bytes([1; 32]),
            root_account_id: ContentId::from_bytes([2; 32]),
            market_instance_id: MarketInstanceV2Id::from_bytes([3; 32]),
            market_binding_id: ContentId::from_bytes([4; 32]),
            terminal_root_semantic_id: ContentId::from_bytes([5; 32]),
            whole_market_terminal_receipt_id: ContentId::from_bytes([6; 32]),
            root_close_disposition_id: ContentId::from_bytes([7; 32]),
            foundation_vault_disposition_receipt_id: ContentId::from_bytes([8; 32]),
            registry_release_id: RegistryProgramReleaseV1Id::from_bytes([9; 32]),
            capability_profile_id: RegistryCapabilityProfileV3Id::from_bytes([10; 32]),
            generation: 1,
            terminal_transition_sequence: 12,
            admitted_series_links: 2,
            retired_series_links: 2,
            live_series_links: 0,
            foundation_initialized_bitmap: expected_foundation_bitmap(2).unwrap(),
            foundation_principal_total_lamports: 99,
            permanent_replay_principal_lamports: 17,
            outcome_count: 2,
        }
    }

    #[test]
    fn exact_round_trip_and_trailing_refusal() {
        let value = value();
        let mut body = [0u8; MARKET_LIFECYCLE_REPLAY_RECEIPT_BYTES_V1];
        value.encode_into(&mut body).unwrap();
        assert_eq!(MarketLifecycleReplayReceiptV1::decode(&body), Ok(value));
        let mut trailing = [0u8; MARKET_LIFECYCLE_REPLAY_RECEIPT_BYTES_V1 + 1];
        trailing[..body.len()].copy_from_slice(&body);
        assert_eq!(
            MarketLifecycleReplayReceiptV1::decode(&trailing),
            Err(Error::TrailingBytes)
        );
    }

    #[test]
    fn live_link_or_aliased_replay_refuses() {
        let mut invalid = value();
        invalid.live_series_links = 1;
        assert_eq!(invalid.validate(), Err(Error::WorkStateMismatch));
        invalid = value();
        invalid.replay_account_id = invalid.root_account_id;
        assert_eq!(invalid.validate(), Err(Error::WorkStateMismatch));
    }
}
