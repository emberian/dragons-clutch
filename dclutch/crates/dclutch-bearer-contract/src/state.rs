//! Fixed bearer-capability state and hostile adapter projections.

use core::convert::TryInto;

use dclutch_market_contract::market::CategoricalMarketV1;
use dclutch_token_svm::{ACCOUNT_BYTES, TOKEN_2022_PROGRAM_ID};

use crate::{Error, Result, require_nonzero};

/// Minimum categorical width in provisional bearer profile V1.
pub const MIN_OUTCOMES: usize = 2;
/// Maximum categorical width in provisional bearer profile V1.
pub const MAX_OUTCOMES: usize = 16;
/// Exact bearer config width.
pub const BEARER_CONFIG_BYTES: usize = 80;
/// Fixed state bytes before `N` eight-byte accounted-supply entries.
pub const BEARER_STATE_BASE_BYTES: usize = 64;
/// Exact binary bearer-state width.
pub const BINARY_BEARER_STATE_BYTES: usize = BEARER_STATE_BASE_BYTES + 16;
/// Exact maximum bearer-state width in provisional profile V1.
pub const MAX_BEARER_STATE_BYTES: usize = BEARER_STATE_BASE_BYTES + MAX_OUTCOMES * 8;
/// Exact Token-2022 Mint width for MintCloseAuthority plus PermissionedBurn.
///
/// This is `165` bytes of extension base/padding, one account-type byte, and
/// two `type:u16 || length:u16 || authority:[u8;32]` TLV entries.
pub const BEARER_MINT_BYTES: usize = 238;
/// Exact admitted holder claim-token Account width with no TLV extensions.
pub const BEARER_TOKEN_ACCOUNT_BYTES: usize = ACCOUNT_BYTES;
/// Bearer config magic.
pub const BEARER_CONFIG_MAGIC: [u8; 8] = *b"DCLTBRC1";
/// Bearer state magic.
pub const BEARER_STATE_MAGIC: [u8; 8] = *b"DCLTBRS1";
/// Implemented config schema.
pub const BEARER_CONFIG_SCHEMA_VERSION: u16 = 1;
/// Implemented state schema.
pub const BEARER_STATE_SCHEMA_VERSION: u16 = 1;
/// Provisional measured categorical bearer profile.
pub const BEARER_PROFILE_V1: u8 = 1;
/// Direct-child PDA domain; seeds are domain, Market, and generation LE bytes.
pub const BEARER_CAPABILITY_PDA_DOMAIN: &[u8] = b"dclutch/bearer-cap/v1";
/// Mint PDA domain; seeds add one canonical outcome byte.
pub const BEARER_MINT_PDA_DOMAIN: &[u8] = b"dclutch/bearer-mint/v1";
/// Chain-derived maximum byte width of one PDA seed component.
pub const SVM_MAX_PDA_SEED_BYTES: usize = 32;

/// SHA-256 of `dclutch:capability-kind:bearer-outcome-claims:v1`.
pub const BEARER_CAPABILITY_KIND_ID: [u8; 32] = [
    0x8e, 0x30, 0x84, 0x90, 0xbf, 0xf0, 0xf7, 0xc1, 0x04, 0x41, 0xb4, 0xb9, 0xb3, 0xcd, 0xfc, 0x7f,
    0x65, 0x70, 0xb2, 0xb8, 0x02, 0x00, 0x7c, 0xdb, 0xb2, 0xa0, 0x72, 0x63, 0x9c, 0x17, 0xb7, 0x5b,
];
/// SHA-256 of `dclutch:bearer-contract:semantic-release:v1`.
pub const BEARER_SEMANTIC_RELEASE_ID: [u8; 32] = [
    0xd2, 0x06, 0x65, 0x99, 0xe5, 0x9a, 0x1c, 0x2f, 0x7f, 0x24, 0xc4, 0xb4, 0x86, 0x90, 0xd1, 0x12,
    0x7e, 0xaa, 0xb5, 0xaf, 0x12, 0x65, 0x1a, 0x4b, 0x70, 0x22, 0xe5, 0x1e, 0x90, 0x4d, 0xa4, 0x0f,
];
/// SHA-256 of `dclutch:bearer-contract:child-schema:v1`.
pub const BEARER_CHILD_SCHEMA_ID: [u8; 32] = [
    0xc1, 0x65, 0x66, 0x92, 0x2d, 0x3a, 0xc1, 0xf6, 0x52, 0xa7, 0x9c, 0x5b, 0x41, 0xfd, 0x75, 0xd8,
    0x07, 0xe5, 0x80, 0x84, 0xb9, 0xb4, 0x3f, 0xe6, 0x23, 0x90, 0xdd, 0x12, 0x4b, 0x22, 0x84, 0x11,
];
/// SHA-256 of `dclutch:bearer-contract:child-derivation:v1`.
pub const BEARER_CHILD_DERIVATION_ID: [u8; 32] = [
    0x2b, 0x2c, 0x1b, 0xfb, 0xe1, 0x8b, 0x99, 0x2f, 0x5a, 0x34, 0x52, 0x1e, 0xcb, 0x0c, 0xc6, 0xa9,
    0x02, 0x01, 0x45, 0x90, 0x06, 0xc7, 0x92, 0xcd, 0x70, 0xfe, 0x71, 0xac, 0x2c, 0x1a, 0x89, 0x44,
];

const CONFIG_RESERVED_OFFSET: usize = 10;
const CONFIG_RESERVED_BYTES: usize = 6;
const CONFIG_TOKEN_PROGRAM_OFFSET: usize = 16;
const CONFIG_RENT_REFUND_OFFSET: usize = 48;
const STATE_OUTCOME_COUNT_OFFSET: usize = 10;
const STATE_PROFILE_OFFSET: usize = 11;
const STATE_RESERVED_OFFSET: usize = 12;
const STATE_RESERVED_BYTES: usize = 4;
const STATE_MARKET_OFFSET: usize = 16;
const STATE_GENERATION_OFFSET: usize = 48;
const STATE_ENTRY_INDEX_OFFSET: usize = 56;
const STATE_BODY_RESERVED_OFFSET: usize = 58;
const STATE_BODY_RESERVED_BYTES: usize = 6;
const STATE_SUPPLY_OFFSET: usize = 64;

/// Immutable bearer configuration selected by one capability-manifest entry.
///
/// Claim Mints always use the official Token-2022 program. `rent_refund` is
/// the sole owner of rent principal recovered from the capability root and
/// claim Mints. It is capability funding, never Hoard collateral.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerConfigV1 {
    token_program: [u8; 32],
    rent_refund: [u8; 32],
}

impl BearerConfigV1 {
    /// Construct the exact immutable Token-2022 bearer profile.
    pub fn new(token_program: [u8; 32], rent_refund: [u8; 32]) -> Result<Self> {
        if token_program != TOKEN_2022_PROGRAM_ID {
            return Err(Error::WrongTokenProgram);
        }
        require_nonzero(&rent_refund)?;
        Ok(Self {
            token_program,
            rent_refund,
        })
    }

    /// Decode one exact canonical config preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != BEARER_CONFIG_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != BEARER_CONFIG_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != BEARER_CONFIG_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, CONFIG_RESERVED_OFFSET, CONFIG_RESERVED_BYTES)?;
        Self::new(
            array(bytes, CONFIG_TOKEN_PROGRAM_OFFSET)?,
            array(bytes, CONFIG_RENT_REFUND_OFFSET)?,
        )
    }

    /// Return the exact content preimage.
    pub fn to_bytes(self) -> [u8; BEARER_CONFIG_BYTES] {
        let mut output = [0; BEARER_CONFIG_BYTES];
        put(&mut output, 0, &BEARER_CONFIG_MAGIC);
        put(&mut output, 8, &BEARER_CONFIG_SCHEMA_VERSION.to_le_bytes());
        put(
            &mut output,
            CONFIG_TOKEN_PROGRAM_OFFSET,
            &self.token_program,
        );
        put(&mut output, CONFIG_RENT_REFUND_OFFSET, &self.rent_refund);
        output
    }

    /// Return the exact Token-2022 program address.
    pub const fn token_program(self) -> [u8; 32] {
        self.token_program
    }

    /// Return the immutable rent-principal refund identity.
    pub const fn rent_refund(self) -> [u8; 32] {
        self.rent_refund
    }
}

/// One direct Market child accounting for every materialized outcome Mint.
///
/// `accounted_supply[i]` is not a second Market liability: it is the exact
/// subset of `Market.supply[i]` represented by the canonical bearer Mint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerCapabilityV1<const N: usize> {
    market: [u8; 32],
    generation: u64,
    manifest_entry_index: u16,
    accounted_supply: [u64; N],
}

impl<const N: usize> BearerCapabilityV1<N> {
    pub(crate) fn activated(
        market: [u8; 32],
        generation: u64,
        manifest_entry_index: u16,
    ) -> Result<Self> {
        validate_width::<N>()?;
        require_nonzero(&market)?;
        Ok(Self {
            market,
            generation,
            manifest_entry_index,
            accounted_supply: [0; N],
        })
    }

    /// Return the checked exact account width, `64 + 8N` bytes.
    pub fn encoded_len() -> Result<usize> {
        validate_width::<N>()?;
        N.checked_mul(8)
            .and_then(|width| BEARER_STATE_BASE_BYTES.checked_add(width))
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Hostile-decode one exact capability root.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::encoded_len()? {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != BEARER_STATE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != BEARER_STATE_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if usize::from(byte(bytes, STATE_OUTCOME_COUNT_OFFSET)?) != N
            || byte(bytes, STATE_PROFILE_OFFSET)? != BEARER_PROFILE_V1
        {
            return Err(Error::InvalidOutcomeCount);
        }
        require_zero(bytes, STATE_RESERVED_OFFSET, STATE_RESERVED_BYTES)?;
        require_zero(bytes, STATE_BODY_RESERVED_OFFSET, STATE_BODY_RESERVED_BYTES)?;
        let mut accounted_supply = [0; N];
        let mut index = 0usize;
        while index < N {
            let offset = STATE_SUPPLY_OFFSET
                .checked_add(index.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            let destination = accounted_supply
                .get_mut(index)
                .ok_or(Error::InvalidOutcome)?;
            *destination = u64::from_le_bytes(array(bytes, offset)?);
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let result = Self {
            market: array(bytes, STATE_MARKET_OFFSET)?,
            generation: u64::from_le_bytes(array(bytes, STATE_GENERATION_OFFSET)?),
            manifest_entry_index: u16::from_le_bytes(array(bytes, STATE_ENTRY_INDEX_OFFSET)?),
            accounted_supply,
        };
        require_nonzero(&result.market)?;
        Ok(result)
    }

    /// Encode atomically into one exact caller-owned buffer.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != Self::encoded_len()? {
            return Err(Error::OutputLength);
        }
        require_nonzero(&self.market)?;
        let outcome_count = u8::try_from(N).map_err(|_| Error::InvalidOutcomeCount)?;
        output.fill(0);
        put(output, 0, &BEARER_STATE_MAGIC);
        put(output, 8, &BEARER_STATE_SCHEMA_VERSION.to_le_bytes());
        put(output, STATE_OUTCOME_COUNT_OFFSET, &[outcome_count]);
        put(output, STATE_PROFILE_OFFSET, &[BEARER_PROFILE_V1]);
        put(output, STATE_MARKET_OFFSET, &self.market);
        put(
            output,
            STATE_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(
            output,
            STATE_ENTRY_INDEX_OFFSET,
            &self.manifest_entry_index.to_le_bytes(),
        );
        for (index, supply) in self.accounted_supply.iter().enumerate() {
            put(
                output,
                STATE_SUPPLY_OFFSET + index * 8,
                &supply.to_le_bytes(),
            );
        }
        Ok(())
    }

    /// Validate Market/generation binding and representation subset bounds.
    pub fn validate_market(
        &self,
        market_key: [u8; 32],
        market: &CategoricalMarketV1<N>,
    ) -> Result<()> {
        if self.market != market_key {
            return Err(Error::MarketMismatch);
        }
        if self.generation != market.root().identity().generation() {
            return Err(Error::GenerationMismatch);
        }
        for (bearer, aggregate) in self.accounted_supply.iter().zip(market.supply()) {
            if bearer > aggregate {
                return Err(Error::UnaccountedMintSupply);
            }
        }
        Ok(())
    }

    /// Return the exact Market key.
    pub const fn market(&self) -> [u8; 32] {
        self.market
    }

    /// Return the immutable Market generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Return the canonical manifest entry index.
    pub const fn manifest_entry_index(&self) -> u16 {
        self.manifest_entry_index
    }

    /// Borrow exact Token-2022-represented supplies.
    pub const fn accounted_supply(&self) -> &[u64; N] {
        &self.accounted_supply
    }

    pub(crate) fn credit(&mut self, outcome: usize, quantity: u64) -> Result<()> {
        let selected = self
            .accounted_supply
            .get_mut(outcome)
            .ok_or(Error::InvalidOutcome)?;
        *selected = selected
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    pub(crate) fn debit(&mut self, outcome: usize, quantity: u64) -> Result<()> {
        let selected = self
            .accounted_supply
            .get_mut(outcome)
            .ok_or(Error::InvalidOutcome)?;
        *selected = selected
            .checked_sub(quantity)
            .ok_or(Error::InsufficientTokenBalance)?;
        Ok(())
    }
}

/// Borrowed hostile-decoded bearer state with a runtime categorical width.
///
/// This is the adapter-facing equivalent of [`BearerCapabilityV1`].  It keeps
/// the canonical bytes borrowed so one SVM implementation can validate every
/// admitted width without monomorphizing the state parser and complete-set
/// loops fifteen times.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerCapabilityViewV1<'a> {
    bytes: &'a [u8],
    outcome_count: u8,
    market: [u8; 32],
    generation: u64,
    manifest_entry_index: u16,
}

impl<'a> BearerCapabilityViewV1<'a> {
    /// Return the checked exact account width, `64 + 8N` bytes.
    pub fn encoded_len(outcome_count: u8) -> Result<usize> {
        let outcomes = validate_runtime_width(outcome_count)?;
        outcomes
            .checked_mul(8)
            .and_then(|width| BEARER_STATE_BASE_BYTES.checked_add(width))
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Hostile-decode one exact canonical capability root.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        let outcome_count = byte(bytes, STATE_OUTCOME_COUNT_OFFSET)?;
        if bytes.len() != Self::encoded_len(outcome_count)? {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != BEARER_STATE_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != BEARER_STATE_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if byte(bytes, STATE_PROFILE_OFFSET)? != BEARER_PROFILE_V1 {
            return Err(Error::InvalidOutcomeCount);
        }
        require_zero(bytes, STATE_RESERVED_OFFSET, STATE_RESERVED_BYTES)?;
        require_zero(bytes, STATE_BODY_RESERVED_OFFSET, STATE_BODY_RESERVED_BYTES)?;
        let market = array(bytes, STATE_MARKET_OFFSET)?;
        require_nonzero(&market)?;
        let result = Self {
            bytes,
            outcome_count,
            market,
            generation: u64::from_le_bytes(array(bytes, STATE_GENERATION_OFFSET)?),
            manifest_entry_index: u16::from_le_bytes(array(bytes, STATE_ENTRY_INDEX_OFFSET)?),
        };
        let mut index = 0usize;
        while index < result.outcomes() {
            result.accounted_supply(index)?;
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(result)
    }

    /// Encode a newly activated, zero-supply capability atomically.
    pub fn encode_activated_into(
        output: &mut [u8],
        outcome_count: u8,
        market: [u8; 32],
        generation: u64,
        manifest_entry_index: u16,
    ) -> Result<()> {
        if output.len() != Self::encoded_len(outcome_count)? {
            return Err(Error::OutputLength);
        }
        require_nonzero(&market)?;
        output.fill(0);
        put(output, 0, &BEARER_STATE_MAGIC);
        put(output, 8, &BEARER_STATE_SCHEMA_VERSION.to_le_bytes());
        put(output, STATE_OUTCOME_COUNT_OFFSET, &[outcome_count]);
        put(output, STATE_PROFILE_OFFSET, &[BEARER_PROFILE_V1]);
        put(output, STATE_MARKET_OFFSET, &market);
        put(output, STATE_GENERATION_OFFSET, &generation.to_le_bytes());
        put(
            output,
            STATE_ENTRY_INDEX_OFFSET,
            &manifest_entry_index.to_le_bytes(),
        );
        Ok(())
    }

    /// Return the hostile-decoded categorical width.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }

    /// Return the exact Market key.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the canonical manifest entry index.
    pub const fn manifest_entry_index(self) -> u16 {
        self.manifest_entry_index
    }

    /// Return one exact Token-2022-represented supply.
    pub fn accounted_supply(self, outcome: usize) -> Result<u64> {
        if outcome >= self.outcomes() {
            return Err(Error::InvalidOutcome);
        }
        let offset = STATE_SUPPLY_OFFSET
            .checked_add(outcome.checked_mul(8).ok_or(Error::ArithmeticOverflow)?)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(u64::from_le_bytes(array(self.bytes, offset)?))
    }

    /// Borrow the exact canonical state preimage.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Validate Market/generation binding and representation subset bounds.
    pub fn validate_market(
        self,
        market_key: [u8; 32],
        generation: u64,
        aggregate_supply: &[u64],
    ) -> Result<()> {
        if self.market != market_key {
            return Err(Error::MarketMismatch);
        }
        if self.generation != generation {
            return Err(Error::GenerationMismatch);
        }
        if aggregate_supply.len() != self.outcomes() {
            return Err(Error::InvalidOutcomeCount);
        }
        for (index, aggregate) in aggregate_supply.iter().enumerate() {
            if self.accounted_supply(index)? > *aggregate {
                return Err(Error::UnaccountedMintSupply);
            }
        }
        Ok(())
    }

    /// Encode the same identity with caller-supplied supplies atomically.
    pub fn encode_with_supplies(self, output: &mut [u8], supplies: &[u64]) -> Result<()> {
        if output.len() != self.bytes.len() {
            return Err(Error::OutputLength);
        }
        if supplies.len() != self.outcomes() {
            return Err(Error::InvalidOutcomeCount);
        }
        output.copy_from_slice(self.bytes);
        for (index, supply) in supplies.iter().enumerate() {
            put(
                output,
                STATE_SUPPLY_OFFSET + index * 8,
                &supply.to_le_bytes(),
            );
        }
        Ok(())
    }

    const fn outcomes(self) -> usize {
        self.outcome_count as usize
    }
}

/// Exact ordered PDA seed projection for the direct capability child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerCapabilityDerivationV1 {
    market: [u8; 32],
    generation_le: [u8; 8],
}

impl BearerCapabilityDerivationV1 {
    /// Construct canonical seeds.
    pub fn new(market: [u8; 32], generation: u64) -> Result<Self> {
        require_nonzero(&market)?;
        Ok(Self {
            market,
            generation_le: generation.to_le_bytes(),
        })
    }

    /// Return seeds in canonical order.
    pub fn seeds(&self) -> [&[u8]; 3] {
        [
            BEARER_CAPABILITY_PDA_DOMAIN,
            self.market.as_slice(),
            self.generation_le.as_slice(),
        ]
    }
}

/// Exact ordered PDA seed projection for one outcome Mint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerMintDerivationV1 {
    market: [u8; 32],
    generation_le: [u8; 8],
    outcome: [u8; 1],
}

impl BearerMintDerivationV1 {
    /// Construct canonical seeds for one checked outcome.
    pub fn new<const N: usize>(market: [u8; 32], generation: u64, outcome: usize) -> Result<Self> {
        validate_width::<N>()?;
        Self::new_v1(
            market,
            generation,
            u8::try_from(N).map_err(|_| Error::InvalidOutcomeCount)?,
            outcome,
        )
    }

    /// Construct canonical seeds for one hostile runtime categorical width.
    pub fn new_v1(
        market: [u8; 32],
        generation: u64,
        outcome_count: u8,
        outcome: usize,
    ) -> Result<Self> {
        let outcomes = validate_runtime_width(outcome_count)?;
        require_nonzero(&market)?;
        if outcome >= outcomes {
            return Err(Error::InvalidOutcome);
        }
        Ok(Self {
            market,
            generation_le: generation.to_le_bytes(),
            outcome: [u8::try_from(outcome).map_err(|_| Error::InvalidOutcome)?],
        })
    }

    /// Return seeds in canonical order.
    pub fn seeds(&self) -> [&[u8]; 4] {
        [
            BEARER_MINT_PDA_DOMAIN,
            self.market.as_slice(),
            self.generation_le.as_slice(),
            self.outcome.as_slice(),
        ]
    }

    /// Return the exact outcome index byte.
    pub const fn outcome(self) -> u8 {
        self.outcome[0]
    }
}

/// Token-2022 account lifecycle projected by a hostile adapter parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenAccountStateV1 {
    /// Account is not initialized.
    Uninitialized,
    /// Account is initialized and thawed.
    Initialized,
    /// Account is initialized and frozen.
    Frozen,
}

/// Full-TLV projection of one canonical claim Mint.
///
/// `extension_count` counts every decoded TLV extension. Exactly
/// `MintCloseAuthority` and `PermissionedBurn` must be present. No permanent
/// delegate, transfer hook, transfer fee, non-transferable marker, default
/// state, confidential state, or unknown extension is admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintObservationV1 {
    /// Observed Mint address.
    pub key: [u8; 32],
    /// Observed owning program.
    pub program_owner: [u8; 32],
    /// Exact observed account-data width.
    pub data_len: usize,
    /// Observed raw supply.
    pub supply: u64,
    /// Observed display decimals.
    pub decimals: u8,
    /// Base Mint initialized flag.
    pub initialized: bool,
    /// Base mint authority.
    pub mint_authority: Option<[u8; 32]>,
    /// Base freeze authority.
    pub freeze_authority: Option<[u8; 32]>,
    /// MintCloseAuthority extension value.
    pub close_authority: Option<[u8; 32]>,
    /// PermissionedBurn extension authority.
    pub permissioned_burn_authority: Option<[u8; 32]>,
    /// Total count of decoded TLV extensions.
    pub extension_count: u16,
}

impl MintObservationV1 {
    /// Validate the exact extension and authority profile.
    pub fn validate_profile(self, expected_key: [u8; 32], controller: [u8; 32]) -> Result<()> {
        require_nonzero(&controller)?;
        if self.program_owner != TOKEN_2022_PROGRAM_ID {
            return Err(Error::WrongTokenProgram);
        }
        if self.data_len != BEARER_MINT_BYTES {
            return Err(Error::WrongMintExtensions);
        }
        if self.key != expected_key {
            return Err(Error::WrongMint);
        }
        if !self.initialized {
            return Err(Error::UninitializedTokenState);
        }
        if self.decimals != 0 {
            return Err(Error::WrongDecimals);
        }
        if self.mint_authority != Some(controller)
            || self.freeze_authority.is_some()
            || self.close_authority != Some(controller)
            || self.permissioned_burn_authority != Some(controller)
        {
            return Err(Error::WrongAuthority);
        }
        if self.extension_count != 2 {
            return Err(Error::WrongMintExtensions);
        }
        Ok(())
    }
}

/// Projection of one Token-2022 claim-holding account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenAccountObservationV1 {
    /// Token Account address.
    pub key: [u8; 32],
    /// Observed owning program.
    pub program_owner: [u8; 32],
    /// Exact observed account-data width.
    pub data_len: usize,
    /// Claim Mint address.
    pub mint: [u8; 32],
    /// Current token authority.
    pub authority: [u8; 32],
    /// Raw claim atoms held.
    pub amount: u64,
    /// Account lifecycle state.
    pub state: TokenAccountStateV1,
    /// Whether a wrapped-native reserve is present.
    pub has_native_reserve: bool,
    /// Total count of decoded token-Account TLV extensions.
    pub extension_count: u16,
}

impl TokenAccountObservationV1 {
    /// Validate a holder-owned, initialized base claim account.
    pub fn validate_holder(
        self,
        mint: [u8; 32],
        holder: [u8; 32],
        minimum_amount: u64,
    ) -> Result<()> {
        require_nonzero(&self.key)?;
        require_nonzero(&holder)?;
        if self.program_owner != TOKEN_2022_PROGRAM_ID {
            return Err(Error::WrongTokenProgram);
        }
        if self.data_len != BEARER_TOKEN_ACCOUNT_BYTES {
            return Err(Error::WrongTokenAccountExtensions);
        }
        if self.mint != mint {
            return Err(Error::WrongMint);
        }
        if self.authority != holder {
            return Err(Error::WrongAuthority);
        }
        if self.state != TokenAccountStateV1::Initialized {
            return Err(Error::TokenAccountNotTransferable);
        }
        if self.has_native_reserve {
            return Err(Error::NativeTokenAccount);
        }
        if self.extension_count != 0 {
            return Err(Error::WrongTokenAccountExtensions);
        }
        if self.amount < minimum_amount {
            return Err(Error::InsufficientTokenBalance);
        }
        Ok(())
    }
}

pub(crate) fn validate_width<const N: usize>() -> Result<()> {
    if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&N) {
        Err(Error::InvalidOutcomeCount)
    } else {
        Ok(())
    }
}

fn validate_runtime_width(outcome_count: u8) -> Result<usize> {
    let outcomes = usize::from(outcome_count);
    if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&outcomes) {
        Err(Error::InvalidOutcomeCount)
    } else {
        Ok(outcomes)
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(Error::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod runtime_view_tests {
    use super::{BEARER_STATE_BASE_BYTES, BearerCapabilityV1, BearerCapabilityViewV1, Error};

    #[test]
    fn runtime_view_matches_typed_state_and_refuses_noncanonical_widths() {
        let mut state = BearerCapabilityV1::<2>::activated([7; 32], 11, 3).expect("state");
        state.credit(0, 19).expect("credit");
        let mut bytes = [0u8; BEARER_STATE_BASE_BYTES + 16];
        state.encode(&mut bytes).expect("encode");

        let view = BearerCapabilityViewV1::decode(&bytes).expect("runtime decode");
        assert_eq!(view.outcome_count(), 2);
        assert_eq!(view.market(), [7; 32]);
        assert_eq!(view.generation(), 11);
        assert_eq!(view.manifest_entry_index(), 3);
        assert_eq!(view.accounted_supply(0), Ok(19));
        assert_eq!(view.accounted_supply(1), Ok(0));
        assert_eq!(view.accounted_supply(2), Err(Error::InvalidOutcome));

        let mut dirty_count = bytes;
        dirty_count[10] = 1;
        assert_eq!(
            BearerCapabilityViewV1::decode(&dirty_count),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(
            BearerCapabilityViewV1::decode(
                bytes.get(..bytes.len() - 1).expect("nonempty state bytes")
            ),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn runtime_supply_encode_refusal_is_atomic() {
        let mut bytes = [0u8; BEARER_STATE_BASE_BYTES + 16];
        BearerCapabilityViewV1::encode_activated_into(&mut bytes, 2, [8; 32], 12, 4)
            .expect("activate bytes");
        let view = BearerCapabilityViewV1::decode(&bytes).expect("decode");
        let mut output = [0xa5; BEARER_STATE_BASE_BYTES + 16];
        let before = output;
        assert_eq!(
            view.encode_with_supplies(&mut output, &[1]),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(output, before);

        view.encode_with_supplies(&mut output, &[9, 10])
            .expect("supply encode");
        let after = BearerCapabilityViewV1::decode(&output).expect("decode after");
        assert_eq!(after.accounted_supply(0), Ok(9));
        assert_eq!(after.accounted_supply(1), Ok(10));
    }
}
