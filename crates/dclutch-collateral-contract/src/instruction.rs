//! Exact fixed-width instruction bytes with canonical semantic dispatch.

use core::convert::TryInto;

use dclutch_core_contract::{MARKET_IDENTITY_BYTES, MarketIdentity};
use dclutch_realm_contract::{MAX_OUTCOMES, MIN_OUTCOMES, REALM_BYTES, RealmV1};

use crate::{Error, Result};

/// Header width shared by every collateral lifecycle instruction.
pub const HEADER_BYTES: usize = 16;
/// Canonical collateral instruction magic.
pub const INSTRUCTION_MAGIC: [u8; 8] = *b"DCLTCOL1";
/// Implemented collateral instruction schema.
pub const INSTRUCTION_SCHEMA_VERSION: u16 = 1;

const TAG_OFFSET: usize = 10;
const FLAGS_OFFSET: usize = 11;
const RESERVED_OFFSET: usize = 12;
const RESERVED_BYTES: usize = 4;

const GENERATION_OFFSET: usize = HEADER_BYTES;
const CHILD_COUNT_OFFSET: usize = HEADER_BYTES + 8;
const QUANTITY_OFFSET: usize = HEADER_BYTES + 8;
const CREATE_POSITION_QUANTITY_OFFSET: usize = HEADER_BYTES + 16;

const FOUND_IDENTITY_OFFSET: usize = HEADER_BYTES;
const FOUND_OUTCOME_COUNT_OFFSET: usize = FOUND_IDENTITY_OFFSET + MARKET_IDENTITY_BYTES;
const FOUND_RESERVED_OFFSET: usize = FOUND_OUTCOME_COUNT_OFFSET + 1;
const FOUND_RESERVED_BYTES: usize = 7;

const REDEEM_OUTCOME_OFFSET: usize = HEADER_BYTES + 16;
const REDEEM_RESERVED_OFFSET: usize = REDEEM_OUTCOME_OFFSET + 1;
const REDEEM_RESERVED_BYTES: usize = 7;

/// Exact immutable-Realm creation instruction width.
pub const CREATE_REALM_BYTES: usize = HEADER_BYTES + REALM_BYTES;
/// Exact atomic Market and resolution-Fund founding instruction width.
pub const FOUND_MARKET_AND_FUND_BYTES: usize = HEADER_BYTES + MARKET_IDENTITY_BYTES + 8;
/// Exact collateral-Vault initialization and Market-open instruction width.
pub const OPEN_COLLATERAL_VAULT_BYTES: usize = HEADER_BYTES + 16;
/// Exact Position creation and first complete-set split instruction width.
pub const CREATE_POSITION_AND_SPLIT_BYTES: usize = HEADER_BYTES + 24;
/// Exact existing-Position complete-set split instruction width.
pub const SPLIT_COMPLETE_SET_BYTES: usize = HEADER_BYTES + 16;
/// Exact complete-set merge instruction width.
pub const MERGE_COMPLETE_SET_BYTES: usize = HEADER_BYTES + 16;
/// Exact resolved-outcome redemption instruction width.
pub const REDEEM_RESOLVED_OUTCOME_BYTES: usize = HEADER_BYTES + 24;
/// Exact permissionless surplus-sweep instruction width.
pub const SWEEP_SURPLUS_BYTES: usize = HEADER_BYTES + 8;
/// Exact empty-Position close instruction width.
pub const CLOSE_EMPTY_POSITION_BYTES: usize = HEADER_BYTES + 16;
/// Exact empty collateral-Vault retirement instruction width.
pub const RETIRE_EMPTY_VAULT_BYTES: usize = HEADER_BYTES + 16;

/// Canonical semantic instruction family tags.
///
/// Values are grouped by lifecycle responsibility rather than inherited from
/// any historical action table. They are wire commitments and may not be
/// renumbered within schema V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InstructionTag {
    /// Create one immutable reusable Realm.
    CreateRealm = 0x10,
    /// Atomically found a Market and its prepaid resolution Fund.
    FoundMarketAndFund = 0x20,
    /// Initialize collateral custody and transition the Market open.
    OpenCollateralVault = 0x30,
    /// Create one Position and perform its first complete-set split.
    CreatePositionAndSplit = 0x40,
    /// Split collateral into an existing Position's complete set.
    SplitCompleteSet = 0x41,
    /// Merge an existing Position's complete set back to collateral.
    MergeCompleteSet = 0x42,
    /// Redeem one resolved outcome balance.
    RedeemResolvedOutcome = 0x43,
    /// Sweep only physical collateral above the Market Hoard.
    SweepSurplus = 0x44,
    /// Close an empty Position and retire its direct child.
    CloseEmptyPosition = 0x50,
    /// Close an empty Vault and retire its direct child.
    RetireEmptyVault = 0x51,
}

impl InstructionTag {
    const fn decode(byte: u8) -> Result<Self> {
        match byte {
            0x10 => Ok(Self::CreateRealm),
            0x20 => Ok(Self::FoundMarketAndFund),
            0x30 => Ok(Self::OpenCollateralVault),
            0x40 => Ok(Self::CreatePositionAndSplit),
            0x41 => Ok(Self::SplitCompleteSet),
            0x42 => Ok(Self::MergeCompleteSet),
            0x43 => Ok(Self::RedeemResolvedOutcome),
            0x44 => Ok(Self::SweepSurplus),
            0x50 => Ok(Self::CloseEmptyPosition),
            0x51 => Ok(Self::RetireEmptyVault),
            _ => Err(Error::UnknownInstructionTag),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::CreateRealm => 0x10,
            Self::FoundMarketAndFund => 0x20,
            Self::OpenCollateralVault => 0x30,
            Self::CreatePositionAndSplit => 0x40,
            Self::SplitCompleteSet => 0x41,
            Self::MergeCompleteSet => 0x42,
            Self::RedeemResolvedOutcome => 0x43,
            Self::SweepSurplus => 0x44,
            Self::CloseEmptyPosition => 0x50,
            Self::RetireEmptyVault => 0x51,
        }
    }
}

/// One exactly decoded collateral lifecycle instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstructionV1 {
    /// Immutable Realm creation.
    CreateRealm(CreateRealmV1),
    /// Atomic Market and resolution-Fund founding.
    FoundMarketAndFund(FoundMarketAndFundV1),
    /// Collateral-Vault initialization and Market opening.
    OpenCollateralVault(OpenCollateralVaultV1),
    /// Position creation combined with its first split.
    CreatePositionAndSplit(CreatePositionAndSplitV1),
    /// Existing-Position complete-set split.
    SplitCompleteSet(SplitCompleteSetV1),
    /// Complete-set merge.
    MergeCompleteSet(MergeCompleteSetV1),
    /// Resolved outcome redemption.
    RedeemResolvedOutcome(RedeemResolvedOutcomeV1),
    /// Permissionless physical surplus sweep.
    SweepSurplus(SweepSurplusV1),
    /// Empty Position close.
    CloseEmptyPosition(CloseEmptyPositionV1),
    /// Empty collateral Vault retirement.
    RetireEmptyVault(RetireEmptyVaultV1),
}

impl InstructionV1 {
    /// Dispatch and decode one exact instruction, refusing trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let tag = decode_header(bytes)?;
        match tag {
            InstructionTag::CreateRealm => CreateRealmV1::decode(bytes).map(Self::CreateRealm),
            InstructionTag::FoundMarketAndFund => {
                FoundMarketAndFundV1::decode(bytes).map(Self::FoundMarketAndFund)
            }
            InstructionTag::OpenCollateralVault => {
                OpenCollateralVaultV1::decode(bytes).map(Self::OpenCollateralVault)
            }
            InstructionTag::CreatePositionAndSplit => {
                CreatePositionAndSplitV1::decode(bytes).map(Self::CreatePositionAndSplit)
            }
            InstructionTag::SplitCompleteSet => {
                SplitCompleteSetV1::decode(bytes).map(Self::SplitCompleteSet)
            }
            InstructionTag::MergeCompleteSet => {
                MergeCompleteSetV1::decode(bytes).map(Self::MergeCompleteSet)
            }
            InstructionTag::RedeemResolvedOutcome => {
                RedeemResolvedOutcomeV1::decode(bytes).map(Self::RedeemResolvedOutcome)
            }
            InstructionTag::SweepSurplus => SweepSurplusV1::decode(bytes).map(Self::SweepSurplus),
            InstructionTag::CloseEmptyPosition => {
                CloseEmptyPositionV1::decode(bytes).map(Self::CloseEmptyPosition)
            }
            InstructionTag::RetireEmptyVault => {
                RetireEmptyVaultV1::decode(bytes).map(Self::RetireEmptyVault)
            }
        }
    }

    /// Return this instruction's semantic tag.
    pub const fn tag(self) -> InstructionTag {
        match self {
            Self::CreateRealm(_) => InstructionTag::CreateRealm,
            Self::FoundMarketAndFund(_) => InstructionTag::FoundMarketAndFund,
            Self::OpenCollateralVault(_) => InstructionTag::OpenCollateralVault,
            Self::CreatePositionAndSplit(_) => InstructionTag::CreatePositionAndSplit,
            Self::SplitCompleteSet(_) => InstructionTag::SplitCompleteSet,
            Self::MergeCompleteSet(_) => InstructionTag::MergeCompleteSet,
            Self::RedeemResolvedOutcome(_) => InstructionTag::RedeemResolvedOutcome,
            Self::SweepSurplus(_) => InstructionTag::SweepSurplus,
            Self::CloseEmptyPosition(_) => InstructionTag::CloseEmptyPosition,
            Self::RetireEmptyVault(_) => InstructionTag::RetireEmptyVault,
        }
    }
}

/// Immutable Realm creation body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateRealmV1 {
    realm: RealmV1,
}

impl CreateRealmV1 {
    /// Construct from one already validated canonical Realm.
    pub const fn new(realm: RealmV1) -> Self {
        Self { realm }
    }

    /// Decode the exact fixed body and revalidate its Realm contract.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(bytes, CREATE_REALM_BYTES, InstructionTag::CreateRealm)?;
        let realm_bytes = bytes
            .get(HEADER_BYTES..CREATE_REALM_BYTES)
            .ok_or(Error::InvalidLength)?;
        let realm = RealmV1::decode(realm_bytes).map_err(|error| Error::InvalidRealm { error })?;
        Ok(Self::new(realm))
    }

    /// Encode into the exact caller-owned output without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        let mut encoded = [0; CREATE_REALM_BYTES];
        write_header(&mut encoded, InstructionTag::CreateRealm)?;
        put(&mut encoded, HEADER_BYTES, &self.realm.to_bytes())?;
        commit(output, &encoded)
    }

    /// Return the validated immutable Realm.
    pub const fn realm(self) -> RealmV1 {
        self.realm
    }
}

/// Atomic Market and prepaid resolution-Fund founding body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundMarketAndFundV1 {
    identity: MarketIdentity,
    outcome_count: u8,
}

impl FoundMarketAndFundV1 {
    /// Validate founding facts.
    ///
    /// The sponsor account is persisted by the Fund implementation as its
    /// refund recipient. Before creating accounts, the adapter must hash and
    /// match the Realm, resolution-policy, and capability-manifest records
    /// supplied by the exact account frame. The manifest must uniquely select
    /// the `RequiredAtFounding` entry whose config equals the Market's
    /// resolution-policy identity. That immutable entry, never instruction
    /// data, owns exact Fund rent, provider reimbursement, and positive bounty.
    /// The adapter must also authenticate the explicit Product Instance,
    /// categorical ClaimBasis, and CapacityProfile records, validate their
    /// links and `outcome_count`, and match both Product IDs in `identity`.
    pub fn new(identity: MarketIdentity, outcome_count: u8) -> Result<Self> {
        validate_outcome_count(outcome_count)?;
        Ok(Self {
            identity,
            outcome_count,
        })
    }

    /// Decode exact founding bytes and the canonical Market identity preimage.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            FOUND_MARKET_AND_FUND_BYTES,
            InstructionTag::FoundMarketAndFund,
        )?;
        require_zero(bytes, FOUND_RESERVED_OFFSET, FOUND_RESERVED_BYTES)?;
        let identity_end = FOUND_IDENTITY_OFFSET
            .checked_add(MARKET_IDENTITY_BYTES)
            .ok_or(Error::InvalidLength)?;
        let identity = MarketIdentity::decode(
            bytes
                .get(FOUND_IDENTITY_OFFSET..identity_end)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|error| Error::InvalidMarketIdentity { error })?;
        Self::new(identity, read_byte(bytes, FOUND_OUTCOME_COUNT_OFFSET)?)
    }

    /// Encode into the exact caller-owned output without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        let mut encoded = [0; FOUND_MARKET_AND_FUND_BYTES];
        write_header(&mut encoded, InstructionTag::FoundMarketAndFund)?;
        put(
            &mut encoded,
            FOUND_IDENTITY_OFFSET,
            &self.identity.to_bytes(),
        )?;
        put(
            &mut encoded,
            FOUND_OUTCOME_COUNT_OFFSET,
            &[self.outcome_count],
        )?;
        commit(output, &encoded)
    }

    /// Return the canonical immutable Market identity.
    pub const fn identity(self) -> MarketIdentity {
        self.identity
    }

    /// Return the exhaustive ordered categorical outcome count.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }
}

/// Collateral-Vault initialization and Market-open replay body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenCollateralVaultV1 {
    generation: u64,
    child_count: u64,
}

impl OpenCollateralVaultV1 {
    /// Construct from exact pre-mutation replay facts.
    ///
    /// The transition registers exactly one direct collateral-custody root;
    /// its deterministically derived token Vault is a contained physical
    /// descendant rather than a second direct Market child.
    pub const fn new(generation: u64, child_count: u64) -> Self {
        Self {
            generation,
            child_count,
        }
    }

    /// Decode one exact body.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode_replay(
            bytes,
            OPEN_COLLATERAL_VAULT_BYTES,
            InstructionTag::OpenCollateralVault,
        )
        .map(|(generation, child_count)| Self::new(generation, child_count))
    }

    /// Encode into the exact caller-owned output without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        encode_replay(
            output,
            InstructionTag::OpenCollateralVault,
            self.generation,
            self.child_count,
        )
    }

    /// Return immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return expected pre-mutation direct-child count.
    pub const fn child_count(self) -> u64 {
        self.child_count
    }
}

/// Position creation plus first nonzero complete-set split body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreatePositionAndSplitV1 {
    generation: u64,
    child_count: u64,
    quantity: u64,
}

impl CreatePositionAndSplitV1 {
    /// Validate and construct from exact replay facts and raw collateral atoms.
    pub fn new(generation: u64, child_count: u64, quantity: u64) -> Result<Self> {
        require_quantity(quantity)?;
        Ok(Self {
            generation,
            child_count,
            quantity,
        })
    }

    /// Decode one exact body.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            CREATE_POSITION_AND_SPLIT_BYTES,
            InstructionTag::CreatePositionAndSplit,
        )?;
        Self::new(
            read_u64(bytes, GENERATION_OFFSET)?,
            read_u64(bytes, CHILD_COUNT_OFFSET)?,
            read_u64(bytes, CREATE_POSITION_QUANTITY_OFFSET)?,
        )
    }

    /// Encode into the exact caller-owned output without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        let mut encoded = [0; CREATE_POSITION_AND_SPLIT_BYTES];
        write_header(&mut encoded, InstructionTag::CreatePositionAndSplit)?;
        write_u64(&mut encoded, GENERATION_OFFSET, self.generation)?;
        write_u64(&mut encoded, CHILD_COUNT_OFFSET, self.child_count)?;
        write_u64(&mut encoded, CREATE_POSITION_QUANTITY_OFFSET, self.quantity)?;
        commit(output, &encoded)
    }

    /// Return immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return expected pre-mutation direct-child count.
    pub const fn child_count(self) -> u64 {
        self.child_count
    }

    /// Return exact raw collateral atoms split into each outcome claim.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }
}

/// Existing-Position nonzero complete-set split body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitCompleteSetV1 {
    generation: u64,
    quantity: u64,
}

impl SplitCompleteSetV1 {
    /// Validate and construct from exact generation and raw collateral atoms.
    pub fn new(generation: u64, quantity: u64) -> Result<Self> {
        require_quantity(quantity)?;
        Ok(Self {
            generation,
            quantity,
        })
    }

    /// Decode one exact body.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode_quantity(
            bytes,
            SPLIT_COMPLETE_SET_BYTES,
            InstructionTag::SplitCompleteSet,
        )
        .and_then(|(generation, quantity)| Self::new(generation, quantity))
    }

    /// Encode into the exact caller-owned output without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        encode_quantity(
            output,
            InstructionTag::SplitCompleteSet,
            self.generation,
            self.quantity,
        )
    }

    /// Return immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return exact raw collateral atoms split into each outcome claim.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }
}

/// Existing-Position nonzero complete-set merge body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergeCompleteSetV1 {
    generation: u64,
    quantity: u64,
}

impl MergeCompleteSetV1 {
    /// Validate and construct from exact generation and claim atoms.
    pub fn new(generation: u64, quantity: u64) -> Result<Self> {
        require_quantity(quantity)?;
        Ok(Self {
            generation,
            quantity,
        })
    }

    /// Decode one exact body.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode_quantity(
            bytes,
            MERGE_COMPLETE_SET_BYTES,
            InstructionTag::MergeCompleteSet,
        )
        .and_then(|(generation, quantity)| Self::new(generation, quantity))
    }

    /// Encode into the exact caller-owned output without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        encode_quantity(
            output,
            InstructionTag::MergeCompleteSet,
            self.generation,
            self.quantity,
        )
    }

    /// Return immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return exact claim atoms removed from every outcome.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }
}

/// Nonzero resolved-outcome redemption body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RedeemResolvedOutcomeV1 {
    generation: u64,
    quantity: u64,
    outcome: u8,
}

impl RedeemResolvedOutcomeV1 {
    /// Validate generation, claim quantity, and the provisional outcome bound.
    ///
    /// The composing Market adapter must additionally require `outcome` below
    /// the actual Market width and derive its winning or losing status solely
    /// from the canonical onchain receipt.
    pub fn new(generation: u64, outcome: u8, quantity: u64) -> Result<Self> {
        require_quantity(quantity)?;
        if usize::from(outcome) >= MAX_OUTCOMES {
            return Err(Error::InvalidOutcomeCount);
        }
        Ok(Self {
            generation,
            quantity,
            outcome,
        })
    }

    /// Decode one exact body and reject nonzero alignment bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(
            bytes,
            REDEEM_RESOLVED_OUTCOME_BYTES,
            InstructionTag::RedeemResolvedOutcome,
        )?;
        require_zero(bytes, REDEEM_RESERVED_OFFSET, REDEEM_RESERVED_BYTES)?;
        Self::new(
            read_u64(bytes, GENERATION_OFFSET)?,
            read_byte(bytes, REDEEM_OUTCOME_OFFSET)?,
            read_u64(bytes, QUANTITY_OFFSET)?,
        )
    }

    /// Encode into the exact caller-owned output without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        let mut encoded = [0; REDEEM_RESOLVED_OUTCOME_BYTES];
        write_header(&mut encoded, InstructionTag::RedeemResolvedOutcome)?;
        write_u64(&mut encoded, GENERATION_OFFSET, self.generation)?;
        write_u64(&mut encoded, QUANTITY_OFFSET, self.quantity)?;
        put(&mut encoded, REDEEM_OUTCOME_OFFSET, &[self.outcome])?;
        commit(output, &encoded)
    }

    /// Return immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return exact claim atoms to burn from the selected outcome.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    /// Return the selected ordered outcome index.
    pub const fn outcome(self) -> u8 {
        self.outcome
    }
}

/// Permissionless all-surplus sweep replay body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SweepSurplusV1 {
    generation: u64,
}

impl SweepSurplusV1 {
    /// Construct from immutable Market generation.
    pub const fn new(generation: u64) -> Self {
        Self { generation }
    }

    /// Decode one exact body.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_header(bytes, SWEEP_SURPLUS_BYTES, InstructionTag::SweepSurplus)?;
        Ok(Self::new(read_u64(bytes, GENERATION_OFFSET)?))
    }

    /// Encode into the exact caller-owned output without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        let mut encoded = [0; SWEEP_SURPLUS_BYTES];
        write_header(&mut encoded, InstructionTag::SweepSurplus)?;
        write_u64(&mut encoded, GENERATION_OFFSET, self.generation)?;
        commit(output, &encoded)
    }

    /// Return immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// Empty-Position close and child-retirement replay body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseEmptyPositionV1 {
    generation: u64,
    child_count: u64,
}

impl CloseEmptyPositionV1 {
    /// Construct from exact pre-mutation replay facts.
    pub const fn new(generation: u64, child_count: u64) -> Self {
        Self {
            generation,
            child_count,
        }
    }

    /// Decode one exact body.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode_replay(
            bytes,
            CLOSE_EMPTY_POSITION_BYTES,
            InstructionTag::CloseEmptyPosition,
        )
        .map(|(generation, child_count)| Self::new(generation, child_count))
    }

    /// Encode into the exact caller-owned output without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        encode_replay(
            output,
            InstructionTag::CloseEmptyPosition,
            self.generation,
            self.child_count,
        )
    }

    /// Return immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return expected pre-mutation direct-child count.
    pub const fn child_count(self) -> u64 {
        self.child_count
    }
}

/// Empty collateral-Vault retirement and child-retirement replay body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetireEmptyVaultV1 {
    generation: u64,
    child_count: u64,
}

impl RetireEmptyVaultV1 {
    /// Construct from exact pre-mutation replay facts.
    ///
    /// After proving zero Hoard, zero claim supplies, and an empty token Vault,
    /// the adapter closes the Vault and custody root to the recorded rent
    /// refund recipient and retires exactly one direct child.
    pub const fn new(generation: u64, child_count: u64) -> Self {
        Self {
            generation,
            child_count,
        }
    }

    /// Decode one exact body.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode_replay(
            bytes,
            RETIRE_EMPTY_VAULT_BYTES,
            InstructionTag::RetireEmptyVault,
        )
        .map(|(generation, child_count)| Self::new(generation, child_count))
    }

    /// Encode into the exact caller-owned output without partial mutation.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        encode_replay(
            output,
            InstructionTag::RetireEmptyVault,
            self.generation,
            self.child_count,
        )
    }

    /// Return immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return expected pre-mutation direct-child count.
    pub const fn child_count(self) -> u64 {
        self.child_count
    }
}

fn validate_outcome_count(outcome_count: u8) -> Result<()> {
    let count = usize::from(outcome_count);
    if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&count) {
        return Err(Error::InvalidOutcomeCount);
    }
    Ok(())
}

const fn require_quantity(quantity: u64) -> Result<()> {
    if quantity == 0 {
        Err(Error::ZeroQuantity)
    } else {
        Ok(())
    }
}

fn decode_header(bytes: &[u8]) -> Result<InstructionTag> {
    if bytes.len() < HEADER_BYTES {
        return Err(Error::InvalidLength);
    }
    if read_array::<8>(bytes, 0)? != INSTRUCTION_MAGIC {
        return Err(Error::InvalidMagic);
    }
    if u16::from_le_bytes(read_array(bytes, 8)?) != INSTRUCTION_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema);
    }
    if read_byte(bytes, FLAGS_OFFSET)? != 0 {
        return Err(Error::NonCanonicalFlags);
    }
    require_zero(bytes, RESERVED_OFFSET, RESERVED_BYTES)?;
    InstructionTag::decode(read_byte(bytes, TAG_OFFSET)?)
}

fn require_header(bytes: &[u8], expected: usize, tag: InstructionTag) -> Result<()> {
    if bytes.len() != expected {
        return Err(Error::InvalidLength);
    }
    if decode_header(bytes)? != tag {
        return Err(Error::InstructionTagMismatch);
    }
    Ok(())
}

fn write_header(output: &mut [u8], tag: InstructionTag) -> Result<()> {
    if output.len() < HEADER_BYTES {
        return Err(Error::OutputLength);
    }
    put(output, 0, &INSTRUCTION_MAGIC)?;
    put(output, 8, &INSTRUCTION_SCHEMA_VERSION.to_le_bytes())?;
    put(output, TAG_OFFSET, &[tag.byte()])?;
    Ok(())
}

fn decode_replay(bytes: &[u8], expected: usize, tag: InstructionTag) -> Result<(u64, u64)> {
    require_header(bytes, expected, tag)?;
    Ok((
        read_u64(bytes, GENERATION_OFFSET)?,
        read_u64(bytes, CHILD_COUNT_OFFSET)?,
    ))
}

fn encode_replay(
    output: &mut [u8],
    tag: InstructionTag,
    generation: u64,
    child_count: u64,
) -> Result<()> {
    if output.len() != OPEN_COLLATERAL_VAULT_BYTES {
        return Err(Error::OutputLength);
    }
    let mut encoded = [0; OPEN_COLLATERAL_VAULT_BYTES];
    write_header(&mut encoded, tag)?;
    write_u64(&mut encoded, GENERATION_OFFSET, generation)?;
    write_u64(&mut encoded, CHILD_COUNT_OFFSET, child_count)?;
    commit(output, &encoded)
}

fn decode_quantity(bytes: &[u8], expected: usize, tag: InstructionTag) -> Result<(u64, u64)> {
    require_header(bytes, expected, tag)?;
    Ok((
        read_u64(bytes, GENERATION_OFFSET)?,
        read_u64(bytes, QUANTITY_OFFSET)?,
    ))
}

fn encode_quantity(
    output: &mut [u8],
    tag: InstructionTag,
    generation: u64,
    quantity: u64,
) -> Result<()> {
    if output.len() != SPLIT_COMPLETE_SET_BYTES {
        return Err(Error::OutputLength);
    }
    let mut encoded = [0; SPLIT_COMPLETE_SET_BYTES];
    write_header(&mut encoded, tag)?;
    write_u64(&mut encoded, GENERATION_OFFSET, generation)?;
    write_u64(&mut encoded, QUANTITY_OFFSET, quantity)?;
    commit(output, &encoded)
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
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
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn write_u64(output: &mut [u8], offset: usize, value: u64) -> Result<()> {
    put(output, offset, &value.to_le_bytes())
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) -> Result<()> {
    let end = offset.checked_add(input.len()).ok_or(Error::OutputLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::OutputLength)?
        .copy_from_slice(input);
    Ok(())
}

fn commit(output: &mut [u8], encoded: &[u8]) -> Result<()> {
    if output.len() != encoded.len() {
        return Err(Error::OutputLength);
    }
    output.copy_from_slice(encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::ContentId;
    use dclutch_realm_contract::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1Input};

    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero content identity")
    }

    fn identity() -> MarketIdentity {
        MarketIdentity::new(id(1), id(2), id(3), id(4), id(5), 9)
    }

    fn realm() -> RealmV1 {
        RealmV1::new(RealmV1Input {
            collateral_semantic_id: [1; 32],
            token_program: [2; 32],
            collateral_mint: [3; 32],
            collateral_adapter_release_id: [4; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("valid Realm")
    }

    fn encode_all() -> [([u8; 256], usize, InstructionTag); 10] {
        let mut outputs = [([0; 256], 0, InstructionTag::CreateRealm); 10];
        let mut cursor = 0usize;

        let mut create_realm = [0; CREATE_REALM_BYTES];
        CreateRealmV1::new(realm())
            .encode(&mut create_realm)
            .expect("Realm encoding");
        put_case(
            &mut outputs,
            &mut cursor,
            &create_realm,
            InstructionTag::CreateRealm,
        );

        let mut found = [0; FOUND_MARKET_AND_FUND_BYTES];
        FoundMarketAndFundV1::new(identity(), 3)
            .expect("valid founding")
            .encode(&mut found)
            .expect("founding encoding");
        put_case(
            &mut outputs,
            &mut cursor,
            &found,
            InstructionTag::FoundMarketAndFund,
        );

        let mut open = [0; OPEN_COLLATERAL_VAULT_BYTES];
        OpenCollateralVaultV1::new(9, 1)
            .encode(&mut open)
            .expect("open encoding");
        put_case(
            &mut outputs,
            &mut cursor,
            &open,
            InstructionTag::OpenCollateralVault,
        );

        let mut create_position = [0; CREATE_POSITION_AND_SPLIT_BYTES];
        CreatePositionAndSplitV1::new(9, 2, 13)
            .expect("valid position split")
            .encode(&mut create_position)
            .expect("position encoding");
        put_case(
            &mut outputs,
            &mut cursor,
            &create_position,
            InstructionTag::CreatePositionAndSplit,
        );

        let mut split = [0; SPLIT_COMPLETE_SET_BYTES];
        SplitCompleteSetV1::new(9, 13)
            .expect("valid split")
            .encode(&mut split)
            .expect("split encoding");
        put_case(
            &mut outputs,
            &mut cursor,
            &split,
            InstructionTag::SplitCompleteSet,
        );

        let mut merge = [0; MERGE_COMPLETE_SET_BYTES];
        MergeCompleteSetV1::new(9, 13)
            .expect("valid merge")
            .encode(&mut merge)
            .expect("merge encoding");
        put_case(
            &mut outputs,
            &mut cursor,
            &merge,
            InstructionTag::MergeCompleteSet,
        );

        let mut redeem = [0; REDEEM_RESOLVED_OUTCOME_BYTES];
        RedeemResolvedOutcomeV1::new(9, 2, 13)
            .expect("valid redemption")
            .encode(&mut redeem)
            .expect("redemption encoding");
        put_case(
            &mut outputs,
            &mut cursor,
            &redeem,
            InstructionTag::RedeemResolvedOutcome,
        );

        let mut sweep = [0; SWEEP_SURPLUS_BYTES];
        SweepSurplusV1::new(9)
            .encode(&mut sweep)
            .expect("sweep encoding");
        put_case(
            &mut outputs,
            &mut cursor,
            &sweep,
            InstructionTag::SweepSurplus,
        );

        let mut close = [0; CLOSE_EMPTY_POSITION_BYTES];
        CloseEmptyPositionV1::new(9, 3)
            .encode(&mut close)
            .expect("close encoding");
        put_case(
            &mut outputs,
            &mut cursor,
            &close,
            InstructionTag::CloseEmptyPosition,
        );

        let mut retire = [0; RETIRE_EMPTY_VAULT_BYTES];
        RetireEmptyVaultV1::new(9, 1)
            .encode(&mut retire)
            .expect("retire encoding");
        put_case(
            &mut outputs,
            &mut cursor,
            &retire,
            InstructionTag::RetireEmptyVault,
        );

        outputs
    }

    fn put_case(
        cases: &mut [([u8; 256], usize, InstructionTag); 10],
        cursor: &mut usize,
        input: &[u8],
        tag: InstructionTag,
    ) {
        if let Some((bytes, length, destination_tag)) = cases.get_mut(*cursor) {
            bytes
                .get_mut(..input.len())
                .expect("bounded fixture")
                .copy_from_slice(input);
            *length = input.len();
            *destination_tag = tag;
        }
        *cursor += 1;
    }

    #[test]
    fn every_semantic_family_round_trips_with_exact_end_of_input() {
        for (bytes, length, tag) in encode_all() {
            let exact = bytes.get(..length).expect("fixture length");
            let decoded = InstructionV1::decode(exact).expect("canonical instruction");
            assert_eq!(decoded.tag(), tag);
            let trailing = bytes.get(..length + 1).expect("one trailing byte");
            assert_eq!(InstructionV1::decode(trailing), Err(Error::InvalidLength));
        }
    }

    #[test]
    fn hostile_common_headers_and_all_truncations_refuse() {
        for (bytes, length, _) in encode_all() {
            for truncated in 0..length {
                let short = bytes.get(..truncated).expect("bounded truncation");
                assert_eq!(InstructionV1::decode(short), Err(Error::InvalidLength));
            }
        }

        let mut encoded = [0; SWEEP_SURPLUS_BYTES];
        SweepSurplusV1::new(9)
            .encode(&mut encoded)
            .expect("canonical sweep");
        for (offset, expected) in [
            (0usize, Error::InvalidMagic),
            (8, Error::UnsupportedSchema),
            (11, Error::NonCanonicalFlags),
            (12, Error::NonCanonicalReservedBytes),
        ] {
            let mut changed = encoded;
            if let Some(byte) = changed.get_mut(offset) {
                *byte ^= 1;
            }
            assert_eq!(InstructionV1::decode(&changed), Err(expected));
        }
        let mut unknown = encoded;
        if let Some(byte) = unknown.get_mut(TAG_OFFSET) {
            *byte = 0xff;
        }
        assert_eq!(
            InstructionV1::decode(&unknown),
            Err(Error::UnknownInstructionTag)
        );
    }

    #[test]
    fn family_reserved_bytes_and_invalid_values_refuse() {
        let mut found = [0; FOUND_MARKET_AND_FUND_BYTES];
        let founding = FoundMarketAndFundV1::new(identity(), 3).expect("valid founding");
        founding.encode(&mut found).expect("found encoding");
        assert_eq!(FOUND_MARKET_AND_FUND_BYTES, 192);
        assert_eq!(founding.identity(), identity());
        assert_eq!(founding.outcome_count(), 3);
        for reserved_offset in FOUND_RESERVED_OFFSET..FOUND_RESERVED_OFFSET + FOUND_RESERVED_BYTES {
            let mut hostile = found;
            if let Some(byte) = hostile.get_mut(reserved_offset) {
                *byte = 1;
            }
            assert_eq!(
                FoundMarketAndFundV1::decode(&hostile),
                Err(Error::NonCanonicalReservedBytes)
            );
        }

        let mut obsolete_width = [0u8; 208];
        obsolete_width
            .get_mut(..FOUND_MARKET_AND_FUND_BYTES)
            .expect("founding prefix")
            .copy_from_slice(&found);
        assert_eq!(
            FoundMarketAndFundV1::decode(&obsolete_width),
            Err(Error::InvalidLength)
        );
        let before = [0x5a; FOUND_MARKET_AND_FUND_BYTES + 1];
        let mut wrong_output = before;
        assert_eq!(founding.encode(&mut wrong_output), Err(Error::OutputLength));
        assert_eq!(wrong_output, before);

        let mut redeem = [0; REDEEM_RESOLVED_OUTCOME_BYTES];
        RedeemResolvedOutcomeV1::new(9, 2, 1)
            .expect("valid redemption")
            .encode(&mut redeem)
            .expect("redeem encoding");
        if let Some(byte) = redeem.get_mut(REDEEM_RESERVED_OFFSET) {
            *byte = 1;
        }
        assert_eq!(
            RedeemResolvedOutcomeV1::decode(&redeem),
            Err(Error::NonCanonicalReservedBytes)
        );

        assert_eq!(
            FoundMarketAndFundV1::new(identity(), 1),
            Err(Error::InvalidOutcomeCount)
        );
        assert_eq!(SplitCompleteSetV1::new(1, 0), Err(Error::ZeroQuantity));
        assert_eq!(MergeCompleteSetV1::new(1, 0), Err(Error::ZeroQuantity));
        assert_eq!(
            CreatePositionAndSplitV1::new(1, 1, 0),
            Err(Error::ZeroQuantity)
        );
        assert_eq!(
            RedeemResolvedOutcomeV1::new(1, 0, 0),
            Err(Error::ZeroQuantity)
        );
        assert_eq!(
            RedeemResolvedOutcomeV1::new(1, 16, 1),
            Err(Error::InvalidOutcomeCount)
        );
    }

    #[test]
    fn wrong_output_length_never_partially_mutates() {
        let before = [0x5a; SPLIT_COMPLETE_SET_BYTES + 1];
        let mut output = before;
        assert_eq!(
            SplitCompleteSetV1::new(1, 2)
                .expect("valid split")
                .encode(&mut output),
            Err(Error::OutputLength)
        );
        assert_eq!(output, before);
    }
}
