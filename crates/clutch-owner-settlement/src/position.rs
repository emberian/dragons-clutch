//! Canonical Position V3 authentication and settlement poststate projection.

use clutch_retirement::{
    PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, PositionV3Fields,
};

use crate::{Error, Result, MAX_OUTCOMES};

/// Adapter-authenticated canonical Position V3 prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedPositionV3 {
    /// Canonical Position V3 PDA.
    pub account: [u8; 32],
    /// General runtime Market PDA, distinct from the full MarketInstance ID.
    pub general_market_runtime: [u8; 32],
    /// Exact decoded 480-byte Position V3 semantic owner.
    pub semantic: PositionAccountV3,
    /// Adapter-authenticated semantic ID of `semantic`.
    pub semantic_id: [u8; 32],
    /// Replay sequence observed before receipt-authorized settlement.
    pub replay_sequence: u64,
    /// True only after program-owner, PDA, and purpose binding authentication.
    pub account_authenticated: bool,
    /// True only after recomputing the Position V3 semantic ID.
    pub semantic_id_authenticated: bool,
    /// True only after joining the General runtime Market to the full
    /// MarketInstanceV2/Realm/policy/release identities in `semantic`.
    pub market_binding_authenticated: bool,
    /// Whether the Position account meta is writable.
    pub writable: bool,
}

impl AuthenticatedPositionV3 {
    /// Validate canonical Position semantics and the adapter-owned account facts.
    pub fn validate(self) -> Result<()> {
        self.semantic
            .validate()
            .map_err(|_| Error::InvalidAccount)?;
        let fields = self.semantic.fields();
        if self.account == [0; 32]
            || self.general_market_runtime == [0; 32]
            || self.semantic_id == [0; 32]
            || fields.purpose != PositionPurposeV3::General
            || fields.lifecycle != PositionLifecycleV3::Open
            || self.account == fields.owner.bytes()
            || self.account == fields.replay_account.bytes()
            || self.account == fields.market_instance_id.bytes()
            || self.account == self.general_market_runtime
            || self.general_market_runtime == fields.market_instance_id.bytes()
            || !self.account_authenticated
            || !self.semantic_id_authenticated
            || !self.market_binding_authenticated
            || !self.writable
        {
            return Err(Error::InvalidAccount);
        }
        Ok(())
    }

    /// Return a successor preserving every identity, lifecycle, purpose, rent,
    /// generation, controller, Replay, and outstanding-reservation fact.
    pub fn settlement_poststate(
        self,
        cash_atoms: u64,
        reserved_cash_atoms: u64,
        native_eggs: [u64; MAX_OUTCOMES],
    ) -> Result<PositionSettlementPoststateV3> {
        self.validate()?;
        let old = self.semantic.fields();
        let semantic = PositionAccountV3::new(PositionV3Fields {
            cash_atoms,
            reserved_cash_atoms,
            native_eggs,
            ..old
        })
        .map_err(|_| Error::InvalidAccount)?;
        Ok(PositionSettlementPoststateV3 {
            account: self.account,
            general_market_runtime: self.general_market_runtime,
            prestate_semantic_id: self.semantic_id,
            semantic,
            replay_sequence: self.replay_sequence,
        })
    }
}

/// Canonical Position V3 poststate awaiting adapter semantic-ID derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PositionSettlementPoststateV3 {
    /// Canonical Position V3 PDA to write.
    pub account: [u8; 32],
    /// General runtime Market PDA preserved across settlement.
    pub general_market_runtime: [u8; 32],
    /// Exact authenticated semantic ID that this successor must replace.
    pub prestate_semantic_id: [u8; 32],
    /// Exact canonical Position V3 successor body.
    pub semantic: PositionAccountV3,
    /// Receipt-authorized settlement preserves Replay sequence.
    pub replay_sequence: u64,
}
