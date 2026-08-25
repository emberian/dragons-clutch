#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout persistence boundary for the shared economic microkernel.
//!
//! This crate is the single owner of one bounded two-holder economic
//! projection. It does not own Market identity, execution-release-set
//! membership, or token state: those values are immutable references to their
//! canonical owners. It deliberately has no Solana types, account borrowing,
//! hashing, CPI, allocation, or token parsing. A physical adapter authenticates
//! the release-set bytes and supplies a custody callback. Projection bytes are
//! committed only after the kernel and that callback both succeed.

use dclutch_economic_kernel::{
    Bindings, CommandTag, CustodyPlan, Holder, Party, Plan, Representation, STATE_HEADER_BYTES,
    State, execute,
};
use dclutch_release_set_contract::{ExecutionReleaseSetV1, ExecutionRoleV1};

/// Largest canonical `DCES` state admitted by the measured 16-outcome profile.
pub const MAX_STATE_BYTES_V1: usize =
    STATE_HEADER_BYTES + dclutch_economic_kernel::MAX_OUTCOMES * 7 * 8;
/// Exact bytes in one canonical successor projection account.
pub const PROJECTION_BYTES_V1: usize = 224 + MAX_STATE_BYTES_V1;
/// Exact bytes in one founding instruction.
pub const FOUNDING_BYTES_V1: usize = 208;
/// Exact bytes in one economic-operation instruction.
pub const OPERATION_BYTES_V1: usize = 32;
/// Canonical successor projection magic.
pub const PROJECTION_MAGIC_V1: [u8; 8] = *b"DCLTECO1";
/// Canonical successor instruction magic.
pub const INSTRUCTION_MAGIC_V1: [u8; 8] = *b"DCLTECI1";
/// Implemented projection and instruction schema.
pub const SCHEMA_VERSION_V1: u8 = 1;

const MARKET_OFFSET: usize = 16;
const RELEASE_SET_OFFSET: usize = 48;
const SOURCE_HOLDER_OFFSET: usize = 80;
const DESTINATION_HOLDER_OFFSET: usize = 112;
const COLLATERAL_MINT_OFFSET: usize = 144;
const HOARD_ACCOUNT_OFFSET: usize = 176;
const REVISION_OFFSET: usize = 208;
const STATE_LENGTH_OFFSET: usize = 216;
const STATE_OFFSET: usize = 224;

const TAG_OFFSET: usize = 9;
const HOLDER_OFFSET: usize = 10;
const REPRESENTATION_OFFSET: usize = 11;
const OUTCOME_OFFSET: usize = 12;
const OUTCOME_COUNT_OFFSET: usize = 13;
const QUANTITY_OFFSET: usize = 16;
const REVISION_INSTRUCTION_OFFSET: usize = 24;

const FOUNDING_MARKET_OFFSET: usize = 16;
const FOUNDING_RELEASE_SET_OFFSET: usize = 48;
const FOUNDING_SOURCE_HOLDER_OFFSET: usize = 80;
const FOUNDING_DESTINATION_HOLDER_OFFSET: usize = 112;
const FOUNDING_COLLATERAL_MINT_OFFSET: usize = 144;
const FOUNDING_HOARD_ACCOUNT_OFFSET: usize = 176;

/// Stable refusal from the fixed-layout successor boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An instruction or account had the wrong exact width.
    InvalidLength,
    /// Magic did not select this schema.
    InvalidMagic,
    /// The schema version is not implemented.
    UnsupportedVersion,
    /// A tag did not identify one supported instruction value.
    UnknownTag,
    /// Padding or a command-inapplicable field was nonzero.
    NoncanonicalReserved,
    /// An immutable identity was the reserved all-zero value.
    ZeroIdentity,
    /// Two identities required to be distinct aliased.
    IdentityAlias,
    /// Founding targeted bytes that were not wholly uninitialized.
    AlreadyInitialized,
    /// The supplied release-set content identity differed from the projection.
    ReleaseSetMismatch,
    /// The release-set account owner was not its canonical Core program.
    ReleaseSetOwnerMismatch,
    /// This adapter was not the release set's exact Claims and Custody program.
    AdapterRoleMismatch,
    /// The signer-owning program did not hold the required admission role.
    AdmissionRoleMismatch,
    /// The embedded execution release set refused canonical decoding.
    InvalidReleaseSet,
    /// The caller supplied a stale or future revision coordinate.
    RevisionMismatch,
    /// Incrementing the revision overflowed.
    RevisionOverflow,
    /// Embedded economic state bytes refused canonical decoding.
    InvalidState,
    /// The shared economic microkernel refused the operation.
    EconomicRefusal,
    /// The physical custody boundary refused the derived exact plan.
    CustodyRefusal,
}

/// Result alias for successor-boundary operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Authenticated release context supplied by the physical account adapter.
///
/// The adapter must compute `release_set_id` from the complete canonical
/// release-set bytes and authenticate `release_set_owner_program` from the
/// account owner. This SDK-free layer then checks the exact named roles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseContextV1 {
    /// Content identity of the complete canonical release-set bytes.
    pub release_set_id: [u8; 32],
    /// Canonically decoded execution release set.
    pub release_set: ExecutionReleaseSetV1,
    /// Program identity executing this Claims/Custody adapter.
    pub current_program: [u8; 32],
    /// Owner program of the authenticated release-set account.
    pub release_set_owner_program: [u8; 32],
    /// Owner program of the authenticated admission signer account.
    pub admission_program: [u8; 32],
}

/// Immutable coordinates accepted while founding one empty economic state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingV1 {
    market_id: [u8; 32],
    release_set_id: [u8; 32],
    source_holder: [u8; 32],
    destination_holder: [u8; 32],
    collateral_mint: [u8; 32],
    hoard_account: [u8; 32],
    outcome_count: u8,
}

impl FoundingV1 {
    /// Construct one checked founding value.
    pub fn new(
        market_id: [u8; 32],
        release_set_id: [u8; 32],
        source_holder: [u8; 32],
        destination_holder: [u8; 32],
        collateral_mint: [u8; 32],
        hoard_account: [u8; 32],
        outcome_count: u8,
    ) -> Result<Self> {
        for identity in [
            market_id,
            release_set_id,
            source_holder,
            destination_holder,
            collateral_mint,
            hoard_account,
        ] {
            require_nonzero(identity)?;
        }
        if source_holder == destination_holder || collateral_mint == hoard_account {
            return Err(Error::IdentityAlias);
        }
        State::empty_open(outcome_count).map_err(|_| Error::InvalidState)?;
        Ok(Self {
            market_id,
            release_set_id,
            source_holder,
            destination_holder,
            collateral_mint,
            hoard_account,
            outcome_count,
        })
    }

    /// Hostile-decode one exact canonical founding instruction.
    pub fn decode(input: &[u8]) -> Result<Self> {
        validate_instruction_header(input, FOUNDING_BYTES_V1, 0)?;
        if read_byte(input, HOLDER_OFFSET)? != 0
            || read_byte(input, REPRESENTATION_OFFSET)? != 0
            || read_byte(input, OUTCOME_OFFSET)? != 0
            || !is_zero(input, 14, 2)?
        {
            return Err(Error::NoncanonicalReserved);
        }
        Self::new(
            read_array(input, FOUNDING_MARKET_OFFSET)?,
            read_array(input, FOUNDING_RELEASE_SET_OFFSET)?,
            read_array(input, FOUNDING_SOURCE_HOLDER_OFFSET)?,
            read_array(input, FOUNDING_DESTINATION_HOLDER_OFFSET)?,
            read_array(input, FOUNDING_COLLATERAL_MINT_OFFSET)?,
            read_array(input, FOUNDING_HOARD_ACCOUNT_OFFSET)?,
            read_byte(input, OUTCOME_COUNT_OFFSET)?,
        )
    }

    /// Encode the one exact canonical founding instruction.
    #[must_use]
    pub fn to_bytes(self) -> [u8; FOUNDING_BYTES_V1] {
        let mut output = [0_u8; FOUNDING_BYTES_V1];
        write_infallible(&mut output, 0, &INSTRUCTION_MAGIC_V1);
        output[8] = SCHEMA_VERSION_V1;
        output[TAG_OFFSET] = 0;
        output[OUTCOME_COUNT_OFFSET] = self.outcome_count;
        write_infallible(&mut output, FOUNDING_MARKET_OFFSET, &self.market_id);
        write_infallible(
            &mut output,
            FOUNDING_RELEASE_SET_OFFSET,
            &self.release_set_id,
        );
        write_infallible(
            &mut output,
            FOUNDING_SOURCE_HOLDER_OFFSET,
            &self.source_holder,
        );
        write_infallible(
            &mut output,
            FOUNDING_DESTINATION_HOLDER_OFFSET,
            &self.destination_holder,
        );
        write_infallible(
            &mut output,
            FOUNDING_COLLATERAL_MINT_OFFSET,
            &self.collateral_mint,
        );
        write_infallible(
            &mut output,
            FOUNDING_HOARD_ACCOUNT_OFFSET,
            &self.hoard_account,
        );
        output
    }

    /// Return the selected execution-release-set content identity.
    #[must_use]
    pub const fn release_set_id(self) -> [u8; 32] {
        self.release_set_id
    }
}

/// One hostile-decodable economic operation and optimistic revision guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationV1 {
    tag: CommandTag,
    holder: Holder,
    representation: Representation,
    outcome: u8,
    quantity: u64,
    expected_revision: u64,
}

impl OperationV1 {
    /// Construct a complete-set split operation.
    #[must_use]
    pub const fn split(
        holder: Holder,
        representation: Representation,
        quantity: u64,
        expected_revision: u64,
    ) -> Self {
        Self::new(
            CommandTag::SplitCompleteSet,
            holder,
            representation,
            0,
            quantity,
            expected_revision,
        )
    }

    /// Construct a complete-set merge operation.
    #[must_use]
    pub const fn merge(
        holder: Holder,
        representation: Representation,
        quantity: u64,
        expected_revision: u64,
    ) -> Self {
        Self::new(
            CommandTag::MergeCompleteSet,
            holder,
            representation,
            0,
            quantity,
            expected_revision,
        )
    }

    /// Construct a claim transfer operation.
    #[must_use]
    pub const fn transfer(
        representation: Representation,
        outcome: u8,
        quantity: u64,
        expected_revision: u64,
    ) -> Self {
        Self::new(
            CommandTag::TransferClaim,
            Holder::Source,
            representation,
            outcome,
            quantity,
            expected_revision,
        )
    }

    /// Construct a native-to-materialized operation.
    #[must_use]
    pub const fn materialize(outcome: u8, quantity: u64, expected_revision: u64) -> Self {
        Self::new(
            CommandTag::MaterializeClaim,
            Holder::Source,
            Representation::Native,
            outcome,
            quantity,
            expected_revision,
        )
    }

    /// Construct a materialized-to-native operation.
    #[must_use]
    pub const fn dematerialize(outcome: u8, quantity: u64, expected_revision: u64) -> Self {
        Self::new(
            CommandTag::DematerializeClaim,
            Holder::Source,
            Representation::Materialized,
            outcome,
            quantity,
            expected_revision,
        )
    }

    /// Construct a terminal redemption operation.
    #[must_use]
    pub const fn redeem(
        holder: Holder,
        representation: Representation,
        outcome: u8,
        quantity: u64,
        expected_revision: u64,
    ) -> Self {
        Self::new(
            CommandTag::RedeemTerminal,
            holder,
            representation,
            outcome,
            quantity,
            expected_revision,
        )
    }

    /// Construct an empty-terminal retirement operation.
    #[must_use]
    pub const fn retire(expected_revision: u64) -> Self {
        Self::new(
            CommandTag::RetireTerminal,
            Holder::Source,
            Representation::Native,
            0,
            0,
            expected_revision,
        )
    }

    const fn new(
        tag: CommandTag,
        holder: Holder,
        representation: Representation,
        outcome: u8,
        quantity: u64,
        expected_revision: u64,
    ) -> Self {
        Self {
            tag,
            holder,
            representation,
            outcome,
            quantity,
            expected_revision,
        }
    }

    /// Hostile-decode one exact canonical economic operation.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != OPERATION_BYTES_V1
            || input.get(..8) != Some(INSTRUCTION_MAGIC_V1.as_slice())
            || read_byte(input, 8)? != SCHEMA_VERSION_V1
            || read_byte(input, OUTCOME_COUNT_OFFSET)? != 0
            || !is_zero(input, 14, 2)?
        {
            return Err(Error::InvalidLength);
        }
        let tag = decode_command_tag(read_byte(input, TAG_OFFSET)?)?;
        let holder = decode_holder(read_byte(input, HOLDER_OFFSET)?)?;
        let representation = decode_representation(read_byte(input, REPRESENTATION_OFFSET)?)?;
        let outcome = read_byte(input, OUTCOME_OFFSET)?;
        let quantity = read_u64(input, QUANTITY_OFFSET)?;
        let expected_revision = read_u64(input, REVISION_INSTRUCTION_OFFSET)?;
        let operation = Self::new(
            tag,
            holder,
            representation,
            outcome,
            quantity,
            expected_revision,
        );
        operation.validate_shape()?;
        Ok(operation)
    }

    /// Encode the one exact canonical economic operation.
    #[must_use]
    pub fn to_bytes(self) -> [u8; OPERATION_BYTES_V1] {
        let mut output = [0_u8; OPERATION_BYTES_V1];
        write_infallible(&mut output, 0, &INSTRUCTION_MAGIC_V1);
        output[8] = SCHEMA_VERSION_V1;
        output[TAG_OFFSET] = command_tag(self.tag);
        output[HOLDER_OFFSET] = holder_tag(self.holder);
        output[REPRESENTATION_OFFSET] = representation_tag(self.representation);
        output[OUTCOME_OFFSET] = self.outcome;
        write_infallible(&mut output, QUANTITY_OFFSET, &self.quantity.to_le_bytes());
        write_infallible(
            &mut output,
            REVISION_INSTRUCTION_OFFSET,
            &self.expected_revision.to_le_bytes(),
        );
        output
    }

    /// Return the optimistic revision required by this operation.
    #[must_use]
    pub const fn expected_revision(self) -> u64 {
        self.expected_revision
    }

    /// Return the admission role selected by this operation.
    #[must_use]
    pub const fn admission_role(self) -> ExecutionRoleV1 {
        match self.tag {
            CommandTag::SplitCompleteSet
            | CommandTag::MergeCompleteSet
            | CommandTag::TransferClaim
            | CommandTag::MaterializeClaim
            | CommandTag::DematerializeClaim => ExecutionRoleV1::Trading,
            CommandTag::RedeemTerminal | CommandTag::RetireTerminal => ExecutionRoleV1::Resolution,
        }
    }

    fn validate_shape(self) -> Result<()> {
        let canonical = match self.tag {
            CommandTag::SplitCompleteSet | CommandTag::MergeCompleteSet => self.outcome == 0,
            CommandTag::TransferClaim => self.holder == Holder::Source,
            CommandTag::MaterializeClaim => {
                self.holder == Holder::Source && self.representation == Representation::Native
            }
            CommandTag::DematerializeClaim => {
                self.holder == Holder::Source && self.representation == Representation::Materialized
            }
            CommandTag::RedeemTerminal => true,
            CommandTag::RetireTerminal => {
                self.holder == Holder::Source
                    && self.representation == Representation::Native
                    && self.outcome == 0
                    && self.quantity == 0
            }
        };
        if canonical {
            Ok(())
        } else {
            Err(Error::NoncanonicalReserved)
        }
    }

    fn plan(self) -> Plan {
        let bindings = canonical_bindings();
        match self.tag {
            CommandTag::SplitCompleteSet => {
                Plan::split_complete_set(bindings, self.holder, self.representation, self.quantity)
            }
            CommandTag::MergeCompleteSet => {
                Plan::merge_complete_set(bindings, self.holder, self.representation, self.quantity)
            }
            CommandTag::TransferClaim => {
                Plan::transfer_claim(bindings, self.representation, self.outcome, self.quantity)
            }
            CommandTag::MaterializeClaim => {
                Plan::materialize_claim(bindings, self.outcome, self.quantity)
            }
            CommandTag::DematerializeClaim => {
                Plan::dematerialize_claim(bindings, self.outcome, self.quantity)
            }
            CommandTag::RedeemTerminal => Plan::redeem_terminal(
                bindings,
                self.holder,
                self.representation,
                self.outcome,
                self.quantity,
            ),
            CommandTag::RetireTerminal => Plan::retire_terminal(bindings),
        }
    }
}

/// Canonically decoded persisted projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionV1 {
    market_id: [u8; 32],
    release_set_id: [u8; 32],
    source_holder: [u8; 32],
    destination_holder: [u8; 32],
    collateral_mint: [u8; 32],
    hoard_account: [u8; 32],
    revision: u64,
    state: State,
}

impl ProjectionV1 {
    /// Hostile-decode one exact fixed-width successor projection.
    #[inline(never)]
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != PROJECTION_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if input.get(..8) != Some(PROJECTION_MAGIC_V1.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_byte(input, 8)? != SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        if !is_zero(input, 9, 7)? || !is_zero(input, 218, 6)? {
            return Err(Error::NoncanonicalReserved);
        }
        let market_id = read_array(input, MARKET_OFFSET)?;
        let release_set_id = read_array(input, RELEASE_SET_OFFSET)?;
        let source_holder = read_array(input, SOURCE_HOLDER_OFFSET)?;
        let destination_holder = read_array(input, DESTINATION_HOLDER_OFFSET)?;
        let collateral_mint = read_array(input, COLLATERAL_MINT_OFFSET)?;
        let hoard_account = read_array(input, HOARD_ACCOUNT_OFFSET)?;
        for identity in [
            market_id,
            release_set_id,
            source_holder,
            destination_holder,
            collateral_mint,
            hoard_account,
        ] {
            require_nonzero(identity)?;
        }
        if source_holder == destination_holder || collateral_mint == hoard_account {
            return Err(Error::IdentityAlias);
        }
        let state_length = usize::from(read_u16(input, STATE_LENGTH_OFFSET)?);
        if state_length > MAX_STATE_BYTES_V1 {
            return Err(Error::InvalidState);
        }
        let state_end = STATE_OFFSET
            .checked_add(state_length)
            .ok_or(Error::InvalidState)?;
        let state = State::decode(
            input
                .get(STATE_OFFSET..state_end)
                .ok_or(Error::InvalidState)?,
        )
        .map_err(|_| Error::InvalidState)?;
        if state.encoded_len() != state_length
            || !is_zero(input, state_end, input.len() - state_end)?
        {
            return Err(Error::InvalidState);
        }
        Ok(Self {
            market_id,
            release_set_id,
            source_holder,
            destination_holder,
            collateral_mint,
            hoard_account,
            revision: read_u64(input, REVISION_OFFSET)?,
            state,
        })
    }

    /// Encode one projection into the exact fixed-width account representation.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != PROJECTION_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        write(output, 0, &PROJECTION_MAGIC_V1)?;
        write_byte(output, 8, SCHEMA_VERSION_V1)?;
        write(output, MARKET_OFFSET, &self.market_id)?;
        write(output, RELEASE_SET_OFFSET, &self.release_set_id)?;
        write(output, SOURCE_HOLDER_OFFSET, &self.source_holder)?;
        write(output, DESTINATION_HOLDER_OFFSET, &self.destination_holder)?;
        write(output, COLLATERAL_MINT_OFFSET, &self.collateral_mint)?;
        write(output, HOARD_ACCOUNT_OFFSET, &self.hoard_account)?;
        write(output, REVISION_OFFSET, &self.revision.to_le_bytes())?;
        let state_length = self.state.encoded_len();
        let encoded_length = u16::try_from(state_length).map_err(|_| Error::InvalidState)?;
        write(output, STATE_LENGTH_OFFSET, &encoded_length.to_le_bytes())?;
        let state_end = STATE_OFFSET
            .checked_add(state_length)
            .ok_or(Error::InvalidState)?;
        self.state
            .encode_into(
                output
                    .get_mut(STATE_OFFSET..state_end)
                    .ok_or(Error::InvalidState)?,
            )
            .map_err(|_| Error::InvalidState)?;
        Ok(())
    }

    /// Return the immutable Market content identity reference.
    #[must_use]
    pub const fn market_id(&self) -> [u8; 32] {
        self.market_id
    }

    /// Return the immutable execution-release-set content identity reference.
    #[must_use]
    pub const fn release_set_id(&self) -> [u8; 32] {
        self.release_set_id
    }

    /// Return the exact optimistic revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Return the canonical economic state owned by this projection.
    #[must_use]
    pub const fn state(&self) -> State {
        self.state
    }

    /// Return the selected collateral Mint address.
    #[must_use]
    pub const fn collateral_mint(&self) -> [u8; 32] {
        self.collateral_mint
    }

    /// Return the selected Hoard token-account address.
    #[must_use]
    pub const fn hoard_account(&self) -> [u8; 32] {
        self.hoard_account
    }

    /// Resolve a kernel party into its canonical physical identity.
    #[must_use]
    pub const fn party_identity(&self, party: Party) -> [u8; 32] {
        match party {
            Party::Seller => self.source_holder,
            Party::Buyer => self.destination_holder,
            Party::Venue => self.hoard_account,
        }
    }
}

/// Physical request delivered after successful kernel staging and before commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyRequestV1 {
    source_holder: [u8; 32],
    destination_holder: [u8; 32],
    collateral_mint: [u8; 32],
    hoard_account: [u8; 32],
    plan: CustodyPlan,
}

impl CustodyRequestV1 {
    /// Return the selected collateral Mint address.
    #[must_use]
    pub const fn collateral_mint(self) -> [u8; 32] {
        self.collateral_mint
    }

    /// Return the selected Hoard token-account address.
    #[must_use]
    pub const fn hoard_account(self) -> [u8; 32] {
        self.hoard_account
    }

    /// Resolve a kernel party into its canonical physical identity.
    #[must_use]
    pub const fn party_identity(self, party: Party) -> [u8; 32] {
        match party {
            Party::Seller => self.source_holder,
            Party::Buyer => self.destination_holder,
            Party::Venue => self.hoard_account,
        }
    }

    /// Return the exact kernel-derived custody plan.
    #[must_use]
    pub const fn plan(self) -> CustodyPlan {
        self.plan
    }
}

/// Found an empty, open projection in wholly zero account bytes.
#[inline(never)]
pub fn found_projection(
    output: &mut [u8],
    founding: FoundingV1,
    context: &ReleaseContextV1,
) -> Result<()> {
    if output.len() != PROJECTION_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    if output.iter().any(|byte| *byte != 0) {
        return Err(Error::AlreadyInitialized);
    }
    validate_context(context, ExecutionRoleV1::Core, founding.release_set_id)?;
    let projection = ProjectionV1 {
        market_id: founding.market_id,
        release_set_id: founding.release_set_id,
        source_holder: founding.source_holder,
        destination_holder: founding.destination_holder,
        collateral_mint: founding.collateral_mint,
        hoard_account: founding.hoard_account,
        revision: 0,
        state: State::empty_open(founding.outcome_count).map_err(|_| Error::InvalidState)?,
    };
    commit_projection(output, &projection)
}

/// Stage, physically execute, and atomically persist one economic operation.
///
/// The supplied callback must either execute the exact custody plan or refuse.
/// The projection remains byte-for-byte unchanged on every local refusal. In a
/// Solana adapter, any CPI effects are transaction-atomic with a later refusal.
#[inline(never)]
pub fn execute_projection<F>(
    output: &mut [u8],
    operation: OperationV1,
    context: &ReleaseContextV1,
    mut custody: F,
) -> Result<()>
where
    F: FnMut(&CustodyRequestV1) -> core::result::Result<(), ()>,
{
    let mut projection = ProjectionV1::decode(output)?;
    operation.validate_shape()?;
    validate_context(
        context,
        operation.admission_role(),
        projection.release_set_id,
    )?;
    if operation.expected_revision != projection.revision {
        return Err(Error::RevisionMismatch);
    }
    let next_revision = projection
        .revision
        .checked_add(1)
        .ok_or(Error::RevisionOverflow)?;
    let custody_plan = execute_kernel(operation, &mut projection.state)?;
    let request = CustodyRequestV1 {
        source_holder: projection.source_holder,
        destination_holder: projection.destination_holder,
        collateral_mint: projection.collateral_mint,
        hoard_account: projection.hoard_account,
        plan: custody_plan,
    };
    custody(&request).map_err(|()| Error::CustodyRefusal)?;
    projection.revision = next_revision;
    commit_projection(output, &projection)?;
    Ok(())
}

#[inline(never)]
fn commit_projection(output: &mut [u8], projection: &ProjectionV1) -> Result<()> {
    let mut candidate = [0_u8; PROJECTION_BYTES_V1];
    projection.encode_into(&mut candidate)?;
    output.copy_from_slice(&candidate);
    Ok(())
}

#[inline(never)]
fn execute_kernel(operation: OperationV1, state: &mut State) -> Result<CustodyPlan> {
    execute(&operation.plan(), state)
        .map(|physical| physical.custody)
        .map_err(|_| Error::EconomicRefusal)
}

fn validate_context(
    context: &ReleaseContextV1,
    admission_role: ExecutionRoleV1,
    expected_release_set_id: [u8; 32],
) -> Result<()> {
    require_nonzero(context.release_set_id)?;
    require_nonzero(context.current_program)?;
    require_nonzero(context.release_set_owner_program)?;
    require_nonzero(context.admission_program)?;
    if context.release_set_id != expected_release_set_id {
        return Err(Error::ReleaseSetMismatch);
    }
    let core = context.release_set.binding(ExecutionRoleV1::Core);
    if core.program().to_bytes() != context.release_set_owner_program {
        return Err(Error::ReleaseSetOwnerMismatch);
    }
    let claims = context.release_set.binding(ExecutionRoleV1::Claims);
    let custody = context.release_set.binding(ExecutionRoleV1::Custody);
    if claims.program().to_bytes() != context.current_program
        || custody.program().to_bytes() != context.current_program
        || claims != custody
    {
        return Err(Error::AdapterRoleMismatch);
    }
    if context
        .release_set
        .binding(admission_role)
        .program()
        .to_bytes()
        != context.admission_program
    {
        return Err(Error::AdmissionRoleMismatch);
    }
    Ok(())
}

const fn canonical_bindings() -> Bindings {
    Bindings {
        source: Party::Seller,
        destination: Party::Buyer,
        hoard: Party::Venue,
    }
}

fn validate_instruction_header(input: &[u8], length: usize, tag: u8) -> Result<()> {
    if input.len() != length {
        return Err(Error::InvalidLength);
    }
    if input.get(..8) != Some(INSTRUCTION_MAGIC_V1.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    if read_byte(input, 8)? != SCHEMA_VERSION_V1 {
        return Err(Error::UnsupportedVersion);
    }
    if read_byte(input, TAG_OFFSET)? != tag {
        return Err(Error::UnknownTag);
    }
    Ok(())
}

fn decode_command_tag(tag: u8) -> Result<CommandTag> {
    match tag {
        1 => Ok(CommandTag::SplitCompleteSet),
        2 => Ok(CommandTag::MergeCompleteSet),
        3 => Ok(CommandTag::TransferClaim),
        4 => Ok(CommandTag::MaterializeClaim),
        5 => Ok(CommandTag::DematerializeClaim),
        6 => Ok(CommandTag::RedeemTerminal),
        7 => Ok(CommandTag::RetireTerminal),
        _ => Err(Error::UnknownTag),
    }
}

const fn command_tag(tag: CommandTag) -> u8 {
    match tag {
        CommandTag::SplitCompleteSet => 1,
        CommandTag::MergeCompleteSet => 2,
        CommandTag::TransferClaim => 3,
        CommandTag::MaterializeClaim => 4,
        CommandTag::DematerializeClaim => 5,
        CommandTag::RedeemTerminal => 6,
        CommandTag::RetireTerminal => 7,
    }
}

fn decode_holder(tag: u8) -> Result<Holder> {
    match tag {
        0 => Ok(Holder::Source),
        1 => Ok(Holder::Destination),
        _ => Err(Error::UnknownTag),
    }
}

const fn holder_tag(holder: Holder) -> u8 {
    match holder {
        Holder::Source => 0,
        Holder::Destination => 1,
    }
}

fn decode_representation(tag: u8) -> Result<Representation> {
    match tag {
        0 => Ok(Representation::Native),
        1 => Ok(Representation::Materialized),
        _ => Err(Error::UnknownTag),
    }
}

const fn representation_tag(representation: Representation) -> u8 {
    match representation {
        Representation::Native => 0,
        Representation::Materialized => 1,
    }
}

fn require_nonzero(identity: [u8; 32]) -> Result<()> {
    if identity.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn checked_end(offset: usize, width: usize) -> Result<usize> {
    offset.checked_add(width).ok_or(Error::InvalidLength)
}

fn is_zero(input: &[u8], offset: usize, width: usize) -> Result<bool> {
    let end = checked_end(offset, width)?;
    Ok(input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0))
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = checked_end(offset, N)?;
    input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = checked_end(offset, value.len())?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn write_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

fn write_infallible(output: &mut [u8], offset: usize, value: &[u8]) {
    let Some(end) = offset.checked_add(value.len()) else {
        return;
    };
    let Some(destination) = output.get_mut(offset..end) else {
        return;
    };
    destination.copy_from_slice(value);
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_economic_kernel::{MAX_OUTCOMES, Phase, PhaseKind, StateParts};
    use dclutch_release_set_contract::{
        ArtifactReleaseIdV1, ExecutionRoleBindingV1, ProgramIdentityV1,
    };

    use super::*;

    fn role(program: u8, release: u8) -> ExecutionRoleBindingV1 {
        ExecutionRoleBindingV1::new(
            ProgramIdentityV1::new([program; 32]).expect("program"),
            ArtifactReleaseIdV1::new([release; 32]).expect("release"),
        )
    }

    fn release_set() -> ExecutionReleaseSetV1 {
        let shared = role(2, 12);
        ExecutionReleaseSetV1::new(role(1, 11), shared, role(3, 13), role(4, 14), shared)
            .expect("release set")
    }

    fn context(admission_program: u8) -> ReleaseContextV1 {
        ReleaseContextV1 {
            release_set_id: [9; 32],
            release_set: release_set(),
            current_program: [2; 32],
            release_set_owner_program: [1; 32],
            admission_program: [admission_program; 32],
        }
    }

    fn founding() -> FoundingV1 {
        FoundingV1::new([8; 32], [9; 32], [5; 32], [6; 32], [7; 32], [10; 32], 3).expect("founding")
    }

    fn founded() -> [u8; PROJECTION_BYTES_V1] {
        let mut bytes = [0_u8; PROJECTION_BYTES_V1];
        found_projection(&mut bytes, founding(), &context(1)).expect("found");
        bytes
    }

    fn encode_projection(state: State, revision: u64) -> [u8; PROJECTION_BYTES_V1] {
        let founding = founding();
        let projection = ProjectionV1 {
            market_id: founding.market_id,
            release_set_id: founding.release_set_id,
            source_holder: founding.source_holder,
            destination_holder: founding.destination_holder,
            collateral_mint: founding.collateral_mint,
            hoard_account: founding.hoard_account,
            revision,
            state,
        };
        let mut bytes = [0_u8; PROJECTION_BYTES_V1];
        projection.encode_into(&mut bytes).expect("projection");
        bytes
    }

    #[test]
    fn fixed_founding_round_trip_has_one_canonical_owner() {
        assert_eq!(MAX_STATE_BYTES_V1, 912);
        assert_eq!(PROJECTION_BYTES_V1, 1_136);
        let value = founding();
        assert_eq!(FoundingV1::decode(&value.to_bytes()), Ok(value));
        let bytes = founded();
        let projection = ProjectionV1::decode(&bytes).expect("projection");
        assert_eq!(projection.market_id(), [8; 32]);
        assert_eq!(projection.release_set_id(), [9; 32]);
        assert_eq!(projection.revision(), 0);
        assert_eq!(projection.state(), State::empty_open(3).expect("state"));

        let mut occupied = bytes;
        assert_eq!(
            found_projection(&mut occupied, value, &context(1)),
            Err(Error::AlreadyInitialized)
        );
        assert_eq!(occupied, bytes);
    }

    #[test]
    fn every_open_command_executes_through_one_shared_kernel() {
        let mut bytes = founded();
        let operations = [
            OperationV1::split(Holder::Source, Representation::Native, 10, 0),
            OperationV1::transfer(Representation::Native, 0, 2, 1),
            OperationV1::materialize(1, 3, 2),
            OperationV1::split(Holder::Source, Representation::Materialized, 5, 3),
            OperationV1::dematerialize(2, 2, 4),
            OperationV1::merge(Holder::Source, Representation::Native, 7, 5),
        ];
        let mut custody_count = 0_u8;
        for operation in operations {
            assert_eq!(OperationV1::decode(&operation.to_bytes()), Ok(operation));
            execute_projection(&mut bytes, operation, &context(3), |request| {
                custody_count = custody_count
                    .checked_add(u8::from(!request.plan().is_empty()))
                    .ok_or(())?;
                Ok(())
            })
            .expect("execute");
        }
        let projection = ProjectionV1::decode(&bytes).expect("post");
        assert_eq!(projection.revision(), 6);
        assert_eq!(projection.state().hoard(), 8);
        assert_eq!(custody_count, 3);
        assert_eq!(projection.state().validate(), Ok(()));
    }

    #[test]
    fn redemption_and_retirement_execute_with_resolution_role() {
        let mut supply = [0_u64; MAX_OUTCOMES];
        let mut native_supply = [0_u64; MAX_OUTCOMES];
        let mut source_native = [0_u64; MAX_OUTCOMES];
        supply[0] = 10;
        supply[1] = 10;
        native_supply[0] = 10;
        native_supply[1] = 10;
        source_native[0] = 10;
        source_native[1] = 10;
        let terminal = State::from_parts(StateParts {
            outcome_count: 2,
            phase: Phase::terminal(1),
            hoard: 10,
            supply,
            native_supply,
            materialized_supply: [0; MAX_OUTCOMES],
            source_native,
            source_materialized: [0; MAX_OUTCOMES],
            destination_native: [0; MAX_OUTCOMES],
            destination_materialized: [0; MAX_OUTCOMES],
        })
        .expect("terminal");
        let mut bytes = encode_projection(terminal, 7);
        execute_projection(
            &mut bytes,
            OperationV1::redeem(Holder::Source, Representation::Native, 0, 10, 7),
            &context(4),
            |request| {
                assert!(request.plan().is_empty());
                Ok(())
            },
        )
        .expect("loser redemption");
        execute_projection(
            &mut bytes,
            OperationV1::redeem(Holder::Source, Representation::Native, 1, 10, 8),
            &context(4),
            |request| {
                let plan = request.plan();
                let transfer = plan.transfer(0).ok_or(())?;
                assert_eq!(transfer.source, Party::Venue);
                assert_eq!(transfer.destination, Party::Seller);
                assert_eq!(transfer.amount, 10);
                Ok(())
            },
        )
        .expect("winner redemption");
        let post = ProjectionV1::decode(&bytes).expect("post");
        assert_eq!(post.state().hoard(), 0);
        assert!(post.state().supply().iter().all(|amount| *amount == 0));

        let retiring = State::from_parts(StateParts {
            outcome_count: 2,
            phase: Phase::retiring(1),
            hoard: 0,
            supply: [0; MAX_OUTCOMES],
            native_supply: [0; MAX_OUTCOMES],
            materialized_supply: [0; MAX_OUTCOMES],
            source_native: [0; MAX_OUTCOMES],
            source_materialized: [0; MAX_OUTCOMES],
            destination_native: [0; MAX_OUTCOMES],
            destination_materialized: [0; MAX_OUTCOMES],
        })
        .expect("retiring");
        let mut retirement = encode_projection(retiring, 9);
        execute_projection(
            &mut retirement,
            OperationV1::retire(9),
            &context(4),
            |request| {
                assert!(request.plan().is_empty());
                Ok(())
            },
        )
        .expect("retire");
        assert_eq!(
            ProjectionV1::decode(&retirement)
                .expect("retired")
                .state()
                .phase()
                .kind(),
            PhaseKind::Retired
        );
    }

    #[test]
    fn stale_kernel_role_and_late_custody_refusals_are_atomic() {
        let mut bytes = founded();
        let before = bytes;
        let mut callback_reached = false;
        assert_eq!(
            execute_projection(
                &mut bytes,
                OperationV1::merge(Holder::Source, Representation::Native, 1, 0),
                &context(3),
                |_| {
                    callback_reached = true;
                    Ok(())
                }
            ),
            Err(Error::EconomicRefusal)
        );
        assert!(!callback_reached);
        assert_eq!(bytes, before);

        let hostile_context = ReleaseContextV1 {
            admission_program: [4; 32],
            ..context(3)
        };
        assert_eq!(
            execute_projection(
                &mut bytes,
                OperationV1::split(Holder::Source, Representation::Native, 2, 0),
                &hostile_context,
                |_| Ok(())
            ),
            Err(Error::AdmissionRoleMismatch)
        );
        assert_eq!(bytes, before);

        let mut external_attempted = false;
        assert_eq!(
            execute_projection(
                &mut bytes,
                OperationV1::split(Holder::Source, Representation::Native, 2, 0),
                &context(3),
                |_| {
                    external_attempted = true;
                    Err(())
                }
            ),
            Err(Error::CustodyRefusal)
        );
        assert!(external_attempted);
        assert_eq!(bytes, before);

        assert_eq!(
            execute_projection(
                &mut bytes,
                OperationV1::split(Holder::Source, Representation::Native, 2, 1),
                &context(3),
                |_| Ok(())
            ),
            Err(Error::RevisionMismatch)
        );
        assert_eq!(bytes, before);
    }

    #[test]
    fn hostile_wire_and_account_tails_refuse() {
        let operation = OperationV1::retire(3);
        let mut wire = operation.to_bytes();
        wire[OUTCOME_OFFSET] = 1;
        assert_eq!(OperationV1::decode(&wire), Err(Error::NoncanonicalReserved));
        let mut unknown = operation.to_bytes();
        unknown[TAG_OFFSET] = 99;
        assert_eq!(OperationV1::decode(&unknown), Err(Error::UnknownTag));

        let bytes = founded();
        assert_eq!(
            ProjectionV1::decode(bytes.get(..bytes.len() - 1).unwrap_or(&[])),
            Err(Error::InvalidLength)
        );
        let mut reserved = bytes;
        reserved[218] = 1;
        assert_eq!(
            ProjectionV1::decode(&reserved),
            Err(Error::NoncanonicalReserved)
        );
        let mut tail = bytes;
        tail[PROJECTION_BYTES_V1 - 1] = 1;
        assert_eq!(ProjectionV1::decode(&tail), Err(Error::InvalidState));
    }

    #[test]
    fn account_and_instruction_widths_are_fixed() {
        assert_eq!(core::mem::size_of::<FoundingV1>(), 193);
        assert_eq!(core::mem::size_of::<OperationV1>(), 24);
        assert_eq!(FOUNDING_BYTES_V1, 208);
        assert_eq!(OPERATION_BYTES_V1, 32);
        assert_eq!(PROJECTION_BYTES_V1, 1_136);
    }
}
