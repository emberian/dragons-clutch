import DClutchSemantics.MarketCoreAbi
import DClutchSemantics.Codec

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
    InvalidChildEffect,
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

    pub(crate) fn valid(self) -> bool {
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
    pub market_registry_program: Identity,
    pub market_release_set_id: Identity,
    pub selected: ReleaseSet,
    pub receipt: ReleaseReceipt,
}

fn admission_valid(admission: Admission, role: Role) -> bool {
    admission.selected.valid()
        && admission.market_release_set_id == admission.selected.release_set_id
        && admission.receipt.registry_program == admission.market_registry_program
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

/// Canonical runtime-width Product projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Product {
    pub product_record: Identity,
    pub product_id: Identity,
    pub result_domain: Identity,
    pub portfolio: Identity,
    pub coordinate_domain: Identity,
    pub result_unit: Identity,
    pub claim_basis: Identity,
    pub liability_basis: Identity,
    pub representation_release: Identity,
    pub mapping_release: Identity,
    pub outcome_count: u32,
}

impl Product {
    pub(crate) fn valid(self) -> bool {
        self.outcome_count > 1
    }
}

/// Immutable Market identity coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketIdentity {
    pub market_id: Identity,
    pub realm_id: Identity,
    pub product_record: Identity,
    pub product_id: Identity,
    pub resolution_policy: Identity,
    pub capability_manifest: Identity,
    pub selected_release_set: Identity,
    pub registry_program: Identity,
    pub generation: u64,
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

/// Persisted fixed Market Core header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreState {
    pub phase: Phase,
    pub readiness: Readiness,
    pub terminal_winner: u32,
    pub identity: MarketIdentity,
    pub outstanding_capabilities: u64,
    pub rent_beneficiary: Identity,
    pub terminal_receipt: Option<Identity>,
}

impl CoreState {
    fn valid_static(self) -> bool {
        match self.phase {
            Phase::Founding => {
                self.readiness != Readiness::Consumed
                    && self.terminal_receipt.is_none()
                    && self.terminal_winner == 0
            }
            Phase::Open => {
                self.readiness == Readiness::Consumed
                    && self.terminal_receipt.is_none()
                    && self.terminal_winner == 0
            }
            Phase::Terminal | Phase::Retiring => {
                self.readiness == Readiness::Consumed
                    && self.terminal_receipt.is_some()
            }
            Phase::Retired => {
                self.readiness == Readiness::Consumed
                    && self.terminal_receipt.is_some()
                    && self.outstanding_capabilities == 0
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
        put_identity(&mut output, STATE_MARKET_ID_OFFSET, self.identity.market_id)?;
        put_identity(&mut output, STATE_IDENTITY_REALM_OFFSET, self.identity.realm_id)?;
        put_identity(&mut output, STATE_PRODUCT_RECORD_OFFSET, self.identity.product_record)?;
        put_identity(&mut output, STATE_PRODUCT_ID_OFFSET, self.identity.product_id)?;
        put_identity(&mut output, STATE_RESOLUTION_POLICY_OFFSET, self.identity.resolution_policy)?;
        put_identity(&mut output, STATE_CAPABILITY_MANIFEST_OFFSET, self.identity.capability_manifest)?;
        put_identity(&mut output, STATE_SELECTED_RELEASE_SET_OFFSET, self.identity.selected_release_set)?;
        put_identity(&mut output, STATE_REGISTRY_PROGRAM_OFFSET, self.identity.registry_program)?;
        put_u64(&mut output, STATE_GENERATION_OFFSET, self.identity.generation)?;
        put_u64(
            &mut output,
            STATE_OUTSTANDING_CAPABILITIES_OFFSET,
            self.outstanding_capabilities,
        )?;
        put_identity(&mut output, STATE_RENT_BENEFICIARY_OFFSET, self.rent_beneficiary)?;
        if let Some(receipt) = self.terminal_receipt {
            put_identity(&mut output, STATE_TERMINAL_RECEIPT_OFFSET, receipt)?;
        }
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
            identity: MarketIdentity {
                market_id: read_identity(input, STATE_MARKET_ID_OFFSET)?,
                realm_id: read_identity(input, STATE_IDENTITY_REALM_OFFSET)?,
                product_record: read_identity(input, STATE_PRODUCT_RECORD_OFFSET)?,
                product_id: read_identity(input, STATE_PRODUCT_ID_OFFSET)?,
                resolution_policy: read_identity(input, STATE_RESOLUTION_POLICY_OFFSET)?,
                capability_manifest: read_identity(input, STATE_CAPABILITY_MANIFEST_OFFSET)?,
                selected_release_set: read_identity(input, STATE_SELECTED_RELEASE_SET_OFFSET)?,
                registry_program: read_identity(input, STATE_REGISTRY_PROGRAM_OFFSET)?,
                generation: read_u64(input, STATE_GENERATION_OFFSET)?,
            },
            outstanding_capabilities: read_u64(input, STATE_OUTSTANDING_CAPABILITIES_OFFSET)?,
            rent_beneficiary: read_identity(input, STATE_RENT_BENEFICIARY_OFFSET)?,
            terminal_receipt,
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
    VerifyReadiness,
    OpenMarket,
    AdmitTerminal,
    Split,
    Redeem,
    BeginRetiring,
    Retire,
    ActivateCapability,
    CloseCapability,
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
}

/// Observed Found accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingAccounts {
    pub payer_lamports: u64,
    pub rent_credit: Identity,
    pub market: VacantAccount,
}

/// Complete chain-derived Found input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingFrame {
    pub realm: Realm,
    pub product: Product,
    pub identity: MarketIdentity,
    pub core_admission: Admission,
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

/// Exact Core-owned Market-account Found plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingPlan {
    pub market: AccountCreation,
    pub payer_debit: u64,
    pub payer_after: u64,
}

/// Successful sparse Core Found output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingResult {
    pub state: CoreState,
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
    pub authenticated: bool,
}

/// Adapter-derived proof that one exact child effect and poststate completed.
///
/// The SBF adapter derives these flags from same-call CPI return-data
/// provenance and authenticated post-accounts; they are never instruction
/// fields or caller-authored attestations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildEffectObservation {
    pub exact_request_authenticated: bool,
    pub exact_receipt_authenticated: bool,
    pub post_resource_authenticated: bool,
}

impl ChildEffectObservation {
    fn complete(self) -> bool {
        self.exact_request_authenticated
            && self.exact_receipt_authenticated
            && self.post_resource_authenticated
    }
}

/// Claims-owned effect evidence plus derived payout/aggregate facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsEffectObservation {
    pub child: ChildEffectObservation,
    pub payout: u64,
    pub aggregate_empty: bool,
}

/// Authenticated generic optional-capability child effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityChildObservation {
    pub target_role: Role,
    pub admission: Admission,
    pub manifest_entry_authenticated: bool,
    pub funding_state_authenticated: bool,
    pub effect: ChildEffectObservation,
}

/// Producer-authenticated evidence for atomic recurring-Series opening.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesOpenObservation {
    pub claims_admission: Admission,
    pub custody_admission: Admission,
    pub quantity: u64,
    pub basis_scale: u64,
    pub source_debit: u64,
    pub hoard_credit: u64,
    pub hoard_funding_authenticated: bool,
    pub found_state_bound_by_custody: bool,
    pub claims_custody_join_authenticated: bool,
    pub ticket_prepared_authenticated: bool,
    pub ticket_consumed_candidate_authenticated: bool,
    pub claims_effect: ChildEffectObservation,
    pub custody_effect: ChildEffectObservation,
}

/// Execute sparse Core Found without mutating external state.
pub fn found(request: Request, frame: FoundingFrame) -> Result<FoundingResult, Error> {
    require_request(request, Action::Found, frame.identity.market_id, frame.identity.generation)?;
    if !frame.product.valid()
        || frame.identity.realm_id != frame.realm.realm_id
        || frame.identity.product_record != frame.product.product_record
        || frame.identity.product_id != frame.product.product_id
        || frame.identity.selected_release_set != frame.core_admission.selected.release_set_id
        || frame.identity.registry_program != frame.core_admission.market_registry_program
        || !admission_valid(frame.core_admission, Role::Core)
        || frame.core_admission.market_release_set_id != frame.identity.selected_release_set
        || frame.quote.market_rent == 0
    {
        return Err(Error::InvalidFunding);
    }
    if !frame.accounts.market.valid() {
        return Err(Error::InvalidAccount);
    }
    if frame.accounts.market.address != frame.identity.market_id
        || frame.accounts.market.address == frame.accounts.rent_credit
    {
        return Err(Error::InvalidCoordinates);
    }
    let market = account_creation(frame.accounts.market, frame.quote.market_rent, 0)?;
    let payer_debit = market.rent_top_up;
    let payer_after = frame
        .accounts
        .payer_lamports
        .checked_sub(payer_debit)
        .ok_or(Error::InsufficientBalance)?;
    let state = CoreState {
        phase: Phase::Founding,
        readiness: Readiness::Prepaid,
        terminal_winner: 0,
        identity: frame.identity,
        outstanding_capabilities: 0,
        rent_beneficiary: frame.accounts.rent_credit,
        terminal_receipt: None,
    };
    if !state.valid_static() {
        return Err(Error::InvalidPhase);
    }
    Ok(FoundingResult {
        state,
        plan: FoundingPlan {
            market,
            payer_debit,
            payer_after,
        },
    })
}

/// Authenticate manifest-complete child readiness and mark Core ready.
pub fn verify_readiness(
    request: Request,
    state: &mut CoreState,
    admission: Admission,
    manifest_readiness_authenticated: bool,
    readiness_effect: ChildEffectObservation,
) -> Result<(), Error> {
    require_request(request, Action::VerifyReadiness, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Founding || state.readiness != Readiness::Prepaid {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, admission, Role::Core)?;
    if !manifest_readiness_authenticated || !readiness_effect.complete() {
        return Err(Error::InvalidChildEffect);
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
#[allow(clippy::too_many_arguments)]
pub fn open_market(
    request: Request,
    state: &mut CoreState,
    admission: Admission,
    realm: Realm,
    realm_record_authenticated: bool,
    custody_derivation_authenticated: bool,
    collateral: CollateralObservation,
    custody: VacantAccount,
    custody_rent_minimum: u64,
    custody_rent_authenticated: bool,
    custody_effect: ChildEffectObservation,
) -> Result<AccountCreation, Error> {
    require_request(request, Action::OpenMarket, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Founding || state.readiness != Readiness::Ready {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, admission, Role::Custody)?;
    if !custody_effect.complete() || !custody_rent_authenticated || custody_rent_minimum == 0 {
        return Err(Error::InvalidChildEffect);
    }
    if !realm_record_authenticated
        || realm.realm_id != state.identity.realm_id
        || !collateral.adapter_authenticated
        || collateral.realm_id != realm.realm_id
        || collateral.collateral_mint != realm.collateral_mint
        || collateral.token_program != realm.token_program
        || collateral.collateral_release != realm.collateral_release
    {
        return Err(Error::InvalidCoordinates);
    }
    if !custody_derivation_authenticated
        || !custody.valid()
        || custody.address == state.identity.market_id
        || custody.address == state.rent_beneficiary
    {
        return Err(Error::InvalidAccount);
    }
    let creation = account_creation(custody, custody_rent_minimum, 0)?;
    let mut candidate = *state;
    candidate.phase = Phase::Open;
    candidate.readiness = Readiness::Consumed;
    if !candidate.valid_static() {
        return Err(Error::InvalidPhase);
    }
    *state = candidate;
    Ok(creation)
}

/// Open one recurring-Series Market from an already-created, exactly funded
/// Hoard, realized Custody replay, Claims founding receipt, and authenticated
/// Trading replay candidate.
pub fn open_series_market(
    request: Request,
    state: &mut CoreState,
    observation: SeriesOpenObservation,
) -> Result<(), Error> {
    require_request(request, Action::OpenMarket, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Founding || state.readiness != Readiness::Prepaid {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, observation.claims_admission, Role::Claims)?;
    require_admission(*state, observation.custody_admission, Role::Custody)?;
    let collateral = observation
        .quantity
        .checked_mul(observation.basis_scale)
        .ok_or(Error::ArithmeticOverflow)?;
    if collateral == 0
        || observation.source_debit != collateral
        || observation.hoard_credit != collateral
        || !observation.hoard_funding_authenticated
        || !observation.found_state_bound_by_custody
        || !observation.claims_custody_join_authenticated
        || !observation.ticket_prepared_authenticated
        || !observation.ticket_consumed_candidate_authenticated
        || !observation.claims_effect.complete()
        || !observation.custody_effect.complete()
    {
        return Err(Error::InvalidChildEffect);
    }
    let mut candidate = *state;
    candidate.phase = Phase::Open;
    candidate.readiness = Readiness::Consumed;
    if !candidate.valid_static() {
        return Err(Error::InvalidPhase);
    }
    *state = candidate;
    Ok(())
}

/// Admit exact Claims mint and Custody principal effects for one complete set.
pub fn split_complete_set(
    request: Request,
    state: &CoreState,
    claims_admission: Admission,
    custody_admission: Admission,
    claims: ClaimsEffectObservation,
    custody: ChildEffectObservation,
) -> Result<(), Error> {
    require_request(request, Action::Split, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Open {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, claims_admission, Role::Claims)?;
    require_admission(*state, custody_admission, Role::Custody)?;
    if !claims.child.complete()
        || !custody.complete()
        || claims.payout != 0
        || claims.aggregate_empty
    {
        return Err(Error::InvalidChildEffect);
    }
    Ok(())
}

/// Admit one exact Source terminal receipt.
pub fn admit_terminal(
    request: Request,
    state: &mut CoreState,
    admission: Admission,
    product: Product,
    product_record_authenticated: bool,
    receipt: TerminalReceipt,
) -> Result<(), Error> {
    require_request(request, Action::AdmitTerminal, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Open {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, admission, Role::Resolution)?;
    if !product_record_authenticated
        || !product.valid()
        || product.product_record != state.identity.product_record
        || product.product_id != state.identity.product_id
        || !receipt.authenticated
        || receipt.market_id != state.identity.market_id
        || receipt.resolution_policy != state.identity.resolution_policy
        || receipt.product_id != product.product_id
        || receipt.generation != state.identity.generation
        || receipt.selector >= product.outcome_count
    {
        return Err(Error::InvalidTerminalReceipt);
    }
    let mut candidate = *state;
    candidate.phase = Phase::Terminal;
    candidate.terminal_winner = receipt.selector;
    candidate.terminal_receipt = Some(receipt.receipt_id);
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

fn require_capability_child(
    state: CoreState,
    observation: CapabilityChildObservation,
) -> Result<(), Error> {
    if observation.target_role == Role::Core
        || !observation.manifest_entry_authenticated
        || !observation.funding_state_authenticated
        || !observation.effect.complete()
    {
        return Err(Error::InvalidChildEffect);
    }
    require_admission(state, observation.admission, observation.target_role)
}

/// Admit one exact manifest/Funding-backed optional child activation.
pub fn activate_capability_child(
    request: Request,
    state: &mut CoreState,
    observation: CapabilityChildObservation,
) -> Result<(), Error> {
    require_request(
        request,
        Action::ActivateCapability,
        state.identity.market_id,
        state.identity.generation,
    )?;
    if state.phase != Phase::Open {
        return Err(Error::InvalidPhase);
    }
    require_capability_child(*state, observation)?;
    let mut candidate = *state;
    candidate.outstanding_capabilities = candidate
        .outstanding_capabilities
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    *state = candidate;
    Ok(())
}

/// Admit one exact optional-child close receipt and release its replay count.
pub fn close_capability_child(
    request: Request,
    state: &mut CoreState,
    observation: CapabilityChildObservation,
) -> Result<(), Error> {
    require_request(
        request,
        Action::CloseCapability,
        state.identity.market_id,
        state.identity.generation,
    )?;
    if state.phase != Phase::Open
        && state.phase != Phase::Terminal
        && state.phase != Phase::Retiring
    {
        return Err(Error::InvalidPhase);
    }
    require_capability_child(*state, observation)?;
    let mut candidate = *state;
    candidate.outstanding_capabilities = candidate
        .outstanding_capabilities
        .checked_sub(1)
        .ok_or(Error::InvalidChildEffect)?;
    *state = candidate;
    Ok(())
}

/// Burn a terminal claim and release exactly the winning payout.
#[allow(clippy::too_many_arguments)]
pub fn redeem_terminal(
    request: Request,
    state: &CoreState,
    claims_admission: Admission,
    custody_admission: Admission,
    product: Product,
    product_record_authenticated: bool,
    claims: ClaimsEffectObservation,
    custody: Option<ChildEffectObservation>,
) -> Result<u64, Error> {
    require_request(request, Action::Redeem, state.identity.market_id, state.identity.generation)?;
    if state.phase != Phase::Terminal && state.phase != Phase::Retiring {
        return Err(Error::InvalidPhase);
    }
    require_admission(*state, claims_admission, Role::Claims)?;
    require_admission(*state, custody_admission, Role::Custody)?;
    if !product_record_authenticated
        || !product.valid()
        || product.product_record != state.identity.product_record
        || product.product_id != state.identity.product_id
        || request.outcome >= product.outcome_count
        || !claims.child.complete()
    {
        return Err(Error::InvalidChildEffect);
    }
    let payout = if request.outcome == state.terminal_winner {
        request.quantity
    } else {
        0
    };
    let custody_matches = match (payout, custody) {
        (0, None) => true,
        (0, Some(_)) | (_, None) => false,
        (_, Some(observation)) => observation.complete(),
    };
    if claims.payout != payout || !custody_matches {
        return Err(Error::InvalidChildEffect);
    }
    Ok(payout)
}

/// Close terminal Core accounts and return every remaining classified and
/// unclassified lamport to the immutable RentCredit.
#[allow(clippy::too_many_arguments)]
pub fn retire(
    request: Request,
    state: &mut CoreState,
    core_admission: Admission,
    claims_admission: Admission,
    resolution_admission: Admission,
    custody_admission: Admission,
    claims: ClaimsEffectObservation,
    source: ChildEffectObservation,
    custody: ChildEffectObservation,
    core_account_lamports: u64,
    core_account_authenticated: bool,
    rent_credit_authenticated: bool,
) -> Result<u64, Error> {
    require_request(request, Action::Retire, state.identity.market_id, state.identity.generation)?;
    require_admission(*state, core_admission, Role::Core)?;
    require_admission(*state, claims_admission, Role::Claims)?;
    require_admission(*state, resolution_admission, Role::Resolution)?;
    require_admission(*state, custody_admission, Role::Custody)?;
    if state.phase != Phase::Retiring {
        return Err(Error::InvalidPhase);
    }
    if state.outstanding_capabilities != 0 {
        return Err(Error::InvalidChildEffect);
    }
    if !claims.child.complete()
        || !claims.aggregate_empty
        || claims.payout != 0
        || !source.complete()
        || !custody.complete()
    {
        return Err(Error::InvalidChildEffect);
    }
    if !core_account_authenticated || !rent_credit_authenticated {
        return Err(Error::InvalidAccount);
    }
    let mut candidate = *state;
    candidate.phase = Phase::Retired;
    if !candidate.valid_static() {
        return Err(Error::InvalidPhase);
    }
    *state = candidate;
    Ok(core_account_lamports)
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
        || admission.market_registry_program != state.identity.registry_program
        || admission.selected.release_set_id != state.identity.selected_release_set
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
        Action::VerifyReadiness => ACTION_VERIFY_READINESS_TAG,
        Action::OpenMarket => ACTION_OPEN_MARKET_TAG,
        Action::AdmitTerminal => ACTION_ADMIT_TERMINAL_TAG,
        Action::Split => ACTION_SPLIT_TAG,
        Action::Redeem => ACTION_REDEEM_TAG,
        Action::BeginRetiring => ACTION_BEGIN_RETIRING_TAG,
        Action::Retire => ACTION_RETIRE_TAG,
        Action::ActivateCapability => ACTION_ACTIVATE_CAPABILITY_TAG,
        Action::CloseCapability => ACTION_CLOSE_CAPABILITY_TAG,
    }
}

fn decode_action(tag: u8) -> Result<Action, Error> {
    match tag {
        ACTION_FOUND_TAG => Ok(Action::Found),
        ACTION_VERIFY_READINESS_TAG => Ok(Action::VerifyReadiness),
        ACTION_OPEN_MARKET_TAG => Ok(Action::OpenMarket),
        ACTION_ADMIT_TERMINAL_TAG => Ok(Action::AdmitTerminal),
        ACTION_SPLIT_TAG => Ok(Action::Split),
        ACTION_REDEEM_TAG => Ok(Action::Redeem),
        ACTION_BEGIN_RETIRING_TAG => Ok(Action::BeginRetiring),
        ACTION_RETIRE_TAG => Ok(Action::Retire),
        ACTION_ACTIVATE_CAPABILITY_TAG => Ok(Action::ActivateCapability),
        ACTION_CLOSE_CAPABILITY_TAG => Ok(Action::CloseCapability),
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
  IO.println s!"pub const ACTION_VERIFY_READINESS_TAG: u8 = {Action.verifyReadiness.tag};"
  IO.println s!"pub const ACTION_OPEN_MARKET_TAG: u8 = {Action.openMarket.tag};"
  IO.println s!"pub const ACTION_ADMIT_TERMINAL_TAG: u8 = {Action.admitTerminal.tag};"
  IO.println s!"pub const ACTION_SPLIT_TAG: u8 = {Action.split.tag};"
  IO.println s!"pub const ACTION_REDEEM_TAG: u8 = {Action.redeem.tag};"
  IO.println s!"pub const ACTION_BEGIN_RETIRING_TAG: u8 = {Action.beginRetiring.tag};"
  IO.println s!"pub const ACTION_RETIRE_TAG: u8 = {Action.retire.tag};"
  IO.println s!"pub const ACTION_ACTIVATE_CAPABILITY_TAG: u8 = {Action.activateCapability.tag};"
  IO.println s!"pub const ACTION_CLOSE_CAPABILITY_TAG: u8 = {Action.closeCapability.tag};"
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
