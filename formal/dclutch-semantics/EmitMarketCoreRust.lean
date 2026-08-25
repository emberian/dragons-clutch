import DClutchSemantics.MarketCoreAbi

/-! Emit the safe fixed-memory Rust interpreter for the Lean-owned Market Core ABI. -/

open DClutch
open DClutch.AbiSchema
open DClutch.MarketCoreAbi

def rustByte (byte : UInt8) : String := s!"0x{Codec.byteHex byte}"

def emitBytes (name : String) (bytes : List UInt8) : IO Unit := do
  IO.println s!"pub const {name}: [u8; {bytes.length}] = ["
  IO.println s!"    {String.intercalate ", " (bytes.map rustByte)},"
  IO.println "];"

def interpreter : String := r#"
/// Width of one opaque nonzero content or account identity.
pub const IDENTITY_BYTES: usize = 32;
/// Number of immutable execution roles.
pub const ROLE_COUNT: usize = 5;
/// Number of exact economic vectors stored behind the fixed Core header.
pub const ECONOMIC_VECTOR_COUNT: usize = 7;
/// Little-endian bytes stored for one vector value.
pub const ECONOMIC_VALUE_BYTES: usize = 8;

/// Strict physical or semantic refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    NonzeroReserved,
    InvalidTag,
    InvalidIdentity,
    InvalidRelease,
    InvalidAlias,
    InvalidCoordinates,
    InvalidAccount,
    InvalidPhase,
    InvalidFunding,
    InvalidTerminalReceipt,
    InvalidOutcomeWidth,
    InvalidEconomicState,
    InsufficientBalance,
    ArithmeticOverflow,
}

/// Opaque nonzero 32-byte identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Identity([u8; IDENTITY_BYTES]);

impl Identity {
    /// Validate one nonzero identity.
    pub fn new(bytes: [u8; IDENTITY_BYTES]) -> Result<Self, Error> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(Error::InvalidIdentity);
        }
        Ok(Self(bytes))
    }

    /// Return exact bytes.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; IDENTITY_BYTES] {
        self.0
    }
}

/// Semantic execution role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Role {
    Core = 0,
    Claims = 1,
    Trading = 2,
    Resolution = 3,
    Custody = 4,
}

/// Exact Program/artifact/semantic release binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    pub program: Identity,
    pub artifact_release: Identity,
    pub semantic_release: Identity,
}

/// Immutable five-role release set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseSet {
    pub release_set_id: Identity,
    pub bindings: [Binding; ROLE_COUNT],
}

impl ReleaseSet {
    fn binding(self, role: Role) -> Binding {
        let [core, claims, trading, resolution, custody] = self.bindings;
        match role {
            Role::Core => core,
            Role::Claims => claims,
            Role::Trading => trading,
            Role::Resolution => resolution,
            Role::Custody => custody,
        }
    }

    fn valid(self) -> bool {
        for left in &self.bindings {
            for right in &self.bindings {
                if left.program == right.program && left != right {
                    return false;
                }
            }
        }
        true
    }
}

/// Current Registry/Core adapter receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseReceipt {
    pub registry_program: Identity,
    pub release_set_id: Identity,
    pub role: Role,
    pub observed: Binding,
    pub activation_cache_authenticated: bool,
    pub current_deployment_reauthenticated: bool,
}

/// Immutable Market selection plus one current receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Admission {
    pub market_release_set_id: Identity,
    pub selected: ReleaseSet,
    pub receipt: ReleaseReceipt,
}

fn admission_valid(admission: Admission, role: Role) -> bool {
    admission.selected.valid()
        && admission.market_release_set_id == admission.selected.release_set_id
        && admission.receipt.registry_program == admission.selected.binding(Role::Core).program
        && admission.receipt.release_set_id == admission.selected.release_set_id
        && admission.receipt.role == role
        && admission.receipt.observed == admission.selected.binding(role)
        && admission.receipt.activation_cache_authenticated
        && admission.receipt.current_deployment_reauthenticated
}

/// Immutable Realm collateral coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Realm {
    pub realm_id: Identity,
    pub collateral_mint: Identity,
    pub token_program: Identity,
    pub collateral_release: Identity,
}

/// Canonical Product/result-domain coordinates and runtime width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Product {
    pub product_id: Identity,
    pub result_domain: Identity,
    pub claim_basis: Identity,
    pub capacity_profile: Identity,
    pub compiler_release: Identity,
    pub outcome_count: u32,
    pub scalar_limit: u64,
}

impl Product {
    fn valid(self) -> bool {
        self.outcome_count > 1 && self.scalar_limit > 0
    }
}

/// Immutable Market identity coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketIdentity {
    pub market_id: Identity,
    pub realm_id: Identity,
    pub product_id: Identity,
    pub result_domain: Identity,
    pub resolution_policy: Identity,
    pub selected_release_set: Identity,
    pub generation: u64,
}

/// Adapter-authenticated fixed Core child coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreCoordinates {
    pub derivation_authenticated: bool,
    pub market: Identity,
    pub hoard: Identity,
    pub fund: Identity,
    pub readiness: Identity,
    pub custody: Identity,
    pub rent_credit: Identity,
}

impl CoreCoordinates {
    fn valid(self) -> bool {
        if !self.derivation_authenticated {
            return false;
        }
        let values = [
            self.market,
            self.hoard,
            self.fund,
            self.readiness,
            self.custody,
            self.rent_credit,
        ];
        for (index, left) in values.iter().enumerate() {
            if values.iter().skip(index.saturating_add(1)).any(|right| left == right) {
                return false;
            }
        }
        true
    }
}

/// Core lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Founding,
    Open,
    Terminal,
    Retiring,
    Retired,
}

/// Resolution Fund readiness phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Readiness {
    Prepaid,
    Ready,
    Consumed,
}

/// Separately classified rent and donation compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capital {
    pub market_rent: u64,
    pub market_donation: u64,
    pub hoard_rent: u64,
    pub hoard_donation: u64,
    pub fund_rent: u64,
    pub fund_donation: u64,
    pub readiness_rent: u64,
    pub readiness_donation: u64,
    pub custody_rent: u64,
    pub custody_donation: u64,
    pub deferred_custody_rent: u64,
    pub rent_credit: u64,
}

/// Immutable Source funding commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingCommitment {
    pub allocation_id: Identity,
    pub initial_work_capital: u64,
}

/// Persisted fixed Market Core header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreState {
    pub phase: Phase,
    pub readiness: Readiness,
    pub terminal_winner: u32,
    pub realm: Realm,
    pub product: Product,
    pub identity: MarketIdentity,
    pub execution_releases: ReleaseSet,
    pub coordinates: CoreCoordinates,
    pub capital: Capital,
    pub funding: FundingCommitment,
    pub terminal_receipt: Option<Identity>,
    pub terminal_funding_remaining: u64,
    pub hoard_principal: u64,
}

impl CoreState {
    fn valid_static(self) -> bool {
        if !self.product.valid()
            || !self.execution_releases.valid()
            || !self.coordinates.valid()
            || self.identity.market_id != self.coordinates.market
            || self.identity.realm_id != self.realm.realm_id
            || self.identity.product_id != self.product.product_id
            || self.identity.result_domain != self.product.result_domain
            || self.identity.selected_release_set != self.execution_releases.release_set_id
            || self.funding.initial_work_capital == 0
        {
            return false;
        }
        match self.phase {
            Phase::Founding => {
                self.readiness != Readiness::Consumed
                    && self.terminal_receipt.is_none()
                    && self.terminal_funding_remaining == 0
                    && self.capital.market_rent > 0
                    && self.capital.hoard_rent > 0
                    && self.capital.fund_rent > 0
                    && self.capital.readiness_rent > 0
                    && self.capital.deferred_custody_rent > 0
                    && self.capital.custody_rent == 0
                    && self.terminal_winner == 0
            }
            Phase::Open => {
                self.readiness == Readiness::Consumed
                    && self.terminal_receipt.is_none()
                    && self.terminal_funding_remaining == 0
                    && live_capital(self.capital)
                    && self.terminal_winner == 0
            }
            Phase::Terminal | Phase::Retiring => {
                self.readiness == Readiness::Consumed
                    && self.terminal_receipt.is_some()
                    && self.terminal_winner < self.product.outcome_count
                    && live_capital(self.capital)
            }
            Phase::Retired => {
                self.readiness == Readiness::Consumed
                    && self.terminal_receipt.is_some()
                    && self.terminal_funding_remaining == 0
                    && capital_cleared(self.capital)
                    && self.hoard_principal == 0
            }
        }
    }

    /// Encode the sole canonical fixed state header.
    pub fn encode(self) -> Result<[u8; STATE_BYTES], Error> {
        if !self.valid_static() {
            return Err(Error::InvalidPhase);
        }
        let mut output = [0_u8; STATE_BYTES];
        put(&mut output, STATE_MAGIC_OFFSET, &STATE_MAGIC)?;
        put_u16(&mut output, STATE_VERSION_OFFSET, VERSION)?;
        put_byte(&mut output, STATE_PHASE_OFFSET, phase_tag(self.phase))?;
        put_byte(&mut output, STATE_READINESS_OFFSET, readiness_tag(self.readiness))?;
        put_u32(&mut output, STATE_TERMINAL_WINNER_OFFSET, self.terminal_winner)?;
        put_realm(&mut output, self.realm)?;
        put_product(&mut output, self.product)?;
        put_identity(&mut output, STATE_MARKET_ID_OFFSET, self.identity.market_id)?;
        put_identity(&mut output, STATE_IDENTITY_REALM_OFFSET, self.identity.realm_id)?;
        put_identity(&mut output, STATE_IDENTITY_PRODUCT_OFFSET, self.identity.product_id)?;
        put_identity(
            &mut output,
            STATE_IDENTITY_RESULT_DOMAIN_OFFSET,
            self.identity.result_domain,
        )?;
        put_identity(&mut output, STATE_RESOLUTION_POLICY_OFFSET, self.identity.resolution_policy)?;
        put_identity(&mut output, STATE_SELECTED_RELEASE_SET_OFFSET, self.identity.selected_release_set)?;
        put_u64(&mut output, STATE_GENERATION_OFFSET, self.identity.generation)?;
        put_release_set(&mut output, self.execution_releases)?;
        put_coordinates(&mut output, self.coordinates)?;
        put_capital(&mut output, self.capital)?;
        put_identity(&mut output, STATE_FUNDING_ALLOCATION_OFFSET, self.funding.allocation_id)?;
        put_u64(&mut output, STATE_INITIAL_WORK_CAPITAL_OFFSET, self.funding.initial_work_capital)?;
        if let Some(receipt) = self.terminal_receipt {
            put_identity(&mut output, STATE_TERMINAL_RECEIPT_OFFSET, receipt)?;
        }
        put_u64(
            &mut output,
            STATE_TERMINAL_FUNDING_REMAINING_OFFSET,
            self.terminal_funding_remaining,
        )?;
        put_u64(&mut output, STATE_HOARD_PRINCIPAL_OFFSET, self.hoard_principal)?;
        Ok(output)
    }

    /// Hostile-decode one exact canonical fixed state header.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != STATE_BYTES {
            return Err(Error::InvalidLength);
        }
        exact_magic(input, STATE_MAGIC_OFFSET, &STATE_MAGIC)?;
        if read_u16(input, STATE_VERSION_OFFSET)? != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, STATE_PRODUCT_RESERVED_OFFSET, 4)?;
        require_zero(input, STATE_COORDINATE_RESERVED_OFFSET, 7)?;
        let phase = decode_phase(read_byte(input, STATE_PHASE_OFFSET)?)?;
        let readiness = decode_readiness(read_byte(input, STATE_READINESS_OFFSET)?)?;
        let receipt_bytes = read_array(input, STATE_TERMINAL_RECEIPT_OFFSET)?;
        let terminal_receipt = if receipt_bytes.iter().all(|byte| *byte == 0) {
            None
        } else {
            Some(Identity::new(receipt_bytes)?)
        };
        let state = Self {
            phase,
            readiness,
            terminal_winner: read_u32(input, STATE_TERMINAL_WINNER_OFFSET)?,
            realm: read_realm(input)?,
            product: read_product(input)?,
            identity: MarketIdentity {
                market_id: read_identity(input, STATE_MARKET_ID_OFFSET)?,
                realm_id: read_identity(input, STATE_IDENTITY_REALM_OFFSET)?,
                product_id: read_identity(input, STATE_IDENTITY_PRODUCT_OFFSET)?,
                result_domain: read_identity(input, STATE_IDENTITY_RESULT_DOMAIN_OFFSET)?,
                resolution_policy: read_identity(input, STATE_RESOLUTION_POLICY_OFFSET)?,
                selected_release_set: read_identity(input, STATE_SELECTED_RELEASE_SET_OFFSET)?,
                generation: read_u64(input, STATE_GENERATION_OFFSET)?,
            },
            execution_releases: read_release_set(input)?,
            coordinates: read_coordinates(input)?,
            capital: read_capital(input)?,
            funding: FundingCommitment {
                allocation_id: read_identity(input, STATE_FUNDING_ALLOCATION_OFFSET)?,
                initial_work_capital: read_u64(input, STATE_INITIAL_WORK_CAPITAL_OFFSET)?,
            },
            terminal_receipt,
            terminal_funding_remaining: read_u64(
                input,
                STATE_TERMINAL_FUNDING_REMAINING_OFFSET,
            )?,
            hoard_principal: read_u64(input, STATE_HOARD_PRINCIPAL_OFFSET)?,
        };
        if !state.valid_static() {
            return Err(Error::InvalidPhase);
        }
        Ok(state)
    }
}

/// Fixed request action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Found,
    ActivateFund,
    OpenMarket,
    AdmitTerminal,
    Split,
    Redeem,
    BeginRetiring,
    Retire,
}

/// Which authenticated holder is selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Holder {
    Source,
    Destination,
}

/// Native or materialized claim representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Representation {
    Native,
    Materialized,
}

/// Canonical fixed action request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    pub action: Action,
    pub holder: Option<Holder>,
    pub representation: Option<Representation>,
    pub outcome: u32,
    pub quantity: u64,
    pub generation: u64,
    pub market: Identity,
}

impl Request {
    /// Construct an administrative action with canonical inactive fields.
    #[must_use]
    pub const fn administrative(
        action: Action,
        generation: u64,
        market: Identity,
    ) -> Self {
        Self {
            action,
            holder: None,
            representation: None,
            outcome: 0,
            quantity: 0,
            generation,
            market,
        }
    }

    /// Construct a split request.
    #[must_use]
    pub const fn split(
        holder: Holder,
        representation: Representation,
        quantity: u64,
        generation: u64,
        market: Identity,
    ) -> Self {
        Self {
            action: Action::Split,
            holder: Some(holder),
            representation: Some(representation),
            outcome: 0,
            quantity,
            generation,
            market,
        }
    }

    /// Construct a terminal redemption request.
    #[must_use]
    pub const fn redeem(
        holder: Holder,
        representation: Representation,
        outcome: u32,
        quantity: u64,
        generation: u64,
        market: Identity,
    ) -> Self {
        Self {
            action: Action::Redeem,
            holder: Some(holder),
            representation: Some(representation),
            outcome,
            quantity,
            generation,
            market,
        }
    }

    /// Encode one canonical request.
    pub fn encode(self) -> Result<[u8; REQUEST_BYTES], Error> {
        self.validate_shape()?;
        let mut output = [0_u8; REQUEST_BYTES];
        put(&mut output, REQUEST_MAGIC_OFFSET, &REQUEST_MAGIC)?;
        put_u16(&mut output, REQUEST_VERSION_OFFSET, VERSION)?;
        put_byte(&mut output, REQUEST_ACTION_OFFSET, action_tag(self.action))?;
        put_byte(&mut output, REQUEST_HOLDER_OFFSET, option_holder_tag(self.holder))?;
        put_byte(
            &mut output,
            REQUEST_REPRESENTATION_OFFSET,
            option_representation_tag(self.representation),
        )?;
        put_u32(&mut output, REQUEST_OUTCOME_OFFSET, self.outcome)?;
        put_u64(&mut output, REQUEST_QUANTITY_OFFSET, self.quantity)?;
        put_u64(&mut output, REQUEST_GENERATION_OFFSET, self.generation)?;
        put_identity(&mut output, REQUEST_MARKET_OFFSET, self.market)?;
        Ok(output)
    }

    /// Hostile-decode one exact request.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != REQUEST_BYTES {
            return Err(Error::InvalidLength);
        }
        exact_magic(input, REQUEST_MAGIC_OFFSET, &REQUEST_MAGIC)?;
        if read_u16(input, REQUEST_VERSION_OFFSET)? != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, REQUEST_RESERVED_A_OFFSET, 3)?;
        require_zero(input, REQUEST_RESERVED_B_OFFSET, 4)?;
        let request = Self {
            action: decode_action(read_byte(input, REQUEST_ACTION_OFFSET)?)?,
            holder: decode_holder(read_byte(input, REQUEST_HOLDER_OFFSET)?)?,
            representation: decode_representation(read_byte(input, REQUEST_REPRESENTATION_OFFSET)?)?,
            outcome: read_u32(input, REQUEST_OUTCOME_OFFSET)?,
            quantity: read_u64(input, REQUEST_QUANTITY_OFFSET)?,
            generation: read_u64(input, REQUEST_GENERATION_OFFSET)?,
            market: read_identity(input, REQUEST_MARKET_OFFSET)?,
        };
        request.validate_shape()?;
        Ok(request)
    }

    fn validate_shape(self) -> Result<(), Error> {
        match self.action {
            Action::Split => {
                if self.holder.is_none()
                    || self.representation.is_none()
                    || self.outcome != 0
                    || self.quantity == 0
                {
                    return Err(Error::InvalidTag);
                }
            }
            Action::Redeem => {
                if self.holder.is_none() || self.representation.is_none() || self.quantity == 0 {
                    return Err(Error::InvalidTag);
                }
            }
            _ => {
                if self.holder.is_some()
                    || self.representation.is_some()
                    || self.outcome != 0
                    || self.quantity != 0
                {
                    return Err(Error::InvalidTag);
                }
            }
        }
        Ok(())
    }
}

/// Vacant System-account observation. Lamports may be nonzero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VacantAccount {
    pub address: Identity,
    pub lamports: u64,
    pub system_owned: bool,
    pub data_empty: bool,
    pub executable: bool,
}

impl VacantAccount {
    fn valid(self) -> bool {
        self.system_owned && self.data_empty && !self.executable
    }
}

/// Exact prepaid Found quote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingQuote {
    pub market_rent: u64,
    pub hoard_rent: u64,
    pub fund_rent: u64,
    pub readiness_rent: u64,
    pub custody_rent: u64,
    pub source_funding_allocation: Identity,
    pub source_work_capital: u64,
}

/// Observed Found accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingAccounts {
    pub payer_lamports: u64,
    pub rent_credit: Identity,
    pub rent_credit_lamports: u64,
    pub market: VacantAccount,
    pub hoard: VacantAccount,
    pub fund: VacantAccount,
    pub readiness: VacantAccount,
}

/// Complete chain-derived Found input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingFrame {
    pub realm: Realm,
    pub product: Product,
    pub identity: MarketIdentity,
    pub core_admission: Admission,
    pub coordinates: CoreCoordinates,
    pub quote: FoundingQuote,
    pub accounts: FoundingAccounts,
}

/// Exact creation decomposition for one account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountCreation {
    pub before: u64,
    pub rent_minimum: u64,
    pub rent_top_up: u64,
    pub semantic_principal: u64,
    pub donation: u64,
    pub after: u64,
}

/// Exact four-account Found plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingPlan {
    pub market: AccountCreation,
    pub hoard: AccountCreation,
    pub fund: AccountCreation,
    pub readiness: AccountCreation,
    pub payer_debit: u64,
    pub payer_after: u64,
}

/// Capability-owned mutable Source funding state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingState {
    pub allocation_id: Identity,
    pub initial_capital: u64,
    pub remaining_capital: u64,
    pub paid_capital: u64,
    pub call_count: u64,
}

impl FundingState {
    fn conserved(self) -> bool {
        self.remaining_capital
            .checked_add(self.paid_capital)
            .is_some_and(|total| total == self.initial_capital)
    }
}

/// Successful funded Found output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingResult {
    pub state: CoreState,
    pub source_funding: FundingState,
    pub plan: FoundingPlan,
}

/// Trusted adapter observation of immutable Realm collateral.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralObservation {
    pub adapter_authenticated: bool,
    pub realm_id: Identity,
    pub collateral_mint: Identity,
    pub token_program: Identity,
    pub collateral_release: Identity,
}

/// Source-owned terminal receipt projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalReceipt {
    pub receipt_id: Identity,
    pub market_id: Identity,
    pub resolution_policy: Identity,
    pub product_id: Identity,
    pub generation: u64,
    pub selector: u32,
    pub funding_allocation: Identity,
    pub funding_remaining: u64,
    pub authenticated: bool,
}

/// One exact vector in the canonical economic tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EconomicVector {
    Supply,
    NativeSupply,
    MaterializedSupply,
    SourceNative,
    SourceMaterialized,
    DestinationNative,
    DestinationMaterialized,
}

impl EconomicVector {
    const fn index(self) -> usize {
        match self {
            Self::Supply => 0,
            Self::NativeSupply => 1,
            Self::MaterializedSupply => 2,
            Self::SourceNative => 3,
            Self::SourceMaterialized => 4,
            Self::DestinationNative => 5,
            Self::DestinationMaterialized => 6,
        }
    }
}

/// Exact mutable account-data tail for all runtime-width economic vectors.
pub struct EconomicTail<'a> {
    bytes: &'a mut [u8],
    outcome_count: u32,
    vector_bytes: usize,
}

impl<'a> EconomicTail<'a> {
    /// Hostile-validate and borrow one exact canonical account-data tail.
    pub fn new(bytes: &'a mut [u8], outcome_count: u32) -> Result<Self, Error> {
        let width = usize::try_from(outcome_count).map_err(|_| Error::InvalidOutcomeWidth)?;
        let vector_bytes = width
            .checked_mul(ECONOMIC_VALUE_BYTES)
            .ok_or(Error::InvalidOutcomeWidth)?;
        let expected = vector_bytes
            .checked_mul(ECONOMIC_VECTOR_COUNT)
            .ok_or(Error::InvalidOutcomeWidth)?;
        if outcome_count < 2 || bytes.len() != expected {
            return Err(Error::InvalidOutcomeWidth);
        }
        Ok(Self {
            bytes,
            outcome_count,
            vector_bytes,
        })
    }

    /// Return the exact required tail width for one runtime Product width.
    pub fn byte_len(outcome_count: u32) -> Result<usize, Error> {
        usize::try_from(outcome_count)
            .map_err(|_| Error::InvalidOutcomeWidth)?
            .checked_mul(ECONOMIC_VALUE_BYTES)
            .and_then(|value| value.checked_mul(ECONOMIC_VECTOR_COUNT))
            .ok_or(Error::InvalidOutcomeWidth)
    }

    /// Read one exact little-endian vector value.
    pub fn value(&self, vector: EconomicVector, outcome: u32) -> Result<u64, Error> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidOutcomeWidth);
        }
        let outcome = usize::try_from(outcome).map_err(|_| Error::InvalidOutcomeWidth)?;
        let offset = vector
            .index()
            .checked_mul(self.vector_bytes)
            .and_then(|value| {
                outcome
                    .checked_mul(ECONOMIC_VALUE_BYTES)
                    .and_then(|outcome_offset| value.checked_add(outcome_offset))
            })
            .ok_or(Error::InvalidOutcomeWidth)?;
        read_u64(self.bytes, offset)
    }

    fn vectors(&self) -> Result<EconomicVectors<'_>, Error> {
        split_vectors(self.bytes, self.vector_bytes)
    }

    fn vectors_mut(&mut self) -> Result<EconomicVectorsMut<'_>, Error> {
        split_vectors_mut(self.bytes, self.vector_bytes)
    }
}

struct EconomicVectors<'a> {
    supply: &'a [u8],
    native_supply: &'a [u8],
    materialized_supply: &'a [u8],
    source_native: &'a [u8],
    source_materialized: &'a [u8],
    destination_native: &'a [u8],
    destination_materialized: &'a [u8],
}

struct EconomicVectorsMut<'a> {
    supply: &'a mut [u8],
    native_supply: &'a mut [u8],
    materialized_supply: &'a mut [u8],
    source_native: &'a mut [u8],
    source_materialized: &'a mut [u8],
    destination_native: &'a mut [u8],
    destination_materialized: &'a mut [u8],
}

/// Execute funded Found without mutating external state.
pub fn found(request: Request, frame: FoundingFrame) -> Result<FoundingResult, Error> {
    require_request(request, Action::Found, frame.identity.market_id, frame.identity.generation)?;
    if !frame.product.valid()
        || frame.identity.market_id != frame.coordinates.market
        || frame.identity.realm_id != frame.realm.realm_id
        || frame.identity.product_id != frame.product.product_id
        || frame.identity.result_domain != frame.product.result_domain
        || frame.identity.selected_release_set != frame.core_admission.selected.release_set_id
        || !frame.coordinates.valid()
        || !admission_valid(frame.core_admission, Role::Core)
        || frame.core_admission.market_release_set_id != frame.identity.selected_release_set
        || frame.quote.market_rent == 0
        || frame.quote.hoard_rent == 0
        || frame.quote.fund_rent == 0
        || frame.quote.readiness_rent == 0
        || frame.quote.custody_rent == 0
        || frame.quote.source_work_capital == 0
    {
        return Err(Error::InvalidFunding);
    }
    let observed = [
        frame.accounts.market,
        frame.accounts.hoard,
        frame.accounts.fund,
        frame.accounts.readiness,
    ];
    if observed.iter().any(|account| !account.valid()) {
        return Err(Error::InvalidAccount);
    }
    if frame.accounts.market.address != frame.coordinates.market
        || frame.accounts.hoard.address != frame.coordinates.hoard
        || frame.accounts.fund.address != frame.coordinates.fund
        || frame.accounts.readiness.address != frame.coordinates.readiness
        || frame.accounts.rent_credit != frame.coordinates.rent_credit
    {
        return Err(Error::InvalidCoordinates);
    }
    let fund_principal = frame
        .quote
        .source_work_capital
        .checked_add(frame.quote.custody_rent)
        .ok_or(Error::ArithmeticOverflow)?;
    let market = account_creation(frame.accounts.market, frame.quote.market_rent, 0)?;
    let hoard = account_creation(frame.accounts.hoard, frame.quote.hoard_rent, 0)?;
    let fund = account_creation(frame.accounts.fund, frame.quote.fund_rent, fund_principal)?;
    let readiness = account_creation(
        frame.accounts.readiness,
        frame.quote.readiness_rent,
        0,
    )?;
    let payer_debit = market
        .rent_top_up
        .checked_add(hoard.rent_top_up)
        .and_then(|value| value.checked_add(fund.rent_top_up))
        .and_then(|value| value.checked_add(fund.semantic_principal))
        .and_then(|value| value.checked_add(readiness.rent_top_up))
        .ok_or(Error::ArithmeticOverflow)?;
    let payer_after = frame
        .accounts
        .payer_lamports
        .checked_sub(payer_debit)
        .ok_or(Error::InsufficientBalance)?;
    let state = CoreState {
        phase: Phase::Founding,
        readiness: Readiness::Prepaid,
        terminal_winner: 0,
        realm: frame.realm,
        product: frame.product,
        identity: frame.identity,
        execution_releases: frame.core_admission.selected,
        coordinates: frame.coordinates,
        capital: Capital {
            market_rent: frame.quote.market_rent,
            market_donation: market.donation,
            hoard_rent: frame.quote.hoard_rent,
            hoard_donation: hoard.donation,
            fund_rent: frame.quote.fund_rent,
            fund_donation: fund.donation,
            readiness_rent: frame.quote.readiness_rent,
            readiness_donation: readiness.donation,
            custody_rent: 0,
            custody_donation: 0,
            deferred_custody_rent: frame.quote.custody_rent,
            rent_credit: frame.accounts.rent_credit_lamports,
        },
        funding: FundingCommitment {
            allocation_id: frame.quote.source_funding_allocation,
            initial_work_capital: frame.quote.source_work_capital,
        },
        terminal_receipt: None,
        terminal_funding_remaining: 0,
        hoard_principal: 0,
    };
    if !state.valid_static() {
        return Err(Error::InvalidPhase);
    }
    Ok(FoundingResult {
        state,
        source_funding: FundingState {
            allocation_id: frame.quote.source_funding_allocation,
            initial_capital: frame.quote.source_work_capital,
            remaining_capital: frame.quote.source_work_capital,
            paid_capital: 0,
            call_count: 0,
        },
        plan: FoundingPlan {
            market,
            hoard,
            fund,
            readiness,
            payer_debit,
            payer_after,
        },
    })
}

/// Authenticate the prepaid Fund and mark readiness complete.
pub fn activate_fund(
    request: Request,
    state: &mut CoreState,
    admission: Admission,
    funding: FundingState,
) -> Result<(), Error> {
    require_request(request, Action::ActivateFund, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Founding || state.readiness != Readiness::Prepaid {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, admission, Role::Resolution)?;
    if funding.allocation_id != state.funding.allocation_id
        || funding.initial_capital != state.funding.initial_work_capital
        || funding.remaining_capital != funding.initial_capital
        || funding.paid_capital != 0
        || funding.call_count != 0
        || !funding.conserved()
    {
        return Err(Error::InvalidFunding);
    }
    let mut candidate = *state;
    candidate.readiness = Readiness::Ready;
    if !candidate.valid_static() {
        return Err(Error::InvalidPhase);
    }
    *state = candidate;
    Ok(())
}

/// Consume readiness and create exact Realm-selected custody.
pub fn open_market(
    request: Request,
    state: &mut CoreState,
    admission: Admission,
    collateral: CollateralObservation,
    custody: VacantAccount,
) -> Result<u64, Error> {
    require_request(request, Action::OpenMarket, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Founding || state.readiness != Readiness::Ready {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, admission, Role::Custody)?;
    if !collateral.adapter_authenticated
        || collateral.realm_id != state.realm.realm_id
        || collateral.collateral_mint != state.realm.collateral_mint
        || collateral.token_program != state.realm.token_program
        || collateral.collateral_release != state.realm.collateral_release
    {
        return Err(Error::InvalidCoordinates);
    }
    if !custody.valid() || custody.address != state.coordinates.custody {
        return Err(Error::InvalidAccount);
    }
    let reserved = state.capital.deferred_custody_rent;
    let top_up = reserved.saturating_sub(custody.lamports);
    let unused = reserved.saturating_sub(top_up);
    let readiness_refund = state
        .capital
        .readiness_rent
        .checked_add(state.capital.readiness_donation)
        .ok_or(Error::ArithmeticOverflow)?;
    let refund = unused
        .checked_add(readiness_refund)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut candidate = *state;
    candidate.phase = Phase::Open;
    candidate.readiness = Readiness::Consumed;
    candidate.capital.readiness_rent = 0;
    candidate.capital.readiness_donation = 0;
    candidate.capital.custody_rent = reserved;
    candidate.capital.custody_donation = custody.lamports.saturating_sub(reserved);
    candidate.capital.deferred_custody_rent = 0;
    candidate.capital.rent_credit = candidate
        .capital
        .rent_credit
        .checked_add(refund)
        .ok_or(Error::ArithmeticOverflow)?;
    if !candidate.valid_static() {
        return Err(Error::InvalidPhase);
    }
    *state = candidate;
    Ok(top_up)
}

/// Split one complete set using runtime Product-width slices.
pub fn split_complete_set(
    request: Request,
    state: &mut CoreState,
    economic: &mut EconomicTail<'_>,
    claims_admission: Admission,
    custody_admission: Admission,
) -> Result<(), Error> {
    require_request(request, Action::Split, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Open {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, claims_admission, Role::Claims)?;
    require_admission(*state, custody_admission, Role::Custody)?;
    validate_economic(*state, economic)?;
    let holder = request.holder.ok_or(Error::InvalidTag)?;
    let representation = request.representation.ok_or(Error::InvalidTag)?;
    let quantity = request.quantity;
    let hoard_after = state
        .hoard_principal
        .checked_add(quantity)
        .filter(|value| *value < state.product.scalar_limit)
        .ok_or(Error::ArithmeticOverflow)?;
    let vectors = economic.vectors()?;
    for values in [
        vectors.supply,
        representation_slice(&vectors, representation),
        holder_slice(&vectors, holder, representation),
    ] {
        if words(values).any(|value| {
            value.checked_add(quantity).is_none_or(|after| after >= state.product.scalar_limit)
        }) {
            return Err(Error::ArithmeticOverflow);
        }
    }
    let mut vectors = economic.vectors_mut()?;
    add_all(vectors.supply, quantity);
    add_all(representation_slice_mut(&mut vectors, representation), quantity);
    add_all(holder_slice_mut(&mut vectors, holder, representation), quantity);
    state.hoard_principal = hoard_after;
    Ok(())
}

/// Admit one exact Source terminal receipt.
pub fn admit_terminal(
    request: Request,
    state: &mut CoreState,
    admission: Admission,
    receipt: TerminalReceipt,
    economic: &EconomicTail<'_>,
) -> Result<(), Error> {
    require_request(request, Action::AdmitTerminal, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Open {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, admission, Role::Resolution)?;
    validate_economic(*state, economic)?;
    if !receipt.authenticated
        || receipt.market_id != state.identity.market_id
        || receipt.resolution_policy != state.identity.resolution_policy
        || receipt.product_id != state.product.product_id
        || receipt.generation != state.identity.generation
        || receipt.selector >= state.product.outcome_count
        || receipt.funding_allocation != state.funding.allocation_id
        || receipt.funding_remaining > state.funding.initial_work_capital
    {
        return Err(Error::InvalidTerminalReceipt);
    }
    let winning_supply = economic.value(EconomicVector::Supply, receipt.selector)?;
    if winning_supply > state.hoard_principal {
        return Err(Error::InvalidEconomicState);
    }
    let mut candidate = *state;
    candidate.phase = Phase::Terminal;
    candidate.terminal_winner = receipt.selector;
    candidate.terminal_receipt = Some(receipt.receipt_id);
    candidate.terminal_funding_remaining = receipt.funding_remaining;
    if !candidate.valid_static() {
        return Err(Error::InvalidPhase);
    }
    *state = candidate;
    Ok(())
}

/// Begin retiring while retaining permissionless redemption.
pub fn begin_retiring(
    request: Request,
    state: &mut CoreState,
    admission: Admission,
) -> Result<(), Error> {
    require_request(request, Action::BeginRetiring, state.identity.market_id, state.identity.generation)?;
    require_admission(*state, admission, Role::Core)?;
    if state.phase != Phase::Terminal {
        return Err(Error::InvalidPhase);
    }
    let mut candidate = *state;
    candidate.phase = Phase::Retiring;
    if !candidate.valid_static() {
        return Err(Error::InvalidPhase);
    }
    *state = candidate;
    Ok(())
}

/// Burn a terminal claim and release exactly the winning payout.
pub fn redeem_terminal(
    request: Request,
    state: &mut CoreState,
    economic: &mut EconomicTail<'_>,
    claims_admission: Admission,
    custody_admission: Admission,
) -> Result<u64, Error> {
    require_request(request, Action::Redeem, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Terminal && state.phase != Phase::Retiring {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, claims_admission, Role::Claims)?;
    require_admission(*state, custody_admission, Role::Custody)?;
    validate_economic(*state, economic)?;
    let holder = request.holder.ok_or(Error::InvalidTag)?;
    let representation = request.representation.ok_or(Error::InvalidTag)?;
    let outcome = usize::try_from(request.outcome).map_err(|_| Error::InvalidOutcomeWidth)?;
    let quantity = request.quantity;
    let outcome_u32 = request.outcome;
    let vectors = economic.vectors()?;
    let aggregate = value_at(vectors.supply, outcome)?;
    let represented = value_at(representation_slice(&vectors, representation), outcome)?;
    let held = value_at(holder_slice(&vectors, holder, representation), outcome)?;
    if aggregate < quantity || represented < quantity || held < quantity {
        return Err(Error::InsufficientBalance);
    }
    let payout = if request.outcome == state.terminal_winner {
        quantity
    } else {
        0
    };
    let hoard_after = state
        .hoard_principal
        .checked_sub(payout)
        .ok_or(Error::InsufficientBalance)?;
    if outcome_u32 >= economic.outcome_count {
        return Err(Error::InvalidOutcomeWidth);
    }
    let mut vectors = economic.vectors_mut()?;
    sub_at(vectors.supply, outcome, quantity);
    sub_at(representation_slice_mut(&mut vectors, representation), outcome, quantity);
    sub_at(holder_slice_mut(&mut vectors, holder, representation), outcome, quantity);
    state.hoard_principal = hoard_after;
    Ok(payout)
}

/// Close terminal Core accounts and return every remaining classified and
/// unclassified lamport to the immutable RentCredit.
pub fn retire(
    request: Request,
    state: &mut CoreState,
    economic: &EconomicTail<'_>,
    core_admission: Admission,
    custody_admission: Admission,
    funding: FundingState,
) -> Result<u64, Error> {
    require_request(request, Action::Retire, state.identity.market_id, state.identity.generation)?;
    require_admission(*state, core_admission, Role::Core)?;
    require_admission(*state, custody_admission, Role::Custody)?;
    if state.phase != Phase::Retiring {
        return Err(Error::InvalidPhase);
    }
    validate_economic(*state, economic)?;
    if state.hoard_principal != 0 || !economic_all_zero(economic)? {
        return Err(Error::InvalidEconomicState);
    }
    if funding.allocation_id != state.funding.allocation_id
        || funding.initial_capital != state.funding.initial_work_capital
        || funding.remaining_capital != state.terminal_funding_remaining
        || !funding.conserved()
    {
        return Err(Error::InvalidFunding);
    }
    let refund = retirement_refund(state.capital, funding.remaining_capital)?;
    let mut candidate = *state;
    candidate.phase = Phase::Retired;
    candidate.capital = Capital {
        market_rent: 0,
        market_donation: 0,
        hoard_rent: 0,
        hoard_donation: 0,
        fund_rent: 0,
        fund_donation: 0,
        readiness_rent: 0,
        readiness_donation: 0,
        custody_rent: 0,
        custody_donation: 0,
        deferred_custody_rent: 0,
        rent_credit: state
            .capital
            .rent_credit
            .checked_add(refund)
            .ok_or(Error::ArithmeticOverflow)?,
    };
    candidate.terminal_funding_remaining = 0;
    if !candidate.valid_static() {
        return Err(Error::InvalidPhase);
    }
    *state = candidate;
    Ok(refund)
}

fn require_request(
    request: Request,
    action: Action,
    market: Identity,
    generation: u64,
) -> Result<(), Error> {
    request.validate_shape()?;
    if request.action != action || request.market != market || request.generation != generation {
        return Err(Error::InvalidTag);
    }
    Ok(())
}

fn require_admission(state: CoreState, admission: Admission, role: Role) -> Result<(), Error> {
    if admission.market_release_set_id != state.identity.selected_release_set
        || admission.selected != state.execution_releases
        || !admission_valid(admission, role)
    {
        return Err(Error::InvalidRelease);
    }
    Ok(())
}

fn account_creation(
    account: VacantAccount,
    rent_minimum: u64,
    semantic_principal: u64,
) -> Result<AccountCreation, Error> {
    let rent_top_up = rent_minimum.saturating_sub(account.lamports);
    let donation = account.lamports.saturating_sub(rent_minimum);
    let after = account
        .lamports
        .checked_add(rent_top_up)
        .and_then(|value| value.checked_add(semantic_principal))
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(AccountCreation {
        before: account.lamports,
        rent_minimum,
        rent_top_up,
        semantic_principal,
        donation,
        after,
    })
}

fn live_capital(capital: Capital) -> bool {
    capital.market_rent > 0
        && capital.hoard_rent > 0
        && capital.fund_rent > 0
        && capital.readiness_rent == 0
        && capital.deferred_custody_rent == 0
        && capital.custody_rent > 0
}

fn capital_cleared(capital: Capital) -> bool {
    capital.market_rent == 0
        && capital.market_donation == 0
        && capital.hoard_rent == 0
        && capital.hoard_donation == 0
        && capital.fund_rent == 0
        && capital.fund_donation == 0
        && capital.readiness_rent == 0
        && capital.readiness_donation == 0
        && capital.custody_rent == 0
        && capital.custody_donation == 0
        && capital.deferred_custody_rent == 0
}

fn validate_economic(state: CoreState, view: &EconomicTail<'_>) -> Result<(), Error> {
    if view.outcome_count != state.product.outcome_count {
        return Err(Error::InvalidOutcomeWidth);
    }
    let vectors = view.vectors()?;
    let all = [
        vectors.supply,
        vectors.native_supply,
        vectors.materialized_supply,
        vectors.source_native,
        vectors.source_materialized,
        vectors.destination_native,
        vectors.destination_materialized,
    ];
    if all
        .iter()
        .any(|values| words(values).any(|value| value >= state.product.scalar_limit))
    {
        return Err(Error::InvalidEconomicState);
    }
    for (((aggregate, native), materialized), ((source_native, destination_native), (source_materialized, destination_materialized))) in view
        .vectors()?
        .supply
        .chunks_exact(ECONOMIC_VALUE_BYTES)
        .map(word)
        .zip(words(vectors.native_supply))
        .zip(words(vectors.materialized_supply))
        .zip(
            words(vectors.source_native)
                .zip(words(vectors.destination_native))
                .zip(words(vectors.source_materialized).zip(words(vectors.destination_materialized))),
        )
    {
        if native.checked_add(materialized) != Some(aggregate)
            || source_native
                .checked_add(destination_native)
                .is_none_or(|total| total > native)
            || source_materialized
                .checked_add(destination_materialized)
                .is_none_or(|total| total > materialized)
        {
            return Err(Error::InvalidEconomicState);
        }
    }
    match state.phase {
        Phase::Founding | Phase::Open => {
            if words(vectors.supply).any(|value| value > state.hoard_principal) {
                return Err(Error::InvalidEconomicState);
            }
        }
        Phase::Terminal | Phase::Retiring => {
            let winner = usize::try_from(state.terminal_winner).map_err(|_| Error::InvalidOutcomeWidth)?;
            if value_at(vectors.supply, winner)? > state.hoard_principal {
                return Err(Error::InvalidEconomicState);
            }
        }
        Phase::Retired => {
            if state.hoard_principal != 0
                || all.iter().any(|values| words(values).any(|value| value != 0))
            {
                return Err(Error::InvalidEconomicState);
            }
        }
    }
    Ok(())
}

fn representation_slice<'a>(
    vectors: &'a EconomicVectors<'_>,
    representation: Representation,
) -> &'a [u8] {
    match representation {
        Representation::Native => vectors.native_supply,
        Representation::Materialized => vectors.materialized_supply,
    }
}

fn representation_slice_mut<'a>(
    vectors: &'a mut EconomicVectorsMut<'_>,
    representation: Representation,
) -> &'a mut [u8] {
    match representation {
        Representation::Native => vectors.native_supply,
        Representation::Materialized => vectors.materialized_supply,
    }
}

fn holder_slice<'a>(
    vectors: &'a EconomicVectors<'_>,
    holder: Holder,
    representation: Representation,
) -> &'a [u8] {
    match (holder, representation) {
        (Holder::Source, Representation::Native) => vectors.source_native,
        (Holder::Source, Representation::Materialized) => vectors.source_materialized,
        (Holder::Destination, Representation::Native) => vectors.destination_native,
        (Holder::Destination, Representation::Materialized) => vectors.destination_materialized,
    }
}

fn holder_slice_mut<'a>(
    vectors: &'a mut EconomicVectorsMut<'_>,
    holder: Holder,
    representation: Representation,
) -> &'a mut [u8] {
    match (holder, representation) {
        (Holder::Source, Representation::Native) => vectors.source_native,
        (Holder::Source, Representation::Materialized) => vectors.source_materialized,
        (Holder::Destination, Representation::Native) => vectors.destination_native,
        (Holder::Destination, Representation::Materialized) => vectors.destination_materialized,
    }
}

fn add_all(values: &mut [u8], quantity: u64) {
    for value in values.chunks_exact_mut(ECONOMIC_VALUE_BYTES) {
        let after = word(value).saturating_add(quantity).to_le_bytes();
        for (destination, source) in value.iter_mut().zip(after) {
            *destination = source;
        }
    }
}

fn sub_at(values: &mut [u8], outcome: usize, quantity: u64) {
    for (index, value) in values.chunks_exact_mut(ECONOMIC_VALUE_BYTES).enumerate() {
        if index == outcome {
            let after = word(value).saturating_sub(quantity).to_le_bytes();
            for (destination, source) in value.iter_mut().zip(after) {
                *destination = source;
            }
        }
    }
}

fn economic_all_zero(view: &EconomicTail<'_>) -> Result<bool, Error> {
    let vectors = view.vectors()?;
    Ok([
        vectors.supply,
        vectors.native_supply,
        vectors.materialized_supply,
        vectors.source_native,
        vectors.source_materialized,
        vectors.destination_native,
        vectors.destination_materialized,
    ]
    .iter()
    .all(|values| words(values).all(|value| value == 0)))
}

fn value_at(values: &[u8], outcome: usize) -> Result<u64, Error> {
    let offset = outcome
        .checked_mul(ECONOMIC_VALUE_BYTES)
        .ok_or(Error::InvalidOutcomeWidth)?;
    read_u64(values, offset)
}

fn words(values: &[u8]) -> impl Iterator<Item = u64> + '_ {
    values.chunks_exact(ECONOMIC_VALUE_BYTES).map(word)
}

fn word(bytes: &[u8]) -> u64 {
    let mut value = [0_u8; ECONOMIC_VALUE_BYTES];
    for (destination, source) in value.iter_mut().zip(bytes.iter().copied()) {
        *destination = source;
    }
    u64::from_le_bytes(value)
}

fn split_vectors(bytes: &[u8], width: usize) -> Result<EconomicVectors<'_>, Error> {
    let (supply, rest) = bytes.split_at_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    let (native_supply, rest) = rest.split_at_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    let (materialized_supply, rest) = rest.split_at_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    let (source_native, rest) = rest.split_at_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    let (source_materialized, rest) = rest.split_at_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    let (destination_native, destination_materialized) =
        rest.split_at_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    if destination_materialized.len() != width {
        return Err(Error::InvalidOutcomeWidth);
    }
    Ok(EconomicVectors {
        supply,
        native_supply,
        materialized_supply,
        source_native,
        source_materialized,
        destination_native,
        destination_materialized,
    })
}

fn split_vectors_mut(bytes: &mut [u8], width: usize) -> Result<EconomicVectorsMut<'_>, Error> {
    let (supply, rest) = bytes.split_at_mut_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    let (native_supply, rest) = rest.split_at_mut_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    let (materialized_supply, rest) =
        rest.split_at_mut_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    let (source_native, rest) = rest.split_at_mut_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    let (source_materialized, rest) =
        rest.split_at_mut_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    let (destination_native, destination_materialized) =
        rest.split_at_mut_checked(width).ok_or(Error::InvalidOutcomeWidth)?;
    if destination_materialized.len() != width {
        return Err(Error::InvalidOutcomeWidth);
    }
    Ok(EconomicVectorsMut {
        supply,
        native_supply,
        materialized_supply,
        source_native,
        source_materialized,
        destination_native,
        destination_materialized,
    })
}

fn retirement_refund(capital: Capital, funding_remaining: u64) -> Result<u64, Error> {
    capital
        .market_rent
        .checked_add(capital.market_donation)
        .and_then(|value| value.checked_add(capital.hoard_rent))
        .and_then(|value| value.checked_add(capital.hoard_donation))
        .and_then(|value| value.checked_add(capital.fund_rent))
        .and_then(|value| value.checked_add(capital.fund_donation))
        .and_then(|value| value.checked_add(funding_remaining))
        .and_then(|value| value.checked_add(capital.custody_rent))
        .and_then(|value| value.checked_add(capital.custody_donation))
        .ok_or(Error::ArithmeticOverflow)
}

fn phase_tag(phase: Phase) -> u8 {
    match phase {
        Phase::Founding => PHASE_FOUNDING_TAG,
        Phase::Open => PHASE_OPEN_TAG,
        Phase::Terminal => PHASE_TERMINAL_TAG,
        Phase::Retiring => PHASE_RETIRING_TAG,
        Phase::Retired => PHASE_RETIRED_TAG,
    }
}

fn readiness_tag(readiness: Readiness) -> u8 {
    match readiness {
        Readiness::Prepaid => READINESS_PREPAID_TAG,
        Readiness::Ready => READINESS_READY_TAG,
        Readiness::Consumed => READINESS_CONSUMED_TAG,
    }
}

fn decode_phase(tag: u8) -> Result<Phase, Error> {
    match tag {
        PHASE_FOUNDING_TAG => Ok(Phase::Founding),
        PHASE_OPEN_TAG => Ok(Phase::Open),
        PHASE_TERMINAL_TAG => Ok(Phase::Terminal),
        PHASE_RETIRING_TAG => Ok(Phase::Retiring),
        PHASE_RETIRED_TAG => Ok(Phase::Retired),
        _ => Err(Error::InvalidTag),
    }
}

fn decode_readiness(tag: u8) -> Result<Readiness, Error> {
    match tag {
        READINESS_PREPAID_TAG => Ok(Readiness::Prepaid),
        READINESS_READY_TAG => Ok(Readiness::Ready),
        READINESS_CONSUMED_TAG => Ok(Readiness::Consumed),
        _ => Err(Error::InvalidTag),
    }
}

fn action_tag(action: Action) -> u8 {
    match action {
        Action::Found => ACTION_FOUND_TAG,
        Action::ActivateFund => ACTION_ACTIVATE_FUND_TAG,
        Action::OpenMarket => ACTION_OPEN_MARKET_TAG,
        Action::AdmitTerminal => ACTION_ADMIT_TERMINAL_TAG,
        Action::Split => ACTION_SPLIT_TAG,
        Action::Redeem => ACTION_REDEEM_TAG,
        Action::BeginRetiring => ACTION_BEGIN_RETIRING_TAG,
        Action::Retire => ACTION_RETIRE_TAG,
    }
}

fn decode_action(tag: u8) -> Result<Action, Error> {
    match tag {
        ACTION_FOUND_TAG => Ok(Action::Found),
        ACTION_ACTIVATE_FUND_TAG => Ok(Action::ActivateFund),
        ACTION_OPEN_MARKET_TAG => Ok(Action::OpenMarket),
        ACTION_ADMIT_TERMINAL_TAG => Ok(Action::AdmitTerminal),
        ACTION_SPLIT_TAG => Ok(Action::Split),
        ACTION_REDEEM_TAG => Ok(Action::Redeem),
        ACTION_BEGIN_RETIRING_TAG => Ok(Action::BeginRetiring),
        ACTION_RETIRE_TAG => Ok(Action::Retire),
        _ => Err(Error::InvalidTag),
    }
}

fn option_holder_tag(holder: Option<Holder>) -> u8 {
    match holder {
        None => HOLDER_NONE_TAG,
        Some(Holder::Source) => HOLDER_SOURCE_TAG,
        Some(Holder::Destination) => HOLDER_DESTINATION_TAG,
    }
}

fn decode_holder(tag: u8) -> Result<Option<Holder>, Error> {
    match tag {
        HOLDER_NONE_TAG => Ok(None),
        HOLDER_SOURCE_TAG => Ok(Some(Holder::Source)),
        HOLDER_DESTINATION_TAG => Ok(Some(Holder::Destination)),
        _ => Err(Error::InvalidTag),
    }
}

fn option_representation_tag(representation: Option<Representation>) -> u8 {
    match representation {
        None => REPRESENTATION_NONE_TAG,
        Some(Representation::Native) => REPRESENTATION_NATIVE_TAG,
        Some(Representation::Materialized) => REPRESENTATION_MATERIALIZED_TAG,
    }
}

fn decode_representation(tag: u8) -> Result<Option<Representation>, Error> {
    match tag {
        REPRESENTATION_NONE_TAG => Ok(None),
        REPRESENTATION_NATIVE_TAG => Ok(Some(Representation::Native)),
        REPRESENTATION_MATERIALIZED_TAG => Ok(Some(Representation::Materialized)),
        _ => Err(Error::InvalidTag),
    }
}

const RELEASE_OFFSETS: [(usize, usize, usize); ROLE_COUNT] = [
    (STATE_CORE_PROGRAM_OFFSET, STATE_CORE_ARTIFACT_OFFSET, STATE_CORE_SEMANTIC_OFFSET),
    (STATE_CLAIMS_PROGRAM_OFFSET, STATE_CLAIMS_ARTIFACT_OFFSET, STATE_CLAIMS_SEMANTIC_OFFSET),
    (STATE_TRADING_PROGRAM_OFFSET, STATE_TRADING_ARTIFACT_OFFSET, STATE_TRADING_SEMANTIC_OFFSET),
    (
        STATE_RESOLUTION_PROGRAM_OFFSET,
        STATE_RESOLUTION_ARTIFACT_OFFSET,
        STATE_RESOLUTION_SEMANTIC_OFFSET,
    ),
    (STATE_CUSTODY_PROGRAM_OFFSET, STATE_CUSTODY_ARTIFACT_OFFSET, STATE_CUSTODY_SEMANTIC_OFFSET),
];

fn put_realm(output: &mut [u8], realm: Realm) -> Result<(), Error> {
    put_identity(output, STATE_REALM_ID_OFFSET, realm.realm_id)?;
    put_identity(output, STATE_COLLATERAL_MINT_OFFSET, realm.collateral_mint)?;
    put_identity(output, STATE_TOKEN_PROGRAM_OFFSET, realm.token_program)?;
    put_identity(output, STATE_COLLATERAL_RELEASE_OFFSET, realm.collateral_release)
}

fn read_realm(input: &[u8]) -> Result<Realm, Error> {
    Ok(Realm {
        realm_id: read_identity(input, STATE_REALM_ID_OFFSET)?,
        collateral_mint: read_identity(input, STATE_COLLATERAL_MINT_OFFSET)?,
        token_program: read_identity(input, STATE_TOKEN_PROGRAM_OFFSET)?,
        collateral_release: read_identity(input, STATE_COLLATERAL_RELEASE_OFFSET)?,
    })
}

fn put_product(output: &mut [u8], product: Product) -> Result<(), Error> {
    put_identity(output, STATE_PRODUCT_ID_OFFSET, product.product_id)?;
    put_identity(output, STATE_RESULT_DOMAIN_OFFSET, product.result_domain)?;
    put_identity(output, STATE_CLAIM_BASIS_OFFSET, product.claim_basis)?;
    put_identity(output, STATE_CAPACITY_PROFILE_OFFSET, product.capacity_profile)?;
    put_identity(output, STATE_COMPILER_RELEASE_OFFSET, product.compiler_release)?;
    put_u32(output, STATE_OUTCOME_COUNT_OFFSET, product.outcome_count)?;
    put_u64(output, STATE_SCALAR_LIMIT_OFFSET, product.scalar_limit)
}

fn read_product(input: &[u8]) -> Result<Product, Error> {
    Ok(Product {
        product_id: read_identity(input, STATE_PRODUCT_ID_OFFSET)?,
        result_domain: read_identity(input, STATE_RESULT_DOMAIN_OFFSET)?,
        claim_basis: read_identity(input, STATE_CLAIM_BASIS_OFFSET)?,
        capacity_profile: read_identity(input, STATE_CAPACITY_PROFILE_OFFSET)?,
        compiler_release: read_identity(input, STATE_COMPILER_RELEASE_OFFSET)?,
        outcome_count: read_u32(input, STATE_OUTCOME_COUNT_OFFSET)?,
        scalar_limit: read_u64(input, STATE_SCALAR_LIMIT_OFFSET)?,
    })
}

fn put_release_set(output: &mut [u8], release_set: ReleaseSet) -> Result<(), Error> {
    put_identity(output, STATE_RELEASE_SET_ID_OFFSET, release_set.release_set_id)?;
    for (binding, (program, artifact, semantic)) in
        release_set.bindings.iter().zip(RELEASE_OFFSETS.iter().copied())
    {
        put_identity(output, program, binding.program)?;
        put_identity(output, artifact, binding.artifact_release)?;
        put_identity(output, semantic, binding.semantic_release)?;
    }
    Ok(())
}

fn read_release_set(input: &[u8]) -> Result<ReleaseSet, Error> {
    let mut bindings = [Binding {
        program: read_identity(input, STATE_CORE_PROGRAM_OFFSET)?,
        artifact_release: read_identity(input, STATE_CORE_ARTIFACT_OFFSET)?,
        semantic_release: read_identity(input, STATE_CORE_SEMANTIC_OFFSET)?,
    }; ROLE_COUNT];
    for (binding, (program, artifact, semantic)) in
        bindings.iter_mut().zip(RELEASE_OFFSETS.iter().copied())
    {
        *binding = Binding {
            program: read_identity(input, program)?,
            artifact_release: read_identity(input, artifact)?,
            semantic_release: read_identity(input, semantic)?,
        };
    }
    let release_set = ReleaseSet {
        release_set_id: read_identity(input, STATE_RELEASE_SET_ID_OFFSET)?,
        bindings,
    };
    if !release_set.valid() {
        return Err(Error::InvalidAlias);
    }
    Ok(release_set)
}

fn put_coordinates(output: &mut [u8], coordinates: CoreCoordinates) -> Result<(), Error> {
    put_byte(
        output,
        STATE_DERIVATION_AUTHENTICATED_OFFSET,
        u8::from(coordinates.derivation_authenticated),
    )?;
    for (offset, identity) in [
        (STATE_COORDINATE_MARKET_OFFSET, coordinates.market),
        (STATE_COORDINATE_HOARD_OFFSET, coordinates.hoard),
        (STATE_COORDINATE_FUND_OFFSET, coordinates.fund),
        (STATE_COORDINATE_READINESS_OFFSET, coordinates.readiness),
        (STATE_COORDINATE_CUSTODY_OFFSET, coordinates.custody),
        (STATE_COORDINATE_RENT_CREDIT_OFFSET, coordinates.rent_credit),
    ] {
        put_identity(output, offset, identity)?;
    }
    Ok(())
}

fn read_coordinates(input: &[u8]) -> Result<CoreCoordinates, Error> {
    let authenticated = read_byte(input, STATE_DERIVATION_AUTHENTICATED_OFFSET)?;
    if authenticated > 1 {
        return Err(Error::InvalidTag);
    }
    let coordinates = CoreCoordinates {
        derivation_authenticated: authenticated == 1,
        market: read_identity(input, STATE_COORDINATE_MARKET_OFFSET)?,
        hoard: read_identity(input, STATE_COORDINATE_HOARD_OFFSET)?,
        fund: read_identity(input, STATE_COORDINATE_FUND_OFFSET)?,
        readiness: read_identity(input, STATE_COORDINATE_READINESS_OFFSET)?,
        custody: read_identity(input, STATE_COORDINATE_CUSTODY_OFFSET)?,
        rent_credit: read_identity(input, STATE_COORDINATE_RENT_CREDIT_OFFSET)?,
    };
    if !coordinates.valid() {
        return Err(Error::InvalidCoordinates);
    }
    Ok(coordinates)
}

fn put_capital(output: &mut [u8], capital: Capital) -> Result<(), Error> {
    for (offset, value) in [
        (STATE_MARKET_RENT_OFFSET, capital.market_rent),
        (STATE_MARKET_DONATION_OFFSET, capital.market_donation),
        (STATE_HOARD_RENT_OFFSET, capital.hoard_rent),
        (STATE_HOARD_DONATION_OFFSET, capital.hoard_donation),
        (STATE_FUND_RENT_OFFSET, capital.fund_rent),
        (STATE_FUND_DONATION_OFFSET, capital.fund_donation),
        (STATE_READINESS_RENT_OFFSET, capital.readiness_rent),
        (STATE_READINESS_DONATION_OFFSET, capital.readiness_donation),
        (STATE_CUSTODY_RENT_OFFSET, capital.custody_rent),
        (STATE_CUSTODY_DONATION_OFFSET, capital.custody_donation),
        (STATE_DEFERRED_CUSTODY_RENT_OFFSET, capital.deferred_custody_rent),
        (STATE_RENT_CREDIT_OFFSET, capital.rent_credit),
    ] {
        put_u64(output, offset, value)?;
    }
    Ok(())
}

fn read_capital(input: &[u8]) -> Result<Capital, Error> {
    Ok(Capital {
        market_rent: read_u64(input, STATE_MARKET_RENT_OFFSET)?,
        market_donation: read_u64(input, STATE_MARKET_DONATION_OFFSET)?,
        hoard_rent: read_u64(input, STATE_HOARD_RENT_OFFSET)?,
        hoard_donation: read_u64(input, STATE_HOARD_DONATION_OFFSET)?,
        fund_rent: read_u64(input, STATE_FUND_RENT_OFFSET)?,
        fund_donation: read_u64(input, STATE_FUND_DONATION_OFFSET)?,
        readiness_rent: read_u64(input, STATE_READINESS_RENT_OFFSET)?,
        readiness_donation: read_u64(input, STATE_READINESS_DONATION_OFFSET)?,
        custody_rent: read_u64(input, STATE_CUSTODY_RENT_OFFSET)?,
        custody_donation: read_u64(input, STATE_CUSTODY_DONATION_OFFSET)?,
        deferred_custody_rent: read_u64(input, STATE_DEFERRED_CUSTODY_RENT_OFFSET)?,
        rent_credit: read_u64(input, STATE_RENT_CREDIT_OFFSET)?,
    })
}

fn put_identity(output: &mut [u8], offset: usize, identity: Identity) -> Result<(), Error> {
    put(output, offset, &identity.to_bytes())
}

fn read_identity(input: &[u8], offset: usize) -> Result<Identity, Error> {
    Identity::new(read_array(input, offset)?)
}

fn exact_magic(input: &[u8], offset: usize, magic: &[u8; 8]) -> Result<(), Error> {
    if input.get(offset..offset.saturating_add(magic.len())) != Some(magic.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<(), Error> {
    let bytes = input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?;
    if bytes.iter().any(|byte| *byte != 0) {
        return Err(Error::NonzeroReserved);
    }
    Ok(())
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    input
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    output
        .get_mut(offset..offset.checked_add(value.len()).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) -> Result<(), Error> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) -> Result<(), Error> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) -> Result<(), Error> {
    put(output, offset, &value.to_le_bytes())
}
"#

def main : IO Unit := do
  IO.println "// @generated by formal/dclutch-semantics/EmitMarketCoreRust.lean; do not edit."
  IO.println s!"pub const VERSION: u16 = {version};"
  IO.println s!"pub const STATE_BYTES: usize = {stateBytes};"
  IO.println s!"pub const REQUEST_BYTES: usize = {requestBytes};"
  IO.println s!"pub const ACTION_FOUND_TAG: u8 = {Action.found.tag};"
  IO.println s!"pub const ACTION_ACTIVATE_FUND_TAG: u8 = {Action.activateFund.tag};"
  IO.println s!"pub const ACTION_OPEN_MARKET_TAG: u8 = {Action.openMarket.tag};"
  IO.println s!"pub const ACTION_ADMIT_TERMINAL_TAG: u8 = {Action.admitTerminal.tag};"
  IO.println s!"pub const ACTION_SPLIT_TAG: u8 = {Action.split.tag};"
  IO.println s!"pub const ACTION_REDEEM_TAG: u8 = {Action.redeem.tag};"
  IO.println s!"pub const ACTION_BEGIN_RETIRING_TAG: u8 = {Action.beginRetiring.tag};"
  IO.println s!"pub const ACTION_RETIRE_TAG: u8 = {Action.retire.tag};"
  IO.println s!"pub const PHASE_FOUNDING_TAG: u8 = {phaseFoundingTag};"
  IO.println s!"pub const PHASE_OPEN_TAG: u8 = {phaseOpenTag};"
  IO.println s!"pub const PHASE_TERMINAL_TAG: u8 = {phaseTerminalTag};"
  IO.println s!"pub const PHASE_RETIRING_TAG: u8 = {phaseRetiringTag};"
  IO.println s!"pub const PHASE_RETIRED_TAG: u8 = {phaseRetiredTag};"
  IO.println s!"pub const READINESS_PREPAID_TAG: u8 = {readinessPrepaidTag};"
  IO.println s!"pub const READINESS_READY_TAG: u8 = {readinessReadyTag};"
  IO.println s!"pub const READINESS_CONSUMED_TAG: u8 = {readinessConsumedTag};"
  IO.println s!"pub const HOLDER_NONE_TAG: u8 = {holderNoneTag};"
  IO.println s!"pub const HOLDER_SOURCE_TAG: u8 = {holderSourceTag};"
  IO.println s!"pub const HOLDER_DESTINATION_TAG: u8 = {holderDestinationTag};"
  IO.println s!"pub const REPRESENTATION_NONE_TAG: u8 = {representationNoneTag};"
  IO.println s!"pub const REPRESENTATION_NATIVE_TAG: u8 = {representationNativeTag};"
  IO.println s!"pub const REPRESENTATION_MATERIALIZED_TAG: u8 = {representationMaterializedTag};"
  emitBytes "STATE_MAGIC" stateMagic
  emitBytes "REQUEST_MAGIC" requestMagic
  for field in stateLayout do
    IO.println s!"const {StateField.rustName field.spec.name}: usize = {field.offset};"
  for field in requestLayout do
    IO.println s!"const {RequestField.rustName field.spec.name}: usize = {field.offset};"
  IO.print interpreter
