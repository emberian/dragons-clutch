// SPDX-License-Identifier: AGPL-3.0-or-later

//! Full-width, purpose-neutral Position successor.
//!
//! This module owns canonical pure bytes only. It does not authenticate a
//! Solana program owner, derive a PDA, inspect an account, authorize a signer,
//! perform CPI, or move collateral. The runtime adapter must establish those
//! facts before treating a decoded body or one of its purpose projections as
//! an account capability.

use crate::{
    retirement_error_v2_from_v1, Identity32V1, RentSplitV2, RetirementErrorV2, IDENTITY_BYTES,
    MAX_OUTCOMES, POSITION_ACCOUNT_TAG, POSITION_ACCOUNT_VERSION_V3, POSITION_TOMBSTONE_TAG,
    POSITION_TOMBSTONE_V3_BYTES, POSITION_TOMBSTONE_VERSION_V3, POSITION_V3_BYTES,
    RENT_SPLIT_V2_BYTES,
};

/// Domain prefix for the canonical live Position V3 PDA.
pub const POSITION_V3_PDA_PREFIX: &[u8] = b"dc-position-v3";
/// Domain for `SHA256(domain || exact canonical Position V3 body)`.
pub const POSITION_V3_SEMANTIC_DOMAIN: &[u8] = b"dragons-clutch/position-account/v3\0";
/// Domain for `SHA256(domain || exact canonical Position tombstone V3 body)`.
pub const POSITION_TOMBSTONE_V3_SEMANTIC_DOMAIN: &[u8] = b"dragons-clutch/position-tombstone/v3\0";

const POSITION_V3_HEADER_BYTES: usize = 16;
const POSITION_V3_ID_COUNT: usize = 8;
const POSITION_V3_IDS_BYTES: usize = POSITION_V3_ID_COUNT * IDENTITY_BYTES;
const POSITION_V3_CASH_OFFSET: usize = POSITION_V3_HEADER_BYTES + POSITION_V3_IDS_BYTES;
const POSITION_V3_RESERVED_CASH_OFFSET: usize = POSITION_V3_CASH_OFFSET + 8;
const POSITION_V3_EGGS_OFFSET: usize = POSITION_V3_RESERVED_CASH_OFFSET + 8;
const POSITION_V3_RESERVATION_COUNT_OFFSET: usize = POSITION_V3_EGGS_OFFSET + MAX_OUTCOMES * 8;
const POSITION_V3_RENT_OFFSET: usize = POSITION_V3_RESERVATION_COUNT_OFFSET + 8;

const POSITION_TOMBSTONE_V3_HEADER_BYTES: usize = 16;
const POSITION_TOMBSTONE_V3_ID_COUNT: usize = 8;
const POSITION_TOMBSTONE_V3_PRINCIPAL_OFFSET: usize =
    POSITION_TOMBSTONE_V3_HEADER_BYTES + POSITION_TOMBSTONE_V3_ID_COUNT * IDENTITY_BYTES;

const _: () = assert!(POSITION_V3_CASH_OFFSET == 272);
const _: () = assert!(POSITION_V3_RESERVED_CASH_OFFSET == 280);
const _: () = assert!(POSITION_V3_EGGS_OFFSET == 288);
const _: () = assert!(POSITION_V3_RESERVATION_COUNT_OFFSET == 416);
const _: () = assert!(POSITION_V3_RENT_OFFSET == 424);
const _: () = assert!(POSITION_V3_RENT_OFFSET + RENT_SPLIT_V2_BYTES == POSITION_V3_BYTES);
const _: () = assert!(POSITION_TOMBSTONE_V3_PRINCIPAL_OFFSET == 272);
const _: () = assert!(POSITION_TOMBSTONE_V3_PRINCIPAL_OFFSET + 8 == POSITION_TOMBSTONE_V3_BYTES);

/// Purpose selected for one canonical Position body.
///
/// The purpose changes its binding join, not its balance representation. User,
/// LP, treasury, Dealer, Series, and structured-claim cash therefore cannot
/// acquire parallel persisted balance DTOs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PositionPurposeV3 {
    /// General owner cash, including ordinary users, LPs, and treasury owners.
    General = 1,
    /// Dealer facility cash joined to a Dealer-owned full-width binding.
    DealerFacility = 2,
    /// Series-owned cash joined to its exact Series funding or runtime binding.
    Series = 3,
    /// Structured-claim custody joined to its exact claim binding.
    StructuredClaim = 4,
}

impl PositionPurposeV3 {
    fn decode(value: u8) -> Result<Self, RetirementErrorV2> {
        match value {
            1 => Ok(Self::General),
            2 => Ok(Self::DealerFacility),
            3 => Ok(Self::Series),
            4 => Ok(Self::StructuredClaim),
            _ => Err(RetirementErrorV2::InvalidEnum),
        }
    }
}

impl From<PositionPurposeV3> for u8 {
    fn from(value: PositionPurposeV3) -> Self {
        match value {
            PositionPurposeV3::General => 1,
            PositionPurposeV3::DealerFacility => 2,
            PositionPurposeV3::Series => 3,
            PositionPurposeV3::StructuredClaim => 4,
        }
    }
}

/// Live lifecycle phase stored by Position V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PositionLifecycleV3 {
    /// The owner/controller may perform admitted economic transitions.
    Open = 1,
    /// Economic mutation is disabled and retirement preconditions are pending.
    CloseRequested = 2,
}

impl PositionLifecycleV3 {
    fn decode(value: u8) -> Result<Self, RetirementErrorV2> {
        match value {
            1 => Ok(Self::Open),
            2 => Ok(Self::CloseRequested),
            _ => Err(RetirementErrorV2::InvalidEnum),
        }
    }
}

impl From<PositionLifecycleV3> for u8 {
    fn from(value: PositionLifecycleV3) -> Self {
        match value {
            PositionLifecycleV3::Open => 1,
            PositionLifecycleV3::CloseRequested => 2,
        }
    }
}

/// Lifecycle marker persisted by every permanent Position V3 tombstone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PositionTombstoneLifecycleV3 {
    /// The live body is retired and its exact-generation Replay was deleted.
    Closed = 1,
}

impl PositionTombstoneLifecycleV3 {
    fn decode(value: u8) -> Result<Self, RetirementErrorV2> {
        match value {
            1 => Ok(Self::Closed),
            _ => Err(RetirementErrorV2::InvalidEnum),
        }
    }
}

impl From<PositionTombstoneLifecycleV3> for u8 {
    fn from(value: PositionTombstoneLifecycleV3) -> Self {
        match value {
            PositionTombstoneLifecycleV3::Closed => 1,
        }
    }
}

/// Hashing boundary supplied by a pure kernel or runtime adapter.
///
/// Keeping the backend external lets this codec remain independent of a
/// particular SHA implementation while freezing the exact domain and body.
pub trait PositionV3Sha256Backend {
    /// Return SHA-256 of the exact concatenation `domain || body`.
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; IDENTITY_BYTES];
}

/// Caller-owned founding or successor fields before canonical validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionV3Fields {
    /// Purpose selecting the one exact external binding interpretation.
    pub purpose: PositionPurposeV3,
    /// Current live lifecycle phase.
    pub lifecycle: PositionLifecycleV3,
    /// Authenticated Market outcome width, in `1..=16`.
    pub outcome_count: u8,
    /// Canonical bump for the Position V3 PDA seed projection.
    pub stored_bump: u8,
    /// Nonzero monotone live generation.
    pub generation: u64,
    /// Full 32-byte MarketInstanceV2 identity; never a lowered legacy MarketId.
    pub market_instance_id: Identity32V1,
    /// Immutable Realm identity selecting collateral semantics.
    pub realm_id: Identity32V1,
    /// Immutable Realm collateral-policy content identity.
    pub collateral_policy_id: Identity32V1,
    /// Exact compiled collateral-adapter release content identity.
    pub collateral_release_id: Identity32V1,
    /// Semantic owner of this cash and native-Egg liability.
    pub owner: Identity32V1,
    /// Exact controller authorized by the purpose owner to mutate this body.
    pub controller: Identity32V1,
    /// Exact current-generation Replay PDA identity.
    pub replay_account: Identity32V1,
    /// Purpose-specific full-width binding identity.
    pub purpose_binding_id: Identity32V1,
    /// Total collateral-denominated cash liability in exact raw atoms.
    pub cash_atoms: u64,
    /// Subset of `cash_atoms` unavailable to an ordinary withdrawal.
    pub reserved_cash_atoms: u64,
    /// Exact native-Egg balances; inactive outcome tail entries must be zero.
    pub native_eggs: [u64; MAX_OUTCOMES],
    /// Authoritative count of outstanding reservation children.
    pub outstanding_reservations: u64,
    /// Lamport-only live/tombstone rent ownership and donation floor.
    pub rent: RentSplitV2,
}

/// Forgeable adapter projection of one authenticated Market/Realm collateral join.
///
/// A live adapter constructs this only after authenticating the full
/// MarketInstanceV2 body and immutable Realm → policy → release chain. The pure
/// codec checks equality but cannot confer that external authenticity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterPositionMarketBindingV3 {
    /// Full MarketInstanceV2 content identity.
    pub market_instance_id: Identity32V1,
    /// Exact authenticated outcome count for that Market, in `1..=16`.
    pub outcome_count: u8,
    /// Immutable Realm identity selected by the Market.
    pub realm_id: Identity32V1,
    /// Immutable Realm collateral-policy content identity.
    pub collateral_policy_id: Identity32V1,
    /// Exact compiled collateral-adapter release content identity.
    pub collateral_release_id: Identity32V1,
}

/// Forgeable adapter projection of one purpose owner's exact external join.
///
/// Dealer, Series, and structured-claim facts remain in their own semantic
/// owners and enter the Position codec only through these full-width joins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterPositionPurposeBindingV3 {
    /// Expected Position semantic owner.
    pub owner: Identity32V1,
    /// Expected Position controller.
    pub controller: Identity32V1,
    /// Expected purpose-specific binding identity.
    pub purpose_binding_id: Identity32V1,
}

/// Canonical global Position V3 body shared by every product purpose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionAccountV3 {
    fields: PositionV3Fields,
}

impl PositionAccountV3 {
    /// Validate untrusted fields and construct one canonical body.
    pub fn new(fields: PositionV3Fields) -> Result<Self, RetirementErrorV2> {
        let value = Self { fields };
        value.validate()?;
        Ok(value)
    }

    /// Validate all invariants owned by the pure Position codec.
    pub fn validate(self) -> Result<(), RetirementErrorV2> {
        if self.fields.generation == 0 {
            return Err(RetirementErrorV2::WrongGeneration);
        }
        let outcome_count = usize::from(self.fields.outcome_count);
        if outcome_count == 0 || outcome_count > MAX_OUTCOMES {
            return Err(RetirementErrorV2::NonCanonicalState);
        }
        if self.fields.reserved_cash_atoms > self.fields.cash_atoms {
            return Err(RetirementErrorV2::NonCanonicalState);
        }
        let mut index = outcome_count;
        while index < MAX_OUTCOMES {
            if self.fields.native_eggs[index] != 0 {
                return Err(RetirementErrorV2::NonCanonicalState);
            }
            index += 1;
        }
        self.fields
            .rent
            .validate()
            .map_err(retirement_error_v2_from_v1)
    }

    /// Return the exact validated fields.
    pub const fn fields(self) -> PositionV3Fields {
        self.fields
    }

    /// Return the purpose selected for the external binding.
    pub const fn purpose(self) -> PositionPurposeV3 {
        self.fields.purpose
    }

    /// Return the current lifecycle phase.
    pub const fn lifecycle(self) -> PositionLifecycleV3 {
        self.fields.lifecycle
    }

    /// Return the full MarketInstanceV2 identity.
    pub const fn market_instance_id(self) -> Identity32V1 {
        self.fields.market_instance_id
    }

    /// Return the immutable Realm identity.
    pub const fn realm_id(self) -> Identity32V1 {
        self.fields.realm_id
    }

    /// Return the immutable collateral policy identity.
    pub const fn collateral_policy_id(self) -> Identity32V1 {
        self.fields.collateral_policy_id
    }

    /// Return the exact collateral adapter release identity.
    pub const fn collateral_release_id(self) -> Identity32V1 {
        self.fields.collateral_release_id
    }

    /// Return the semantic owner.
    pub const fn owner(self) -> Identity32V1 {
        self.fields.owner
    }

    /// Return the exact controller.
    pub const fn controller(self) -> Identity32V1 {
        self.fields.controller
    }

    /// Return the exact current-generation Replay account identity.
    pub const fn replay_account(self) -> Identity32V1 {
        self.fields.replay_account
    }

    /// Return the purpose-specific binding identity.
    pub const fn purpose_binding_id(self) -> Identity32V1 {
        self.fields.purpose_binding_id
    }

    /// Return the authenticated Market outcome width.
    pub const fn outcome_count(self) -> u8 {
        self.fields.outcome_count
    }

    /// Return the nonzero generation.
    pub const fn generation(self) -> u64 {
        self.fields.generation
    }

    /// Return the canonical PDA bump.
    pub const fn stored_bump(self) -> u8 {
        self.fields.stored_bump
    }

    /// Return the total cash liability in raw collateral atoms.
    pub const fn cash_atoms(self) -> u64 {
        self.fields.cash_atoms
    }

    /// Return the reserved subset of cash liability.
    pub const fn reserved_cash_atoms(self) -> u64 {
        self.fields.reserved_cash_atoms
    }

    /// Return the exact fixed-width native-Egg vector.
    pub const fn native_eggs(self) -> [u64; MAX_OUTCOMES] {
        self.fields.native_eggs
    }

    /// Return the authoritative outstanding reservation count.
    pub const fn outstanding_reservations(self) -> u64 {
        self.fields.outstanding_reservations
    }

    /// Return the lamport-only rent ownership compartments.
    pub const fn rent(self) -> RentSplitV2 {
        self.fields.rent
    }

    /// Return the exact canonical PDA seed facts plus stored bump.
    pub const fn pda_seeds(self) -> PositionV3PdaSeeds {
        PositionV3PdaSeeds {
            market_instance_id: self.fields.market_instance_id,
            owner: self.fields.owner,
            purpose: self.fields.purpose,
            purpose_binding_id: self.fields.purpose_binding_id,
            stored_bump: self.fields.stored_bump,
        }
    }

    /// Encode exactly 480 canonical bytes.
    pub fn encode(self) -> Result<[u8; POSITION_V3_BYTES], RetirementErrorV2> {
        self.validate()?;
        let mut output = [0u8; POSITION_V3_BYTES];
        output[0] = POSITION_ACCOUNT_TAG;
        output[1] = POSITION_ACCOUNT_VERSION_V3;
        output[2] = u8::from(self.fields.purpose);
        output[3] = u8::from(self.fields.lifecycle);
        output[4] = self.fields.outcome_count;
        output[5] = self.fields.stored_bump;
        output[8..16].copy_from_slice(&self.fields.generation.to_le_bytes());

        let identities = [
            self.fields.market_instance_id,
            self.fields.realm_id,
            self.fields.collateral_policy_id,
            self.fields.collateral_release_id,
            self.fields.owner,
            self.fields.controller,
            self.fields.replay_account,
            self.fields.purpose_binding_id,
        ];
        let mut index = 0usize;
        while index < identities.len() {
            let offset = POSITION_V3_HEADER_BYTES + index * IDENTITY_BYTES;
            output[offset..offset + IDENTITY_BYTES].copy_from_slice(&identities[index].bytes());
            index += 1;
        }
        output[POSITION_V3_CASH_OFFSET..POSITION_V3_CASH_OFFSET + 8]
            .copy_from_slice(&self.fields.cash_atoms.to_le_bytes());
        output[POSITION_V3_RESERVED_CASH_OFFSET..POSITION_V3_RESERVED_CASH_OFFSET + 8]
            .copy_from_slice(&self.fields.reserved_cash_atoms.to_le_bytes());
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            let offset = POSITION_V3_EGGS_OFFSET + outcome * 8;
            output[offset..offset + 8]
                .copy_from_slice(&self.fields.native_eggs[outcome].to_le_bytes());
            outcome += 1;
        }
        output[POSITION_V3_RESERVATION_COUNT_OFFSET..POSITION_V3_RESERVATION_COUNT_OFFSET + 8]
            .copy_from_slice(&self.fields.outstanding_reservations.to_le_bytes());
        output[POSITION_V3_RENT_OFFSET..].copy_from_slice(
            &self
                .fields
                .rent
                .encode()
                .map_err(retirement_error_v2_from_v1)?,
        );
        Ok(output)
    }

    /// Decode exactly 480 hostile bytes and refuse every noncanonical field.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV2> {
        require_exact(input, POSITION_V3_BYTES)?;
        if input[0] != POSITION_ACCOUNT_TAG {
            return Err(RetirementErrorV2::WrongTag);
        }
        if input[1] != POSITION_ACCOUNT_VERSION_V3 {
            return Err(RetirementErrorV2::WrongVersion);
        }
        require_zeroes(&input[6..8])?;
        let purpose = PositionPurposeV3::decode(input[2])?;
        let lifecycle = PositionLifecycleV3::decode(input[3])?;
        let fields = PositionV3Fields {
            purpose,
            lifecycle,
            outcome_count: input[4],
            stored_bump: input[5],
            generation: read_u64(input, 8),
            market_instance_id: read_identity(input, 16)?,
            realm_id: read_identity(input, 48)?,
            collateral_policy_id: read_identity(input, 80)?,
            collateral_release_id: read_identity(input, 112)?,
            owner: read_identity(input, 144)?,
            controller: read_identity(input, 176)?,
            replay_account: read_identity(input, 208)?,
            purpose_binding_id: read_identity(input, 240)?,
            cash_atoms: read_u64(input, POSITION_V3_CASH_OFFSET),
            reserved_cash_atoms: read_u64(input, POSITION_V3_RESERVED_CASH_OFFSET),
            native_eggs: read_eggs(input),
            outstanding_reservations: read_u64(input, POSITION_V3_RESERVATION_COUNT_OFFSET),
            rent: RentSplitV2::decode(&input[POSITION_V3_RENT_OFFSET..])
                .map_err(retirement_error_v2_from_v1)?,
        };
        Self::new(fields)
    }

    /// Compute the canonical semantic identity with an injected SHA-256 backend.
    pub fn semantic_id<B: PositionV3Sha256Backend>(
        self,
        backend: &B,
    ) -> Result<Identity32V1, RetirementErrorV2> {
        let body = self.encode()?;
        Identity32V1::new(backend.sha256(POSITION_V3_SEMANTIC_DOMAIN, &body))
            .map_err(retirement_error_v2_from_v1)
    }

    /// Mint a terminal economic projection only from a fully empty close request.
    pub fn terminal_projection(self) -> Result<PositionTerminalProjectionV3, RetirementErrorV2> {
        self.validate()?;
        if self.fields.lifecycle != PositionLifecycleV3::CloseRequested {
            return Err(RetirementErrorV2::WrongPhase);
        }
        if self.fields.cash_atoms != 0
            || self.fields.reserved_cash_atoms != 0
            || self.fields.native_eggs != [0; MAX_OUTCOMES]
        {
            return Err(RetirementErrorV2::EconomicBalanceOutstanding);
        }
        if self.fields.outstanding_reservations != 0 {
            return Err(RetirementErrorV2::ReservationOutstanding);
        }
        Ok(PositionTerminalProjectionV3 { position: self })
    }
}

/// Exact canonical seed projection; runtime code still derives and checks the PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionV3PdaSeeds {
    market_instance_id: Identity32V1,
    owner: Identity32V1,
    purpose: PositionPurposeV3,
    purpose_binding_id: Identity32V1,
    stored_bump: u8,
}

impl PositionV3PdaSeeds {
    /// Full MarketInstance seed.
    pub const fn market_instance_id(self) -> Identity32V1 {
        self.market_instance_id
    }

    /// Semantic-owner seed.
    pub const fn owner(self) -> Identity32V1 {
        self.owner
    }

    /// One-byte purpose seed.
    pub const fn purpose(self) -> PositionPurposeV3 {
        self.purpose
    }

    /// Purpose-binding seed.
    pub const fn purpose_binding_id(self) -> Identity32V1 {
        self.purpose_binding_id
    }

    /// Stored canonical bump.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }
}

/// Opaque proof that the pure Position body is economically terminal.
///
/// This is not runtime account authentication. The close adapter must also
/// authenticate owner, PDA, Replay, account balance, neutral sink, and atomic
/// lamport writes before consuming it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionTerminalProjectionV3 {
    position: PositionAccountV3,
}

impl PositionTerminalProjectionV3 {
    /// Return the exact terminal Position body.
    pub const fn position(self) -> PositionAccountV3 {
        self.position
    }

    /// Construct the full-identity permanent tombstone.
    pub fn tombstone(self) -> Result<PositionTombstoneV3, RetirementErrorV2> {
        PositionTombstoneV3::new(PositionTombstoneV3Fields {
            purpose: self.position.fields.purpose,
            stored_bump: self.position.fields.stored_bump,
            generation: self.position.fields.generation,
            market_instance_id: self.position.fields.market_instance_id,
            realm_id: self.position.fields.realm_id,
            collateral_policy_id: self.position.fields.collateral_policy_id,
            collateral_release_id: self.position.fields.collateral_release_id,
            owner: self.position.fields.owner,
            controller: self.position.fields.controller,
            replay_account: self.position.fields.replay_account,
            purpose_binding_id: self.position.fields.purpose_binding_id,
            permanent_tombstone_principal: self.position.fields.rent.permanent_tombstone_principal,
        })
    }
}

/// Exact retained fields for the permanent full-identity tombstone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionTombstoneV3Fields {
    /// Purpose retained to prevent cross-purpose reopen.
    pub purpose: PositionPurposeV3,
    /// Canonical Position PDA bump.
    pub stored_bump: u8,
    /// Exact closed generation.
    pub generation: u64,
    /// Full MarketInstanceV2 identity.
    pub market_instance_id: Identity32V1,
    /// Immutable Realm identity.
    pub realm_id: Identity32V1,
    /// Immutable Realm collateral policy identity.
    pub collateral_policy_id: Identity32V1,
    /// Exact collateral adapter release identity.
    pub collateral_release_id: Identity32V1,
    /// Semantic owner identity.
    pub owner: Identity32V1,
    /// Exact controller at close.
    pub controller: Identity32V1,
    /// Exact deleted Replay PDA whose absence must be authenticated.
    pub replay_account: Identity32V1,
    /// Exact purpose-specific binding identity.
    pub purpose_binding_id: Identity32V1,
    /// Lamport-only principal permanently retained in this tombstone.
    pub permanent_tombstone_principal: u64,
}

/// Permanent full-width identity occupying the canonical Position V3 PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionTombstoneV3 {
    fields: PositionTombstoneV3Fields,
}

impl PositionTombstoneV3 {
    /// Validate and construct one canonical tombstone.
    pub fn new(fields: PositionTombstoneV3Fields) -> Result<Self, RetirementErrorV2> {
        let value = Self { fields };
        value.validate()?;
        Ok(value)
    }

    /// Validate the nonzero generation and retained lamport principal.
    pub const fn validate(self) -> Result<(), RetirementErrorV2> {
        if self.fields.generation == 0 {
            return Err(RetirementErrorV2::WrongGeneration);
        }
        if self.fields.permanent_tombstone_principal == 0 {
            return Err(RetirementErrorV2::NonCanonicalState);
        }
        Ok(())
    }

    /// Return exact retained fields.
    pub const fn fields(self) -> PositionTombstoneV3Fields {
        self.fields
    }

    /// Return the deleted Replay identity that must be proven absent.
    pub const fn replay_account(self) -> Identity32V1 {
        self.fields.replay_account
    }

    /// Return the exact Position PDA seed facts retained across reopen.
    pub const fn pda_seeds(self) -> PositionV3PdaSeeds {
        PositionV3PdaSeeds {
            market_instance_id: self.fields.market_instance_id,
            owner: self.fields.owner,
            purpose: self.fields.purpose,
            purpose_binding_id: self.fields.purpose_binding_id,
            stored_bump: self.fields.stored_bump,
        }
    }

    /// Encode exactly 280 canonical bytes.
    pub fn encode(self) -> Result<[u8; POSITION_TOMBSTONE_V3_BYTES], RetirementErrorV2> {
        self.validate()?;
        let mut output = [0u8; POSITION_TOMBSTONE_V3_BYTES];
        output[0] = POSITION_TOMBSTONE_TAG;
        output[1] = POSITION_TOMBSTONE_VERSION_V3;
        output[2] = u8::from(self.fields.purpose);
        output[3] = u8::from(PositionTombstoneLifecycleV3::Closed);
        output[4] = self.fields.stored_bump;
        output[8..16].copy_from_slice(&self.fields.generation.to_le_bytes());
        let identities = [
            self.fields.market_instance_id,
            self.fields.realm_id,
            self.fields.collateral_policy_id,
            self.fields.collateral_release_id,
            self.fields.owner,
            self.fields.controller,
            self.fields.replay_account,
            self.fields.purpose_binding_id,
        ];
        let mut index = 0usize;
        while index < identities.len() {
            let offset = POSITION_TOMBSTONE_V3_HEADER_BYTES + index * IDENTITY_BYTES;
            output[offset..offset + IDENTITY_BYTES].copy_from_slice(&identities[index].bytes());
            index += 1;
        }
        output[POSITION_TOMBSTONE_V3_PRINCIPAL_OFFSET..]
            .copy_from_slice(&self.fields.permanent_tombstone_principal.to_le_bytes());
        Ok(output)
    }

    /// Decode exactly 280 hostile bytes and refuse noncanonical reserved data.
    pub fn decode(input: &[u8]) -> Result<Self, RetirementErrorV2> {
        require_exact(input, POSITION_TOMBSTONE_V3_BYTES)?;
        if input[0] != POSITION_TOMBSTONE_TAG {
            return Err(RetirementErrorV2::WrongTag);
        }
        if input[1] != POSITION_TOMBSTONE_VERSION_V3 {
            return Err(RetirementErrorV2::WrongVersion);
        }
        PositionTombstoneLifecycleV3::decode(input[3])?;
        require_zeroes(&input[5..8])?;
        Self::new(PositionTombstoneV3Fields {
            purpose: PositionPurposeV3::decode(input[2])?,
            stored_bump: input[4],
            generation: read_u64(input, 8),
            market_instance_id: read_identity(input, 16)?,
            realm_id: read_identity(input, 48)?,
            collateral_policy_id: read_identity(input, 80)?,
            collateral_release_id: read_identity(input, 112)?,
            owner: read_identity(input, 144)?,
            controller: read_identity(input, 176)?,
            replay_account: read_identity(input, 208)?,
            purpose_binding_id: read_identity(input, 240)?,
            permanent_tombstone_principal: read_u64(input, POSITION_TOMBSTONE_V3_PRINCIPAL_OFFSET),
        })
    }

    /// Compute the canonical tombstone identity with an injected SHA backend.
    pub fn semantic_id<B: PositionV3Sha256Backend>(
        self,
        backend: &B,
    ) -> Result<Identity32V1, RetirementErrorV2> {
        let body = self.encode()?;
        Identity32V1::new(backend.sha256(POSITION_TOMBSTONE_V3_SEMANTIC_DOMAIN, &body))
            .map_err(retirement_error_v2_from_v1)
    }
}

/// Canonical general-purpose projection of the global Position body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralPositionProjectionV3(PositionAccountV3);

impl GeneralPositionProjectionV3 {
    /// Return the canonical global body; this does not add runtime authority.
    pub const fn position(self) -> PositionAccountV3 {
        self.0
    }
}

/// Canonical Dealer-facility projection of the global Position body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPositionProjectionV3(PositionAccountV3);

impl DealerPositionProjectionV3 {
    /// Return the canonical global body; Dealer facts remain in its binding owner.
    pub const fn position(self) -> PositionAccountV3 {
        self.0
    }
}

/// Canonical Series projection of the global Position body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesPositionProjectionV3(PositionAccountV3);

impl SeriesPositionProjectionV3 {
    /// Return the canonical global body; Series facts remain in its binding owner.
    pub const fn position(self) -> PositionAccountV3 {
        self.0
    }
}

/// Canonical structured-claim projection of the global Position body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredClaimPositionProjectionV3(PositionAccountV3);

impl StructuredClaimPositionProjectionV3 {
    /// Return the canonical global body; claim facts remain in its binding owner.
    pub const fn position(self) -> PositionAccountV3 {
        self.0
    }
}

/// Project a validated body for ordinary user, LP, or treasury joins.
pub fn project_general_position_v3(
    position: PositionAccountV3,
    market: AdapterPositionMarketBindingV3,
    binding: AdapterPositionPurposeBindingV3,
) -> Result<GeneralPositionProjectionV3, RetirementErrorV2> {
    require_projection(position, PositionPurposeV3::General, market, binding)?;
    Ok(GeneralPositionProjectionV3(position))
}

/// Project a validated body for a Dealer-owned facility binding.
pub fn project_dealer_position_v3(
    position: PositionAccountV3,
    market: AdapterPositionMarketBindingV3,
    binding: AdapterPositionPurposeBindingV3,
) -> Result<DealerPositionProjectionV3, RetirementErrorV2> {
    require_projection(position, PositionPurposeV3::DealerFacility, market, binding)?;
    Ok(DealerPositionProjectionV3(position))
}

/// Project a validated body for a Series-owned binding.
pub fn project_series_position_v3(
    position: PositionAccountV3,
    market: AdapterPositionMarketBindingV3,
    binding: AdapterPositionPurposeBindingV3,
) -> Result<SeriesPositionProjectionV3, RetirementErrorV2> {
    require_projection(position, PositionPurposeV3::Series, market, binding)?;
    Ok(SeriesPositionProjectionV3(position))
}

/// Project a validated body for a structured-claim binding.
pub fn project_structured_claim_position_v3(
    position: PositionAccountV3,
    market: AdapterPositionMarketBindingV3,
    binding: AdapterPositionPurposeBindingV3,
) -> Result<StructuredClaimPositionProjectionV3, RetirementErrorV2> {
    require_projection(
        position,
        PositionPurposeV3::StructuredClaim,
        market,
        binding,
    )?;
    Ok(StructuredClaimPositionProjectionV3(position))
}

fn require_projection(
    position: PositionAccountV3,
    expected: PositionPurposeV3,
    market: AdapterPositionMarketBindingV3,
    binding: AdapterPositionPurposeBindingV3,
) -> Result<(), RetirementErrorV2> {
    position.validate()?;
    if position.purpose() != expected
        || position.market_instance_id() != market.market_instance_id
        || position.outcome_count() != market.outcome_count
        || position.realm_id() != market.realm_id
        || position.collateral_policy_id() != market.collateral_policy_id
        || position.collateral_release_id() != market.collateral_release_id
        || position.owner() != binding.owner
        || position.controller() != binding.controller
        || position.purpose_binding_id() != binding.purpose_binding_id
    {
        Err(RetirementErrorV2::WrongParent)
    } else {
        Ok(())
    }
}

fn require_exact(input: &[u8], expected: usize) -> Result<(), RetirementErrorV2> {
    if input.len() < expected {
        Err(RetirementErrorV2::Truncated)
    } else if input.len() > expected {
        Err(RetirementErrorV2::TrailingBytes)
    } else {
        Ok(())
    }
}

fn require_zeroes(input: &[u8]) -> Result<(), RetirementErrorV2> {
    let mut index = 0usize;
    while index < input.len() {
        if input[index] != 0 {
            return Err(RetirementErrorV2::NonCanonicalState);
        }
        index += 1;
    }
    Ok(())
}

fn read_u64(input: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&input[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

fn read_identity(input: &[u8], offset: usize) -> Result<Identity32V1, RetirementErrorV2> {
    let mut bytes = [0u8; IDENTITY_BYTES];
    bytes.copy_from_slice(&input[offset..offset + IDENTITY_BYTES]);
    Identity32V1::new(bytes).map_err(retirement_error_v2_from_v1)
}

fn read_eggs(input: &[u8]) -> [u64; MAX_OUTCOMES] {
    let mut eggs = [0u64; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        eggs[index] = read_u64(input, POSITION_V3_EGGS_OFFSET + index * 8);
        index += 1;
    }
    eggs
}
