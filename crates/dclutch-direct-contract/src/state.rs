use dclutch_realm_contract::PositionV1;
use dclutch_rent_contract::{RefundAuthority, RentCreditV1, SourceCloseCreditPlanV1};

use crate::{
    DirectPositionV2, Error, FEE_BASIS_POINTS_DENOMINATOR, PRICE_SCALE, Result, adapter, array,
    fee, nonzero, one, position_error, put, width, zeros,
};

/// Canonical signed-intent preimage magic.
pub const DIRECT_INTENT_MAGIC_V2: [u8; 8] = *b"DCLTDIR2";
/// Signed-intent schema version.
pub const DIRECT_INTENT_SCHEMA_VERSION_V2: u16 = 2;
/// Exact signed-intent preimage width.
pub const DIRECT_INTENT_BYTES_V2: usize = 232;
/// Canonical maker replay-root magic.
pub const MAKER_REPLAY_ROOT_MAGIC_V2: [u8; 8] = *b"DCLTRPY2";
/// Maker replay-root schema version.
pub const MAKER_REPLAY_ROOT_SCHEMA_VERSION_V2: u16 = 2;
/// Exact maker replay-root width.
pub const MAKER_REPLAY_ROOT_BYTES_V2: usize = 144;
/// Canonical live intent-record magic.
pub const DIRECT_INTENT_RECORD_MAGIC_V2: [u8; 8] = *b"DCLTREC2";
/// Live intent-record schema version.
pub const DIRECT_INTENT_RECORD_SCHEMA_VERSION_V2: u16 = 2;
/// Exact live intent-record width.
pub const DIRECT_INTENT_RECORD_BYTES_V2: usize = 320;
/// Canonical cancellation-message magic.
pub const DIRECT_CANCEL_MAGIC_V2: [u8; 8] = *b"DCLTCAN2";
/// Cancellation-message schema version.
pub const DIRECT_CANCEL_SCHEMA_VERSION_V2: u16 = 2;
/// Exact signed cancellation-message width.
pub const DIRECT_CANCEL_BYTES_V2: usize = 96;
/// Canonical O(1) cancel-through message magic.
pub const DIRECT_CANCEL_THROUGH_MAGIC_V1: [u8; 8] = *b"DCLTCTH1";
/// Cancel-through message schema version.
pub const DIRECT_CANCEL_THROUGH_SCHEMA_VERSION_V1: u16 = 1;
/// Exact cancel-through signed-message width.
pub const DIRECT_CANCEL_THROUGH_BYTES_V1: usize = 96;
/// Canonical market-independent venue-fee-policy magic.
pub const VENUE_FEE_POLICY_MAGIC_V3: [u8; 8] = *b"DCLTFEE3";
/// Venue-fee-policy schema version.
pub const VENUE_FEE_POLICY_SCHEMA_VERSION_V3: u16 = 3;
/// Exact venue-fee-policy width.
pub const VENUE_FEE_POLICY_BYTES_V3: usize = 48;
/// Immutable-record schema/release identity for one Direct V3 venue policy.
///
/// This is SHA-256 of `dclutch/schema/direct-venue-fee-policy-v3`. The SBF
/// adapter derives the raw-record PDA from this identity and the SHA-256 digest
/// of the exact 48 policy bytes. The policy deliberately excludes the Market
/// address: the authenticated Market manifest selects this digest, while the
/// signed intent independently binds the Market and generation. Including the
/// downstream Market PDA here would create an unconstructible hash cycle.
pub const VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3: [u8; 32] = [
    0x28, 0x1d, 0x89, 0x6e, 0xc0, 0xce, 0x69, 0xb5, 0x24, 0x43, 0x42, 0x08, 0x20, 0xbc, 0x58, 0x0e,
    0xf1, 0x8e, 0xf2, 0x97, 0xe1, 0x39, 0x11, 0x5d, 0xf9, 0x1c, 0xea, 0x91, 0x56, 0x5c, 0x45, 0x1d,
];
/// Domain preceding Market, generation, and maker in replay-root PDA seeds.
pub const MAKER_REPLAY_ROOT_PDA_DOMAIN_V2: &[u8] = b"dclutch/direct-replay/v2";
/// Domain preceding Market, generation, maker, and nonce in live-record PDA seeds.
pub const DIRECT_INTENT_RECORD_PDA_DOMAIN_V2: &[u8] = b"dclutch/direct-intent/v2";
/// Domain preceding live-record address in collateral-escrow PDA seeds.
pub const DIRECT_INTENT_ESCROW_PDA_DOMAIN_V2: &[u8] = b"dclutch/direct-escrow/v2";

const SIDE_OFFSET: usize = 10;
const OUTCOME_OFFSET: usize = 11;
const LIFECYCLE_OFFSET: usize = 12;
const INTENT_RESERVED_OFFSET: usize = 13;
const MARKET_OFFSET: usize = 16;
const GENERATION_OFFSET: usize = 48;
const MAKER_OFFSET: usize = 56;
const NONCE_OFFSET: usize = 88;
const START_OFFSET: usize = 96;
const END_OFFSET: usize = 104;
const MAX_FILL_OFFSET: usize = 112;
const LIMIT_OFFSET: usize = 120;
const FEE_CONFIG_OFFSET: usize = 128;
const INTENT_FEE_BPS_OFFSET: usize = 160;
const INTENT_FEE_RESERVED_OFFSET: usize = 162;
const POSITION_ACCOUNT_OFFSET: usize = 168;
const COLLATERAL_ACCOUNT_OFFSET: usize = 200;

const ROOT_STATUS_OFFSET: usize = 10;
const ROOT_BUMP_OFFSET: usize = 11;
const ROOT_RESERVED_OFFSET: usize = 12;
const ROOT_NEXT_NONCE_OFFSET: usize = 88;
const ROOT_LIVE_COUNT_OFFSET: usize = 96;
const ROOT_MINIMUM_LIVE_NONCE_OFFSET: usize = 104;
const ROOT_RENT_PAYER_OFFSET: usize = 112;

const RECORD_STATUS_OFFSET: usize = 10;
const RECORD_BUMP_OFFSET: usize = 11;
const RECORD_RESERVED_OFFSET: usize = 12;
const RECORD_INTENT_OFFSET: usize = 16;
const RECORD_FILLED_OFFSET: usize = 248;
const RECORD_CLAIMS_OFFSET: usize = 256;
const RECORD_COLLATERAL_OFFSET: usize = 264;
const RECORD_FEE_GROSS_OFFSET: usize = 272;
const RECORD_CUMULATIVE_FEE_OFFSET: usize = 280;
const RECORD_RENT_PAYER_OFFSET: usize = 288;

const FEE_BPS_OFFSET: usize = 10;
const FEE_RESERVED_OFFSET: usize = 12;
const FEE_RECIPIENT_OFFSET: usize = 16;

/// Signed direction of a Direct intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Side {
    /// Reserve collateral to buy one selected outcome.
    Buy = 0,
    /// Reserve one selected outcome claim to sell it.
    Sell = 1,
}

impl Side {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Buy),
            1 => Ok(Self::Sell),
            _ => Err(Error::UnknownSide),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Buy => 0,
            Self::Sell => 1,
        }
    }
}

/// Maker-selected state lifecycle for one signed intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum IntentLifecycleV2 {
    /// Immediate fill must consume full signed capacity or all effects refuse.
    InlineFillOrKill = 0,
    /// Immediate fill may consume positive capacity and discards remainder.
    InlineImmediateOrCancel = 1,
    /// Registration creates live, partially fillable, cancellable custody.
    Registered = 2,
}

impl IntentLifecycleV2 {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::InlineFillOrKill),
            1 => Ok(Self::InlineImmediateOrCancel),
            2 => Ok(Self::Registered),
            _ => Err(Error::UnknownIntentLifecycle),
        }
    }
    const fn byte(self) -> u8 {
        match self {
            Self::InlineFillOrKill => 0,
            Self::InlineImmediateOrCancel => 1,
            Self::Registered => 2,
        }
    }
}

/// All maker-signed facts for one persisted Direct intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectIntentInputV2 {
    /// Exact nonzero Market identity.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact nonzero maker Ed25519 public key.
    pub maker: [u8; 32],
    /// Exact gap-free maker replay nonce.
    pub nonce: u64,
    /// Inclusive starting slot.
    pub valid_from_slot: u64,
    /// Inclusive expiry slot.
    pub valid_through_slot: u64,
    /// Buy or sell reservation.
    pub side: Side,
    /// Immediate FOK/IOC or registered resting lifecycle.
    pub lifecycle: IntentLifecycleV2,
    /// Selected canonical outcome.
    pub outcome: u8,
    /// Aggregate capacity across all partial fills.
    pub max_fill: u64,
    /// Price in [`PRICE_SCALE`] units per claim atom.
    pub limit_price: u64,
    /// Immutable Market-selected fee configuration/release identity.
    pub fee_config: [u8; 32],
    /// Exact fee rate accepted by maker.
    pub fee_basis_points: u16,
    /// Exact native Position account to reserve from or credit.
    pub position_account: [u8; 32],
    /// Exact token account to debit on registration or credit on sale/close.
    pub collateral_account: [u8; 32],
}

/// Canonical 232-byte semantic owner of maker authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectIntentV2 {
    market: [u8; 32],
    generation: u64,
    maker: [u8; 32],
    nonce: u64,
    from: u64,
    through: u64,
    side: Side,
    lifecycle: IntentLifecycleV2,
    outcome: u8,
    max_fill: u64,
    limit: u64,
    fee_config: [u8; 32],
    fee_bps: u16,
    position_account: [u8; 32],
    collateral_account: [u8; 32],
}

impl DirectIntentV2 {
    /// Validate one exact semantic intent.
    pub fn new(input: DirectIntentInputV2) -> Result<Self> {
        nonzero(&input.market)?;
        nonzero(&input.maker)?;
        nonzero(&input.fee_config)?;
        nonzero(&input.position_account)?;
        nonzero(&input.collateral_account)?;
        if input.valid_from_slot > input.valid_through_slot {
            return Err(Error::InvalidSlotInterval);
        }
        if input.max_fill == 0 {
            return Err(Error::ZeroQuantity);
        }
        if input.limit_price > PRICE_SCALE {
            return Err(Error::InvalidLimitPrice);
        }
        if u64::from(input.fee_basis_points) > FEE_BASIS_POINTS_DENOMINATOR {
            return Err(Error::InvalidFeeRate);
        }
        Ok(Self {
            market: input.market,
            generation: input.generation,
            maker: input.maker,
            nonce: input.nonce,
            from: input.valid_from_slot,
            through: input.valid_through_slot,
            side: input.side,
            lifecycle: input.lifecycle,
            outcome: input.outcome,
            max_fill: input.max_fill,
            limit: input.limit_price,
            fee_config: input.fee_config,
            fee_bps: input.fee_basis_points,
            position_account: input.position_account,
            collateral_account: input.collateral_account,
        })
    }

    /// Decode the one canonical sequence accepted by registration.
    pub fn decode_signed_preimage(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != DIRECT_INTENT_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != DIRECT_INTENT_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != DIRECT_INTENT_SCHEMA_VERSION_V2 {
            return Err(Error::UnsupportedSchema);
        }
        zeros(bytes, INTENT_RESERVED_OFFSET, 3)?;
        zeros(bytes, INTENT_FEE_RESERVED_OFFSET, 6)?;
        Self::new(DirectIntentInputV2 {
            market: array(bytes, MARKET_OFFSET)?,
            generation: u64::from_le_bytes(array(bytes, GENERATION_OFFSET)?),
            maker: array(bytes, MAKER_OFFSET)?,
            nonce: u64::from_le_bytes(array(bytes, NONCE_OFFSET)?),
            valid_from_slot: u64::from_le_bytes(array(bytes, START_OFFSET)?),
            valid_through_slot: u64::from_le_bytes(array(bytes, END_OFFSET)?),
            side: Side::decode(one(bytes, SIDE_OFFSET)?)?,
            lifecycle: IntentLifecycleV2::decode(one(bytes, LIFECYCLE_OFFSET)?)?,
            outcome: one(bytes, OUTCOME_OFFSET)?,
            max_fill: u64::from_le_bytes(array(bytes, MAX_FILL_OFFSET)?),
            limit_price: u64::from_le_bytes(array(bytes, LIMIT_OFFSET)?),
            fee_config: array(bytes, FEE_CONFIG_OFFSET)?,
            fee_basis_points: u16::from_le_bytes(array(bytes, INTENT_FEE_BPS_OFFSET)?),
            position_account: array(bytes, POSITION_ACCOUNT_OFFSET)?,
            collateral_account: array(bytes, COLLATERAL_ACCOUNT_OFFSET)?,
        })
    }

    /// Return exact canonical maker-signed bytes.
    pub fn signed_preimage(self) -> [u8; DIRECT_INTENT_BYTES_V2] {
        let mut output = [0; DIRECT_INTENT_BYTES_V2];
        put(&mut output, 0, &DIRECT_INTENT_MAGIC_V2);
        put(
            &mut output,
            8,
            &DIRECT_INTENT_SCHEMA_VERSION_V2.to_le_bytes(),
        );
        output[SIDE_OFFSET] = self.side.byte();
        output[OUTCOME_OFFSET] = self.outcome;
        output[LIFECYCLE_OFFSET] = self.lifecycle.byte();
        put(&mut output, MARKET_OFFSET, &self.market);
        put(
            &mut output,
            GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(&mut output, MAKER_OFFSET, &self.maker);
        put(&mut output, NONCE_OFFSET, &self.nonce.to_le_bytes());
        put(&mut output, START_OFFSET, &self.from.to_le_bytes());
        put(&mut output, END_OFFSET, &self.through.to_le_bytes());
        put(&mut output, MAX_FILL_OFFSET, &self.max_fill.to_le_bytes());
        put(&mut output, LIMIT_OFFSET, &self.limit.to_le_bytes());
        put(&mut output, FEE_CONFIG_OFFSET, &self.fee_config);
        put(
            &mut output,
            INTENT_FEE_BPS_OFFSET,
            &self.fee_bps.to_le_bytes(),
        );
        put(&mut output, POSITION_ACCOUNT_OFFSET, &self.position_account);
        put(
            &mut output,
            COLLATERAL_ACCOUNT_OFFSET,
            &self.collateral_account,
        );
        output
    }

    /// Return Market identity.
    pub const fn market(&self) -> &[u8; 32] {
        &self.market
    }
    /// Return Market generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Return maker public key.
    pub const fn maker(&self) -> &[u8; 32] {
        &self.maker
    }
    /// Return maker replay nonce.
    pub const fn nonce(&self) -> u64 {
        self.nonce
    }
    /// Return side.
    pub const fn side(&self) -> Side {
        self.side
    }
    /// Return maker-selected state lifecycle.
    pub const fn lifecycle(&self) -> IntentLifecycleV2 {
        self.lifecycle
    }
    /// Return outcome.
    pub const fn outcome(&self) -> u8 {
        self.outcome
    }
    /// Return aggregate capacity.
    pub const fn max_fill(&self) -> u64 {
        self.max_fill
    }
    /// Return inclusive validity start.
    pub const fn valid_from_slot(&self) -> u64 {
        self.from
    }
    /// Return inclusive validity end.
    pub const fn valid_through_slot(&self) -> u64 {
        self.through
    }
    /// Return signed limit price.
    pub const fn limit_price(&self) -> u64 {
        self.limit
    }
    /// Return fee config identity.
    pub const fn fee_config(&self) -> &[u8; 32] {
        &self.fee_config
    }
    /// Return fee basis points.
    pub const fn fee_basis_points(&self) -> u16 {
        self.fee_bps
    }
    /// Return exact signed Position account.
    pub const fn position_account(&self) -> &[u8; 32] {
        &self.position_account
    }
    /// Return exact signed collateral account.
    pub const fn collateral_account(&self) -> &[u8; 32] {
        &self.collateral_account
    }
}

/// Whether a maker replay root still admits new signed registrations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReplayRegistrationStatusV2 {
    /// Market remains able to register exactly the next nonce.
    Open = 0,
    /// Market retirement irreversibly closed registration.
    Closed = 1,
}

impl ReplayRegistrationStatusV2 {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Open),
            1 => Ok(Self::Closed),
            _ => Err(Error::UnknownIntentStatus),
        }
    }
    const fn byte(self) -> u8 {
        match self {
            Self::Open => 0,
            Self::Closed => 1,
        }
    }
}

/// Hostile account-state branch for the one canonical maker replay root.
/// `Absent` is accepted only for the first gap-free nonce and causes atomic
/// creation using the current instruction's separate System payer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayRootStateV2 {
    /// Existing program-owned root decoded from the canonical PDA.
    Existing(MakerReplayRootV2),
    /// Canonical PDA is absent and must be created at this bump.
    Absent {
        /// Canonical PDA bump checked by the SBF adapter.
        bump: u8,
    },
}

impl ReplayRootStateV2 {
    /// Wrap an existing canonical replay root.
    pub const fn existing(root: MakerReplayRootV2) -> Self {
        Self::Existing(root)
    }

    /// Describe a first-use canonical replay-root creation.
    pub const fn absent(bump: u8) -> Self {
        Self::Absent { bump }
    }

    pub(crate) fn open_for_intent(
        self,
        intent: DirectIntentV2,
        creation_payer: [u8; 32],
    ) -> Result<MakerReplayRootV2> {
        nonzero(&creation_payer)?;
        match self {
            Self::Existing(root) => Ok(root),
            Self::Absent { bump } => MakerReplayRootV2::new(
                *intent.market(),
                intent.generation(),
                *intent.maker(),
                creation_payer,
                bump,
            ),
        }
    }
}

/// One compact replay high-water mark per `(Market, generation, maker)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MakerReplayRootV2 {
    market: [u8; 32],
    generation: u64,
    maker: [u8; 32],
    next_nonce: u64,
    live_count: u64,
    minimum_live_nonce: u64,
    rent_payer: [u8; 32],
    bump: u8,
    registration: ReplayRegistrationStatusV2,
}

impl MakerReplayRootV2 {
    /// Construct a new gap-free root before the maker's first Direct nonce.
    pub fn new(
        market: [u8; 32],
        generation: u64,
        maker: [u8; 32],
        rent_payer: [u8; 32],
        bump: u8,
    ) -> Result<Self> {
        nonzero(&market)?;
        nonzero(&maker)?;
        nonzero(&rent_payer)?;
        Ok(Self {
            market,
            generation,
            maker,
            next_nonce: 0,
            live_count: 0,
            minimum_live_nonce: 0,
            rent_payer,
            bump,
            registration: ReplayRegistrationStatusV2::Open,
        })
    }

    /// Decode one exact replay root.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != MAKER_REPLAY_ROOT_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != MAKER_REPLAY_ROOT_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != MAKER_REPLAY_ROOT_SCHEMA_VERSION_V2 {
            return Err(Error::UnsupportedSchema);
        }
        zeros(bytes, ROOT_RESERVED_OFFSET, 4)?;
        let root = Self {
            market: array(bytes, MARKET_OFFSET)?,
            generation: u64::from_le_bytes(array(bytes, GENERATION_OFFSET)?),
            maker: array(bytes, MAKER_OFFSET)?,
            next_nonce: u64::from_le_bytes(array(bytes, ROOT_NEXT_NONCE_OFFSET)?),
            live_count: u64::from_le_bytes(array(bytes, ROOT_LIVE_COUNT_OFFSET)?),
            minimum_live_nonce: u64::from_le_bytes(array(bytes, ROOT_MINIMUM_LIVE_NONCE_OFFSET)?),
            rent_payer: array(bytes, ROOT_RENT_PAYER_OFFSET)?,
            bump: one(bytes, ROOT_BUMP_OFFSET)?,
            registration: ReplayRegistrationStatusV2::decode(one(bytes, ROOT_STATUS_OFFSET)?)?,
        };
        root.validate()?;
        Ok(root)
    }

    /// Encode one exact replay root.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        if output.len() != MAKER_REPLAY_ROOT_BYTES_V2 {
            return Err(Error::OutputLength);
        }
        self.validate()?;
        output.fill(0);
        put(output, 0, &MAKER_REPLAY_ROOT_MAGIC_V2);
        put(
            output,
            8,
            &MAKER_REPLAY_ROOT_SCHEMA_VERSION_V2.to_le_bytes(),
        );
        *output
            .get_mut(ROOT_STATUS_OFFSET)
            .ok_or(Error::OutputLength)? = self.registration.byte();
        *output
            .get_mut(ROOT_BUMP_OFFSET)
            .ok_or(Error::OutputLength)? = self.bump;
        put(output, MARKET_OFFSET, &self.market);
        put(output, GENERATION_OFFSET, &self.generation.to_le_bytes());
        put(output, MAKER_OFFSET, &self.maker);
        put(
            output,
            ROOT_NEXT_NONCE_OFFSET,
            &self.next_nonce.to_le_bytes(),
        );
        put(
            output,
            ROOT_LIVE_COUNT_OFFSET,
            &self.live_count.to_le_bytes(),
        );
        put(
            output,
            ROOT_MINIMUM_LIVE_NONCE_OFFSET,
            &self.minimum_live_nonce.to_le_bytes(),
        );
        put(output, ROOT_RENT_PAYER_OFFSET, &self.rent_payer);
        Ok(())
    }

    /// Return Market identity.
    pub const fn market(&self) -> &[u8; 32] {
        &self.market
    }
    /// Return Market generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Return maker public key.
    pub const fn maker(&self) -> &[u8; 32] {
        &self.maker
    }
    /// Return exact next accepted nonce.
    pub const fn next_registration_nonce(&self) -> u64 {
        self.next_nonce
    }
    /// Return number of currently live intent accounts.
    pub const fn live_intent_count(&self) -> u64 {
        self.live_count
    }
    /// Return the first nonce still eligible for matching.
    pub const fn minimum_live_nonce(&self) -> u64 {
        self.minimum_live_nonce
    }
    /// Return original root-rent payer.
    pub const fn rent_payer(&self) -> &[u8; 32] {
        &self.rent_payer
    }
    /// Return stored PDA bump.
    pub const fn bump(&self) -> u8 {
        self.bump
    }
    /// Return registration status.
    pub const fn registration_status(&self) -> ReplayRegistrationStatusV2 {
        self.registration
    }

    fn validate(self) -> Result<()> {
        nonzero(&self.market)?;
        nonzero(&self.maker)?;
        nonzero(&self.rent_payer)?;
        if self.live_count > self.next_nonce {
            return Err(Error::LiveCountInvariant);
        }
        if self.minimum_live_nonce > self.next_nonce {
            return Err(Error::InvalidCancelThrough);
        }
        Ok(())
    }

    fn for_intent(self, intent: DirectIntentV2) -> Result<()> {
        self.validate()?;
        if self.market != *intent.market()
            || self.generation != intent.generation()
            || self.maker != *intent.maker()
        {
            return Err(Error::ReplayRootMismatch);
        }
        Ok(())
    }

    fn for_active_intent(self, intent: DirectIntentV2) -> Result<()> {
        self.for_intent(intent)?;
        if intent.nonce() < self.minimum_live_nonce {
            return Err(Error::IntentInvalidated);
        }
        Ok(())
    }

    fn register(self, intent: DirectIntentV2) -> Result<Self> {
        if intent.lifecycle != IntentLifecycleV2::Registered {
            return Err(Error::IntentLifecycleMismatch);
        }
        self.advance_nonce(intent, true)
    }

    pub(crate) fn consume_inline(self, intent: DirectIntentV2, fill: u64) -> Result<Self> {
        match intent.lifecycle {
            IntentLifecycleV2::InlineFillOrKill if fill != intent.max_fill => {
                return Err(Error::InvalidFill);
            }
            IntentLifecycleV2::InlineFillOrKill | IntentLifecycleV2::InlineImmediateOrCancel => {}
            IntentLifecycleV2::Registered => return Err(Error::IntentLifecycleMismatch),
        }
        if fill == 0 || fill > intent.max_fill {
            return Err(Error::InvalidFill);
        }
        self.advance_nonce(intent, false)
    }

    fn advance_nonce(self, intent: DirectIntentV2, create_live: bool) -> Result<Self> {
        self.for_intent(intent)?;
        if self.registration != ReplayRegistrationStatusV2::Open {
            return Err(Error::RegistrationClosed);
        }
        if intent.nonce() != self.next_nonce {
            return Err(Error::NonceMismatch);
        }
        let live_count = if create_live {
            self.live_count
                .checked_add(1)
                .ok_or(Error::LiveCountInvariant)?
        } else {
            self.live_count
        };
        let next_nonce = self.next_nonce.checked_add(1).ok_or(Error::NonceMismatch)?;
        let next = Self {
            next_nonce,
            live_count,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    pub(crate) fn close_live(self, intent: DirectIntentV2) -> Result<Self> {
        self.for_intent(intent)?;
        let live_count = self
            .live_count
            .checked_sub(1)
            .ok_or(Error::LiveCountInvariant)?;
        Ok(Self { live_count, ..self })
    }
}

/// Irreversibly close registration during the authenticated Market-retirement
/// handler. The SBF entrypoint must authenticate the canonical Market account,
/// generation, program owner, and terminal phase; no generic caller authority
/// or boolean retirement attestation is accepted by this contract.
pub fn close_replay_registration_v2(
    root: MakerReplayRootV2,
    phase: adapter::MarketPhaseV2,
) -> Result<MakerReplayRootV2> {
    adapter::require_market_phase_v2(adapter::AdapterActionV2::CloseReplayRegistration, phase)?;
    root.validate()?;
    if root.registration != ReplayRegistrationStatusV2::Open {
        return Err(Error::RegistrationClosed);
    }
    Ok(MakerReplayRootV2 {
        registration: ReplayRegistrationStatusV2::Closed,
        ..root
    })
}

/// Authorized zero-live root-close effects after Market retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootClosureV2 {
    /// Root rent returns only to its persisted original payer.
    pub rent_refund_payer: [u8; 32],
    /// Final refused nonce high-water retained in retirement evidence/logging.
    pub final_next_nonce: u64,
    /// Final maker-signed minimum-live threshold retained in retirement evidence.
    pub final_minimum_live_nonce: u64,
}

/// Prepare root closure inside the authenticated Market-retirement handler.
pub fn prepare_replay_root_close_v2(
    root: MakerReplayRootV2,
    phase: adapter::MarketPhaseV2,
) -> Result<RootClosureV2> {
    adapter::require_market_phase_v2(adapter::AdapterActionV2::CloseReplayRoot, phase)?;
    root.validate()?;
    if root.registration != ReplayRegistrationStatusV2::Closed {
        return Err(Error::RegistrationStillOpen);
    }
    if root.live_count != 0 {
        return Err(Error::LiveIntentsRemain);
    }
    Ok(RootClosureV2 {
        rent_refund_payer: root.rent_payer,
        final_next_nonce: root.next_nonce,
        final_minimum_live_nonce: root.minimum_live_nonce,
    })
}

/// Physical participant keys checked against one persisted intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParticipantAccountsV2 {
    /// Canonical maker replay-root PDA.
    pub replay_root: [u8; 32],
    /// Canonical live intent-record PDA.
    pub record: [u8; 32],
    /// Native Position account.
    pub position: [u8; 32],
    /// Maker-bound collateral token account.
    pub collateral: [u8; 32],
    /// Record-associated buy escrow, or canonical zero absence for a sell.
    pub escrow: [u8; 32],
}

impl ParticipantAccountsV2 {
    pub(crate) fn validate(self, intent: DirectIntentV2) -> Result<()> {
        nonzero(&self.replay_root)?;
        nonzero(&self.record)?;
        if self.replay_root == self.record
            || self.position != *intent.position_account()
            || self.collateral != *intent.collateral_account()
        {
            return Err(Error::AccountBindingMismatch);
        }
        match intent.side() {
            Side::Buy => nonzero(&self.escrow)?,
            Side::Sell if self.escrow != [0; 32] => return Err(Error::AccountBindingMismatch),
            Side::Sell => {}
        }
        Ok(())
    }
}

/// Physical maker accounts for an immediate no-record execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineParticipantAccountsV2 {
    /// Canonical maker replay-root PDA.
    pub replay_root: [u8; 32],
    /// Native Position debited or credited immediately.
    pub position: [u8; 32],
    /// Maker collateral account debited or credited immediately.
    pub collateral: [u8; 32],
}

impl InlineParticipantAccountsV2 {
    pub(crate) fn validate(self, intent: DirectIntentV2) -> Result<()> {
        nonzero(&self.replay_root)?;
        if self.position != *intent.position_account()
            || self.collateral != *intent.collateral_account()
        {
            return Err(Error::AccountBindingMismatch);
        }
        if self.replay_root == self.position
            || self.replay_root == self.collateral
            || self.position == self.collateral
        {
            return Err(Error::Alias);
        }
        Ok(())
    }
}

/// Sole live maker authorization and reserved-asset owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectIntentRecordV2 {
    intent: DirectIntentV2,
    filled: u64,
    reserved_claims: u64,
    reserved_collateral: u64,
    fee_basis_gross: u64,
    cumulative_fee: u64,
    rent_payer: [u8; 32],
    bump: u8,
}

impl DirectIntentRecordV2 {
    /// Decode and validate one exact live record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != DIRECT_INTENT_RECORD_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != DIRECT_INTENT_RECORD_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != DIRECT_INTENT_RECORD_SCHEMA_VERSION_V2 {
            return Err(Error::UnsupportedSchema);
        }
        if one(bytes, RECORD_STATUS_OFFSET)? != 0 {
            return Err(Error::UnknownIntentStatus);
        }
        zeros(bytes, RECORD_RESERVED_OFFSET, 4)?;
        let record = Self {
            intent: DirectIntentV2::decode_signed_preimage(
                bytes
                    .get(RECORD_INTENT_OFFSET..RECORD_FILLED_OFFSET)
                    .ok_or(Error::InvalidLength)?,
            )?,
            filled: u64::from_le_bytes(array(bytes, RECORD_FILLED_OFFSET)?),
            reserved_claims: u64::from_le_bytes(array(bytes, RECORD_CLAIMS_OFFSET)?),
            reserved_collateral: u64::from_le_bytes(array(bytes, RECORD_COLLATERAL_OFFSET)?),
            fee_basis_gross: u64::from_le_bytes(array(bytes, RECORD_FEE_GROSS_OFFSET)?),
            cumulative_fee: u64::from_le_bytes(array(bytes, RECORD_CUMULATIVE_FEE_OFFSET)?),
            rent_payer: array(bytes, RECORD_RENT_PAYER_OFFSET)?,
            bump: one(bytes, RECORD_BUMP_OFFSET)?,
        };
        record.validate()?;
        Ok(record)
    }

    /// Encode one exact live record.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        if output.len() != DIRECT_INTENT_RECORD_BYTES_V2 {
            return Err(Error::OutputLength);
        }
        self.validate()?;
        output.fill(0);
        put(output, 0, &DIRECT_INTENT_RECORD_MAGIC_V2);
        put(
            output,
            8,
            &DIRECT_INTENT_RECORD_SCHEMA_VERSION_V2.to_le_bytes(),
        );
        *output
            .get_mut(RECORD_BUMP_OFFSET)
            .ok_or(Error::OutputLength)? = self.bump;
        put(output, RECORD_INTENT_OFFSET, &self.intent.signed_preimage());
        put(output, RECORD_FILLED_OFFSET, &self.filled.to_le_bytes());
        put(
            output,
            RECORD_CLAIMS_OFFSET,
            &self.reserved_claims.to_le_bytes(),
        );
        put(
            output,
            RECORD_COLLATERAL_OFFSET,
            &self.reserved_collateral.to_le_bytes(),
        );
        put(
            output,
            RECORD_FEE_GROSS_OFFSET,
            &self.fee_basis_gross.to_le_bytes(),
        );
        put(
            output,
            RECORD_CUMULATIVE_FEE_OFFSET,
            &self.cumulative_fee.to_le_bytes(),
        );
        put(output, RECORD_RENT_PAYER_OFFSET, &self.rent_payer);
        Ok(())
    }

    /// Return persisted semantic intent.
    pub const fn intent(&self) -> DirectIntentV2 {
        self.intent
    }
    /// Return aggregate fill.
    pub const fn filled(&self) -> u64 {
        self.filled
    }
    /// Return remaining reserved claims.
    pub const fn reserved_claims(&self) -> u64 {
        self.reserved_claims
    }
    /// Return remaining reserved collateral.
    pub const fn reserved_collateral(&self) -> u64 {
        self.reserved_collateral
    }
    /// Aggregate gross on which this intent has actually owed venue fees.
    pub const fn fee_basis_gross(&self) -> u64 {
        self.fee_basis_gross
    }
    /// Cumulative venue fee charged at the one named floor boundary.
    pub const fn cumulative_fee(&self) -> u64 {
        self.cumulative_fee
    }
    /// Return original live-record-rent payer.
    pub const fn rent_payer(&self) -> &[u8; 32] {
        &self.rent_payer
    }
    /// Return stored PDA bump.
    pub const fn bump(&self) -> u8 {
        self.bump
    }

    fn validate(self) -> Result<()> {
        nonzero(&self.rent_payer)?;
        let maximum_fee_basis_gross = self
            .filled
            .checked_mul(PRICE_SCALE)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.fee_basis_gross > maximum_fee_basis_gross {
            return Err(Error::InvalidReservation);
        }
        if self.cumulative_fee != fee(self.fee_basis_gross, self.intent.fee_basis_points())? {
            return Err(Error::InvalidReservation);
        }
        if self.filled >= self.intent.max_fill {
            return Err(Error::StateOverfilled);
        }
        let remaining = self
            .intent
            .max_fill
            .checked_sub(self.filled)
            .ok_or(Error::StateOverfilled)?;
        match self.intent.side {
            Side::Buy => {
                if self.reserved_claims != 0
                    || self.reserved_collateral < maximum_buy_reserve_for(self.intent, remaining)?
                    || self.reserved_collateral > maximum_buy_reserve(self.intent)?
                {
                    return Err(Error::InvalidReservation);
                }
            }
            Side::Sell => {
                if self.reserved_collateral != 0 || self.reserved_claims != remaining {
                    return Err(Error::InvalidReservation);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn consume(
        self,
        root: MakerReplayRootV2,
        slot: u64,
        fill: u64,
        fee_basis_gross: u64,
        buy_gross_debit: u64,
    ) -> Result<(RecordAfterFillV2, MakerReplayRootV2, u64)> {
        self.validate()?;
        root.for_active_intent(self.intent)?;
        if slot < self.intent.from || slot > self.intent.through {
            return Err(Error::IntentExpired);
        }
        let remaining = self
            .intent
            .max_fill
            .checked_sub(self.filled)
            .ok_or(Error::StateOverfilled)?;
        if fill == 0 || fill > remaining {
            return Err(Error::InvalidFill);
        }
        let filled = self
            .filled
            .checked_add(fill)
            .ok_or(Error::ArithmeticOverflow)?;
        let next_fee_basis_gross = self
            .fee_basis_gross
            .checked_add(fee_basis_gross)
            .ok_or(Error::ArithmeticOverflow)?;
        let next_cumulative_fee = fee(next_fee_basis_gross, self.intent.fee_basis_points())?;
        let fee_delta = next_cumulative_fee
            .checked_sub(self.cumulative_fee)
            .ok_or(Error::InvalidReservation)?;
        let (claims, collateral) = match self.intent.side {
            Side::Buy => {
                if fee_basis_gross != buy_gross_debit {
                    return Err(Error::InvalidReservation);
                }
                let collateral_debit = buy_gross_debit
                    .checked_add(fee_delta)
                    .ok_or(Error::ArithmeticOverflow)?;
                let collateral = self
                    .reserved_collateral
                    .checked_sub(collateral_debit)
                    .ok_or(Error::InvalidReservation)?;
                (0, collateral)
            }
            Side::Sell => {
                if buy_gross_debit != 0 {
                    return Err(Error::InvalidReservation);
                }
                (
                    self.reserved_claims
                        .checked_sub(fill)
                        .ok_or(Error::InvalidReservation)?,
                    0,
                )
            }
        };
        if filled == self.intent.max_fill {
            let next_root = root.close_live(self.intent)?;
            Ok((
                RecordAfterFillV2::closed(LiveRecordCloseV2 {
                    closed_nonce: self.intent.nonce,
                    rent_refund_payer: self.rent_payer,
                    collateral_refund: collateral,
                    claim_refund: claims,
                }),
                next_root,
                fee_delta,
            ))
        } else {
            let next = Self {
                filled,
                reserved_claims: claims,
                reserved_collateral: collateral,
                fee_basis_gross: next_fee_basis_gross,
                cumulative_fee: next_cumulative_fee,
                ..self
            };
            next.validate()?;
            Ok((RecordAfterFillV2::live(next), root, fee_delta))
        }
    }
}

/// Resulting per-order account state after a fill. Exactly one field is present.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordAfterFillV2 {
    /// Partial-fill live replacement, absent on full close.
    pub live_record: Option<DirectIntentRecordV2>,
    /// Full-close effects, absent on partial fill.
    pub close: Option<LiveRecordCloseV2>,
}

impl RecordAfterFillV2 {
    pub(crate) const fn live(record: DirectIntentRecordV2) -> Self {
        Self {
            live_record: Some(record),
            close: None,
        }
    }
    const fn closed(close: LiveRecordCloseV2) -> Self {
        Self {
            live_record: None,
            close: Some(close),
        }
    }
    /// Return whether live record is closed.
    pub const fn is_closed(&self) -> bool {
        self.close.is_some() && self.live_record.is_none()
    }
}

/// Exact same-transaction effects when one live intent account closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRecordCloseV2 {
    /// Closed maker nonce, already covered by replay-root high-water.
    pub closed_nonce: u64,
    /// Persisted recipient of the full live-record rent principal.
    pub rent_refund_payer: [u8; 32],
    /// Remaining buy collateral returned to signed maker token account.
    pub collateral_refund: u64,
    /// Remaining sell claims returned to signed Position.
    pub claim_refund: u64,
}

/// Result of atomically reserving one maker-authorized intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistrationV2<const N: usize> {
    /// Updated sole replay high-water root.
    pub replay_root: MakerReplayRootV2,
    /// Program-owned live intent account to create.
    pub record: DirectIntentRecordV2,
    /// Replacement Position after sell reservation; unchanged for buy.
    pub position: PositionV1<N>,
    /// Claims debited into live record custody.
    pub reserved_claim_debit: u64,
    /// Collateral and maximum fee debited into associated escrow.
    pub reserved_collateral_debit: u64,
}

/// Complete hostile input for one atomic registered-intent creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistrationInputV2<const N: usize> {
    /// Existing replay root or canonical first-use absence.
    pub replay_root: ReplayRootStateV2,
    /// Exact maker-signed registered intent.
    pub intent: DirectIntentV2,
    /// Sealed immediately preceding native authorization.
    pub authorization: adapter::Ed25519AuthorizationV2,
    /// Canonical Market phase projection.
    pub phase: adapter::MarketPhaseV2,
    /// Trusted `Clock::get()` slot.
    pub slot: u64,
    /// Exact root/record/Position/collateral physical bindings.
    pub accounts: ParticipantAccountsV2,
    /// Separate signer paying root/record/escrow creation and persisted refund beneficiary.
    pub system_payer: [u8; 32],
    /// Realm-selected collateral mint for Buy; absent for Sell.
    pub collateral_mint: Option<[u8; 32]>,
    /// Authenticated exact source delegate for Buy; absent for Sell.
    pub buy_debit_authority: Option<adapter::BuyDebitAuthorityV2>,
    /// Canonical live-record PDA bump.
    pub record_bump: u8,
    /// Hostile-decoded immutable venue policy.
    pub fee_policy: VenueFeePolicyV3,
    /// SHA-256 digest of the exact policy bytes, authenticated through the Market manifest.
    pub fee_config_digest: [u8; 32],
    /// Current native Position.
    pub position: PositionV1<N>,
}

/// Create one live authorization/custody record inside the signed slot window
/// using the SBF adapter's trusted `Clock::get()` slot, and advance replay.
pub fn register_intent_v2<const N: usize>(
    input: RegistrationInputV2<N>,
) -> Result<RegistrationV2<N>> {
    let RegistrationInputV2 {
        replay_root,
        intent,
        authorization,
        phase,
        slot,
        accounts,
        system_payer,
        collateral_mint,
        buy_debit_authority,
        record_bump,
        fee_policy,
        fee_config_digest,
        mut position,
    } = input;
    width(N)?;
    let action = match intent.side() {
        Side::Buy => adapter::AdapterActionV2::RegisterBuy,
        Side::Sell => adapter::AdapterActionV2::RegisterSell,
    };
    adapter::require_market_phase_v2(action, phase)?;
    authorization.authorizes_registration(intent)?;
    if slot < intent.valid_from_slot() || slot > intent.valid_through_slot() {
        return Err(Error::IntentExpired);
    }
    accounts.validate(intent)?;
    validate_venue_policy_selection_v3(intent, fee_policy, fee_config_digest)?;
    nonzero(&system_payer)?;
    position_matches(position, intent)?;
    let outcome = usize::from(intent.outcome);
    if outcome >= N {
        return Err(Error::InvalidOutcome);
    }
    let next_root = replay_root
        .open_for_intent(intent, system_payer)?
        .register(intent)?;
    let (claims, collateral) = match intent.side {
        Side::Buy => {
            let collateral = maximum_buy_reserve(intent)?;
            adapter::validate_registered_buy_debit_authority_v2(
                buy_debit_authority.ok_or(Error::InvalidBuyDebitAuthority)?,
                intent,
                accounts.replay_root,
                collateral_mint.ok_or(Error::InvalidBuyDebitAuthority)?,
                collateral,
            )?;
            (0, collateral)
        }
        Side::Sell => {
            if collateral_mint.is_some() || buy_debit_authority.is_some() {
                return Err(Error::InvalidBuyDebitAuthority);
            }
            position
                .debit_outcome(outcome, intent.max_fill)
                .map_err(position_error)?;
            (intent.max_fill, 0)
        }
    };
    Ok(RegistrationV2 {
        replay_root: next_root,
        record: DirectIntentRecordV2 {
            intent,
            filled: 0,
            reserved_claims: claims,
            reserved_collateral: collateral,
            fee_basis_gross: 0,
            cumulative_fee: 0,
            rent_payer: system_payer,
            bump: record_bump,
        },
        position,
        reserved_claim_debit: claims,
        reserved_collateral_debit: collateral,
    })
}

/// Runtime-width registration input for the monomorphic SBF path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeRegistrationInputV2 {
    /// Existing replay root or canonical first-use absence.
    pub replay_root: ReplayRootStateV2,
    /// Exact maker-signed registered intent.
    pub intent: DirectIntentV2,
    /// Sealed immediately preceding native authorization.
    pub authorization: adapter::Ed25519AuthorizationV2,
    /// Canonical Market phase projection.
    pub phase: adapter::MarketPhaseV2,
    /// Trusted current slot.
    pub slot: u64,
    /// Exact physical account bindings.
    pub accounts: ParticipantAccountsV2,
    /// Separate account-creation payer and rent beneficiary.
    pub system_payer: [u8; 32],
    /// Realm collateral mint for Buy, absent for Sell.
    pub collateral_mint: Option<[u8; 32]>,
    /// Authenticated Buy debit authority, absent for Sell.
    pub buy_debit_authority: Option<adapter::BuyDebitAuthorityV2>,
    /// Canonical live-record PDA bump.
    pub record_bump: u8,
    /// Canonical venue policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated policy digest.
    pub fee_config_digest: [u8; 32],
    /// Runtime-width native Position.
    pub position: DirectPositionV2,
}

/// Runtime-width registration effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeRegistrationV2 {
    /// Updated replay root.
    pub replay_root: MakerReplayRootV2,
    /// New live intent record.
    pub record: DirectIntentRecordV2,
    /// Replacement Position.
    pub position: DirectPositionV2,
    /// Exact claims reserved from the Position.
    pub reserved_claim_debit: u64,
    /// Exact collateral and maximum fee reserve.
    pub reserved_collateral_debit: u64,
}

/// Register one intent without specializing execution by outcome width.
pub fn register_intent_runtime_v2(
    input: RuntimeRegistrationInputV2,
) -> Result<RuntimeRegistrationV2> {
    let RuntimeRegistrationInputV2 {
        replay_root,
        intent,
        authorization,
        phase,
        slot,
        accounts,
        system_payer,
        collateral_mint,
        buy_debit_authority,
        record_bump,
        fee_policy,
        fee_config_digest,
        mut position,
    } = input;
    let action = match intent.side() {
        Side::Buy => adapter::AdapterActionV2::RegisterBuy,
        Side::Sell => adapter::AdapterActionV2::RegisterSell,
    };
    adapter::require_market_phase_v2(action, phase)?;
    authorization.authorizes_registration(intent)?;
    if slot < intent.valid_from_slot() || slot > intent.valid_through_slot() {
        return Err(Error::IntentExpired);
    }
    accounts.validate(intent)?;
    validate_venue_policy_selection_v3(intent, fee_policy, fee_config_digest)?;
    nonzero(&system_payer)?;
    runtime_position_matches(position, intent)?;
    let outcome = usize::from(intent.outcome());
    if outcome >= usize::from(position.outcome_count()) {
        return Err(Error::InvalidOutcome);
    }
    let next_root = replay_root
        .open_for_intent(intent, system_payer)?
        .register(intent)?;
    let (claims, collateral) = match intent.side() {
        Side::Buy => {
            let collateral = maximum_buy_reserve(intent)?;
            adapter::validate_registered_buy_debit_authority_v2(
                buy_debit_authority.ok_or(Error::InvalidBuyDebitAuthority)?,
                intent,
                accounts.replay_root,
                collateral_mint.ok_or(Error::InvalidBuyDebitAuthority)?,
                collateral,
            )?;
            (0, collateral)
        }
        Side::Sell => {
            if collateral_mint.is_some() || buy_debit_authority.is_some() {
                return Err(Error::InvalidBuyDebitAuthority);
            }
            position.debit_outcome(outcome, intent.max_fill())?;
            (intent.max_fill(), 0)
        }
    };
    Ok(RuntimeRegistrationV2 {
        replay_root: next_root,
        record: DirectIntentRecordV2 {
            intent,
            filled: 0,
            reserved_claims: claims,
            reserved_collateral: collateral,
            fee_basis_gross: 0,
            cumulative_fee: 0,
            rent_payer: system_payer,
            bump: record_bump,
        },
        position,
        reserved_claim_debit: claims,
        reserved_collateral_debit: collateral,
    })
}

/// Canonical cancellation authorization message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCancelV2 {
    market: [u8; 32],
    generation: u64,
    maker: [u8; 32],
    nonce: u64,
}

impl DirectCancelV2 {
    /// Build exact cancellation message for live record.
    pub const fn for_record(record: DirectIntentRecordV2) -> Self {
        Self {
            market: record.intent.market,
            generation: record.intent.generation,
            maker: record.intent.maker,
            nonce: record.intent.nonce,
        }
    }
    /// Decode exact signed cancellation message.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != DIRECT_CANCEL_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != DIRECT_CANCEL_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != DIRECT_CANCEL_SCHEMA_VERSION_V2 {
            return Err(Error::UnsupportedSchema);
        }
        zeros(bytes, 10, 6)?;
        let value = Self {
            market: array(bytes, MARKET_OFFSET)?,
            generation: u64::from_le_bytes(array(bytes, GENERATION_OFFSET)?),
            maker: array(bytes, MAKER_OFFSET)?,
            nonce: u64::from_le_bytes(array(bytes, NONCE_OFFSET)?),
        };
        nonzero(&value.market)?;
        nonzero(&value.maker)?;
        Ok(value)
    }
    /// Return exact maker-signed cancellation bytes.
    pub fn signed_preimage(self) -> [u8; DIRECT_CANCEL_BYTES_V2] {
        let mut output = [0; DIRECT_CANCEL_BYTES_V2];
        put(&mut output, 0, &DIRECT_CANCEL_MAGIC_V2);
        put(
            &mut output,
            8,
            &DIRECT_CANCEL_SCHEMA_VERSION_V2.to_le_bytes(),
        );
        put(&mut output, MARKET_OFFSET, &self.market);
        put(
            &mut output,
            GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(&mut output, MAKER_OFFSET, &self.maker);
        put(&mut output, NONCE_OFFSET, &self.nonce.to_le_bytes());
        output
    }
}

/// Maker-signed O(1) replay-root invalidation threshold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CancelThroughV1 {
    market: [u8; 32],
    generation: u64,
    maker: [u8; 32],
    minimum_live_nonce: u64,
}

impl CancelThroughV1 {
    /// Construct a strictly advancing threshold no greater than the root's
    /// already-consumed next nonce.
    pub fn new(root: MakerReplayRootV2, minimum_live_nonce: u64) -> Result<Self> {
        root.validate()?;
        if minimum_live_nonce <= root.minimum_live_nonce || minimum_live_nonce > root.next_nonce {
            return Err(Error::InvalidCancelThrough);
        }
        Ok(Self {
            market: root.market,
            generation: root.generation,
            maker: root.maker,
            minimum_live_nonce,
        })
    }

    /// Decode one exact signed cancel-through message.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != DIRECT_CANCEL_THROUGH_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != DIRECT_CANCEL_THROUGH_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != DIRECT_CANCEL_THROUGH_SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedSchema);
        }
        zeros(bytes, 10, 6)?;
        let value = Self {
            market: array(bytes, MARKET_OFFSET)?,
            generation: u64::from_le_bytes(array(bytes, GENERATION_OFFSET)?),
            maker: array(bytes, MAKER_OFFSET)?,
            minimum_live_nonce: u64::from_le_bytes(array(bytes, NONCE_OFFSET)?),
        };
        nonzero(&value.market)?;
        nonzero(&value.maker)?;
        Ok(value)
    }

    /// Return exact maker-signed bytes.
    pub fn signed_preimage(self) -> [u8; DIRECT_CANCEL_THROUGH_BYTES_V1] {
        let mut output = [0; DIRECT_CANCEL_THROUGH_BYTES_V1];
        put(&mut output, 0, &DIRECT_CANCEL_THROUGH_MAGIC_V1);
        put(
            &mut output,
            8,
            &DIRECT_CANCEL_THROUGH_SCHEMA_VERSION_V1.to_le_bytes(),
        );
        put(&mut output, MARKET_OFFSET, &self.market);
        put(
            &mut output,
            GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(&mut output, MAKER_OFFSET, &self.maker);
        put(
            &mut output,
            NONCE_OFFSET,
            &self.minimum_live_nonce.to_le_bytes(),
        );
        output
    }

    /// Return the first nonce remaining matchable after the update.
    pub const fn minimum_live_nonce(self) -> u64 {
        self.minimum_live_nonce
    }
    /// Return the signing maker.
    pub const fn maker(&self) -> &[u8; 32] {
        &self.maker
    }
}

/// Apply one maker-authorized O(1) invalidation threshold to the sole replay
/// root. Persisted records are unwound separately and permissionlessly.
pub fn cancel_through_v1(
    root: MakerReplayRootV2,
    message: CancelThroughV1,
    authorization: adapter::Ed25519AuthorizationV2,
    phase: adapter::MarketPhaseV2,
) -> Result<MakerReplayRootV2> {
    adapter::require_market_phase_v2(adapter::AdapterActionV2::CancelThrough, phase)?;
    authorization.authorizes_cancel_through(message)?;
    root.validate()?;
    if message.market != root.market
        || message.generation != root.generation
        || message.maker != root.maker
        || message.minimum_live_nonce <= root.minimum_live_nonce
        || message.minimum_live_nonce > root.next_nonce
    {
        return Err(Error::InvalidCancelThrough);
    }
    Ok(MakerReplayRootV2 {
        minimum_live_nonce: message.minimum_live_nonce,
        ..root
    })
}

/// Atomic cancellation effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CancellationV2<const N: usize> {
    /// Updated replay root after live-count decrement.
    pub replay_root: MakerReplayRootV2,
    /// Closed live-account effects and persisted refund recipients.
    pub close: LiveRecordCloseV2,
    /// Position after returning remaining sell claims.
    pub position: PositionV1<N>,
}

/// Complete hostile input for one signed live-intent cancellation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CancellationInputV2<const N: usize> {
    /// Existing maker replay root.
    pub replay_root: MakerReplayRootV2,
    /// Program-owned live record.
    pub record: DirectIntentRecordV2,
    /// Sealed exact cancellation authorization.
    pub authorization: adapter::Ed25519AuthorizationV2,
    /// Canonical Market phase projection.
    pub phase: adapter::MarketPhaseV2,
    /// Exact physical bindings.
    pub accounts: ParticipantAccountsV2,
    /// Realm collateral mint for Buy; absent for Sell.
    pub collateral_mint: Option<[u8; 32]>,
    /// Live-record escrow authority for Buy; absent for Sell.
    pub escrow_authority: Option<adapter::EscrowAuthorityV2>,
    /// Current native Position.
    pub position: PositionV1<N>,
}

/// Cancel live intent with exact native-Ed25519 maker authorization.
pub fn cancel_intent_v2<const N: usize>(
    input: CancellationInputV2<N>,
) -> Result<CancellationV2<N>> {
    let CancellationInputV2 {
        replay_root,
        record,
        authorization,
        phase,
        accounts,
        collateral_mint,
        escrow_authority,
        mut position,
    } = input;
    width(N)?;
    let action = match record.intent.side() {
        Side::Buy => adapter::AdapterActionV2::CancelBuy,
        Side::Sell => adapter::AdapterActionV2::CancelSell,
    };
    adapter::require_market_phase_v2(action, phase)?;
    authorization.authorizes_cancellation(record)?;
    accounts.validate(record.intent)?;
    match record.intent.side() {
        Side::Buy => adapter::validate_registered_escrow_authority_v2(
            escrow_authority.ok_or(Error::InvalidEscrowAuthority)?,
            record,
            accounts.record,
            accounts.escrow,
            collateral_mint.ok_or(Error::InvalidEscrowAuthority)?,
        )?,
        Side::Sell if collateral_mint.is_some() || escrow_authority.is_some() => {
            return Err(Error::InvalidEscrowAuthority);
        }
        Side::Sell => {}
    }
    position_matches(position, record.intent)?;
    replay_root.for_intent(record.intent)?;
    let outcome = usize::from(record.intent.outcome);
    if outcome >= N {
        return Err(Error::InvalidOutcome);
    }
    if record.reserved_claims != 0 {
        position
            .credit_outcome(outcome, record.reserved_claims)
            .map_err(position_error)?;
    }
    let next_root = replay_root.close_live(record.intent)?;
    Ok(CancellationV2 {
        replay_root: next_root,
        close: LiveRecordCloseV2 {
            closed_nonce: record.intent.nonce,
            rent_refund_payer: record.rent_payer,
            collateral_refund: record.reserved_collateral,
            claim_refund: record.reserved_claims,
        },
        position,
    })
}

/// Atomic permissionless post-expiry close effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpirationV2<const N: usize> {
    /// Updated replay root after live-count decrement.
    pub replay_root: MakerReplayRootV2,
    /// Closed live-account effects and persisted refund recipients.
    pub close: LiveRecordCloseV2,
    /// Position after returning remaining sell claims.
    pub position: PositionV1<N>,
}

/// Complete hostile input for one permissionless post-expiry close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpirationInputV2<const N: usize> {
    /// Existing maker replay root.
    pub replay_root: MakerReplayRootV2,
    /// Program-owned live record.
    pub record: DirectIntentRecordV2,
    /// Canonical Market phase projection.
    pub phase: adapter::MarketPhaseV2,
    /// Trusted `Clock::get()` slot.
    pub slot: u64,
    /// Exact physical bindings.
    pub accounts: ParticipantAccountsV2,
    /// Realm collateral mint for Buy; absent for Sell.
    pub collateral_mint: Option<[u8; 32]>,
    /// Live-record escrow authority for Buy; absent for Sell.
    pub escrow_authority: Option<adapter::EscrowAuthorityV2>,
    /// Current native Position.
    pub position: PositionV1<N>,
}

/// Close and refund an intent strictly after its inclusive validity boundary.
pub fn expire_intent_v2<const N: usize>(input: ExpirationInputV2<N>) -> Result<ExpirationV2<N>> {
    let ExpirationInputV2 {
        replay_root,
        record,
        phase,
        slot,
        accounts,
        collateral_mint,
        escrow_authority,
        mut position,
    } = input;
    width(N)?;
    let action = match record.intent.side() {
        Side::Buy => adapter::AdapterActionV2::ExpireBuy,
        Side::Sell => adapter::AdapterActionV2::ExpireSell,
    };
    adapter::require_market_phase_v2(action, phase)?;
    if slot <= record.intent.through {
        return Err(Error::IntentNotExpired);
    }
    accounts.validate(record.intent)?;
    match record.intent.side() {
        Side::Buy => adapter::validate_registered_escrow_authority_v2(
            escrow_authority.ok_or(Error::InvalidEscrowAuthority)?,
            record,
            accounts.record,
            accounts.escrow,
            collateral_mint.ok_or(Error::InvalidEscrowAuthority)?,
        )?,
        Side::Sell if collateral_mint.is_some() || escrow_authority.is_some() => {
            return Err(Error::InvalidEscrowAuthority);
        }
        Side::Sell => {}
    }
    position_matches(position, record.intent)?;
    replay_root.for_intent(record.intent)?;
    let outcome = usize::from(record.intent.outcome);
    if outcome >= N {
        return Err(Error::InvalidOutcome);
    }
    if record.reserved_claims != 0 {
        position
            .credit_outcome(outcome, record.reserved_claims)
            .map_err(position_error)?;
    }
    let next_root = replay_root.close_live(record.intent)?;
    Ok(ExpirationV2 {
        replay_root: next_root,
        close: LiveRecordCloseV2 {
            closed_nonce: record.intent.nonce,
            rent_refund_payer: record.rent_payer,
            collateral_refund: record.reserved_collateral,
            claim_refund: record.reserved_claims,
        },
        position,
    })
}

/// Complete hostile input for permissionlessly unwinding one record invalidated
/// by the maker's replay-root minimum-live nonce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidatedCloseInputV1<const N: usize> {
    /// Maker replay root carrying the signed threshold.
    pub replay_root: MakerReplayRootV2,
    /// Program-owned invalidated live record.
    pub record: DirectIntentRecordV2,
    /// Canonical Market phase projection.
    pub phase: adapter::MarketPhaseV2,
    /// Exact physical bindings.
    pub accounts: ParticipantAccountsV2,
    /// Realm collateral mint for Buy; absent for Sell.
    pub collateral_mint: Option<[u8; 32]>,
    /// Live-record escrow authority for Buy; absent for Sell.
    pub escrow_authority: Option<adapter::EscrowAuthorityV2>,
    /// Current native Position.
    pub position: PositionV1<N>,
}

/// Permissionlessly close and refund a registered record below the maker's
/// signed minimum-live nonce. No enumeration or centralized cancel service is
/// required; each close returns assets to the record's signed destinations.
pub fn close_invalidated_intent_v1<const N: usize>(
    input: InvalidatedCloseInputV1<N>,
) -> Result<ExpirationV2<N>> {
    let InvalidatedCloseInputV1 {
        replay_root,
        record,
        phase,
        accounts,
        collateral_mint,
        escrow_authority,
        mut position,
    } = input;
    width(N)?;
    let action = match record.intent.side() {
        Side::Buy => adapter::AdapterActionV2::CloseInvalidatedBuy,
        Side::Sell => adapter::AdapterActionV2::CloseInvalidatedSell,
    };
    adapter::require_market_phase_v2(action, phase)?;
    replay_root.for_intent(record.intent)?;
    if record.intent.nonce >= replay_root.minimum_live_nonce {
        return Err(Error::IntentNotInvalidated);
    }
    accounts.validate(record.intent)?;
    match record.intent.side() {
        Side::Buy => adapter::validate_registered_escrow_authority_v2(
            escrow_authority.ok_or(Error::InvalidEscrowAuthority)?,
            record,
            accounts.record,
            accounts.escrow,
            collateral_mint.ok_or(Error::InvalidEscrowAuthority)?,
        )?,
        Side::Sell if collateral_mint.is_some() || escrow_authority.is_some() => {
            return Err(Error::InvalidEscrowAuthority);
        }
        Side::Sell => {}
    }
    position_matches(position, record.intent)?;
    let outcome = usize::from(record.intent.outcome);
    if outcome >= N {
        return Err(Error::InvalidOutcome);
    }
    if record.reserved_claims != 0 {
        position
            .credit_outcome(outcome, record.reserved_claims)
            .map_err(position_error)?;
    }
    let next_root = replay_root.close_live(record.intent)?;
    Ok(ExpirationV2 {
        replay_root: next_root,
        close: LiveRecordCloseV2 {
            closed_nonce: record.intent.nonce,
            rent_refund_payer: record.rent_payer,
            collateral_refund: record.reserved_collateral,
            claim_refund: record.reserved_claims,
        },
        position,
    })
}

/// Runtime-width reason for unwinding one live record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeUnwindKindV2<'a> {
    /// Maker-authorized cancellation.
    Cancel {
        /// Sealed authorization of the exact cancellation preimage.
        authorization: &'a adapter::Ed25519AuthorizationV2,
    },
    /// Permissionless close strictly after the inclusive expiry slot.
    Expire {
        /// Trusted current slot.
        slot: u64,
    },
    /// Permissionless close below the maker-signed live-nonce threshold.
    Invalidated,
}

/// Runtime-width live-record unwind input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeUnwindInputV2<'a> {
    /// Existing maker replay root.
    pub replay_root: MakerReplayRootV2,
    /// Program-owned live record.
    pub record: DirectIntentRecordV2,
    /// Selected unwind authorization or clock condition.
    pub kind: RuntimeUnwindKindV2<'a>,
    /// Canonical Market phase.
    pub phase: adapter::MarketPhaseV2,
    /// Exact participant bindings.
    pub accounts: ParticipantAccountsV2,
    /// Realm collateral mint for Buy, absent for Sell.
    pub collateral_mint: Option<[u8; 32]>,
    /// Authenticated Buy escrow authority, absent for Sell.
    pub escrow_authority: Option<adapter::EscrowAuthorityV2>,
    /// Current runtime-width Position.
    pub position: DirectPositionV2,
}

/// Runtime-width unwind effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeUnwindV2 {
    /// Replay root after live-count decrement.
    pub replay_root: MakerReplayRootV2,
    /// Closed-record asset and rent effects.
    pub close: LiveRecordCloseV2,
    /// Position after any remaining sell claims are returned.
    pub position: DirectPositionV2,
}

/// Unwind cancellation, expiry, and nonce invalidation through one runtime path.
pub fn unwind_intent_runtime_v2(input: RuntimeUnwindInputV2<'_>) -> Result<RuntimeUnwindV2> {
    let RuntimeUnwindInputV2 {
        replay_root,
        record,
        kind,
        phase,
        accounts,
        collateral_mint,
        escrow_authority,
        mut position,
    } = input;
    let intent = record.intent();
    let action = match (kind, intent.side()) {
        (RuntimeUnwindKindV2::Cancel { .. }, Side::Buy) => adapter::AdapterActionV2::CancelBuy,
        (RuntimeUnwindKindV2::Cancel { .. }, Side::Sell) => adapter::AdapterActionV2::CancelSell,
        (RuntimeUnwindKindV2::Expire { .. }, Side::Buy) => adapter::AdapterActionV2::ExpireBuy,
        (RuntimeUnwindKindV2::Expire { .. }, Side::Sell) => adapter::AdapterActionV2::ExpireSell,
        (RuntimeUnwindKindV2::Invalidated, Side::Buy) => {
            adapter::AdapterActionV2::CloseInvalidatedBuy
        }
        (RuntimeUnwindKindV2::Invalidated, Side::Sell) => {
            adapter::AdapterActionV2::CloseInvalidatedSell
        }
    };
    adapter::require_market_phase_v2(action, phase)?;
    match kind {
        RuntimeUnwindKindV2::Cancel { authorization } => {
            authorization.authorizes_cancellation(record)?;
        }
        RuntimeUnwindKindV2::Expire { slot } => {
            if slot <= intent.valid_through_slot() {
                return Err(Error::IntentNotExpired);
            }
        }
        RuntimeUnwindKindV2::Invalidated => {
            replay_root.for_intent(intent)?;
            if intent.nonce() >= replay_root.minimum_live_nonce() {
                return Err(Error::IntentNotInvalidated);
            }
        }
    }
    accounts.validate(intent)?;
    match intent.side() {
        Side::Buy => adapter::validate_registered_escrow_authority_v2(
            escrow_authority.ok_or(Error::InvalidEscrowAuthority)?,
            record,
            accounts.record,
            accounts.escrow,
            collateral_mint.ok_or(Error::InvalidEscrowAuthority)?,
        )?,
        Side::Sell if collateral_mint.is_some() || escrow_authority.is_some() => {
            return Err(Error::InvalidEscrowAuthority);
        }
        Side::Sell => {}
    }
    runtime_position_matches(position, intent)?;
    replay_root.for_intent(intent)?;
    let outcome = usize::from(intent.outcome());
    if outcome >= usize::from(position.outcome_count()) {
        return Err(Error::InvalidOutcome);
    }
    if record.reserved_claims() != 0 {
        position.credit_outcome(outcome, record.reserved_claims())?;
    }
    let next_root = replay_root.close_live(intent)?;
    Ok(RuntimeUnwindV2 {
        replay_root: next_root,
        close: LiveRecordCloseV2 {
            closed_nonce: intent.nonce(),
            rent_refund_payer: *record.rent_payer(),
            collateral_refund: record.reserved_collateral(),
            claim_refund: record.reserved_claims(),
        },
        position,
    })
}

/// Immutable market-independent fee policy read from a canonical record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VenueFeePolicyV3 {
    recipient: [u8; 32],
    bps: u16,
}

impl VenueFeePolicyV3 {
    /// Validate immutable policy.
    pub fn new(recipient: [u8; 32], fee_basis_points: u16) -> Result<Self> {
        nonzero(&recipient)?;
        if u64::from(fee_basis_points) > FEE_BASIS_POINTS_DENOMINATOR {
            return Err(Error::InvalidFeeRate);
        }
        Ok(Self {
            recipient,
            bps: fee_basis_points,
        })
    }
    /// Decode canonical policy account.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != VENUE_FEE_POLICY_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != VENUE_FEE_POLICY_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != VENUE_FEE_POLICY_SCHEMA_VERSION_V3 {
            return Err(Error::UnsupportedSchema);
        }
        zeros(bytes, FEE_RESERVED_OFFSET, 4)?;
        Self::new(
            array(bytes, FEE_RECIPIENT_OFFSET)?,
            u16::from_le_bytes(array(bytes, FEE_BPS_OFFSET)?),
        )
    }
    /// Encode canonical policy account.
    pub fn encode(self, output: &mut [u8]) -> Result<()> {
        if output.len() != VENUE_FEE_POLICY_BYTES_V3 {
            return Err(Error::OutputLength);
        }
        output.fill(0);
        put(output, 0, &VENUE_FEE_POLICY_MAGIC_V3);
        put(output, 8, &VENUE_FEE_POLICY_SCHEMA_VERSION_V3.to_le_bytes());
        put(output, FEE_BPS_OFFSET, &self.bps.to_le_bytes());
        put(output, FEE_RECIPIENT_OFFSET, &self.recipient);
        Ok(())
    }
    /// Return recipient account.
    pub const fn recipient(&self) -> &[u8; 32] {
        &self.recipient
    }
    /// Return fee basis points.
    pub const fn fee_basis_points(&self) -> u16 {
        self.bps
    }
}

/// Require one exact content-addressed policy selected by the signed intent.
///
/// Market and generation do not belong to policy content. The SBF boundary
/// authenticates the Market whose immutable manifest selects
/// `fee_config_digest`; the signed intent and Position bind that Market and
/// generation independently.
pub fn validate_venue_policy_selection_v3(
    intent: DirectIntentV2,
    policy: VenueFeePolicyV3,
    fee_config_digest: [u8; 32],
) -> Result<()> {
    if fee_config_digest != intent.fee_config || policy.bps != intent.fee_bps {
        return Err(Error::VenueUnauthorized);
    }
    Ok(())
}

pub(crate) fn venue_authorized(
    intent: DirectIntentV2,
    policy: VenueFeePolicyV3,
    fee_config_digest: [u8; 32],
    recipient_account: [u8; 32],
) -> Result<()> {
    validate_venue_policy_selection_v3(intent, policy, fee_config_digest)?;
    if policy.recipient != recipient_account {
        return Err(Error::VenueUnauthorized);
    }
    Ok(())
}

pub(crate) fn position_matches<const N: usize>(
    position: PositionV1<N>,
    intent: DirectIntentV2,
) -> Result<()> {
    if position.market() != intent.market() || position.generation() != intent.generation {
        return Err(Error::PositionMarketMismatch);
    }
    if position.owner() != intent.maker() {
        return Err(Error::PositionOwnerMismatch);
    }
    Ok(())
}

pub(crate) fn runtime_position_matches(
    position: DirectPositionV2,
    intent: DirectIntentV2,
) -> Result<()> {
    if position.market() != *intent.market() || position.generation() != intent.generation() {
        return Err(Error::PositionMarketMismatch);
    }
    if position.owner() != *intent.maker() {
        return Err(Error::PositionOwnerMismatch);
    }
    Ok(())
}

fn maximum_buy_reserve(intent: DirectIntentV2) -> Result<u64> {
    maximum_buy_reserve_for(intent, intent.max_fill)
}

fn maximum_buy_reserve_for(intent: DirectIntentV2, quantity: u64) -> Result<u64> {
    let product = u128::from(quantity)
        .checked_mul(u128::from(intent.limit))
        .ok_or(Error::ArithmeticOverflow)?;
    let gross =
        u64::try_from(product / u128::from(PRICE_SCALE)).map_err(|_| Error::ArithmeticOverflow)?;
    gross
        .checked_add(fee(gross, intent.fee_bps)?)
        .ok_or(Error::ArithmeticOverflow)
}

/// Exact rent consequences of fully closing one live account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalRentTransitionV2 {
    /// Observed source lamports up to the current account rent minimum.
    pub rent_principal: u64,
    /// Lamports above principal, explicitly unclassified as economic revenue.
    pub unclassified_donation: u64,
    /// Total lamports credited to the persisted payer's canonical RentCredit.
    pub rent_credit_total: u64,
}

/// Canonical RentCredit binding and exact complete-source close plan.
///
/// The returned [`RentCreditV1`] exposes the landed contract's exact PDA seed
/// projection. The composing SBF adapter must additionally authenticate the
/// Rent program owner and prove that the supplied account key is the PDA
/// derived from those seeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRentCreditClosePlanV1 {
    rent_credit: RentCreditV1,
    classification: TerminalRentTransitionV2,
    source_close: SourceCloseCreditPlanV1,
}

impl DirectRentCreditClosePlanV1 {
    /// Return the hostile-decoded credit whose state binds authority and bump.
    pub const fn rent_credit(self) -> RentCreditV1 {
        self.rent_credit
    }

    /// Return Direct's honest rent-principal/donation classification.
    pub const fn classification(self) -> TerminalRentTransitionV2 {
        self.classification
    }

    /// Return the Rent contract's exact complete-source credit transition.
    pub const fn source_close(self) -> SourceCloseCreditPlanV1 {
        self.source_close
    }
}

/// Classify a close balance without reclassifying donated lamports as rent.
///
/// An older source may hold less than today's rent minimum after the schedule
/// changes. Its full balance remains returnable principal rather than becoming
/// an uncloseable account. The full returned total is always for the persisted
/// payer's canonical RentCredit.
pub fn terminal_rent_transition_v2(
    current_lamports: u64,
    current_rent_minimum: u64,
) -> Result<TerminalRentTransitionV2> {
    let rent_principal = core::cmp::min(current_lamports, current_rent_minimum);
    Ok(TerminalRentTransitionV2 {
        rent_principal,
        unclassified_donation: current_lamports
            .checked_sub(rent_principal)
            .ok_or(Error::ArithmeticOverflow)?,
        rent_credit_total: current_lamports,
    })
}

/// Bind one terminal Direct source to the landed permanent RentCredit contract.
///
/// `persisted_refund_authority` is the immutable beneficiary stored in the
/// replay root or live record. `derived_pda_bump` must come from deriving the
/// supplied RentCredit account under the Rent program; it is not caller-authored
/// authority. The complete observed source balance, including any explicitly
/// unclassified donation, becomes the exact credit delta.
pub fn terminal_rent_credit_close_plan_v1(
    persisted_refund_authority: [u8; 32],
    rent_credit_account_data: &[u8],
    derived_pda_bump: u8,
    source_before: u64,
    current_rent_minimum: u64,
    rent_credit_before: u64,
) -> Result<DirectRentCreditClosePlanV1> {
    let authority = RefundAuthority::new(persisted_refund_authority)?;
    let rent_credit = RentCreditV1::decode(rent_credit_account_data)?;
    rent_credit.validate_binding(authority, derived_pda_bump)?;
    let classification = terminal_rent_transition_v2(source_before, current_rent_minimum)?;
    let source_close = SourceCloseCreditPlanV1::new(
        source_before,
        rent_credit_before,
        classification.rent_credit_total,
    )?;
    Ok(DirectRentCreditClosePlanV1 {
        rent_credit,
        classification,
        source_close,
    })
}
