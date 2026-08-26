//! Data-defined Direct execution configuration and replay ownership.
//!
//! The immutable config is selected by the authenticated CapabilityProgram
//! descriptor and owns the one positive price scale, fee rate, and fee
//! recipient. The common capability-root header continues to own Market,
//! generation, manifest, release, and config identities; [`DirectRootStateV1`]
//! persists only global Direct admission and the count of open maker roots.
//! Each [`MakerReplayRootV1`] is the sole nonce/live/minimum-live/rent owner for
//! one maker.
//!
//! Account/PDA/System-owner vacancy checks, native Ed25519 authentication,
//! finalized-record hashing, Registry admission, and physical account writes
//! remain in the Trading adapter. These transitions return complete candidates
//! by value, so a refusal cannot partially mutate caller state.

use crate::CompactIntentV1;

#[rustfmt::skip]
#[allow(missing_docs)]
#[path = "generated_successor.rs"]
mod generated;

/// Immutable execution-config byte width.
pub const DIRECT_EXECUTION_CONFIG_BYTES_V1: usize = generated::DIRECT_EXECUTION_CONFIG_BYTES_V1;
/// Mutable Direct capability-root tail width.
pub const DIRECT_ROOT_STATE_BYTES_V1: usize = generated::DIRECT_ROOT_STATE_BYTES_V1;
/// Per-maker replay-root byte width.
pub const DIRECT_MAKER_REPLAY_BYTES_V1: usize = generated::DIRECT_MAKER_REPLAY_BYTES_V1;
/// One live registered-intent record width.
pub const DIRECT_REGISTERED_RECORD_BYTES_V1: usize = generated::DIRECT_REGISTERED_RECORD_BYTES_V1;
/// Basis-point denominator and sole fee floor denominator.
pub const DIRECT_FEE_DENOMINATOR_V1: u16 = 10_000;

/// Finalized-record schema label for [`DirectExecutionConfigV1`].
pub const DIRECT_EXECUTION_CONFIG_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/direct-execution-config-v1";
/// SHA-256 of [`DIRECT_EXECUTION_CONFIG_SCHEMA_PREIMAGE_V1`].
pub const DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1: [u8; 32] = [
    0x33, 0xe0, 0xce, 0x77, 0xaa, 0xa0, 0xb5, 0x18, 0x78, 0xa3, 0x43, 0x16, 0x21, 0x68, 0xbf, 0x6a,
    0x7e, 0x56, 0x57, 0xc2, 0x48, 0x27, 0x08, 0x24, 0x2d, 0x1d, 0x36, 0x2e, 0xed, 0x57, 0x32, 0x6d,
];
/// Descriptor-selected root-tail schema label.
pub const DIRECT_ROOT_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/direct-root-v1";
/// SHA-256 of [`DIRECT_ROOT_SCHEMA_PREIMAGE_V1`].
pub const DIRECT_ROOT_SCHEMA_ID_V1: [u8; 32] = [
    0x84, 0xeb, 0x0a, 0xaa, 0x82, 0x54, 0xb4, 0x9c, 0x75, 0x86, 0x18, 0xe7, 0xcf, 0xd2, 0xcd, 0x2d,
    0xbc, 0xde, 0xf6, 0x70, 0x3f, 0x54, 0x78, 0x53, 0xc7, 0x6e, 0xf8, 0xae, 0x1a, 0x0e, 0xd6, 0x85,
];
/// Per-maker replay account schema label.
pub const DIRECT_MAKER_REPLAY_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/direct-maker-replay-v1";
/// SHA-256 of [`DIRECT_MAKER_REPLAY_SCHEMA_PREIMAGE_V1`].
pub const DIRECT_MAKER_REPLAY_SCHEMA_ID_V1: [u8; 32] = [
    0x02, 0xef, 0xe1, 0x37, 0x1c, 0xab, 0xbf, 0x04, 0x61, 0xf4, 0xa1, 0xb8, 0x59, 0xae, 0xa4, 0xa3,
    0x60, 0x08, 0x07, 0x06, 0xc6, 0x9b, 0xa1, 0x04, 0xfc, 0x46, 0x06, 0x5a, 0x30, 0xfd, 0x96, 0x15,
];
/// Per-maker replay derivation-policy label.
pub const DIRECT_MAKER_REPLAY_DERIVATION_PREIMAGE_V1: &[u8] =
    b"dclutch/derivation/direct-maker-replay-v1";
/// SHA-256 of [`DIRECT_MAKER_REPLAY_DERIVATION_PREIMAGE_V1`].
pub const DIRECT_MAKER_REPLAY_DERIVATION_ID_V1: [u8; 32] = [
    0xe2, 0x15, 0x48, 0x89, 0x1e, 0x52, 0xe9, 0x67, 0x5d, 0x12, 0x3e, 0x2f, 0x38, 0xc1, 0x41, 0xf4,
    0xf6, 0x9a, 0x53, 0x33, 0x85, 0xa2, 0x8e, 0x7f, 0x85, 0x5c, 0x63, 0x51, 0xb9, 0xdd, 0x23, 0x8c,
];
/// Live registered-intent schema label.
pub const DIRECT_REGISTERED_RECORD_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/direct-registered-intent-v1";
/// SHA-256 of [`DIRECT_REGISTERED_RECORD_SCHEMA_PREIMAGE_V1`].
pub const DIRECT_REGISTERED_RECORD_SCHEMA_ID_V1: [u8; 32] = [
    0x70, 0xf4, 0x0a, 0x55, 0x3b, 0x86, 0xd0, 0x22, 0xf1, 0x33, 0xac, 0x0f, 0x25, 0xdb, 0x5c, 0x0e,
    0x51, 0x56, 0x99, 0x99, 0xae, 0xd0, 0x28, 0x56, 0x4e, 0x13, 0x4d, 0xdf, 0x8c, 0xd5, 0x44, 0x94,
];
/// Live registered-intent derivation-policy label.
pub const DIRECT_REGISTERED_RECORD_DERIVATION_PREIMAGE_V1: &[u8] =
    b"dclutch/derivation/direct-registered-intent-v1";
/// SHA-256 of [`DIRECT_REGISTERED_RECORD_DERIVATION_PREIMAGE_V1`].
pub const DIRECT_REGISTERED_RECORD_DERIVATION_ID_V1: [u8; 32] = [
    0x28, 0xf6, 0xe4, 0xfe, 0x58, 0x98, 0x1b, 0x4f, 0x70, 0xc0, 0x6e, 0x09, 0x73, 0xa0, 0x25, 0xe3,
    0x1f, 0xc5, 0x5a, 0x29, 0x09, 0x7d, 0xb1, 0x74, 0x23, 0xc1, 0xd2, 0x21, 0x8e, 0xdc, 0xbb, 0x4e,
];
/// Canonical maker-root PDA domain under the selected Trading program.
pub const DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1: &[u8] = b"dclutch:direct-maker:v1";
/// Canonical live-record PDA domain under the selected Trading program.
pub const DIRECT_REGISTERED_RECORD_PDA_DOMAIN_V1: &[u8] = b"dclutch:direct-intent:v1";

const _: () = assert!(DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DIRECT_REGISTERED_RECORD_PDA_DOMAIN_V1.len() <= 32);

/// Stable refusal from the Direct successor config/replay contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuccessorError {
    /// A byte slice had another width.
    InvalidLength,
    /// Magic selected another record family.
    InvalidMagic,
    /// The record named an unsupported schema version.
    UnsupportedVersion,
    /// Reserved bytes were nonzero.
    NonCanonicalReserved,
    /// A required content, account, maker, or beneficiary identity was zero.
    ZeroIdentity,
    /// Authenticated config content differed from the descriptor selection.
    ConfigSelectionMismatch,
    /// Price scale was zero or the fee rate exceeded 10,000 basis points.
    InvalidExecutionConfig,
    /// Root phase was unknown or did not admit the requested action.
    InvalidRootPhase,
    /// A maker root differed from Market/generation/maker coordinates.
    MakerCoordinateMismatch,
    /// A nonce was stale, skipped, or overflowed.
    NonceMismatch,
    /// A live-intent count was inconsistent or overflowed.
    LiveCountInvariant,
    /// A cancel-through minimum moved backward or beyond the nonce high-water.
    MinimumLiveNonceInvariant,
    /// First-use presence/funding fields were not canonical.
    InvalidFirstUse,
    /// Historical rent principal or observed close balance was invalid.
    InvalidRent,
    /// Global maker-root count underflowed or overflowed.
    MakerRootCountInvariant,
    /// A caller output slice had another exact width.
    OutputLength,
    /// Signed intent fields were not canonical for the requested lifecycle.
    InvalidIntent,
    /// Product width or outcome coordinate was not representable.
    InvalidOutcome,
    /// Trusted slot was outside the signed inclusive interval.
    IntentExpired,
    /// Permissionless expiry was attempted before the inclusive boundary ended.
    IntentNotExpired,
    /// Price, side, Market, or generation did not form the selected match.
    IncompatibleMatch,
    /// A quote was not exactly representable at the immutable price scale.
    NonIntegralQuote,
    /// Checked integer arithmetic overflowed.
    ArithmeticOverflow,
    /// Persisted claim/collateral/fee reservation facts were inconsistent.
    InvalidReservation,
    /// Live record and maker replay coordinates differed.
    RecordCoordinateMismatch,
    /// Terminal cancel, expiry, or invalidation evidence did not authorize close.
    InvalidTerminal,
    /// Runtime-width complementary slices or scratch did not have one exact width.
    ComplementWidth,
    /// Outcomes/prices did not form one canonical exhaustive complement.
    NonCanonicalComplement,
    /// Two independently authorized makers aliased.
    Alias,
}

/// Result alias for Direct successor operations.
pub type SuccessorResult<T> = core::result::Result<T, SuccessorError>;

/// Descriptor-owned requirements known before account/effect profile selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSuccessorRequirementsV1;

impl DirectSuccessorRequirementsV1 {
    /// Require the descriptor's config/root/derivation coordinates.
    pub fn validate(
        config_schema: [u8; 32],
        root_schema: [u8; 32],
        maker_derivation_policy: [u8; 32],
        record_schema: [u8; 32],
        record_derivation_policy: [u8; 32],
    ) -> SuccessorResult<()> {
        if config_schema != DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1
            || root_schema != DIRECT_ROOT_SCHEMA_ID_V1
            || maker_derivation_policy != DIRECT_MAKER_REPLAY_DERIVATION_ID_V1
            || record_schema != DIRECT_REGISTERED_RECORD_SCHEMA_ID_V1
            || record_derivation_policy != DIRECT_REGISTERED_RECORD_DERIVATION_ID_V1
        {
            Err(SuccessorError::ConfigSelectionMismatch)
        } else {
            Ok(())
        }
    }
}

/// Immutable content-selected Direct price and fee policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectExecutionConfigV1 {
    price_scale: u64,
    fee_basis_points: u16,
    fee_recipient: [u8; 32],
}

impl DirectExecutionConfigV1 {
    /// Construct and validate one canonical config.
    pub fn new(
        price_scale: u64,
        fee_basis_points: u16,
        fee_recipient: [u8; 32],
    ) -> SuccessorResult<Self> {
        if price_scale == 0 || fee_basis_points > DIRECT_FEE_DENOMINATOR_V1 {
            return Err(SuccessorError::InvalidExecutionConfig);
        }
        require_nonzero(fee_recipient)?;
        Ok(Self {
            price_scale,
            fee_basis_points,
            fee_recipient,
        })
    }

    /// Hostile-decode only after exact descriptor-to-record content selection.
    pub fn decode_selected(
        selected_config_id: [u8; 32],
        authenticated_config_id: [u8; 32],
        input: &[u8],
    ) -> SuccessorResult<Self> {
        require_nonzero(selected_config_id)?;
        if selected_config_id != authenticated_config_id {
            return Err(SuccessorError::ConfigSelectionMismatch);
        }
        exact_width(input, DIRECT_EXECUTION_CONFIG_BYTES_V1)?;
        exact(
            input,
            generated::DIRECT_CONFIG_MAGIC_OFFSET_V1,
            &generated::DIRECT_CONFIG_MAGIC_V1,
        )?;
        version(input, generated::DIRECT_CONFIG_VERSION_OFFSET_V1)?;
        zero_range(input, generated::DIRECT_CONFIG_RESERVED_A_OFFSET_V1, 6)?;
        zero_range(input, generated::DIRECT_CONFIG_RESERVED_B_OFFSET_V1, 6)?;
        Self::new(
            u64_at(input, generated::DIRECT_CONFIG_PRICE_SCALE_OFFSET_V1)?,
            u16_at(input, generated::DIRECT_CONFIG_FEE_BPS_OFFSET_V1)?,
            array_at(input, generated::DIRECT_CONFIG_FEE_RECIPIENT_OFFSET_V1)?,
        )
    }

    /// Encode the exact finalized-record preimage.
    pub fn encode(self) -> [u8; DIRECT_EXECUTION_CONFIG_BYTES_V1] {
        let mut output = [0_u8; DIRECT_EXECUTION_CONFIG_BYTES_V1];
        put(
            &mut output,
            generated::DIRECT_CONFIG_MAGIC_OFFSET_V1,
            &generated::DIRECT_CONFIG_MAGIC_V1,
        );
        put(
            &mut output,
            generated::DIRECT_CONFIG_VERSION_OFFSET_V1,
            &generated::DIRECT_SUCCESSOR_ABI_VERSION_V1.to_le_bytes(),
        );
        put(
            &mut output,
            generated::DIRECT_CONFIG_PRICE_SCALE_OFFSET_V1,
            &self.price_scale.to_le_bytes(),
        );
        put(
            &mut output,
            generated::DIRECT_CONFIG_FEE_BPS_OFFSET_V1,
            &self.fee_basis_points.to_le_bytes(),
        );
        put(
            &mut output,
            generated::DIRECT_CONFIG_FEE_RECIPIENT_OFFSET_V1,
            &self.fee_recipient,
        );
        output
    }

    /// Exact positive quote scale projected into the semantic register bank.
    pub const fn price_scale(self) -> u64 {
        self.price_scale
    }

    /// Exact maker-accepted venue fee rate.
    pub const fn fee_basis_points(self) -> u16 {
        self.fee_basis_points
    }

    /// Exact external collateral recipient for realized venue fees.
    pub const fn fee_recipient(self) -> [u8; 32] {
        self.fee_recipient
    }
}

/// Canonical signed Direct side tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectSideV1 {
    /// Claims leave the maker; collateral returns to the signed destination.
    Sell,
    /// Collateral leaves the signed source; claims return to the maker Position.
    Buy,
}

impl DirectSideV1 {
    fn decode(value: u8) -> SuccessorResult<Self> {
        match value {
            0 => Ok(Self::Sell),
            1 => Ok(Self::Buy),
            _ => Err(SuccessorError::InvalidIntent),
        }
    }
}

/// Canonical signed Direct execution lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectLifecycleV1 {
    /// Inline execution must consume the full signed maximum.
    InlineFillOrKill,
    /// Inline execution may consume any positive amount through the signed maximum.
    InlineImmediateOrCancel,
    /// Registration may rest and be filled partially until cancel or expiry.
    Registered,
}

impl DirectLifecycleV1 {
    fn decode(value: u8) -> SuccessorResult<Self> {
        match value {
            0 => Ok(Self::InlineFillOrKill),
            1 => Ok(Self::InlineImmediateOrCancel),
            2 => Ok(Self::Registered),
            _ => Err(SuccessorError::InvalidIntent),
        }
    }
}

/// Exact compact intent after the adapter authenticates the immediately
/// preceding native Ed25519 instruction and signer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedCompactIntentV1 {
    maker: [u8; 32],
    intent: CompactIntentV1,
}

impl AuthenticatedCompactIntentV1 {
    /// Seal an adapter-authenticated signer/message pair.
    ///
    /// This is a trust-boundary constructor, not a signature verifier. The
    /// Trading adapter must call it only after proving exact native-program
    /// identity, adjacency, descriptor offsets, and message equality.
    pub fn from_adjacent_ed25519(
        maker: [u8; 32],
        intent: CompactIntentV1,
    ) -> SuccessorResult<Self> {
        require_nonzero(maker)?;
        Ok(Self { maker, intent })
    }

    /// Verified maker public key.
    pub const fn maker(self) -> [u8; 32] {
        self.maker
    }

    /// Exact signed CompactIntent bytes decoded into their sole DTO.
    pub const fn intent(self) -> CompactIntentV1 {
        self.intent
    }

    /// Project only the replay coordinate used by the maker-root transition.
    pub fn replay(self) -> SuccessorResult<AuthenticatedIntentReplayV1> {
        AuthenticatedIntentReplayV1::from_signed_intent(self.maker, self.intent)
    }
}

/// Global Direct lifecycle inside one composite Trading capability root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRootPhaseV1 {
    /// New inline nonces and registered intents are admitted.
    Open,
    /// New nonces are permanently refused while maker roots drain and close.
    Retiring,
}

impl DirectRootPhaseV1 {
    fn byte(self) -> u8 {
        match self {
            Self::Open => 0,
            Self::Retiring => 1,
        }
    }

    fn decode(value: u8) -> SuccessorResult<Self> {
        match value {
            0 => Ok(Self::Open),
            1 => Ok(Self::Retiring),
            _ => Err(SuccessorError::InvalidRootPhase),
        }
    }
}

/// Sole global mutable Direct state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRootStateV1 {
    phase: DirectRootPhaseV1,
    open_maker_root_count: u64,
}

impl DirectRootStateV1 {
    /// Initial open state with no maker replay accounts.
    pub const fn new() -> Self {
        Self {
            phase: DirectRootPhaseV1::Open,
            open_maker_root_count: 0,
        }
    }

    /// Hostile-decode one exact mutable root tail.
    pub fn decode(input: &[u8]) -> SuccessorResult<Self> {
        exact_width(input, DIRECT_ROOT_STATE_BYTES_V1)?;
        exact(
            input,
            generated::DIRECT_ROOT_MAGIC_OFFSET_V1,
            &generated::DIRECT_ROOT_MAGIC_V1,
        )?;
        version(input, generated::DIRECT_ROOT_VERSION_OFFSET_V1)?;
        zero_range(input, generated::DIRECT_ROOT_RESERVED_OFFSET_V1, 5)?;
        Ok(Self {
            phase: DirectRootPhaseV1::decode(byte_at(
                input,
                generated::DIRECT_ROOT_PHASE_OFFSET_V1,
            )?)?,
            open_maker_root_count: u64_at(
                input,
                generated::DIRECT_ROOT_OPEN_MAKER_COUNT_OFFSET_V1,
            )?,
        })
    }

    /// Encode one exact mutable root tail.
    pub fn encode(self) -> [u8; DIRECT_ROOT_STATE_BYTES_V1] {
        let mut output = [0_u8; DIRECT_ROOT_STATE_BYTES_V1];
        put(
            &mut output,
            generated::DIRECT_ROOT_MAGIC_OFFSET_V1,
            &generated::DIRECT_ROOT_MAGIC_V1,
        );
        put(
            &mut output,
            generated::DIRECT_ROOT_VERSION_OFFSET_V1,
            &generated::DIRECT_SUCCESSOR_ABI_VERSION_V1.to_le_bytes(),
        );
        put_byte(
            &mut output,
            generated::DIRECT_ROOT_PHASE_OFFSET_V1,
            self.phase.byte(),
        );
        put(
            &mut output,
            generated::DIRECT_ROOT_OPEN_MAKER_COUNT_OFFSET_V1,
            &self.open_maker_root_count.to_le_bytes(),
        );
        output
    }

    /// Global admission phase.
    pub const fn phase(self) -> DirectRootPhaseV1 {
        self.phase
    }

    /// Exact number of maker replay accounts not yet closed.
    pub const fn open_maker_root_count(self) -> u64 {
        self.open_maker_root_count
    }

    /// Irreversibly stop new maker nonce consumption.
    pub fn begin_retiring(self) -> SuccessorResult<Self> {
        if self.phase != DirectRootPhaseV1::Open {
            return Err(SuccessorError::InvalidRootPhase);
        }
        Ok(Self {
            phase: DirectRootPhaseV1::Retiring,
            ..self
        })
    }

    /// Refuse physical root closure until retirement and zero maker roots.
    pub fn require_closable(self) -> SuccessorResult<()> {
        if self.phase == DirectRootPhaseV1::Retiring && self.open_maker_root_count == 0 {
            Ok(())
        } else {
            Err(SuccessorError::MakerRootCountInvariant)
        }
    }
}

impl Default for DirectRootStateV1 {
    fn default() -> Self {
        Self::new()
    }
}

/// Market/generation binding supplied by the authenticated common root header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCoordinatesV1 {
    market: [u8; 32],
    generation: u64,
}

impl DirectCoordinatesV1 {
    /// Construct exact immutable common-root coordinates.
    pub fn new(market: [u8; 32], generation: u64) -> SuccessorResult<Self> {
        require_nonzero(market)?;
        Ok(Self { market, generation })
    }

    /// Market account identity.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// Canonical PDA seeds for one maker replay root under Trading.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MakerReplaySeedsV1 {
    market: [u8; 32],
    generation: [u8; 8],
    maker: [u8; 32],
}

impl MakerReplaySeedsV1 {
    /// Project the exact replay coordinate.
    pub fn new(coordinates: DirectCoordinatesV1, maker: [u8; 32]) -> SuccessorResult<Self> {
        require_nonzero(maker)?;
        Ok(Self {
            market: coordinates.market,
            generation: coordinates.generation.to_le_bytes(),
            maker,
        })
    }

    /// Borrow ordered seeds excluding the bump.
    pub fn as_slices(&self) -> [&[u8]; 4] {
        [
            DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1,
            &self.market,
            &self.generation,
            &self.maker,
        ]
    }
}

/// Per-maker replay, live-intent, cancel-through, and rent owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MakerReplayRootV1 {
    market: [u8; 32],
    generation: u64,
    maker: [u8; 32],
    next_nonce: u64,
    live_count: u64,
    minimum_live_nonce: u64,
    rent_owner: [u8; 32],
    rent_principal: u64,
    bump: u8,
}

impl MakerReplayRootV1 {
    fn new(
        coordinates: DirectCoordinatesV1,
        maker: [u8; 32],
        rent_owner: [u8; 32],
        rent_principal: u64,
        bump: u8,
    ) -> SuccessorResult<Self> {
        require_nonzero(maker)?;
        require_nonzero(rent_owner)?;
        if rent_principal == 0 {
            return Err(SuccessorError::InvalidRent);
        }
        Ok(Self {
            market: coordinates.market,
            generation: coordinates.generation,
            maker,
            next_nonce: 0,
            live_count: 0,
            minimum_live_nonce: 0,
            rent_owner,
            rent_principal,
            bump,
        })
    }

    /// Hostile-decode one exact Trading-owned replay account.
    pub fn decode(input: &[u8]) -> SuccessorResult<Self> {
        exact_width(input, DIRECT_MAKER_REPLAY_BYTES_V1)?;
        exact(
            input,
            generated::DIRECT_MAKER_MAGIC_OFFSET_V1,
            &generated::DIRECT_MAKER_MAGIC_V1,
        )?;
        version(input, generated::DIRECT_MAKER_VERSION_OFFSET_V1)?;
        zero_range(input, generated::DIRECT_MAKER_RESERVED_OFFSET_V1, 5)?;
        let value = Self {
            market: array_at(input, generated::DIRECT_MAKER_MARKET_OFFSET_V1)?,
            generation: u64_at(input, generated::DIRECT_MAKER_GENERATION_OFFSET_V1)?,
            maker: array_at(input, generated::DIRECT_MAKER_IDENTITY_OFFSET_V1)?,
            next_nonce: u64_at(input, generated::DIRECT_MAKER_NEXT_NONCE_OFFSET_V1)?,
            live_count: u64_at(input, generated::DIRECT_MAKER_LIVE_COUNT_OFFSET_V1)?,
            minimum_live_nonce: u64_at(
                input,
                generated::DIRECT_MAKER_MINIMUM_LIVE_NONCE_OFFSET_V1,
            )?,
            rent_owner: array_at(input, generated::DIRECT_MAKER_RENT_OWNER_OFFSET_V1)?,
            rent_principal: u64_at(input, generated::DIRECT_MAKER_RENT_PRINCIPAL_OFFSET_V1)?,
            bump: byte_at(input, generated::DIRECT_MAKER_BUMP_OFFSET_V1)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the exact persisted replay state.
    pub fn encode(self) -> SuccessorResult<[u8; DIRECT_MAKER_REPLAY_BYTES_V1]> {
        self.validate()?;
        let mut output = [0_u8; DIRECT_MAKER_REPLAY_BYTES_V1];
        put(
            &mut output,
            generated::DIRECT_MAKER_MAGIC_OFFSET_V1,
            &generated::DIRECT_MAKER_MAGIC_V1,
        );
        put(
            &mut output,
            generated::DIRECT_MAKER_VERSION_OFFSET_V1,
            &generated::DIRECT_SUCCESSOR_ABI_VERSION_V1.to_le_bytes(),
        );
        put_byte(
            &mut output,
            generated::DIRECT_MAKER_BUMP_OFFSET_V1,
            self.bump,
        );
        put(
            &mut output,
            generated::DIRECT_MAKER_MARKET_OFFSET_V1,
            &self.market,
        );
        put(
            &mut output,
            generated::DIRECT_MAKER_GENERATION_OFFSET_V1,
            &self.generation.to_le_bytes(),
        );
        put(
            &mut output,
            generated::DIRECT_MAKER_IDENTITY_OFFSET_V1,
            &self.maker,
        );
        put(
            &mut output,
            generated::DIRECT_MAKER_NEXT_NONCE_OFFSET_V1,
            &self.next_nonce.to_le_bytes(),
        );
        put(
            &mut output,
            generated::DIRECT_MAKER_LIVE_COUNT_OFFSET_V1,
            &self.live_count.to_le_bytes(),
        );
        put(
            &mut output,
            generated::DIRECT_MAKER_MINIMUM_LIVE_NONCE_OFFSET_V1,
            &self.minimum_live_nonce.to_le_bytes(),
        );
        put(
            &mut output,
            generated::DIRECT_MAKER_RENT_OWNER_OFFSET_V1,
            &self.rent_owner,
        );
        put(
            &mut output,
            generated::DIRECT_MAKER_RENT_PRINCIPAL_OFFSET_V1,
            &self.rent_principal.to_le_bytes(),
        );
        Ok(output)
    }

    fn validate(self) -> SuccessorResult<()> {
        require_nonzero(self.market)?;
        require_nonzero(self.maker)?;
        require_nonzero(self.rent_owner)?;
        if self.rent_principal == 0 {
            return Err(SuccessorError::InvalidRent);
        }
        if self.live_count > self.next_nonce {
            return Err(SuccessorError::LiveCountInvariant);
        }
        if self.minimum_live_nonce > self.next_nonce {
            return Err(SuccessorError::MinimumLiveNonceInvariant);
        }
        Ok(())
    }

    fn validate_coordinate(
        self,
        coordinates: DirectCoordinatesV1,
        maker: [u8; 32],
    ) -> SuccessorResult<()> {
        self.validate()?;
        if self.market != coordinates.market
            || self.generation != coordinates.generation
            || self.maker != maker
        {
            Err(SuccessorError::MakerCoordinateMismatch)
        } else {
            Ok(())
        }
    }

    /// Exact next signed nonce.
    pub const fn next_nonce(self) -> u64 {
        self.next_nonce
    }

    /// Number of registered intent records not yet closed.
    pub const fn live_count(self) -> u64 {
        self.live_count
    }

    /// Lowest nonce still eligible for a registered fill.
    pub const fn minimum_live_nonce(self) -> u64 {
        self.minimum_live_nonce
    }

    /// Immutable RentCredit beneficiary for this account.
    pub const fn rent_owner(self) -> [u8; 32] {
        self.rent_owner
    }

    /// Historical account-rent principal, never recomputed at close.
    pub const fn rent_principal(self) -> u64 {
        self.rent_principal
    }

    /// Stored canonical PDA bump.
    pub const fn bump(self) -> u8 {
        self.bump
    }

    /// Close one live registered record after every child resource closes.
    pub fn close_live(self) -> SuccessorResult<Self> {
        self.validate()?;
        let live_count = self
            .live_count
            .checked_sub(1)
            .ok_or(SuccessorError::LiveCountInvariant)?;
        Ok(Self { live_count, ..self })
    }

    /// Monotonically invalidate all registered nonces below `minimum`.
    fn cancel_through(self, minimum: u64) -> SuccessorResult<Self> {
        self.validate()?;
        if minimum <= self.minimum_live_nonce || minimum > self.next_nonce {
            return Err(SuccessorError::MinimumLiveNonceInvariant);
        }
        Ok(Self {
            minimum_live_nonce: minimum,
            ..self
        })
    }
}

/// Signature-authenticated replay projection of the sole compact intent DTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedIntentReplayV1 {
    coordinates: DirectCoordinatesV1,
    maker: [u8; 32],
    nonce: u64,
}

impl AuthenticatedIntentReplayV1 {
    /// Project replay facts from the exact compact message and verified signer.
    ///
    /// Constructing this value does not itself verify a signature. Trading must
    /// expose it only after adjacent native-Ed25519 authentication of `intent`.
    pub fn from_signed_intent(maker: [u8; 32], intent: CompactIntentV1) -> SuccessorResult<Self> {
        require_nonzero(maker)?;
        Ok(Self {
            coordinates: DirectCoordinatesV1::new(intent.market, intent.generation)?,
            maker,
            nonce: intent.nonce,
        })
    }

    /// Authenticated common Market/generation tuple.
    pub const fn coordinates(self) -> DirectCoordinatesV1 {
        self.coordinates
    }

    /// Verified native Ed25519 public key.
    pub const fn maker(self) -> [u8; 32] {
        self.maker
    }

    /// Exact gap-free signed nonce.
    pub const fn nonce(self) -> u64 {
        self.nonce
    }
}

/// Whether nonce consumption creates a live registered record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NonceConsumptionV1 {
    /// Immediate execution consumes the nonce without a live record.
    Inline,
    /// Resting registration consumes the nonce and creates one live record.
    Register,
}

/// Authenticated vacancy facts for the exact maker-root PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MakerReplayVacancyV1 {
    bump: u8,
    observed_lamports: u64,
}

impl MakerReplayVacancyV1 {
    /// Construct after the adapter proves exact PDA, System owner, and empty data.
    pub const fn new(bump: u8, observed_lamports: u64) -> Self {
        Self {
            bump,
            observed_lamports,
        }
    }
}

/// Existing or authenticated-vacant maker replay account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MakerReplayObservationV1 {
    /// Existing exact Trading-owned replay state.
    Existing(MakerReplayRootV1),
    /// Exact PDA is an empty System-owned account, with arbitrary dust.
    Vacant(MakerReplayVacancyV1),
}

/// First-use rent funding persisted in the maker root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MakerReplayFirstUseV1 {
    /// Immutable RentCredit beneficiary.
    pub rent_owner: [u8; 32],
    /// Exact historical account-rent principal.
    pub rent_principal: u64,
}

/// Dust-tolerant account creation plan for a vacant maker root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MakerReplayCreationPlanV1 {
    /// Existing system-account dust observed before creation.
    pub observed_lamports: u64,
    /// Exact payer debit necessary to reach historical rent principal.
    pub top_up_lamports: u64,
    /// Required balance after allocate/assign.
    pub post_lamports: u64,
}

/// Atomic state candidates after one signed nonce is consumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonceConsumptionResultV1 {
    /// Global root after an optional first-use count increment.
    pub root: DirectRootStateV1,
    /// Per-maker replay root after exact nonce/live transition.
    pub maker_root: MakerReplayRootV1,
    /// Present exactly for dust-tolerant first use.
    pub creation: Option<MakerReplayCreationPlanV1>,
}

/// Consume one inline or registered nonce atomically.
pub fn consume_nonce_v1(
    root: DirectRootStateV1,
    observation: MakerReplayObservationV1,
    intent: AuthenticatedIntentReplayV1,
    consumption: NonceConsumptionV1,
    first_use: Option<MakerReplayFirstUseV1>,
) -> SuccessorResult<NonceConsumptionResultV1> {
    if root.phase != DirectRootPhaseV1::Open {
        return Err(SuccessorError::InvalidRootPhase);
    }
    let (root_after, mut maker_root, creation) = match (observation, first_use) {
        (MakerReplayObservationV1::Vacant(vacancy), Some(funding)) => {
            if intent.nonce != 0 {
                return Err(SuccessorError::NonceMismatch);
            }
            require_nonzero(funding.rent_owner)?;
            if funding.rent_principal == 0 {
                return Err(SuccessorError::InvalidRent);
            }
            let count = root
                .open_maker_root_count
                .checked_add(1)
                .ok_or(SuccessorError::MakerRootCountInvariant)?;
            let top_up_lamports = funding
                .rent_principal
                .saturating_sub(vacancy.observed_lamports);
            let post_lamports = core::cmp::max(vacancy.observed_lamports, funding.rent_principal);
            (
                DirectRootStateV1 {
                    open_maker_root_count: count,
                    ..root
                },
                MakerReplayRootV1::new(
                    intent.coordinates,
                    intent.maker,
                    funding.rent_owner,
                    funding.rent_principal,
                    vacancy.bump,
                )?,
                Some(MakerReplayCreationPlanV1 {
                    observed_lamports: vacancy.observed_lamports,
                    top_up_lamports,
                    post_lamports,
                }),
            )
        }
        (MakerReplayObservationV1::Existing(existing), None) => {
            if root.open_maker_root_count == 0 {
                return Err(SuccessorError::MakerRootCountInvariant);
            }
            existing.validate_coordinate(intent.coordinates, intent.maker)?;
            (root, existing, None)
        }
        _ => return Err(SuccessorError::InvalidFirstUse),
    };
    if maker_root.next_nonce != intent.nonce {
        return Err(SuccessorError::NonceMismatch);
    }
    maker_root.next_nonce = maker_root
        .next_nonce
        .checked_add(1)
        .ok_or(SuccessorError::NonceMismatch)?;
    if consumption == NonceConsumptionV1::Register {
        maker_root.live_count = maker_root
            .live_count
            .checked_add(1)
            .ok_or(SuccessorError::LiveCountInvariant)?;
    }
    maker_root.validate()?;
    Ok(NonceConsumptionResultV1 {
        root: root_after,
        maker_root,
        creation,
    })
}

/// Canonical PDA seeds for one live registered intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredIntentSeedsV1 {
    market: [u8; 32],
    generation: [u8; 8],
    maker: [u8; 32],
    nonce: [u8; 8],
}

impl RegisteredIntentSeedsV1 {
    /// Project the exact live-record coordinate from an authenticated intent.
    pub fn new(authenticated: AuthenticatedCompactIntentV1) -> SuccessorResult<Self> {
        let replay = authenticated.replay()?;
        Ok(Self {
            market: replay.coordinates.market,
            generation: replay.coordinates.generation.to_le_bytes(),
            maker: replay.maker,
            nonce: replay.nonce.to_le_bytes(),
        })
    }

    /// Borrow ordered seeds excluding the bump.
    pub fn as_slices(&self) -> [&[u8]; 5] {
        [
            DIRECT_REGISTERED_RECORD_PDA_DOMAIN_V1,
            &self.market,
            &self.generation,
            &self.maker,
            &self.nonce,
        ]
    }
}

/// Sole live registered-intent, custody-reservation, and cumulative-fee state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRegisteredIntentV1 {
    maker: [u8; 32],
    intent: CompactIntentV1,
    filled: u64,
    reserved_claims: u64,
    reserved_collateral: u64,
    cumulative_gross: u64,
    cumulative_fee: u64,
    rent_owner: [u8; 32],
    rent_principal: u64,
    bump: u8,
}

impl DirectRegisteredIntentV1 {
    /// Hostile-decode a live record against the immutable selected config and Product width.
    pub fn decode_selected(
        config: DirectExecutionConfigV1,
        outcome_count: u16,
        input: &[u8],
    ) -> SuccessorResult<Self> {
        exact_width(input, DIRECT_REGISTERED_RECORD_BYTES_V1)?;
        exact(
            input,
            generated::DIRECT_RECORD_MAGIC_OFFSET_V1,
            &generated::DIRECT_RECORD_MAGIC_V1,
        )?;
        version(input, generated::DIRECT_RECORD_VERSION_OFFSET_V1)?;
        zero_range(input, generated::DIRECT_RECORD_RESERVED_OFFSET_V1, 5)?;
        let intent_bytes = slice(
            input,
            generated::DIRECT_RECORD_INTENT_OFFSET_V1,
            crate::COMPACT_INTENT_BYTES,
        )?;
        let value = Self {
            maker: array_at(input, generated::DIRECT_RECORD_MAKER_OFFSET_V1)?,
            intent: CompactIntentV1::decode(intent_bytes)
                .map_err(|_| SuccessorError::InvalidIntent)?,
            filled: u64_at(input, generated::DIRECT_RECORD_FILLED_OFFSET_V1)?,
            reserved_claims: u64_at(input, generated::DIRECT_RECORD_RESERVED_CLAIMS_OFFSET_V1)?,
            reserved_collateral: u64_at(
                input,
                generated::DIRECT_RECORD_RESERVED_COLLATERAL_OFFSET_V1,
            )?,
            cumulative_gross: u64_at(input, generated::DIRECT_RECORD_CUMULATIVE_GROSS_OFFSET_V1)?,
            cumulative_fee: u64_at(input, generated::DIRECT_RECORD_CUMULATIVE_FEE_OFFSET_V1)?,
            rent_owner: array_at(input, generated::DIRECT_RECORD_RENT_OWNER_OFFSET_V1)?,
            rent_principal: u64_at(input, generated::DIRECT_RECORD_RENT_PRINCIPAL_OFFSET_V1)?,
            bump: byte_at(input, generated::DIRECT_RECORD_BUMP_OFFSET_V1)?,
        };
        value.validate(config, outcome_count)?;
        Ok(value)
    }

    /// Encode one exact live record after revalidating every economic invariant.
    pub fn encode_selected(
        self,
        config: DirectExecutionConfigV1,
        outcome_count: u16,
    ) -> SuccessorResult<[u8; DIRECT_REGISTERED_RECORD_BYTES_V1]> {
        self.validate(config, outcome_count)?;
        let mut output = [0_u8; DIRECT_REGISTERED_RECORD_BYTES_V1];
        put(
            &mut output,
            generated::DIRECT_RECORD_MAGIC_OFFSET_V1,
            &generated::DIRECT_RECORD_MAGIC_V1,
        );
        put(
            &mut output,
            generated::DIRECT_RECORD_VERSION_OFFSET_V1,
            &generated::DIRECT_SUCCESSOR_ABI_VERSION_V1.to_le_bytes(),
        );
        put_byte(
            &mut output,
            generated::DIRECT_RECORD_BUMP_OFFSET_V1,
            self.bump,
        );
        put(
            &mut output,
            generated::DIRECT_RECORD_MAKER_OFFSET_V1,
            &self.maker,
        );
        put(
            &mut output,
            generated::DIRECT_RECORD_INTENT_OFFSET_V1,
            &self
                .intent
                .encode()
                .map_err(|_| SuccessorError::InvalidIntent)?,
        );
        put_u64(
            &mut output,
            generated::DIRECT_RECORD_FILLED_OFFSET_V1,
            self.filled,
        );
        put_u64(
            &mut output,
            generated::DIRECT_RECORD_RESERVED_CLAIMS_OFFSET_V1,
            self.reserved_claims,
        );
        put_u64(
            &mut output,
            generated::DIRECT_RECORD_RESERVED_COLLATERAL_OFFSET_V1,
            self.reserved_collateral,
        );
        put_u64(
            &mut output,
            generated::DIRECT_RECORD_CUMULATIVE_GROSS_OFFSET_V1,
            self.cumulative_gross,
        );
        put_u64(
            &mut output,
            generated::DIRECT_RECORD_CUMULATIVE_FEE_OFFSET_V1,
            self.cumulative_fee,
        );
        put(
            &mut output,
            generated::DIRECT_RECORD_RENT_OWNER_OFFSET_V1,
            &self.rent_owner,
        );
        put_u64(
            &mut output,
            generated::DIRECT_RECORD_RENT_PRINCIPAL_OFFSET_V1,
            self.rent_principal,
        );
        Ok(output)
    }

    fn validate(self, config: DirectExecutionConfigV1, outcome_count: u16) -> SuccessorResult<()> {
        validate_intent_v1(
            config,
            self.intent,
            outcome_count,
            DirectLifecycleV1::Registered,
        )?;
        require_nonzero(self.maker)?;
        require_nonzero(self.rent_owner)?;
        if self.rent_principal == 0 || self.filled >= self.intent.maximum_fill {
            return Err(SuccessorError::InvalidReservation);
        }
        if self.cumulative_gross > self.filled
            || self.cumulative_fee != fee_floor_v1(self.cumulative_gross, config.fee_basis_points)?
        {
            return Err(SuccessorError::InvalidReservation);
        }
        let remaining = self
            .intent
            .maximum_fill
            .checked_sub(self.filled)
            .ok_or(SuccessorError::InvalidReservation)?;
        match DirectSideV1::decode(self.intent.side)? {
            DirectSideV1::Sell => {
                if self.reserved_claims != remaining || self.reserved_collateral != 0 {
                    return Err(SuccessorError::InvalidReservation);
                }
            }
            DirectSideV1::Buy => {
                let initial = maximum_buy_reserve_v1(config, self.intent)?;
                let spent = self
                    .cumulative_gross
                    .checked_add(self.cumulative_fee)
                    .ok_or(SuccessorError::ArithmeticOverflow)?;
                let expected = initial
                    .checked_sub(spent)
                    .ok_or(SuccessorError::InvalidReservation)?;
                if self.reserved_claims != 0 || self.reserved_collateral != expected {
                    return Err(SuccessorError::InvalidReservation);
                }
            }
        }
        Ok(())
    }

    fn validate_coordinate(self, root: MakerReplayRootV1) -> SuccessorResult<()> {
        root.validate()?;
        if self.intent.market != root.market
            || self.intent.generation != root.generation
            || self.maker != root.maker
            || self.intent.nonce >= root.next_nonce
        {
            Err(SuccessorError::RecordCoordinateMismatch)
        } else {
            Ok(())
        }
    }

    /// Verified maker identity.
    pub const fn maker(self) -> [u8; 32] {
        self.maker
    }

    /// Sole persisted signed intent.
    pub const fn intent(self) -> CompactIntentV1 {
        self.intent
    }

    /// Aggregate executed quantity.
    pub const fn filled(self) -> u64 {
        self.filled
    }

    /// Claims still held by the live Sell record.
    pub const fn reserved_claims(self) -> u64 {
        self.reserved_claims
    }

    /// Collateral still held by the live Buy record.
    pub const fn reserved_collateral(self) -> u64 {
        self.reserved_collateral
    }

    /// Aggregate executed gross across every partial fill.
    pub const fn cumulative_gross(self) -> u64 {
        self.cumulative_gross
    }

    /// Cumulative floor fee, charged by differences between successive totals.
    pub const fn cumulative_fee(self) -> u64 {
        self.cumulative_fee
    }

    /// Immutable live-record RentCredit beneficiary.
    pub const fn rent_owner(self) -> [u8; 32] {
        self.rent_owner
    }

    /// Historical record-rent principal.
    pub const fn rent_principal(self) -> u64 {
        self.rent_principal
    }

    /// Stored canonical PDA bump.
    pub const fn bump(self) -> u8 {
        self.bump
    }
}

/// Dust-tolerant first-use facts for one live registered record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredRecordFirstUseV1 {
    /// Canonical PDA bump.
    pub bump: u8,
    /// Existing lamports on the empty System-owned candidate.
    pub observed_lamports: u64,
    /// Immutable RentCredit beneficiary.
    pub rent_owner: [u8; 32],
    /// Historical account-rent principal.
    pub rent_principal: u64,
}

/// Atomic candidates from a maker-authorized resting registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredIntentCreationV1 {
    /// Global root after possible maker-root first use.
    pub root: DirectRootStateV1,
    /// Maker root after nonce consumption and live-count increment.
    pub maker_root: MakerReplayRootV1,
    /// Sole live registered record.
    pub record: DirectRegisteredIntentV1,
    /// Present only when the maker replay root is first created.
    pub maker_creation: Option<MakerReplayCreationPlanV1>,
    /// Exact live-record creation funding.
    pub record_creation: MakerReplayCreationPlanV1,
}

/// Register one authenticated CompactIntent and reserve its exact worst case.
pub fn register_intent_v1(
    root: DirectRootStateV1,
    maker_observation: MakerReplayObservationV1,
    authenticated: AuthenticatedCompactIntentV1,
    config: DirectExecutionConfigV1,
    outcome_count: u16,
    maker_first_use: Option<MakerReplayFirstUseV1>,
    record_first_use: RegisteredRecordFirstUseV1,
) -> SuccessorResult<RegisteredIntentCreationV1> {
    validate_intent_v1(
        config,
        authenticated.intent,
        outcome_count,
        DirectLifecycleV1::Registered,
    )?;
    require_nonzero(record_first_use.rent_owner)?;
    if record_first_use.rent_principal == 0 {
        return Err(SuccessorError::InvalidRent);
    }
    let consumed = consume_nonce_v1(
        root,
        maker_observation,
        authenticated.replay()?,
        NonceConsumptionV1::Register,
        maker_first_use,
    )?;
    let (reserved_claims, reserved_collateral) =
        match DirectSideV1::decode(authenticated.intent.side)? {
            DirectSideV1::Sell => (authenticated.intent.maximum_fill, 0),
            DirectSideV1::Buy => (0, maximum_buy_reserve_v1(config, authenticated.intent)?),
        };
    let record = DirectRegisteredIntentV1 {
        maker: authenticated.maker,
        intent: authenticated.intent,
        filled: 0,
        reserved_claims,
        reserved_collateral,
        cumulative_gross: 0,
        cumulative_fee: 0,
        rent_owner: record_first_use.rent_owner,
        rent_principal: record_first_use.rent_principal,
        bump: record_first_use.bump,
    };
    record.validate(config, outcome_count)?;
    let top_up_lamports = record_first_use
        .rent_principal
        .saturating_sub(record_first_use.observed_lamports);
    Ok(RegisteredIntentCreationV1 {
        root: consumed.root,
        maker_root: consumed.maker_root,
        record,
        maker_creation: consumed.creation,
        record_creation: MakerReplayCreationPlanV1 {
            observed_lamports: record_first_use.observed_lamports,
            top_up_lamports,
            post_lamports: core::cmp::max(
                record_first_use.observed_lamports,
                record_first_use.rent_principal,
            ),
        },
    })
}

/// Exact asset and rent disposition when one live record closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredRecordCloseV1 {
    /// Signed nonce whose replay remains rejected by the maker root.
    pub closed_nonce: u64,
    /// Remaining Sell claims returned to the maker Position.
    pub claim_refund: u64,
    /// Remaining Buy collateral returned to the signed source account.
    pub collateral_refund: u64,
    /// Immutable record RentCredit beneficiary.
    pub rent_owner: [u8; 32],
    /// Historical rent principal.
    pub rent_principal: u64,
    /// Lamports above principal, never classified as fees or reserves.
    pub unclassified_donation: u64,
    /// Total native credit to the persisted RentCredit.
    pub total_rent_credit: u64,
}

/// Live replacement or terminal close; never both.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisteredRecordAfterFillV1 {
    /// Partial fill leaves one validated live record.
    Live(DirectRegisteredIntentV1),
    /// Full fill closes the live record and returns residual assets/rent.
    Closed(RegisteredRecordCloseV1),
}

/// One participant's exact claim/collateral effects after a checked fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredParticipantEffectsV1 {
    /// Claims debited from registered Sell custody.
    pub claim_custody_debit: u64,
    /// Claims credited to a Buy maker Position.
    pub claim_position_credit: u64,
    /// Gross collateral debited from registered Buy custody.
    pub gross_collateral_debit: u64,
    /// Gross collateral allocated to a Sell maker before fee withholding.
    pub gross_collateral_credit: u64,
    /// Difference-of-rounded fee charged for this partial fill.
    pub fee_transfer: u64,
    /// Net Sell collateral credit after fee withholding.
    pub net_collateral_credit: u64,
}

/// Complete candidate for one participant; physical state remains unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredFillCandidateV1 {
    /// Maker replay root after optional live-count decrement.
    pub maker_root: MakerReplayRootV1,
    /// Live replacement or terminal disposition.
    pub record: RegisteredRecordAfterFillV1,
    /// Exact effect projection for Claims and distinct-owner Custody requests.
    pub effects: RegisteredParticipantEffectsV1,
}

/// Common checked execution facts for one registered participant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredExecutionV1 {
    /// Immutable descriptor-selected economics.
    pub config: DirectExecutionConfigV1,
    /// Authenticated Product result-domain width.
    pub outcome_count: u16,
    /// Trusted `Clock::get()` slot.
    pub slot: u64,
    /// Positive matcher-selected quantity.
    pub fill: u64,
    /// Exact scaled execution price.
    pub execution_price: u64,
}

/// One live participant observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredParticipantV1 {
    /// Exact maker replay root.
    pub maker_root: MakerReplayRootV1,
    /// Exact live record.
    pub record: DirectRegisteredIntentV1,
    /// Current record lamports used only if the fill closes it.
    pub observed_record_lamports: u64,
}

/// Complete preview input for one participant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredFillInputV1 {
    /// Global Direct root.
    pub root: DirectRootStateV1,
    /// Live participant observation.
    pub participant: RegisteredParticipantV1,
    /// Common execution facts.
    pub execution: RegisteredExecutionV1,
}

/// Preview one registered partial/full fill with all arithmetic checked.
pub fn preview_registered_fill_v1(
    input: RegisteredFillInputV1,
) -> SuccessorResult<RegisteredFillCandidateV1> {
    let root = input.root;
    let maker_root = input.participant.maker_root;
    let record = input.participant.record;
    let config = input.execution.config;
    let outcome_count = input.execution.outcome_count;
    let slot = input.execution.slot;
    let fill = input.execution.fill;
    let execution_price = input.execution.execution_price;
    let observed_record_lamports = input.participant.observed_record_lamports;
    if root.phase != DirectRootPhaseV1::Open || root.open_maker_root_count == 0 {
        return Err(SuccessorError::InvalidRootPhase);
    }
    record.validate(config, outcome_count)?;
    record.validate_coordinate(maker_root)?;
    if record.intent.nonce < maker_root.minimum_live_nonce {
        return Err(SuccessorError::InvalidTerminal);
    }
    if slot < record.intent.valid_from || slot > record.intent.valid_through {
        return Err(SuccessorError::IntentExpired);
    }
    if fill == 0 {
        return Err(SuccessorError::InvalidIntent);
    }
    let side = DirectSideV1::decode(record.intent.side)?;
    match side {
        DirectSideV1::Sell if execution_price < record.intent.limit_price => {
            return Err(SuccessorError::IncompatibleMatch);
        }
        DirectSideV1::Buy if execution_price > record.intent.limit_price => {
            return Err(SuccessorError::IncompatibleMatch);
        }
        DirectSideV1::Sell | DirectSideV1::Buy => {}
    }
    if execution_price > config.price_scale {
        return Err(SuccessorError::IncompatibleMatch);
    }
    let filled = record
        .filled
        .checked_add(fill)
        .ok_or(SuccessorError::ArithmeticOverflow)?;
    if filled > record.intent.maximum_fill {
        return Err(SuccessorError::InvalidReservation);
    }
    let gross = exact_quote_v1(fill, execution_price, config.price_scale)?;
    let cumulative_gross = record
        .cumulative_gross
        .checked_add(gross)
        .ok_or(SuccessorError::ArithmeticOverflow)?;
    let cumulative_fee = fee_floor_v1(cumulative_gross, config.fee_basis_points)?;
    let fee_delta = cumulative_fee
        .checked_sub(record.cumulative_fee)
        .ok_or(SuccessorError::InvalidReservation)?;
    let (reserved_claims, reserved_collateral, effects) = match side {
        DirectSideV1::Sell => {
            let claims = record
                .reserved_claims
                .checked_sub(fill)
                .ok_or(SuccessorError::InvalidReservation)?;
            let net = gross
                .checked_sub(fee_delta)
                .ok_or(SuccessorError::InvalidReservation)?;
            (
                claims,
                0,
                RegisteredParticipantEffectsV1 {
                    claim_custody_debit: fill,
                    claim_position_credit: 0,
                    gross_collateral_debit: 0,
                    gross_collateral_credit: gross,
                    fee_transfer: fee_delta,
                    net_collateral_credit: net,
                },
            )
        }
        DirectSideV1::Buy => {
            let debit = gross
                .checked_add(fee_delta)
                .ok_or(SuccessorError::ArithmeticOverflow)?;
            let collateral = record
                .reserved_collateral
                .checked_sub(debit)
                .ok_or(SuccessorError::InvalidReservation)?;
            (
                0,
                collateral,
                RegisteredParticipantEffectsV1 {
                    claim_custody_debit: 0,
                    claim_position_credit: fill,
                    gross_collateral_debit: gross,
                    gross_collateral_credit: 0,
                    fee_transfer: fee_delta,
                    net_collateral_credit: 0,
                },
            )
        }
    };
    if filled == record.intent.maximum_fill {
        let maker_root = maker_root.close_live()?;
        let close = close_record_plan_v1(
            record,
            reserved_claims,
            reserved_collateral,
            observed_record_lamports,
        )?;
        Ok(RegisteredFillCandidateV1 {
            maker_root,
            record: RegisteredRecordAfterFillV1::Closed(close),
            effects,
        })
    } else {
        let next = DirectRegisteredIntentV1 {
            filled,
            reserved_claims,
            reserved_collateral,
            cumulative_gross,
            cumulative_fee,
            ..record
        };
        next.validate(config, outcome_count)?;
        Ok(RegisteredFillCandidateV1 {
            maker_root,
            record: RegisteredRecordAfterFillV1::Live(next),
            effects,
        })
    }
}

/// Atomic ordinary seller/buyer candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredOrdinarySettlementV1 {
    /// Seller candidate.
    pub seller: RegisteredFillCandidateV1,
    /// Buyer candidate.
    pub buyer: RegisteredFillCandidateV1,
    /// Gross amount split into seller net plus seller fee.
    pub gross_collateral: u64,
    /// Net seller destination credit.
    pub seller_net_collateral_credit: u64,
    /// Buyer escrow debit, including only the buyer's fee above gross.
    pub buyer_collateral_debit: u64,
    /// Seller-withheld plus buyer-added venue fee.
    pub total_fee_transfer: u64,
}

/// Complete ordinary match observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredOrdinaryInputV1 {
    /// Global Direct root.
    pub root: DirectRootStateV1,
    /// Registered seller.
    pub seller: RegisteredParticipantV1,
    /// Registered buyer.
    pub buyer: RegisteredParticipantV1,
    /// Common fill/price/config facts.
    pub execution: RegisteredExecutionV1,
}

/// Preview one ordinary match. No account or output buffer is mutated.
pub fn settle_registered_ordinary_v1(
    input: RegisteredOrdinaryInputV1,
) -> SuccessorResult<RegisteredOrdinarySettlementV1> {
    let root = input.root;
    let seller_record = input.seller.record;
    let buyer_record = input.buyer.record;
    let seller_intent = seller_record.intent;
    let buyer_intent = buyer_record.intent;
    if DirectSideV1::decode(seller_intent.side)? != DirectSideV1::Sell
        || DirectSideV1::decode(buyer_intent.side)? != DirectSideV1::Buy
        || seller_intent.market != buyer_intent.market
        || seller_intent.generation != buyer_intent.generation
        || seller_intent.outcome != buyer_intent.outcome
        || seller_record.maker == buyer_record.maker
    {
        return Err(SuccessorError::IncompatibleMatch);
    }
    let seller = preview_registered_fill_v1(RegisteredFillInputV1 {
        root,
        participant: input.seller,
        execution: input.execution,
    })?;
    let buyer = preview_registered_fill_v1(RegisteredFillInputV1 {
        root,
        participant: input.buyer,
        execution: input.execution,
    })?;
    let gross = seller.effects.gross_collateral_credit;
    if gross != buyer.effects.gross_collateral_debit {
        return Err(SuccessorError::IncompatibleMatch);
    }
    let total_fee_transfer = seller
        .effects
        .fee_transfer
        .checked_add(buyer.effects.fee_transfer)
        .ok_or(SuccessorError::ArithmeticOverflow)?;
    let buyer_collateral_debit = gross
        .checked_add(buyer.effects.fee_transfer)
        .ok_or(SuccessorError::ArithmeticOverflow)?;
    Ok(RegisteredOrdinarySettlementV1 {
        seller,
        buyer,
        gross_collateral: gross,
        seller_net_collateral_credit: seller.effects.net_collateral_credit,
        buyer_collateral_debit,
        total_fee_transfer,
    })
}

/// Runtime-width complementary split or merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComplementaryActionV1 {
    /// N canonical Buy records mint one complete set.
    Split,
    /// N canonical Sell records burn one complete set.
    Merge,
}

/// Aggregate result over caller-owned candidate scratch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComplementarySettlementV1 {
    /// Split vault credit or merge vault debit; always the common fill.
    pub market_vault_transfer: u64,
    /// Sum of cumulative-difference participant fees.
    pub total_fee_transfer: u64,
}

/// Runtime-width complementary participant slices.
pub struct ComplementaryParticipantsV1<'a> {
    /// Maker roots in canonical outcome order.
    pub maker_roots: &'a [MakerReplayRootV1],
    /// Live records in canonical outcome order.
    pub records: &'a [DirectRegisteredIntentV1],
    /// Current record lamports in canonical outcome order.
    pub record_lamports: &'a [u64],
    /// Exact execution prices in canonical outcome order.
    pub execution_prices: &'a [u64],
}

/// Runtime-width complementary preview input.
pub struct ComplementaryInputV1<'a> {
    /// Split or merge.
    pub action: ComplementaryActionV1,
    /// Global Direct root.
    pub root: DirectRootStateV1,
    /// Canonically ordered participant observations.
    pub participants: ComplementaryParticipantsV1<'a>,
    /// Caller-owned, non-authoritative candidate scratch.
    pub scratch: &'a mut [RegisteredFillCandidateV1],
    /// Immutable descriptor-selected economics.
    pub config: DirectExecutionConfigV1,
    /// Authenticated Product result-domain width.
    pub outcome_count: u16,
    /// Trusted `Clock::get()` slot.
    pub slot: u64,
    /// Common positive quantity.
    pub fill: u64,
}

/// Preview one exhaustive runtime-width complementary settlement.
///
/// `scratch` may change on refusal and is never authority. Records, roots, and
/// child accounts remain untouched until the adapter observes success, checks
/// every child CPI receipt, and commits the candidates last.
pub fn settle_registered_complementary_v1(
    input: ComplementaryInputV1<'_>,
) -> SuccessorResult<ComplementarySettlementV1> {
    let action = input.action;
    let root = input.root;
    let maker_roots = input.participants.maker_roots;
    let records = input.participants.records;
    let record_lamports = input.participants.record_lamports;
    let execution_prices = input.participants.execution_prices;
    let scratch = input.scratch;
    let config = input.config;
    let outcome_count = input.outcome_count;
    let slot = input.slot;
    let fill = input.fill;
    let count = usize::from(outcome_count);
    if count < 2
        || count > usize::from(u8::MAX) + 1
        || maker_roots.len() != count
        || records.len() != count
        || record_lamports.len() != count
        || execution_prices.len() != count
        || scratch.len() != count
    {
        return Err(SuccessorError::ComplementWidth);
    }
    if fill == 0 {
        return Err(SuccessorError::InvalidIntent);
    }
    let first = records
        .first()
        .ok_or(SuccessorError::ComplementWidth)?
        .intent;
    let expected_side = match action {
        ComplementaryActionV1::Split => DirectSideV1::Buy,
        ComplementaryActionV1::Merge => DirectSideV1::Sell,
    };
    let mut price_sum = 0_u64;
    let mut gross_sum = 0_u64;
    let mut fee_sum = 0_u64;
    for (index, (((maker_root, record), lamports), price)) in maker_roots
        .iter()
        .zip(records.iter())
        .zip(record_lamports.iter())
        .zip(execution_prices.iter())
        .enumerate()
    {
        let expected_outcome = u8::try_from(index).map_err(|_| SuccessorError::InvalidOutcome)?;
        if DirectSideV1::decode(record.intent.side)? != expected_side
            || record.intent.market != first.market
            || record.intent.generation != first.generation
            || record.intent.outcome != expected_outcome
            || records
                .iter()
                .take(index)
                .any(|prior| prior.maker == record.maker)
        {
            return Err(SuccessorError::NonCanonicalComplement);
        }
        let candidate = preview_registered_fill_v1(RegisteredFillInputV1 {
            root,
            participant: RegisteredParticipantV1 {
                maker_root: *maker_root,
                record: *record,
                observed_record_lamports: *lamports,
            },
            execution: RegisteredExecutionV1 {
                config,
                outcome_count,
                slot,
                fill,
                execution_price: *price,
            },
        })?;
        price_sum = price_sum
            .checked_add(*price)
            .ok_or(SuccessorError::ArithmeticOverflow)?;
        let gross = match action {
            ComplementaryActionV1::Split => candidate.effects.gross_collateral_debit,
            ComplementaryActionV1::Merge => candidate.effects.gross_collateral_credit,
        };
        gross_sum = gross_sum
            .checked_add(gross)
            .ok_or(SuccessorError::ArithmeticOverflow)?;
        fee_sum = fee_sum
            .checked_add(candidate.effects.fee_transfer)
            .ok_or(SuccessorError::ArithmeticOverflow)?;
        *scratch
            .get_mut(index)
            .ok_or(SuccessorError::ComplementWidth)? = candidate;
    }
    if price_sum != config.price_scale || gross_sum != fill {
        return Err(SuccessorError::NonCanonicalComplement);
    }
    Ok(ComplementarySettlementV1 {
        market_vault_transfer: fill,
        total_fee_transfer: fee_sum,
    })
}

/// Adapter-authenticated maker kill-switch projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedCancelThroughV1 {
    coordinates: DirectCoordinatesV1,
    maker: [u8; 32],
    minimum_live_nonce: u64,
}

impl AuthenticatedCancelThroughV1 {
    /// Seal exact coordinates only after Trading verifies the adjacent native
    /// Ed25519 message for this maker.
    pub fn from_adjacent_ed25519(
        coordinates: DirectCoordinatesV1,
        maker: [u8; 32],
        minimum_live_nonce: u64,
    ) -> SuccessorResult<Self> {
        require_nonzero(maker)?;
        Ok(Self {
            coordinates,
            maker,
            minimum_live_nonce,
        })
    }

    /// Signed minimum nonce that remains live.
    pub const fn minimum_live_nonce(self) -> u64 {
        self.minimum_live_nonce
    }
}

/// Apply one O(1) maker-authorized invalidation threshold.
pub fn apply_cancel_through_v1(
    root: DirectRootStateV1,
    maker_root: MakerReplayRootV1,
    authenticated: AuthenticatedCancelThroughV1,
) -> SuccessorResult<MakerReplayRootV1> {
    if root.open_maker_root_count == 0 {
        return Err(SuccessorError::MakerRootCountInvariant);
    }
    maker_root.validate_coordinate(authenticated.coordinates, authenticated.maker)?;
    maker_root.cancel_through(authenticated.minimum_live_nonce)
}

/// Exact authority for a terminal live-record unwind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisteredTerminalEvidenceV1 {
    /// Maker reauthenticates the exact signed CompactIntent being cancelled.
    Cancel(AuthenticatedCompactIntentV1),
    /// Anyone may close strictly after the signed inclusive slot interval.
    Expire {
        /// Trusted `Clock::get()` slot.
        slot: u64,
    },
    /// Anyone may close a nonce below the maker's signed minimum-live threshold.
    Invalidated,
}

/// Atomic terminal candidate for a registered intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredTerminalResultV1 {
    /// Maker replay root after exact live-count decrement.
    pub maker_root: MakerReplayRootV1,
    /// Asset and RentCredit disposition.
    pub close: RegisteredRecordCloseV1,
}

/// Cancel, expire, or permissionlessly unwind one invalidated live record.
pub fn terminate_registered_intent_v1(
    root: DirectRootStateV1,
    maker_root: MakerReplayRootV1,
    record: DirectRegisteredIntentV1,
    config: DirectExecutionConfigV1,
    outcome_count: u16,
    evidence: RegisteredTerminalEvidenceV1,
    observed_record_lamports: u64,
) -> SuccessorResult<RegisteredTerminalResultV1> {
    if root.open_maker_root_count == 0 {
        return Err(SuccessorError::MakerRootCountInvariant);
    }
    record.validate(config, outcome_count)?;
    record.validate_coordinate(maker_root)?;
    match evidence {
        RegisteredTerminalEvidenceV1::Cancel(authenticated)
            if authenticated.maker == record.maker && authenticated.intent == record.intent => {}
        RegisteredTerminalEvidenceV1::Expire { slot } if slot > record.intent.valid_through => {}
        RegisteredTerminalEvidenceV1::Invalidated
            if record.intent.nonce < maker_root.minimum_live_nonce => {}
        RegisteredTerminalEvidenceV1::Cancel(_)
        | RegisteredTerminalEvidenceV1::Expire { .. }
        | RegisteredTerminalEvidenceV1::Invalidated => {
            return Err(SuccessorError::InvalidTerminal);
        }
    }
    let maker_root = maker_root.close_live()?;
    Ok(RegisteredTerminalResultV1 {
        maker_root,
        close: close_record_plan_v1(
            record,
            record.reserved_claims,
            record.reserved_collateral,
            observed_record_lamports,
        )?,
    })
}

fn close_record_plan_v1(
    record: DirectRegisteredIntentV1,
    claim_refund: u64,
    collateral_refund: u64,
    observed_lamports: u64,
) -> SuccessorResult<RegisteredRecordCloseV1> {
    if observed_lamports < record.rent_principal {
        return Err(SuccessorError::InvalidRent);
    }
    Ok(RegisteredRecordCloseV1 {
        closed_nonce: record.intent.nonce,
        claim_refund,
        collateral_refund,
        rent_owner: record.rent_owner,
        rent_principal: record.rent_principal,
        unclassified_donation: observed_lamports - record.rent_principal,
        total_rent_credit: observed_lamports,
    })
}

fn validate_intent_v1(
    config: DirectExecutionConfigV1,
    intent: CompactIntentV1,
    outcome_count: u16,
    required_lifecycle: DirectLifecycleV1,
) -> SuccessorResult<()> {
    require_nonzero(intent.market)?;
    require_nonzero(intent.collateral_account)?;
    if DirectLifecycleV1::decode(intent.lifecycle)? != required_lifecycle
        || DirectSideV1::decode(intent.side).is_err()
        || outcome_count < 2
        || u16::from(intent.outcome) >= outcome_count
        || intent.valid_from > intent.valid_through
        || intent.maximum_fill == 0
        || intent.limit_price > config.price_scale
        || intent.fee_basis_points != config.fee_basis_points
    {
        return Err(SuccessorError::InvalidIntent);
    }
    Ok(())
}

fn maximum_buy_reserve_v1(
    config: DirectExecutionConfigV1,
    intent: CompactIntentV1,
) -> SuccessorResult<u64> {
    let product = u128::from(intent.maximum_fill) * u128::from(intent.limit_price);
    let gross = u64::try_from(product / u128::from(config.price_scale))
        .map_err(|_| SuccessorError::ArithmeticOverflow)?;
    gross
        .checked_add(fee_floor_v1(gross, config.fee_basis_points)?)
        .ok_or(SuccessorError::ArithmeticOverflow)
}

fn exact_quote_v1(quantity: u64, price: u64, scale: u64) -> SuccessorResult<u64> {
    let product = u128::from(quantity) * u128::from(price);
    let denominator = u128::from(scale);
    if product % denominator != 0 {
        return Err(SuccessorError::NonIntegralQuote);
    }
    u64::try_from(product / denominator).map_err(|_| SuccessorError::ArithmeticOverflow)
}

fn fee_floor_v1(gross: u64, basis_points: u16) -> SuccessorResult<u64> {
    let product = u128::from(gross) * u128::from(basis_points);
    u64::try_from(product / u128::from(DIRECT_FEE_DENOMINATOR_V1))
        .map_err(|_| SuccessorError::ArithmeticOverflow)
}

/// Exact rent/donation return when one maker root closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MakerReplayClosePlanV1 {
    /// Immutable RentCredit beneficiary.
    pub rent_owner: [u8; 32],
    /// Exact historical account-rent principal.
    pub rent_principal: u64,
    /// Lamports above historical principal, explicitly not fees or reserves.
    pub unclassified_donation: u64,
    /// Exact total lamports credited to the beneficiary RentCredit.
    pub total_credit: u64,
}

/// Atomic candidates after one terminal maker-root close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MakerReplayCloseResultV1 {
    /// Global root after exact count decrement.
    pub root: DirectRootStateV1,
    /// Exact refund classification.
    pub plan: MakerReplayClosePlanV1,
}

/// Close one zero-live maker root after global retirement begins.
pub fn close_maker_replay_v1(
    root: DirectRootStateV1,
    maker_root: MakerReplayRootV1,
    observed_lamports: u64,
) -> SuccessorResult<MakerReplayCloseResultV1> {
    if root.phase != DirectRootPhaseV1::Retiring {
        return Err(SuccessorError::InvalidRootPhase);
    }
    maker_root.validate()?;
    if maker_root.live_count != 0 {
        return Err(SuccessorError::LiveCountInvariant);
    }
    if observed_lamports < maker_root.rent_principal {
        return Err(SuccessorError::InvalidRent);
    }
    let open_maker_root_count = root
        .open_maker_root_count
        .checked_sub(1)
        .ok_or(SuccessorError::MakerRootCountInvariant)?;
    Ok(MakerReplayCloseResultV1 {
        root: DirectRootStateV1 {
            open_maker_root_count,
            ..root
        },
        plan: MakerReplayClosePlanV1 {
            rent_owner: maker_root.rent_owner,
            rent_principal: maker_root.rent_principal,
            unclassified_donation: observed_lamports - maker_root.rent_principal,
            total_credit: observed_lamports,
        },
    })
}

fn exact_width(input: &[u8], width: usize) -> SuccessorResult<()> {
    if input.len() == width {
        Ok(())
    } else {
        Err(SuccessorError::InvalidLength)
    }
}

fn exact(input: &[u8], offset: usize, value: &[u8]) -> SuccessorResult<()> {
    if slice(input, offset, value.len())? == value {
        Ok(())
    } else {
        Err(SuccessorError::InvalidMagic)
    }
}

fn version(input: &[u8], offset: usize) -> SuccessorResult<()> {
    if u16_at(input, offset)? == generated::DIRECT_SUCCESSOR_ABI_VERSION_V1 {
        Ok(())
    } else {
        Err(SuccessorError::UnsupportedVersion)
    }
}

fn zero_range(input: &[u8], offset: usize, width: usize) -> SuccessorResult<()> {
    if slice(input, offset, width)?.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(SuccessorError::NonCanonicalReserved)
    }
}

fn require_nonzero(value: [u8; 32]) -> SuccessorResult<()> {
    if value == [0; 32] {
        Err(SuccessorError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn byte_at(input: &[u8], offset: usize) -> SuccessorResult<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(SuccessorError::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> SuccessorResult<u16> {
    Ok(u16::from_le_bytes(array_at(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> SuccessorResult<u64> {
    Ok(u64::from_le_bytes(array_at(input, offset)?))
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> SuccessorResult<[u8; N]> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| SuccessorError::InvalidLength)
}

fn slice(input: &[u8], offset: usize, width: usize) -> SuccessorResult<&[u8]> {
    let end = offset
        .checked_add(width)
        .ok_or(SuccessorError::InvalidLength)?;
    input.get(offset..end).ok_or(SuccessorError::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    let Some(end) = offset.checked_add(value.len()) else {
        return;
    };
    let Some(destination) = output.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(value);
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) {
    if let Some(destination) = output.get_mut(offset) {
        *destination = value;
    }
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn overwrite(bytes: &mut [u8], offset: usize, value: &[u8]) {
        let end = offset.checked_add(value.len()).expect("test range");
        bytes
            .get_mut(offset..end)
            .expect("test destination")
            .copy_from_slice(value);
    }

    fn intent(maker: [u8; 32], nonce: u64) -> AuthenticatedIntentReplayV1 {
        AuthenticatedIntentReplayV1::from_signed_intent(
            maker,
            CompactIntentV1 {
                side: 0,
                outcome: 1,
                lifecycle: 1,
                market: id(1),
                generation: 4,
                nonce,
                valid_from: 1,
                valid_through: 9,
                maximum_fill: 10,
                limit_price: 500_000,
                fee_basis_points: 25,
                collateral_account: id(8),
            },
        )
        .expect("intent replay")
    }

    fn config() -> DirectExecutionConfigV1 {
        DirectExecutionConfigV1::new(100, 1_000, id(99)).expect("config")
    }

    fn registered_intent(
        side: u8,
        outcome: u8,
        market: [u8; 32],
        nonce: u64,
        maximum_fill: u64,
        limit_price: u64,
        collateral: [u8; 32],
    ) -> CompactIntentV1 {
        CompactIntentV1 {
            side,
            outcome,
            lifecycle: 2,
            market,
            generation: 4,
            nonce,
            valid_from: 2,
            valid_through: 20,
            maximum_fill,
            limit_price,
            fee_basis_points: 1_000,
            collateral_account: collateral,
        }
    }

    fn register(
        root: DirectRootStateV1,
        maker: [u8; 32],
        intent: CompactIntentV1,
        bump: u8,
    ) -> RegisteredIntentCreationV1 {
        register_intent_v1(
            root,
            MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(bump, 3)),
            AuthenticatedCompactIntentV1::from_adjacent_ed25519(maker, intent)
                .expect("authentication projection"),
            config(),
            3,
            Some(MakerReplayFirstUseV1 {
                rent_owner: id(90),
                rent_principal: 100,
            }),
            RegisteredRecordFirstUseV1 {
                bump,
                observed_lamports: 7,
                rent_owner: id(91),
                rent_principal: 100,
            },
        )
        .expect("registration")
    }

    fn preview(
        root: DirectRootStateV1,
        maker_root: MakerReplayRootV1,
        record: DirectRegisteredIntentV1,
        fill: u64,
        execution_price: u64,
        lamports: u64,
    ) -> SuccessorResult<RegisteredFillCandidateV1> {
        preview_registered_fill_v1(RegisteredFillInputV1 {
            root,
            participant: RegisteredParticipantV1 {
                maker_root,
                record,
                observed_record_lamports: lamports,
            },
            execution: RegisteredExecutionV1 {
                config: config(),
                outcome_count: 3,
                slot: 5,
                fill,
                execution_price,
            },
        })
    }

    #[test]
    fn lean_generated_examples_decode_and_round_trip() {
        let selected = id(11);
        let config = DirectExecutionConfigV1::decode_selected(
            selected,
            selected,
            &generated::DIRECT_CONFIG_EXAMPLE_V1,
        )
        .expect("config");
        assert_eq!(config.price_scale(), 1_000_000);
        assert_eq!(config.fee_basis_points(), 25);
        assert_eq!(config.fee_recipient(), id(9));
        assert_eq!(config.encode(), generated::DIRECT_CONFIG_EXAMPLE_V1);

        let root = DirectRootStateV1::decode(&generated::DIRECT_ROOT_EXAMPLE_V1).expect("root");
        assert_eq!(root.phase(), DirectRootPhaseV1::Open);
        assert_eq!(root.open_maker_root_count(), 3);
        assert_eq!(root.encode(), generated::DIRECT_ROOT_EXAMPLE_V1);

        let maker = MakerReplayRootV1::decode(&generated::DIRECT_MAKER_EXAMPLE_V1).expect("maker");
        assert_eq!(maker.next_nonce(), 9);
        assert_eq!(maker.live_count(), 2);
        assert_eq!(maker.minimum_live_nonce(), 5);
        assert_eq!(maker.rent_principal(), 2_000_000);
        assert_eq!(
            maker.encode().expect("encode"),
            generated::DIRECT_MAKER_EXAMPLE_V1
        );
        let record = DirectRegisteredIntentV1::decode_selected(
            config,
            2,
            &generated::DIRECT_RECORD_EXAMPLE_V1,
        )
        .expect("record");
        assert_eq!(record.filled(), 3);
        assert_eq!(record.reserved_claims(), 1_997);
        assert_eq!(record.cumulative_gross(), 1);
        assert_eq!(
            record.encode_selected(config, 2).expect("record encode"),
            generated::DIRECT_RECORD_EXAMPLE_V1
        );
    }

    #[test]
    fn config_substitution_and_hostile_economics_refuse() {
        let exact = id(5);
        DirectSuccessorRequirementsV1::validate(
            DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
            DIRECT_ROOT_SCHEMA_ID_V1,
            DIRECT_MAKER_REPLAY_DERIVATION_ID_V1,
            DIRECT_REGISTERED_RECORD_SCHEMA_ID_V1,
            DIRECT_REGISTERED_RECORD_DERIVATION_ID_V1,
        )
        .expect("successor descriptor requirements");
        assert_eq!(
            DirectSuccessorRequirementsV1::validate(
                DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
                DIRECT_ROOT_SCHEMA_ID_V1,
                DIRECT_MAKER_REPLAY_DERIVATION_ID_V1,
                id(1),
                DIRECT_REGISTERED_RECORD_DERIVATION_ID_V1,
            ),
            Err(SuccessorError::ConfigSelectionMismatch)
        );
        assert_eq!(
            DirectExecutionConfigV1::decode_selected(
                exact,
                id(6),
                &generated::DIRECT_CONFIG_EXAMPLE_V1,
            ),
            Err(SuccessorError::ConfigSelectionMismatch)
        );
        let mut zero_scale = generated::DIRECT_CONFIG_EXAMPLE_V1;
        overwrite(
            &mut zero_scale,
            generated::DIRECT_CONFIG_PRICE_SCALE_OFFSET_V1,
            &[0; 8],
        );
        assert_eq!(
            DirectExecutionConfigV1::decode_selected(exact, exact, &zero_scale),
            Err(SuccessorError::InvalidExecutionConfig)
        );
        let mut high_fee = generated::DIRECT_CONFIG_EXAMPLE_V1;
        overwrite(
            &mut high_fee,
            generated::DIRECT_CONFIG_FEE_BPS_OFFSET_V1,
            &10_001_u16.to_le_bytes(),
        );
        assert_eq!(
            DirectExecutionConfigV1::decode_selected(exact, exact, &high_fee),
            Err(SuccessorError::InvalidExecutionConfig)
        );
        let mut zero_recipient = generated::DIRECT_CONFIG_EXAMPLE_V1;
        overwrite(
            &mut zero_recipient,
            generated::DIRECT_CONFIG_FEE_RECIPIENT_OFFSET_V1,
            &[0; 32],
        );
        assert_eq!(
            DirectExecutionConfigV1::decode_selected(exact, exact, &zero_recipient),
            Err(SuccessorError::ZeroIdentity)
        );
    }

    #[test]
    fn inline_first_use_is_dust_tolerant_and_counts_once() {
        let root = DirectRootStateV1::new();
        let consumed = consume_nonce_v1(
            root,
            MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(7, 3)),
            intent(id(2), 0),
            NonceConsumptionV1::Inline,
            Some(MakerReplayFirstUseV1 {
                rent_owner: id(9),
                rent_principal: 100,
            }),
        )
        .expect("first use");
        assert_eq!(consumed.root.open_maker_root_count(), 1);
        assert_eq!(consumed.maker_root.next_nonce(), 1);
        assert_eq!(consumed.maker_root.live_count(), 0);
        assert_eq!(
            consumed.creation,
            Some(MakerReplayCreationPlanV1 {
                observed_lamports: 3,
                top_up_lamports: 97,
                post_lamports: 100,
            })
        );

        let replayed = consume_nonce_v1(
            consumed.root,
            MakerReplayObservationV1::Existing(consumed.maker_root),
            intent(id(2), 0),
            NonceConsumptionV1::Inline,
            None,
        );
        assert_eq!(replayed, Err(SuccessorError::NonceMismatch));

        let next = consume_nonce_v1(
            consumed.root,
            MakerReplayObservationV1::Existing(consumed.maker_root),
            intent(id(2), 1),
            NonceConsumptionV1::Inline,
            None,
        )
        .expect("next nonce");
        assert_eq!(next.root, consumed.root);
        assert_eq!(next.maker_root.next_nonce(), 2);
    }

    #[test]
    fn registration_live_and_cancel_through_are_single_owner() {
        let created = consume_nonce_v1(
            DirectRootStateV1::new(),
            MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(4, 100)),
            intent(id(2), 0),
            NonceConsumptionV1::Register,
            Some(MakerReplayFirstUseV1 {
                rent_owner: id(9),
                rent_principal: 100,
            }),
        )
        .expect("register");
        assert_eq!(created.maker_root.live_count(), 1);
        let invalidated = created
            .maker_root
            .cancel_through(1)
            .expect("cancel through");
        assert_eq!(invalidated.minimum_live_nonce(), 1);
        assert_eq!(
            invalidated.cancel_through(0),
            Err(SuccessorError::MinimumLiveNonceInvariant)
        );
        let drained = invalidated.close_live().expect("close live");
        assert_eq!(drained.live_count(), 0);
        assert_eq!(
            drained.close_live(),
            Err(SuccessorError::LiveCountInvariant)
        );
    }

    #[test]
    fn coordinate_count_and_first_use_substitutions_refuse() {
        let created = consume_nonce_v1(
            DirectRootStateV1::new(),
            MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(1, 0)),
            intent(id(2), 0),
            NonceConsumptionV1::Inline,
            Some(MakerReplayFirstUseV1 {
                rent_owner: id(9),
                rent_principal: 100,
            }),
        )
        .expect("create");
        assert_eq!(
            consume_nonce_v1(
                created.root,
                MakerReplayObservationV1::Existing(created.maker_root),
                intent(id(3), 1),
                NonceConsumptionV1::Inline,
                None,
            ),
            Err(SuccessorError::MakerCoordinateMismatch)
        );
        assert_eq!(
            consume_nonce_v1(
                DirectRootStateV1::new(),
                MakerReplayObservationV1::Existing(created.maker_root),
                intent(id(2), 1),
                NonceConsumptionV1::Inline,
                None,
            ),
            Err(SuccessorError::MakerRootCountInvariant)
        );
        assert_eq!(
            consume_nonce_v1(
                created.root,
                MakerReplayObservationV1::Existing(created.maker_root),
                intent(id(2), 1),
                NonceConsumptionV1::Inline,
                Some(MakerReplayFirstUseV1 {
                    rent_owner: id(9),
                    rent_principal: 100,
                }),
            ),
            Err(SuccessorError::InvalidFirstUse)
        );
    }

    #[test]
    fn maker_and_root_closure_conserve_count_and_refund() {
        let created = consume_nonce_v1(
            DirectRootStateV1::new(),
            MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(8, 3)),
            intent(id(2), 0),
            NonceConsumptionV1::Inline,
            Some(MakerReplayFirstUseV1 {
                rent_owner: id(9),
                rent_principal: 100,
            }),
        )
        .expect("create");
        assert_eq!(
            close_maker_replay_v1(created.root, created.maker_root, 111),
            Err(SuccessorError::InvalidRootPhase)
        );
        let retiring = created.root.begin_retiring().expect("retiring");
        let closed = close_maker_replay_v1(retiring, created.maker_root, 111).expect("close maker");
        assert_eq!(closed.root.open_maker_root_count(), 0);
        assert_eq!(closed.plan.rent_owner, id(9));
        assert_eq!(closed.plan.rent_principal, 100);
        assert_eq!(closed.plan.unclassified_donation, 11);
        assert_eq!(closed.plan.total_credit, 111);
        closed.root.require_closable().expect("root closable");
        assert_eq!(
            close_maker_replay_v1(closed.root, created.maker_root, 111),
            Err(SuccessorError::MakerRootCountInvariant)
        );
    }

    #[test]
    fn malformed_root_and_maker_bytes_refuse() {
        let mut root = generated::DIRECT_ROOT_EXAMPLE_V1;
        overwrite(&mut root, generated::DIRECT_ROOT_PHASE_OFFSET_V1, &[2]);
        assert_eq!(
            DirectRootStateV1::decode(&root),
            Err(SuccessorError::InvalidRootPhase)
        );
        let mut maker = generated::DIRECT_MAKER_EXAMPLE_V1;
        overwrite(&mut maker, generated::DIRECT_MAKER_RESERVED_OFFSET_V1, &[1]);
        assert_eq!(
            MakerReplayRootV1::decode(&maker),
            Err(SuccessorError::NonCanonicalReserved)
        );
        let mut bad_live = generated::DIRECT_MAKER_EXAMPLE_V1;
        overwrite(
            &mut bad_live,
            generated::DIRECT_MAKER_LIVE_COUNT_OFFSET_V1,
            &10_u64.to_le_bytes(),
        );
        assert_eq!(
            MakerReplayRootV1::decode(&bad_live),
            Err(SuccessorError::LiveCountInvariant)
        );
        let selected = DirectExecutionConfigV1::new(1_000_000, 25, id(9)).expect("selected config");
        let mut bad_record = generated::DIRECT_RECORD_EXAMPLE_V1;
        overwrite(
            &mut bad_record,
            generated::DIRECT_RECORD_CUMULATIVE_FEE_OFFSET_V1,
            &1_u64.to_le_bytes(),
        );
        assert_eq!(
            DirectRegisteredIntentV1::decode_selected(selected, 2, &bad_record),
            Err(SuccessorError::InvalidReservation)
        );
    }

    #[test]
    fn registered_partial_fees_are_partition_independent() {
        let signed = registered_intent(1, 0, id(1), 0, 10, 100, id(20));
        let created = register(DirectRootStateV1::new(), id(2), signed, 3);
        assert_eq!(created.root.open_maker_root_count(), 1);
        assert_eq!(created.maker_root.live_count(), 1);
        assert_eq!(created.record.reserved_collateral(), 11);
        assert_eq!(created.record_creation.top_up_lamports, 93);
        assert_eq!(
            register_intent_v1(
                created.root,
                MakerReplayObservationV1::Existing(created.maker_root),
                AuthenticatedCompactIntentV1::from_adjacent_ed25519(id(2), signed)
                    .expect("replay projection"),
                config(),
                3,
                None,
                RegisteredRecordFirstUseV1 {
                    bump: 3,
                    observed_lamports: 0,
                    rent_owner: id(91),
                    rent_principal: 100,
                },
            ),
            Err(SuccessorError::NonceMismatch)
        );

        let mut maker_root = created.maker_root;
        let mut record = created.record;
        let mut partition_fee = 0_u64;
        for step in 0..10 {
            let candidate =
                preview(created.root, maker_root, record, 1, 100, 107).expect("one-atom fill");
            partition_fee = partition_fee
                .checked_add(candidate.effects.fee_transfer)
                .expect("fee sum");
            maker_root = candidate.maker_root;
            match candidate.record {
                RegisteredRecordAfterFillV1::Live(next) => {
                    assert!(step < 9);
                    record = next;
                }
                RegisteredRecordAfterFillV1::Closed(close) => {
                    assert_eq!(step, 9);
                    assert_eq!(close.collateral_refund, 0);
                    assert_eq!(close.unclassified_donation, 7);
                }
            }
        }
        assert_eq!(partition_fee, 1);
        assert_eq!(maker_root.live_count(), 0);

        let single = register(DirectRootStateV1::new(), id(2), signed, 3);
        let candidate = preview(single.root, single.maker_root, single.record, 10, 100, 100)
            .expect("single fill");
        assert_eq!(candidate.effects.fee_transfer, partition_fee);
    }

    #[test]
    fn ordinary_partial_fill_charges_each_signed_side_once() {
        let seller = register(
            DirectRootStateV1::new(),
            id(2),
            registered_intent(0, 1, id(1), 0, 100, 40, id(30)),
            2,
        );
        let buyer = register(
            seller.root,
            id(3),
            registered_intent(1, 1, id(1), 0, 100, 60, id(31)),
            3,
        );
        let input = RegisteredOrdinaryInputV1 {
            root: buyer.root,
            seller: RegisteredParticipantV1 {
                maker_root: seller.maker_root,
                record: seller.record,
                observed_record_lamports: 100,
            },
            buyer: RegisteredParticipantV1 {
                maker_root: buyer.maker_root,
                record: buyer.record,
                observed_record_lamports: 100,
            },
            execution: RegisteredExecutionV1 {
                config: config(),
                outcome_count: 3,
                slot: 5,
                fill: 20,
                execution_price: 50,
            },
        };
        let settled = settle_registered_ordinary_v1(input).expect("ordinary");
        assert_eq!(settled.gross_collateral, 10);
        assert_eq!(settled.seller_net_collateral_credit, 9);
        assert_eq!(settled.buyer_collateral_debit, 11);
        assert_eq!(settled.total_fee_transfer, 2);
        assert_eq!(settled.seller.effects.claim_custody_debit, 20);
        assert_eq!(settled.buyer.effects.claim_position_credit, 20);
        assert_eq!(
            settle_registered_ordinary_v1(RegisteredOrdinaryInputV1 {
                execution: RegisteredExecutionV1 {
                    execution_price: 39,
                    ..input.execution
                },
                ..input
            }),
            Err(SuccessorError::IncompatibleMatch)
        );
    }

    #[test]
    fn terminal_paths_refuse_substitution_and_preserve_refunds() {
        let signed = registered_intent(0, 1, id(1), 0, 10, 40, id(20));
        let created = register(DirectRootStateV1::new(), id(2), signed, 4);
        let wrong = AuthenticatedCompactIntentV1::from_adjacent_ed25519(
            id(2),
            CompactIntentV1 { nonce: 1, ..signed },
        )
        .expect("wrong auth projection");
        assert_eq!(
            terminate_registered_intent_v1(
                created.root,
                created.maker_root,
                created.record,
                config(),
                3,
                RegisteredTerminalEvidenceV1::Cancel(wrong),
                100,
            ),
            Err(SuccessorError::InvalidTerminal)
        );
        assert_eq!(
            terminate_registered_intent_v1(
                created.root,
                created.maker_root,
                created.record,
                config(),
                3,
                RegisteredTerminalEvidenceV1::Expire { slot: 20 },
                100,
            ),
            Err(SuccessorError::InvalidTerminal)
        );
        let expired = terminate_registered_intent_v1(
            created.root,
            created.maker_root,
            created.record,
            config(),
            3,
            RegisteredTerminalEvidenceV1::Expire { slot: 21 },
            109,
        )
        .expect("expiry");
        assert_eq!(expired.close.claim_refund, 10);
        assert_eq!(expired.close.collateral_refund, 0);
        assert_eq!(expired.close.total_rent_credit, 109);
        assert_eq!(expired.maker_root.live_count(), 0);
    }

    #[test]
    fn cancel_through_is_strict_and_invalidated_close_is_permissionless() {
        let signed = registered_intent(0, 0, id(1), 0, 10, 40, id(20));
        let created = register(DirectRootStateV1::new(), id(2), signed, 4);
        let kill = AuthenticatedCancelThroughV1::from_adjacent_ed25519(
            DirectCoordinatesV1::new(id(1), 4).expect("coordinates"),
            id(2),
            1,
        )
        .expect("kill switch auth");
        let invalidated =
            apply_cancel_through_v1(created.root, created.maker_root, kill).expect("invalidate");
        assert_eq!(invalidated.minimum_live_nonce(), 1);
        assert_eq!(
            apply_cancel_through_v1(created.root, invalidated, kill),
            Err(SuccessorError::MinimumLiveNonceInvariant)
        );
        let closed = terminate_registered_intent_v1(
            created.root,
            invalidated,
            created.record,
            config(),
            3,
            RegisteredTerminalEvidenceV1::Invalidated,
            100,
        )
        .expect("permissionless invalidated close");
        assert_eq!(closed.close.claim_refund, 10);
        assert_eq!(closed.maker_root.live_count(), 0);
    }

    #[test]
    fn runtime_width_split_and_merge_are_canonical_and_atomic_candidates() {
        let prices = [20_u64, 30, 50];
        let mut root = DirectRootStateV1::new();
        let mut buy_roots = [MakerReplayRootV1::new(
            DirectCoordinatesV1::new(id(1), 4).expect("coordinates"),
            id(10),
            id(90),
            100,
            1,
        )
        .expect("placeholder"); 3];
        let mut buy_records = [register(
            root,
            id(10),
            registered_intent(1, 0, id(1), 0, 100, 100, id(30)),
            1,
        )
        .record; 3];
        for (index, price) in prices.iter().copied().enumerate() {
            let maker = id(u8::try_from(index + 10).expect("maker byte"));
            let outcome = u8::try_from(index).expect("outcome");
            let creation = register(
                root,
                maker,
                registered_intent(1, outcome, id(1), 0, 100, price, id(30)),
                outcome,
            );
            root = creation.root;
            *buy_roots.get_mut(index).expect("root slot") = creation.maker_root;
            *buy_records.get_mut(index).expect("record slot") = creation.record;
        }
        let seed = preview(
            root,
            *buy_roots.first().expect("first root"),
            *buy_records.first().expect("first record"),
            100,
            *prices.first().expect("first price"),
            100,
        )
        .expect("seed candidate");
        let mut scratch = [seed; 3];
        let split = settle_registered_complementary_v1(ComplementaryInputV1 {
            action: ComplementaryActionV1::Split,
            root,
            participants: ComplementaryParticipantsV1 {
                maker_roots: &buy_roots,
                records: &buy_records,
                record_lamports: &[100; 3],
                execution_prices: &prices,
            },
            scratch: &mut scratch,
            config: config(),
            outcome_count: 3,
            slot: 5,
            fill: 100,
        })
        .expect("split");
        assert_eq!(split.market_vault_transfer, 100);
        assert_eq!(split.total_fee_transfer, 10);
        assert!(
            scratch.iter().all(|candidate| matches!(
                candidate.record,
                RegisteredRecordAfterFillV1::Closed(_)
            ))
        );

        let mut aliased = buy_records;
        let first_maker = aliased.first().expect("first record").maker;
        aliased.get_mut(1).expect("second record").maker = first_maker;
        assert_eq!(
            settle_registered_complementary_v1(ComplementaryInputV1 {
                action: ComplementaryActionV1::Split,
                root,
                participants: ComplementaryParticipantsV1 {
                    maker_roots: &buy_roots,
                    records: &aliased,
                    record_lamports: &[100; 3],
                    execution_prices: &prices,
                },
                scratch: &mut scratch,
                config: config(),
                outcome_count: 3,
                slot: 5,
                fill: 100,
            }),
            Err(SuccessorError::NonCanonicalComplement)
        );

        let mut sell_root = DirectRootStateV1::new();
        let mut sell_roots = buy_roots;
        let mut sell_records = buy_records;
        for (index, price) in prices.iter().copied().enumerate() {
            let maker = id(u8::try_from(index + 20).expect("maker byte"));
            let outcome = u8::try_from(index).expect("outcome");
            let creation = register(
                sell_root,
                maker,
                registered_intent(0, outcome, id(1), 0, 100, price, id(40)),
                outcome,
            );
            sell_root = creation.root;
            *sell_roots.get_mut(index).expect("root slot") = creation.maker_root;
            *sell_records.get_mut(index).expect("record slot") = creation.record;
        }
        let merge = settle_registered_complementary_v1(ComplementaryInputV1 {
            action: ComplementaryActionV1::Merge,
            root: sell_root,
            participants: ComplementaryParticipantsV1 {
                maker_roots: &sell_roots,
                records: &sell_records,
                record_lamports: &[100; 3],
                execution_prices: &prices,
            },
            scratch: &mut scratch,
            config: config(),
            outcome_count: 3,
            slot: 5,
            fill: 100,
        })
        .expect("merge");
        assert_eq!(merge.market_vault_transfer, 100);
        assert_eq!(merge.total_fee_transfer, 10);
    }

    #[test]
    fn first_use_count_overflow_refuses_without_state_candidate() {
        let root = DirectRootStateV1 {
            phase: DirectRootPhaseV1::Open,
            open_maker_root_count: u64::MAX,
        };
        assert_eq!(
            consume_nonce_v1(
                root,
                MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(1, 0)),
                intent(id(2), 0),
                NonceConsumptionV1::Inline,
                Some(MakerReplayFirstUseV1 {
                    rent_owner: id(9),
                    rent_principal: 100,
                }),
            ),
            Err(SuccessorError::MakerRootCountInvariant)
        );
    }
}
