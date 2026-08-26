#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Bounded execution refinement for `DClutchSemantics.EconomicKernel`.
//!
//! The crate has one fixed-capacity implementation instead of one Rust
//! monomorphization per categorical width.  It owns no Solana accounts, CPI,
//! token program, allocation, floating point, signatures, or adapter policy.
//! A caller supplies an already authenticated role binding and one typed plan.
//! The executor validates the complete pre-state, stages every mutation in a
//! local candidate, revalidates it, and commits only on success.

/// Provisional measured-profile maximum categorical width.
pub const MAX_OUTCOMES: usize = 16;
/// Maximum claim effects emitted by one shared economic command.
pub const MAX_CLAIM_EFFECTS: usize = MAX_OUTCOMES;
/// Maximum custody transfers emitted by one shared economic command.
pub const MAX_CUSTODY_TRANSFERS: usize = 1;
/// Canonical state wire magic (`DCES`).
pub const STATE_MAGIC: [u8; 4] = *b"DCES";
/// Canonical state wire version.
pub const STATE_VERSION: u8 = 1;
/// Bytes before the canonical state's seven active vectors.
pub const STATE_HEADER_BYTES: usize = 16;
/// Canonical claim-plan wire magic (`DCEF`).
pub const CLAIM_MAGIC: [u8; 4] = *b"DCEF";
/// Canonical claim-plan wire version.
pub const CLAIM_VERSION: u8 = 1;
/// Bytes in a canonical claim-plan header.
pub const CLAIM_HEADER_BYTES: usize = 8;
/// Bytes in one canonical claim effect.
pub const CLAIM_EFFECT_BYTES: usize = 16;
/// Largest encoded claim plan in this measured profile.
pub const MAX_CLAIM_PLAN_BYTES: usize = CLAIM_HEADER_BYTES + MAX_CLAIM_EFFECTS * CLAIM_EFFECT_BYTES;
/// Canonical custody-plan wire magic (`DCCP`).
pub const CUSTODY_MAGIC: [u8; 4] = *b"DCCP";
/// Canonical custody-plan wire version.
pub const CUSTODY_VERSION: u8 = 1;
/// Bytes in a canonical custody-plan header.
pub const CUSTODY_HEADER_BYTES: usize = 8;
/// Bytes in one canonical custody transfer.
pub const CUSTODY_TRANSFER_BYTES: usize = 16;
/// Largest encoded custody plan in this measured profile.
pub const MAX_CUSTODY_PLAN_BYTES: usize =
    CUSTODY_HEADER_BYTES + MAX_CUSTODY_TRANSFERS * CUSTODY_TRANSFER_BYTES;

const VECTOR_COUNT: usize = 7;

/// Stable semantic, state, arithmetic, or encoding refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The active outcome count was zero or exceeded the measured profile.
    InvalidOutcomeCount,
    /// A selected outcome was outside the active prefix.
    InvalidOutcome,
    /// An inactive fixed-capacity slot was nonzero.
    NoncanonicalTail,
    /// A phase carried an invalid or noncanonical winner.
    InvalidWinner,
    /// Aggregate and representation supply did not agree exactly.
    SupplyPartitionMismatch,
    /// The projected local claims exceeded aggregate representation supply.
    HolderProjectionExceedsSupply,
    /// Hoard principal did not cover phase-specific liabilities.
    Insolvent,
    /// A quantity required to move value was zero.
    ZeroQuantity,
    /// The command was not admitted in the current phase.
    InvalidPhase,
    /// Two roles that must identify different physical accounts aliased.
    AccountAlias,
    /// A debit or burn exceeded the selected balance.
    InsufficientBalance,
    /// Checked `u64` arithmetic overflowed.
    ArithmeticOverflow,
    /// Terminal retirement was attempted with an outstanding liability.
    OutstandingLiability,
    /// A staged candidate failed the complete invariant recheck.
    CandidateInvariantFailure,
    /// A wire input or output had a noncanonical length.
    InvalidLength,
    /// Wire magic did not select the expected schema.
    InvalidMagic,
    /// The wire version is not implemented.
    UnsupportedVersion,
    /// A reserved byte or canonical zero field was nonzero.
    NonzeroReserved,
}

/// Semantic party tags shared with Lean `DClutch.Party`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Party {
    /// First authenticated role.
    Seller = 0,
    /// Second authenticated role.
    Buyer = 1,
    /// Hoard or venue role.
    Venue = 2,
}

impl Party {
    const fn tag(self) -> u8 {
        match self {
            Self::Seller => 0,
            Self::Buyer => 1,
            Self::Venue => 2,
        }
    }
}

/// Which local claim projection a command selects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Holder {
    /// Source projection.
    Source = 0,
    /// Destination projection.
    Destination = 1,
}

/// Claim representation under one conservative Market supply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Representation {
    /// Program-native claim representation.
    Native = 0,
    /// Materialized claim representation.
    Materialized = 1,
}

/// Lifecycle tag used by the shared economic kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PhaseKind {
    /// Complete sets and open claim operations are admitted.
    Open = 0,
    /// One winner is terminal and claims may redeem.
    Terminal = 1,
    /// Terminal redemption remains active while children retire.
    Retiring = 2,
    /// Empty terminal state; no further economic transition is live.
    Retired = 3,
}

impl PhaseKind {
    const fn tag(self) -> u8 {
        match self {
            Self::Open => 0,
            Self::Terminal => 1,
            Self::Retiring => 2,
            Self::Retired => 3,
        }
    }
}

/// Fixed-layout lifecycle value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Phase {
    kind: PhaseKind,
    winner: u8,
}

impl Phase {
    /// Construct the open phase.
    #[must_use]
    pub const fn open() -> Self {
        Self {
            kind: PhaseKind::Open,
            winner: 0,
        }
    }

    /// Construct a terminal phase with one untrusted winner coordinate.
    #[must_use]
    pub const fn terminal(winner: u8) -> Self {
        Self {
            kind: PhaseKind::Terminal,
            winner,
        }
    }

    /// Construct a retiring phase with one untrusted winner coordinate.
    #[must_use]
    pub const fn retiring(winner: u8) -> Self {
        Self {
            kind: PhaseKind::Retiring,
            winner,
        }
    }

    /// Construct the retired phase.
    #[must_use]
    pub const fn retired() -> Self {
        Self {
            kind: PhaseKind::Retired,
            winner: 0,
        }
    }

    /// Return the lifecycle tag.
    #[must_use]
    pub const fn kind(self) -> PhaseKind {
        self.kind
    }

    /// Return the terminal winner when the phase carries one.
    #[must_use]
    pub const fn winner(self) -> Option<u8> {
        match self.kind {
            PhaseKind::Terminal | PhaseKind::Retiring => Some(self.winner),
            PhaseKind::Open | PhaseKind::Retired => None,
        }
    }

    const fn is_live(self) -> bool {
        !matches!(self.kind, PhaseKind::Retired)
    }
}

/// Public fixed-layout construction boundary for one economic state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct StateParts {
    /// Active categorical width.
    pub outcome_count: u8,
    /// Current lifecycle.
    pub phase: Phase,
    /// Claimant-backing collateral atoms.
    pub hoard: u64,
    /// One conservative supply per outcome.
    pub supply: [u64; MAX_OUTCOMES],
    /// Native-representation supply partition.
    pub native_supply: [u64; MAX_OUTCOMES],
    /// Materialized-representation supply partition.
    pub materialized_supply: [u64; MAX_OUTCOMES],
    /// Source native claims in the two-party projection.
    pub source_native: [u64; MAX_OUTCOMES],
    /// Source materialized claims in the two-party projection.
    pub source_materialized: [u64; MAX_OUTCOMES],
    /// Destination native claims in the two-party projection.
    pub destination_native: [u64; MAX_OUTCOMES],
    /// Destination materialized claims in the two-party projection.
    pub destination_materialized: [u64; MAX_OUTCOMES],
}

/// Validated fixed-capacity economic state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct State(StateParts);

impl State {
    /// Validate hostile composition parts before admitting them as state.
    pub fn from_parts(parts: StateParts) -> Result<Self, Error> {
        let state = Self(parts);
        state.validate()?;
        Ok(state)
    }

    /// Construct one empty open state at a checked active width.
    pub fn empty_open(outcome_count: u8) -> Result<Self, Error> {
        Self::from_parts(StateParts {
            outcome_count,
            phase: Phase::open(),
            hoard: 0,
            supply: [0; MAX_OUTCOMES],
            native_supply: [0; MAX_OUTCOMES],
            materialized_supply: [0; MAX_OUTCOMES],
            source_native: [0; MAX_OUTCOMES],
            source_materialized: [0; MAX_OUTCOMES],
            destination_native: [0; MAX_OUTCOMES],
            destination_materialized: [0; MAX_OUTCOMES],
        })
    }

    /// Return the exact fixed-layout composition parts.
    #[must_use]
    pub const fn into_parts(self) -> StateParts {
        self.0
    }

    /// Return the active categorical width.
    #[must_use]
    pub const fn outcome_count(&self) -> u8 {
        self.0.outcome_count
    }

    /// Return the lifecycle.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.0.phase
    }

    /// Return claimant-backing Hoard atoms.
    #[must_use]
    pub const fn hoard(&self) -> u64 {
        self.0.hoard
    }

    /// Return the active conservative supply prefix.
    pub fn supply(&self) -> &[u64] {
        active(&self.0.supply, self.active_len()).unwrap_or(&[])
    }

    /// Return the active native-supply prefix.
    pub fn native_supply(&self) -> &[u64] {
        active(&self.0.native_supply, self.active_len()).unwrap_or(&[])
    }

    /// Return the active materialized-supply prefix.
    pub fn materialized_supply(&self) -> &[u64] {
        active(&self.0.materialized_supply, self.active_len()).unwrap_or(&[])
    }

    /// Return the active source native-claim prefix.
    pub fn source_native(&self) -> &[u64] {
        active(&self.0.source_native, self.active_len()).unwrap_or(&[])
    }

    /// Return the active source materialized-claim prefix.
    pub fn source_materialized(&self) -> &[u64] {
        active(&self.0.source_materialized, self.active_len()).unwrap_or(&[])
    }

    /// Return the active destination native-claim prefix.
    pub fn destination_native(&self) -> &[u64] {
        active(&self.0.destination_native, self.active_len()).unwrap_or(&[])
    }

    /// Return the active destination materialized-claim prefix.
    pub fn destination_materialized(&self) -> &[u64] {
        active(&self.0.destination_materialized, self.active_len()).unwrap_or(&[])
    }

    /// Validate width, canonical tail, exact representation partition, holder
    /// projection bounds, phase shape, and full collateralization.
    pub fn validate(&self) -> Result<(), Error> {
        let count = self.active_len();
        if count == 0 || count > MAX_OUTCOMES {
            return Err(Error::InvalidOutcomeCount);
        }
        validate_tail(&self.0.supply, count)?;
        validate_tail(&self.0.native_supply, count)?;
        validate_tail(&self.0.materialized_supply, count)?;
        validate_tail(&self.0.source_native, count)?;
        validate_tail(&self.0.source_materialized, count)?;
        validate_tail(&self.0.destination_native, count)?;
        validate_tail(&self.0.destination_materialized, count)?;

        let mut outcome = 0_usize;
        while outcome < count {
            let supply = get(&self.0.supply, outcome)?;
            let native = get(&self.0.native_supply, outcome)?;
            let materialized = get(&self.0.materialized_supply, outcome)?;
            if native
                .checked_add(materialized)
                .ok_or(Error::ArithmeticOverflow)?
                != supply
            {
                return Err(Error::SupplyPartitionMismatch);
            }
            let projected_native = get(&self.0.source_native, outcome)?
                .checked_add(get(&self.0.destination_native, outcome)?)
                .ok_or(Error::ArithmeticOverflow)?;
            if projected_native > native {
                return Err(Error::HolderProjectionExceedsSupply);
            }
            let projected_materialized = get(&self.0.source_materialized, outcome)?
                .checked_add(get(&self.0.destination_materialized, outcome)?)
                .ok_or(Error::ArithmeticOverflow)?;
            if projected_materialized > materialized {
                return Err(Error::HolderProjectionExceedsSupply);
            }
            if self.0.phase.kind == PhaseKind::Open && supply > self.0.hoard {
                return Err(Error::Insolvent);
            }
            outcome = outcome.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
        }

        match self.0.phase.kind {
            PhaseKind::Open => {
                if self.0.phase.winner != 0 {
                    return Err(Error::InvalidWinner);
                }
            }
            PhaseKind::Terminal | PhaseKind::Retiring => {
                let winner = usize::from(self.0.phase.winner);
                if winner >= count {
                    return Err(Error::InvalidWinner);
                }
                if get(&self.0.supply, winner)? > self.0.hoard {
                    return Err(Error::Insolvent);
                }
            }
            PhaseKind::Retired => {
                if self.0.phase.winner != 0 {
                    return Err(Error::InvalidWinner);
                }
                if self.0.hoard != 0 || !all_zero(active(&self.0.supply, count)?) {
                    return Err(Error::OutstandingLiability);
                }
            }
        }
        Ok(())
    }

    /// Return the exact canonical encoded state length.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        STATE_HEADER_BYTES + self.active_len() * VECTOR_COUNT * 8
    }

    /// Encode the active state in the canonical `DCES` V1 format.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<usize, Error> {
        self.validate()?;
        let length = self.encoded_len();
        if output.len() != length {
            return Err(Error::InvalidLength);
        }
        write_slice(output, 0, &STATE_MAGIC)?;
        write_byte(output, 4, STATE_VERSION)?;
        write_byte(output, 5, self.0.phase.kind.tag())?;
        write_byte(output, 6, self.0.outcome_count)?;
        write_byte(output, 7, self.0.phase.winner)?;
        write_slice(output, 8, &self.0.hoard.to_le_bytes())?;
        let vectors = [
            &self.0.supply,
            &self.0.native_supply,
            &self.0.materialized_supply,
            &self.0.source_native,
            &self.0.source_materialized,
            &self.0.destination_native,
            &self.0.destination_materialized,
        ];
        let mut offset = STATE_HEADER_BYTES;
        for values in vectors {
            let mut outcome = 0_usize;
            while outcome < self.active_len() {
                write_slice(output, offset, &get(values, outcome)?.to_le_bytes())?;
                offset = offset.checked_add(8).ok_or(Error::InvalidLength)?;
                outcome = outcome.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
            }
        }
        Ok(length)
    }

    /// Hostile-decode and validate one exact canonical `DCES` V1 state.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() < STATE_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if input.get(..4) != Some(STATE_MAGIC.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_byte(input, 4)? != STATE_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let phase_tag = read_byte(input, 5)?;
        let count = read_byte(input, 6)?;
        let winner = read_byte(input, 7)?;
        let active_len = usize::from(count);
        if active_len == 0 || active_len > MAX_OUTCOMES {
            return Err(Error::InvalidOutcomeCount);
        }
        let expected = STATE_HEADER_BYTES
            .checked_add(
                active_len
                    .checked_mul(VECTOR_COUNT)
                    .and_then(|values| values.checked_mul(8))
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        if input.len() != expected {
            return Err(Error::InvalidLength);
        }
        let phase = match phase_tag {
            0 => {
                if winner != 0 {
                    return Err(Error::InvalidWinner);
                }
                Phase::open()
            }
            1 => Phase::terminal(winner),
            2 => Phase::retiring(winner),
            3 => {
                if winner != 0 {
                    return Err(Error::InvalidWinner);
                }
                Phase::retired()
            }
            _ => return Err(Error::InvalidPhase),
        };
        let mut vectors = [[0_u64; MAX_OUTCOMES]; VECTOR_COUNT];
        let mut offset = STATE_HEADER_BYTES;
        let mut vector_index = 0_usize;
        while vector_index < VECTOR_COUNT {
            let vector = vectors.get_mut(vector_index).ok_or(Error::InvalidLength)?;
            let mut outcome = 0_usize;
            while outcome < active_len {
                *vector.get_mut(outcome).ok_or(Error::InvalidOutcome)? = read_u64(input, offset)?;
                offset = offset.checked_add(8).ok_or(Error::InvalidLength)?;
                outcome = outcome.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
            }
            vector_index = vector_index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let parts = StateParts {
            outcome_count: count,
            phase,
            hoard: read_u64(input, 8)?,
            supply: *vectors.first().ok_or(Error::InvalidLength)?,
            native_supply: *vectors.get(1).ok_or(Error::InvalidLength)?,
            materialized_supply: *vectors.get(2).ok_or(Error::InvalidLength)?,
            source_native: *vectors.get(3).ok_or(Error::InvalidLength)?,
            source_materialized: *vectors.get(4).ok_or(Error::InvalidLength)?,
            destination_native: *vectors.get(5).ok_or(Error::InvalidLength)?,
            destination_materialized: *vectors.get(6).ok_or(Error::InvalidLength)?,
        };
        Self::from_parts(parts)
    }

    fn active_len(&self) -> usize {
        usize::from(self.0.outcome_count)
    }

    fn representation_supply(&self, representation: Representation) -> &[u64; MAX_OUTCOMES] {
        match representation {
            Representation::Native => &self.0.native_supply,
            Representation::Materialized => &self.0.materialized_supply,
        }
    }

    fn representation_supply_mut(
        &mut self,
        representation: Representation,
    ) -> &mut [u64; MAX_OUTCOMES] {
        match representation {
            Representation::Native => &mut self.0.native_supply,
            Representation::Materialized => &mut self.0.materialized_supply,
        }
    }

    fn holder_claims(
        &self,
        holder: Holder,
        representation: Representation,
    ) -> &[u64; MAX_OUTCOMES] {
        match (holder, representation) {
            (Holder::Source, Representation::Native) => &self.0.source_native,
            (Holder::Source, Representation::Materialized) => &self.0.source_materialized,
            (Holder::Destination, Representation::Native) => &self.0.destination_native,
            (Holder::Destination, Representation::Materialized) => &self.0.destination_materialized,
        }
    }

    fn holder_claims_mut(
        &mut self,
        holder: Holder,
        representation: Representation,
    ) -> &mut [u64; MAX_OUTCOMES] {
        match (holder, representation) {
            (Holder::Source, Representation::Native) => &mut self.0.source_native,
            (Holder::Source, Representation::Materialized) => &mut self.0.source_materialized,
            (Holder::Destination, Representation::Native) => &mut self.0.destination_native,
            (Holder::Destination, Representation::Materialized) => {
                &mut self.0.destination_materialized
            }
        }
    }
}

/// Exact role binding compiled into claim and custody plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Bindings {
    /// Debit/source role.
    pub source: Party,
    /// Credit/destination role.
    pub destination: Party,
    /// Claimant-backing Hoard role.
    pub hoard: Party,
}

/// Command discriminant in the fixed-layout typed plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CommandTag {
    /// Deposit backing and issue every outcome.
    SplitCompleteSet = 0,
    /// Burn every outcome and release backing.
    MergeCompleteSet = 1,
    /// Liability-neutral claim transfer.
    TransferClaim = 2,
    /// Move native claims into materialized representation.
    MaterializeClaim = 3,
    /// Move materialized claims into native representation.
    DematerializeClaim = 4,
    /// Burn a terminal claim and release its exact payout.
    RedeemTerminal = 5,
    /// Mark an empty retiring state retired.
    RetireTerminal = 6,
}

/// Canonical fixed-layout command data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Command {
    tag: CommandTag,
    holder: Holder,
    representation: Representation,
    outcome: u8,
    reserved: [u8; 4],
    quantity: u64,
}

impl Command {
    const fn new(
        tag: CommandTag,
        holder: Holder,
        representation: Representation,
        outcome: u8,
        quantity: u64,
    ) -> Self {
        Self {
            tag,
            holder,
            representation,
            outcome,
            reserved: [0; 4],
            quantity,
        }
    }

    /// Return the command tag.
    #[must_use]
    pub const fn tag(self) -> CommandTag {
        self.tag
    }

    /// Return the exact command quantity.
    #[must_use]
    pub const fn quantity(self) -> u64 {
        self.quantity
    }
}

/// Typed canonical economic plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Plan {
    bindings: Bindings,
    command: Command,
}

impl Plan {
    /// Construct a complete-set split plan.
    #[must_use]
    pub const fn split_complete_set(
        bindings: Bindings,
        holder: Holder,
        representation: Representation,
        quantity: u64,
    ) -> Self {
        Self {
            bindings,
            command: Command::new(
                CommandTag::SplitCompleteSet,
                holder,
                representation,
                0,
                quantity,
            ),
        }
    }

    /// Construct a complete-set merge plan.
    #[must_use]
    pub const fn merge_complete_set(
        bindings: Bindings,
        holder: Holder,
        representation: Representation,
        quantity: u64,
    ) -> Self {
        Self {
            bindings,
            command: Command::new(
                CommandTag::MergeCompleteSet,
                holder,
                representation,
                0,
                quantity,
            ),
        }
    }

    /// Construct a liability-neutral claim transfer plan.
    #[must_use]
    pub const fn transfer_claim(
        bindings: Bindings,
        representation: Representation,
        outcome: u8,
        quantity: u64,
    ) -> Self {
        Self {
            bindings,
            command: Command::new(
                CommandTag::TransferClaim,
                Holder::Source,
                representation,
                outcome,
                quantity,
            ),
        }
    }

    /// Construct a native-to-materialized claim plan.
    #[must_use]
    pub const fn materialize_claim(bindings: Bindings, outcome: u8, quantity: u64) -> Self {
        Self {
            bindings,
            command: Command::new(
                CommandTag::MaterializeClaim,
                Holder::Source,
                Representation::Native,
                outcome,
                quantity,
            ),
        }
    }

    /// Construct a materialized-to-native claim plan.
    #[must_use]
    pub const fn dematerialize_claim(bindings: Bindings, outcome: u8, quantity: u64) -> Self {
        Self {
            bindings,
            command: Command::new(
                CommandTag::DematerializeClaim,
                Holder::Source,
                Representation::Materialized,
                outcome,
                quantity,
            ),
        }
    }

    /// Construct a terminal redemption plan.
    #[must_use]
    pub const fn redeem_terminal(
        bindings: Bindings,
        holder: Holder,
        representation: Representation,
        outcome: u8,
        quantity: u64,
    ) -> Self {
        Self {
            bindings,
            command: Command::new(
                CommandTag::RedeemTerminal,
                holder,
                representation,
                outcome,
                quantity,
            ),
        }
    }

    /// Construct a terminal retirement plan.
    #[must_use]
    pub const fn retire_terminal(bindings: Bindings) -> Self {
        Self {
            bindings,
            command: Command::new(
                CommandTag::RetireTerminal,
                Holder::Source,
                Representation::Native,
                0,
                0,
            ),
        }
    }

    /// Return the exact role binding.
    #[must_use]
    pub const fn bindings(self) -> Bindings {
        self.bindings
    }

    /// Return the canonical command.
    #[must_use]
    pub const fn command(self) -> Command {
        self.command
    }
}

/// Claim effect operation shared with the Lean Effect IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ClaimOperation {
    /// Debit one claim balance.
    Debit = 1,
    /// Credit one claim balance.
    Credit = 2,
}

impl ClaimOperation {
    const fn tag(self) -> u8 {
        match self {
            Self::Debit => 1,
            Self::Credit => 2,
        }
    }
}

/// One fixed-layout claim effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct ClaimEffect {
    /// Debit or credit.
    pub operation: ClaimOperation,
    /// Bound semantic party.
    pub party: Party,
    /// Product-owned outcome coordinate.
    pub outcome: u8,
    /// Exact claim atoms.
    pub amount: u64,
}

const EMPTY_CLAIM_EFFECT: ClaimEffect = ClaimEffect {
    operation: ClaimOperation::Debit,
    party: Party::Seller,
    outcome: 0,
    amount: 0,
};

/// One exact custody movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct CustodyTransfer {
    /// Debited party.
    pub source: Party,
    /// Credited party.
    pub destination: Party,
    /// Exact collateral atoms.
    pub amount: u64,
}

const EMPTY_CUSTODY_TRANSFER: CustodyTransfer = CustodyTransfer {
    source: Party::Seller,
    destination: Party::Seller,
    amount: 0,
};

/// Fixed-capacity claim effect plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct ClaimPlan {
    count: u8,
    effects: [ClaimEffect; MAX_CLAIM_EFFECTS],
}

impl ClaimPlan {
    fn empty() -> Self {
        Self {
            count: 0,
            effects: [EMPTY_CLAIM_EFFECT; MAX_CLAIM_EFFECTS],
        }
    }

    fn push(&mut self, effect: ClaimEffect) -> Result<(), Error> {
        let index = usize::from(self.count);
        *self
            .effects
            .get_mut(index)
            .ok_or(Error::InvalidOutcomeCount)? = effect;
        self.count = self
            .count
            .checked_add(1)
            .ok_or(Error::InvalidOutcomeCount)?;
        Ok(())
    }

    /// Return the number of active effects.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.count)
    }

    /// Return whether no claim effect is active.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Return one active effect.
    #[must_use]
    pub fn effect(&self, index: usize) -> Option<&ClaimEffect> {
        if index < self.len() {
            self.effects.get(index)
        } else {
            None
        }
    }

    /// Return the exact canonical encoded length.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        CLAIM_HEADER_BYTES + self.len() * CLAIM_EFFECT_BYTES
    }

    /// Encode the existing Lean `DCEF` V1 claim-plan representation.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<usize, Error> {
        let length = self.encoded_len();
        if output.len() != length {
            return Err(Error::InvalidLength);
        }
        write_slice(output, 0, &CLAIM_MAGIC)?;
        write_byte(output, 4, CLAIM_VERSION)?;
        write_byte(output, 5, self.count)?;
        write_byte(output, 6, 0)?;
        write_byte(output, 7, 0)?;
        let mut index = 0_usize;
        while index < self.len() {
            let effect = self.effect(index).ok_or(Error::InvalidLength)?;
            let offset = CLAIM_HEADER_BYTES
                .checked_add(
                    index
                        .checked_mul(CLAIM_EFFECT_BYTES)
                        .ok_or(Error::InvalidLength)?,
                )
                .ok_or(Error::InvalidLength)?;
            write_byte(output, offset, effect.operation.tag())?;
            write_byte(output, checked_add(offset, 1)?, effect.party.tag())?;
            write_byte(output, checked_add(offset, 2)?, 1)?;
            write_byte(output, checked_add(offset, 3)?, 0)?;
            write_slice(
                output,
                checked_add(offset, 4)?,
                &u32::from(effect.outcome).to_le_bytes(),
            )?;
            write_slice(
                output,
                checked_add(offset, 8)?,
                &effect.amount.to_le_bytes(),
            )?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(length)
    }
}

/// Fixed-capacity custody plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct CustodyPlan {
    count: u8,
    transfers: [CustodyTransfer; MAX_CUSTODY_TRANSFERS],
}

impl CustodyPlan {
    fn empty() -> Self {
        Self {
            count: 0,
            transfers: [EMPTY_CUSTODY_TRANSFER; MAX_CUSTODY_TRANSFERS],
        }
    }

    fn push(&mut self, transfer: CustodyTransfer) -> Result<(), Error> {
        let index = usize::from(self.count);
        *self.transfers.get_mut(index).ok_or(Error::InvalidLength)? = transfer;
        self.count = self.count.checked_add(1).ok_or(Error::InvalidLength)?;
        Ok(())
    }

    /// Return the number of active transfers.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.count)
    }

    /// Return whether no custody transfer is active.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Return one active transfer.
    #[must_use]
    pub fn transfer(&self, index: usize) -> Option<&CustodyTransfer> {
        if index < self.len() {
            self.transfers.get(index)
        } else {
            None
        }
    }

    /// Return the exact canonical encoded length.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        CUSTODY_HEADER_BYTES + self.len() * CUSTODY_TRANSFER_BYTES
    }

    /// Encode the existing Lean `DCCP` V1 custody-plan representation.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<usize, Error> {
        let length = self.encoded_len();
        if output.len() != length {
            return Err(Error::InvalidLength);
        }
        write_slice(output, 0, &CUSTODY_MAGIC)?;
        write_byte(output, 4, CUSTODY_VERSION)?;
        write_byte(output, 5, self.count)?;
        write_byte(output, 6, 0)?;
        write_byte(output, 7, 0)?;
        let mut index = 0_usize;
        while index < self.len() {
            let transfer = self.transfer(index).ok_or(Error::InvalidLength)?;
            let offset = CUSTODY_HEADER_BYTES
                .checked_add(
                    index
                        .checked_mul(CUSTODY_TRANSFER_BYTES)
                        .ok_or(Error::InvalidLength)?,
                )
                .ok_or(Error::InvalidLength)?;
            write_byte(output, offset, transfer.source.tag())?;
            write_byte(output, checked_add(offset, 1)?, transfer.destination.tag())?;
            let mut reserved = 2_usize;
            while reserved < 8 {
                write_byte(output, checked_add(offset, reserved)?, 0)?;
                reserved = reserved.checked_add(1).ok_or(Error::InvalidLength)?;
            }
            write_slice(
                output,
                checked_add(offset, 8)?,
                &transfer.amount.to_le_bytes(),
            )?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(length)
    }
}

/// Exact disjoint claim/custody plan emitted only after successful execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PhysicalPlan {
    /// Program-owned claim effects.
    pub claims: ClaimPlan,
    /// Realm-custody collateral transfers.
    pub custody: CustodyPlan,
}

/// Execute one canonical economic plan transactionally.
///
/// The original state is unchanged on every refusal.  A physical plan is
/// returned only after the complete candidate invariant check succeeds.
pub fn execute(plan: &Plan, state: &mut State) -> Result<PhysicalPlan, Error> {
    state.validate()?;
    validate_binding(plan)?;
    validate_command(plan, state)?;
    let mut candidate = *state;
    apply_command(plan.command, &mut candidate)?;
    candidate
        .validate()
        .map_err(|_| Error::CandidateInvariantFailure)?;
    let physical = compile(plan, state)?;
    *state = candidate;
    Ok(physical)
}

fn validate_binding(plan: &Plan) -> Result<(), Error> {
    let holder_party = party_of(plan.bindings, plan.command.holder);
    let distinct = match plan.command.tag {
        CommandTag::SplitCompleteSet
        | CommandTag::MergeCompleteSet
        | CommandTag::RedeemTerminal => holder_party != plan.bindings.hoard,
        CommandTag::TransferClaim
        | CommandTag::MaterializeClaim
        | CommandTag::DematerializeClaim => plan.bindings.source != plan.bindings.destination,
        CommandTag::RetireTerminal => true,
    };
    if distinct {
        Ok(())
    } else {
        Err(Error::AccountAlias)
    }
}

fn validate_command(plan: &Plan, state: &State) -> Result<(), Error> {
    let command = plan.command;
    match command.tag {
        CommandTag::SplitCompleteSet => {
            require_phase(state, PhaseKind::Open)?;
            require_quantity(command.quantity)?;
            state
                .0
                .hoard
                .checked_add(command.quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            all_add_fits(state.supply(), command.quantity)?;
            all_add_fits(
                active(
                    state.representation_supply(command.representation),
                    state.active_len(),
                )?,
                command.quantity,
            )?;
            all_add_fits(
                active(
                    state.holder_claims(command.holder, command.representation),
                    state.active_len(),
                )?,
                command.quantity,
            )
        }
        CommandTag::MergeCompleteSet => {
            require_phase(state, PhaseKind::Open)?;
            require_quantity(command.quantity)?;
            if command.quantity > state.0.hoard {
                return Err(Error::InsufficientBalance);
            }
            all_has(state.supply(), command.quantity)?;
            all_has(
                active(
                    state.representation_supply(command.representation),
                    state.active_len(),
                )?,
                command.quantity,
            )?;
            all_has(
                active(
                    state.holder_claims(command.holder, command.representation),
                    state.active_len(),
                )?,
                command.quantity,
            )
        }
        CommandTag::TransferClaim => {
            require_live(state)?;
            require_quantity(command.quantity)?;
            let outcome = require_outcome(state, command.outcome)?;
            require_debit(
                state.holder_claims(Holder::Source, command.representation),
                outcome,
                command.quantity,
            )?;
            require_credit(
                state.holder_claims(Holder::Destination, command.representation),
                outcome,
                command.quantity,
            )
        }
        CommandTag::MaterializeClaim => {
            require_phase(state, PhaseKind::Open)?;
            require_quantity(command.quantity)?;
            let outcome = require_outcome(state, command.outcome)?;
            require_debit(&state.0.native_supply, outcome, command.quantity)?;
            require_debit(&state.0.source_native, outcome, command.quantity)?;
            require_credit(&state.0.materialized_supply, outcome, command.quantity)?;
            require_credit(&state.0.destination_materialized, outcome, command.quantity)
        }
        CommandTag::DematerializeClaim => {
            require_live(state)?;
            require_quantity(command.quantity)?;
            let outcome = require_outcome(state, command.outcome)?;
            require_debit(&state.0.materialized_supply, outcome, command.quantity)?;
            require_debit(&state.0.source_materialized, outcome, command.quantity)?;
            require_credit(&state.0.native_supply, outcome, command.quantity)?;
            require_credit(&state.0.destination_native, outcome, command.quantity)
        }
        CommandTag::RedeemTerminal => {
            require_quantity(command.quantity)?;
            let winner = state.phase().winner().ok_or(Error::InvalidPhase)?;
            let outcome = require_outcome(state, command.outcome)?;
            require_debit(&state.0.supply, outcome, command.quantity)?;
            require_debit(
                state.representation_supply(command.representation),
                outcome,
                command.quantity,
            )?;
            require_debit(
                state.holder_claims(command.holder, command.representation),
                outcome,
                command.quantity,
            )?;
            if winner == command.outcome && command.quantity > state.0.hoard {
                return Err(Error::InsufficientBalance);
            }
            Ok(())
        }
        CommandTag::RetireTerminal => {
            require_phase(state, PhaseKind::Retiring)?;
            if state.0.hoard != 0
                || !all_zero(state.supply())
                || !all_zero(state.materialized_supply())
            {
                return Err(Error::OutstandingLiability);
            }
            Ok(())
        }
    }
}

fn apply_command(command: Command, state: &mut State) -> Result<(), Error> {
    match command.tag {
        CommandTag::SplitCompleteSet => {
            let count = state.active_len();
            state.0.hoard = state
                .0
                .hoard
                .checked_add(command.quantity)
                .ok_or(Error::ArithmeticOverflow)?;
            add_every(&mut state.0.supply, count, command.quantity)?;
            add_every(
                state.representation_supply_mut(command.representation),
                count,
                command.quantity,
            )?;
            add_every(
                state.holder_claims_mut(command.holder, command.representation),
                count,
                command.quantity,
            )
        }
        CommandTag::MergeCompleteSet => {
            let count = state.active_len();
            state.0.hoard = state
                .0
                .hoard
                .checked_sub(command.quantity)
                .ok_or(Error::InsufficientBalance)?;
            sub_every(&mut state.0.supply, count, command.quantity)?;
            sub_every(
                state.representation_supply_mut(command.representation),
                count,
                command.quantity,
            )?;
            sub_every(
                state.holder_claims_mut(command.holder, command.representation),
                count,
                command.quantity,
            )
        }
        CommandTag::TransferClaim => {
            let outcome = require_outcome(state, command.outcome)?;
            debit_at(
                state.holder_claims_mut(Holder::Source, command.representation),
                outcome,
                command.quantity,
            )?;
            credit_at(
                state.holder_claims_mut(Holder::Destination, command.representation),
                outcome,
                command.quantity,
            )
        }
        CommandTag::MaterializeClaim => {
            let outcome = require_outcome(state, command.outcome)?;
            debit_at(&mut state.0.native_supply, outcome, command.quantity)?;
            credit_at(&mut state.0.materialized_supply, outcome, command.quantity)?;
            debit_at(&mut state.0.source_native, outcome, command.quantity)?;
            credit_at(
                &mut state.0.destination_materialized,
                outcome,
                command.quantity,
            )
        }
        CommandTag::DematerializeClaim => {
            let outcome = require_outcome(state, command.outcome)?;
            credit_at(&mut state.0.native_supply, outcome, command.quantity)?;
            debit_at(&mut state.0.materialized_supply, outcome, command.quantity)?;
            debit_at(&mut state.0.source_materialized, outcome, command.quantity)?;
            credit_at(&mut state.0.destination_native, outcome, command.quantity)
        }
        CommandTag::RedeemTerminal => {
            let outcome = require_outcome(state, command.outcome)?;
            let payout = redemption_payout(state.phase(), command.outcome, command.quantity);
            state.0.hoard = state
                .0
                .hoard
                .checked_sub(payout)
                .ok_or(Error::InsufficientBalance)?;
            debit_at(&mut state.0.supply, outcome, command.quantity)?;
            debit_at(
                state.representation_supply_mut(command.representation),
                outcome,
                command.quantity,
            )?;
            debit_at(
                state.holder_claims_mut(command.holder, command.representation),
                outcome,
                command.quantity,
            )
        }
        CommandTag::RetireTerminal => {
            state.0.phase = Phase::retired();
            Ok(())
        }
    }
}

fn compile(plan: &Plan, pre: &State) -> Result<PhysicalPlan, Error> {
    let mut claims = ClaimPlan::empty();
    let mut custody = CustodyPlan::empty();
    let command = plan.command;
    match command.tag {
        CommandTag::SplitCompleteSet | CommandTag::MergeCompleteSet => {
            let operation = if command.tag == CommandTag::SplitCompleteSet {
                ClaimOperation::Credit
            } else {
                ClaimOperation::Debit
            };
            let holder = party_of(plan.bindings, command.holder);
            let mut outcome = 0_usize;
            while outcome < pre.active_len() {
                claims.push(ClaimEffect {
                    operation,
                    party: holder,
                    outcome: u8::try_from(outcome).map_err(|_| Error::InvalidOutcome)?,
                    amount: command.quantity,
                })?;
                outcome = outcome.checked_add(1).ok_or(Error::InvalidOutcomeCount)?;
            }
            let (source, destination) = if command.tag == CommandTag::SplitCompleteSet {
                (holder, plan.bindings.hoard)
            } else {
                (plan.bindings.hoard, holder)
            };
            custody.push(CustodyTransfer {
                source,
                destination,
                amount: command.quantity,
            })?;
        }
        CommandTag::TransferClaim
        | CommandTag::MaterializeClaim
        | CommandTag::DematerializeClaim => {
            claims.push(ClaimEffect {
                operation: ClaimOperation::Debit,
                party: plan.bindings.source,
                outcome: command.outcome,
                amount: command.quantity,
            })?;
            claims.push(ClaimEffect {
                operation: ClaimOperation::Credit,
                party: plan.bindings.destination,
                outcome: command.outcome,
                amount: command.quantity,
            })?;
        }
        CommandTag::RedeemTerminal => {
            let holder = party_of(plan.bindings, command.holder);
            claims.push(ClaimEffect {
                operation: ClaimOperation::Debit,
                party: holder,
                outcome: command.outcome,
                amount: command.quantity,
            })?;
            let payout = redemption_payout(pre.phase(), command.outcome, command.quantity);
            if payout != 0 {
                custody.push(CustodyTransfer {
                    source: plan.bindings.hoard,
                    destination: holder,
                    amount: payout,
                })?;
            }
        }
        CommandTag::RetireTerminal => {}
    }
    Ok(PhysicalPlan { claims, custody })
}

fn party_of(bindings: Bindings, holder: Holder) -> Party {
    match holder {
        Holder::Source => bindings.source,
        Holder::Destination => bindings.destination,
    }
}

fn redemption_payout(phase: Phase, outcome: u8, quantity: u64) -> u64 {
    if phase.winner() == Some(outcome) {
        quantity
    } else {
        0
    }
}

fn require_phase(state: &State, expected: PhaseKind) -> Result<(), Error> {
    if state.phase().kind() == expected {
        Ok(())
    } else {
        Err(Error::InvalidPhase)
    }
}

fn require_live(state: &State) -> Result<(), Error> {
    if state.phase().is_live() {
        Ok(())
    } else {
        Err(Error::InvalidPhase)
    }
}

fn require_quantity(quantity: u64) -> Result<(), Error> {
    if quantity == 0 {
        Err(Error::ZeroQuantity)
    } else {
        Ok(())
    }
}

fn require_outcome(state: &State, outcome: u8) -> Result<usize, Error> {
    let index = usize::from(outcome);
    if index < state.active_len() {
        Ok(index)
    } else {
        Err(Error::InvalidOutcome)
    }
}

fn require_debit(values: &[u64; MAX_OUTCOMES], outcome: usize, quantity: u64) -> Result<(), Error> {
    if get(values, outcome)? < quantity {
        Err(Error::InsufficientBalance)
    } else {
        Ok(())
    }
}

fn require_credit(
    values: &[u64; MAX_OUTCOMES],
    outcome: usize,
    quantity: u64,
) -> Result<(), Error> {
    get(values, outcome)?
        .checked_add(quantity)
        .ok_or(Error::ArithmeticOverflow)
        .map(|_| ())
}

fn all_add_fits(values: &[u64], quantity: u64) -> Result<(), Error> {
    for value in values {
        value
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn all_has(values: &[u64], quantity: u64) -> Result<(), Error> {
    for value in values {
        if *value < quantity {
            return Err(Error::InsufficientBalance);
        }
    }
    Ok(())
}

fn all_zero(values: &[u64]) -> bool {
    values.iter().all(|value| *value == 0)
}

fn add_every(values: &mut [u64; MAX_OUTCOMES], count: usize, quantity: u64) -> Result<(), Error> {
    let active_values = active_mut(values, count)?;
    for value in active_values {
        *value = value
            .checked_add(quantity)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn sub_every(values: &mut [u64; MAX_OUTCOMES], count: usize, quantity: u64) -> Result<(), Error> {
    let active_values = active_mut(values, count)?;
    for value in active_values {
        *value = value
            .checked_sub(quantity)
            .ok_or(Error::InsufficientBalance)?;
    }
    Ok(())
}

fn debit_at(values: &mut [u64; MAX_OUTCOMES], outcome: usize, quantity: u64) -> Result<(), Error> {
    let value = values.get_mut(outcome).ok_or(Error::InvalidOutcome)?;
    *value = value
        .checked_sub(quantity)
        .ok_or(Error::InsufficientBalance)?;
    Ok(())
}

fn credit_at(values: &mut [u64; MAX_OUTCOMES], outcome: usize, quantity: u64) -> Result<(), Error> {
    let value = values.get_mut(outcome).ok_or(Error::InvalidOutcome)?;
    *value = value
        .checked_add(quantity)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(())
}

fn validate_tail(values: &[u64; MAX_OUTCOMES], count: usize) -> Result<(), Error> {
    let tail = values.get(count..).ok_or(Error::InvalidOutcomeCount)?;
    if all_zero(tail) {
        Ok(())
    } else {
        Err(Error::NoncanonicalTail)
    }
}

fn active(values: &[u64; MAX_OUTCOMES], count: usize) -> Result<&[u64], Error> {
    values.get(..count).ok_or(Error::InvalidOutcomeCount)
}

fn active_mut(values: &mut [u64; MAX_OUTCOMES], count: usize) -> Result<&mut [u64], Error> {
    values.get_mut(..count).ok_or(Error::InvalidOutcomeCount)
}

fn get(values: &[u64; MAX_OUTCOMES], outcome: usize) -> Result<u64, Error> {
    values.get(outcome).copied().ok_or(Error::InvalidOutcome)
}

fn checked_add(base: usize, delta: usize) -> Result<usize, Error> {
    base.checked_add(delta).ok_or(Error::InvalidLength)
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    let end = checked_add(offset, 8)?;
    let bytes: &[u8; 8] = input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)?;
    Ok(u64::from_le_bytes(*bytes))
}

fn write_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

fn write_slice(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = checked_add(offset, value.len())?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use core::mem::size_of;
    use std::{format, vec, vec::Vec};

    use super::*;

    const FIXTURES: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../formal/dclutch-semantics/vectors/economic-kernel-v1.txt"
    ));

    const BINDINGS: Bindings = Bindings {
        source: Party::Seller,
        destination: Party::Buyer,
        hoard: Party::Venue,
    };

    fn fixture(name: &str, field: &str) -> Vec<u8> {
        let prefix = format!("{name}.{field}=");
        let value = FIXTURES
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .expect("named Lean fixture exists");
        decode_hex(value)
    }

    fn decode_hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let pair = core::str::from_utf8(pair).expect("fixture is UTF-8");
                u8::from_str_radix(pair, 16).expect("fixture is lowercase hexadecimal")
            })
            .collect()
    }

    fn encode_state(state: &State) -> Vec<u8> {
        let mut bytes = vec![0_u8; state.encoded_len()];
        assert_eq!(state.encode_into(&mut bytes), Ok(bytes.len()));
        bytes
    }

    fn encode_claims(plan: &ClaimPlan) -> Vec<u8> {
        let mut bytes = vec![0_u8; plan.encoded_len()];
        assert_eq!(plan.encode_into(&mut bytes), Ok(bytes.len()));
        bytes
    }

    fn encode_custody(plan: &CustodyPlan) -> Vec<u8> {
        let mut bytes = vec![0_u8; plan.encoded_len()];
        assert_eq!(plan.encode_into(&mut bytes), Ok(bytes.len()));
        bytes
    }

    fn assert_lean_fixture(name: &str, plan: Plan) {
        let pre_bytes = fixture(name, "pre");
        let mut state = State::decode(&pre_bytes).expect("Lean pre-state decodes");
        assert_eq!(encode_state(&state), pre_bytes);

        let expected_post_bytes = fixture(name, "post");
        let expected_post = State::decode(&expected_post_bytes).expect("Lean post-state decodes");
        let physical = execute(&plan, &mut state).expect("Lean command is admitted");
        assert_eq!(state, expected_post);
        assert_eq!(encode_state(&state), expected_post_bytes);
        assert_eq!(encode_claims(&physical.claims), fixture(name, "claims"));
        assert_eq!(encode_custody(&physical.custody), fixture(name, "custody"));
    }

    fn active_three(first: u64, second: u64, third: u64) -> [u64; MAX_OUTCOMES] {
        let mut values = [0_u64; MAX_OUTCOMES];
        *values.first_mut().expect("profile has first slot") = first;
        *values.get_mut(1).expect("profile has second slot") = second;
        *values.get_mut(2).expect("profile has third slot") = third;
        values
    }

    #[test]
    fn lean_vectors_match_all_state_and_physical_outputs_exactly() {
        let fixtures = [
            (
                "split",
                Plan::split_complete_set(BINDINGS, Holder::Destination, Representation::Native, 10),
            ),
            (
                "merge",
                Plan::merge_complete_set(BINDINGS, Holder::Destination, Representation::Native, 10),
            ),
            (
                "transfer",
                Plan::transfer_claim(BINDINGS, Representation::Native, 2, 5),
            ),
            ("materialize", Plan::materialize_claim(BINDINGS, 1, 4)),
            ("dematerialize", Plan::dematerialize_claim(BINDINGS, 1, 4)),
            (
                "redeem_winner",
                Plan::redeem_terminal(BINDINGS, Holder::Source, Representation::Native, 1, 6),
            ),
            (
                "redeem_loser",
                Plan::redeem_terminal(BINDINGS, Holder::Source, Representation::Native, 0, 4),
            ),
            ("retire", Plan::retire_terminal(BINDINGS)),
        ];
        for (name, plan) in fixtures {
            assert_lean_fixture(name, plan);
        }
    }

    #[test]
    fn complete_set_refusals_roll_back_every_field() {
        let mut empty = State::empty_open(3).expect("valid width");
        let before = empty;
        let zero =
            Plan::split_complete_set(BINDINGS, Holder::Destination, Representation::Native, 0);
        assert_eq!(execute(&zero, &mut empty), Err(Error::ZeroQuantity));
        assert_eq!(empty, before);

        let mut saturated = State::from_parts(StateParts {
            outcome_count: 3,
            phase: Phase::open(),
            hoard: u64::MAX,
            supply: active_three(u64::MAX, u64::MAX, u64::MAX),
            native_supply: active_three(u64::MAX, u64::MAX, u64::MAX),
            materialized_supply: [0; MAX_OUTCOMES],
            source_native: [0; MAX_OUTCOMES],
            source_materialized: [0; MAX_OUTCOMES],
            destination_native: active_three(u64::MAX, u64::MAX, u64::MAX),
            destination_materialized: [0; MAX_OUTCOMES],
        })
        .expect("saturated state remains valid");
        let before = saturated;
        let overflow =
            Plan::split_complete_set(BINDINGS, Holder::Destination, Representation::Native, 1);
        assert_eq!(
            execute(&overflow, &mut saturated),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(saturated, before);

        let mut split = State::decode(&fixture("split", "post")).expect("fixture decodes");
        let before = split;
        let overdraw =
            Plan::merge_complete_set(BINDINGS, Holder::Destination, Representation::Native, 11);
        assert_eq!(
            execute(&overdraw, &mut split),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(split, before);
    }

    #[test]
    fn claim_refusals_roll_back_alias_outcome_balance_and_wrong_holder() {
        let mut state = State::decode(&fixture("transfer", "pre")).expect("fixture decodes");
        let before = state;
        let aliased = Plan::transfer_claim(
            Bindings {
                destination: Party::Seller,
                ..BINDINGS
            },
            Representation::Native,
            2,
            1,
        );
        assert_eq!(execute(&aliased, &mut state), Err(Error::AccountAlias));
        assert_eq!(state, before);

        let wrong_outcome = Plan::transfer_claim(BINDINGS, Representation::Native, 3, 1);
        assert_eq!(
            execute(&wrong_outcome, &mut state),
            Err(Error::InvalidOutcome)
        );
        assert_eq!(state, before);

        let overdraw = Plan::transfer_claim(BINDINGS, Representation::Native, 2, 8);
        assert_eq!(
            execute(&overdraw, &mut state),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(state, before);

        let mut wrong_holder =
            State::decode(&fixture("split", "post")).expect("split fixture decodes");
        let before = wrong_holder;
        let materialize = Plan::materialize_claim(BINDINGS, 1, 1);
        assert_eq!(
            execute(&materialize, &mut wrong_holder),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(wrong_holder, before);
    }

    #[test]
    fn terminal_refusals_roll_back_overdraw_and_nonempty_retirement() {
        let mut winning = State::decode(&fixture("redeem_winner", "pre")).expect("fixture decodes");
        let before = winning;
        let overdraw =
            Plan::redeem_terminal(BINDINGS, Holder::Source, Representation::Native, 1, 7);
        assert_eq!(
            execute(&overdraw, &mut winning),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(winning, before);

        let retire = Plan::retire_terminal(BINDINGS);
        assert_eq!(
            execute(&retire, &mut winning),
            Err(Error::OutstandingLiability)
        );
        assert_eq!(winning, before);

        let mut open = State::empty_open(3).expect("valid state");
        let before = open;
        assert_eq!(execute(&retire, &mut open), Err(Error::InvalidPhase));
        assert_eq!(open, before);
    }

    #[test]
    fn state_decoder_refuses_noncanonical_and_insolvent_inputs() {
        let bytes = fixture("transfer", "pre");
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(State::decode(&trailing), Err(Error::InvalidLength));

        let mut bad_magic = bytes.clone();
        *bad_magic.first_mut().expect("magic exists") = 0;
        assert_eq!(State::decode(&bad_magic), Err(Error::InvalidMagic));

        let mut bad_phase = bytes.clone();
        *bad_phase.get_mut(5).expect("phase exists") = 9;
        assert_eq!(State::decode(&bad_phase), Err(Error::InvalidPhase));

        let mut bad_open_winner = bytes.clone();
        *bad_open_winner.get_mut(7).expect("winner exists") = 1;
        assert_eq!(State::decode(&bad_open_winner), Err(Error::InvalidWinner));

        let mut insolvent = bytes;
        insolvent
            .get_mut(8..16)
            .expect("hoard exists")
            .copy_from_slice(&0_u64.to_le_bytes());
        assert_eq!(State::decode(&insolvent), Err(Error::Insolvent));
    }

    #[test]
    fn selected_profile_is_single_fixed_layout() {
        assert_eq!(size_of::<Command>(), 16);
        assert_eq!(size_of::<Plan>(), 24);
        assert_eq!(size_of::<State>(), 912);
        assert_eq!(size_of::<State>(), size_of::<StateParts>());
        assert_eq!(size_of::<ClaimEffect>(), 16);
        assert_eq!(size_of::<CustodyTransfer>(), 16);
        assert!(size_of::<PhysicalPlan>() <= 304);
        assert_eq!(STATE_HEADER_BYTES + VECTOR_COUNT * MAX_OUTCOMES * 8, 912);
    }
}
