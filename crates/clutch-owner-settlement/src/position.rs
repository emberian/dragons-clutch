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
        {
            return Err(Error::InvalidAccount);
        }
        Ok(())
    }

    /// Validate this Position for an action that stages a body mutation.
    pub fn validate_writable(self) -> Result<()> {
        self.validate()?;
        if !self.writable {
            return Err(Error::InvalidAccount);
        }
        Ok(())
    }

    /// Return the exact unchanged Position projection used by an accounting-
    /// only outer transition. This does not authorize a Position write.
    pub fn unchanged_poststate(self) -> Result<PositionSettlementPoststateV3> {
        self.validate()?;
        Ok(PositionSettlementPoststateV3 {
            account: self.account,
            general_market_runtime: self.general_market_runtime,
            prestate_semantic_id: self.semantic_id,
            semantic: self.semantic,
        })
    }

    /// Return a successor preserving every identity, lifecycle, purpose, rent,
    /// generation, controller, Replay, and outstanding-reservation fact.
    pub fn settlement_poststate(
        self,
        cash_atoms: u64,
        reserved_cash_atoms: u64,
        native_eggs: [u64; MAX_OUTCOMES],
    ) -> Result<PositionSettlementPoststateV3> {
        self.validate_writable()?;
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
        })
    }

    /// Credit ordinary free cash while preserving every identity, lifecycle,
    /// reservation, inventory, generation, controller, and rent fact.
    pub fn credit_free_cash_poststate(
        self,
        credited_atoms: u64,
    ) -> Result<PositionSettlementPoststateV3> {
        self.validate_writable()?;
        let old = self.semantic.fields();
        let cash_atoms = old
            .cash_atoms
            .checked_add(credited_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        self.settlement_poststate(cash_atoms, old.reserved_cash_atoms, old.native_eggs)
    }

    /// Return the exact Position successor for one atomically closed active
    /// Reservation. Total cash stays in Position while the released amount
    /// leaves `reserved_cash`; remaining Eggs return to `native_eggs`; and the
    /// authoritative outstanding-child count decreases exactly once.
    pub fn release_reservation_poststate(
        self,
        released_reserved_cash_atoms: u64,
        released_internal: [u64; MAX_OUTCOMES],
    ) -> Result<PositionSettlementPoststateV3> {
        self.validate_writable()?;
        let old = self.semantic.fields();
        let reserved_cash_atoms = old
            .reserved_cash_atoms
            .checked_sub(released_reserved_cash_atoms)
            .ok_or(Error::InsufficientCash)?;
        let outstanding_reservations = old
            .outstanding_reservations
            .checked_sub(1)
            .ok_or(Error::InvalidAccount)?;
        let mut native_eggs = old.native_eggs;
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            native_eggs[outcome] = native_eggs[outcome]
                .checked_add(released_internal[outcome])
                .ok_or(Error::ArithmeticOverflow)?;
            outcome += 1;
        }
        let semantic = PositionAccountV3::new(PositionV3Fields {
            reserved_cash_atoms,
            native_eggs,
            outstanding_reservations,
            ..old
        })
        .map_err(|_| Error::InvalidAccount)?;
        Ok(PositionSettlementPoststateV3 {
            account: self.account,
            general_market_runtime: self.general_market_runtime,
            prestate_semantic_id: self.semantic_id,
            semantic,
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
}

impl PositionSettlementPoststateV3 {
    /// Validate this poststate as the exact permitted balance successor of an
    /// authenticated Position V3 prestate.
    ///
    /// Every identity, purpose, lifecycle, generation, controller, Replay,
    /// Reservation-count, and rent field must be byte-for-byte preserved. The
    /// caller names the only three mutable balance compartments explicitly.
    pub fn validate_successor_of(
        self,
        prestate: AuthenticatedPositionV3,
        expected_cash_atoms: u64,
        expected_reserved_cash_atoms: u64,
        expected_native_eggs: [u64; MAX_OUTCOMES],
    ) -> Result<()> {
        prestate.validate_writable()?;
        let expected = PositionAccountV3::new(PositionV3Fields {
            cash_atoms: expected_cash_atoms,
            reserved_cash_atoms: expected_reserved_cash_atoms,
            native_eggs: expected_native_eggs,
            ..prestate.semantic.fields()
        })
        .map_err(|_| Error::InvalidAccount)?;
        if self.account != prestate.account
            || self.general_market_runtime != prestate.general_market_runtime
            || self.prestate_semantic_id != prestate.semantic_id
            || self.semantic != expected
        {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }
}
