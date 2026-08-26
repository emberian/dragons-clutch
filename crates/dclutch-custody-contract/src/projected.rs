//! Canonical pre-founding custody for a Core-authenticated future Market.

use dclutch_market_core_codec::{CoreState, Phase, ProjectFoundReceiptV1};
use dclutch_release_set_contract::ExecutionRoleV1;

use crate::{CompartmentV1, CustodyReplayV1};

/// Projected-custody request magic.
pub const PROJECTED_CUSTODY_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLPCQ01";
/// Projected-custody state magic.
pub const PROJECTED_CUSTODY_STATE_MAGIC_V1: [u8; 8] = *b"DCLPCS01";
/// Projected-custody terminal receipt magic.
pub const PROJECTED_CUSTODY_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLPCR01";
/// Exact request width.
pub const PROJECTED_CUSTODY_REQUEST_BYTES_V1: usize = 768;
/// Exact persisted-state width.
pub const PROJECTED_CUSTODY_STATE_BYTES_V1: usize = 808;
/// Exact terminal receipt width.
pub const PROJECTED_CUSTODY_RECEIPT_BYTES_V1: usize = 320;
/// Exact receipt width for atomic Hoard credit plus source-vault/replay closure.
pub const PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1: usize = 320;
/// Domain separating the projected-Hoard replay context from its funding source.
pub const PROJECTED_HOARD_CONTEXT_DOMAIN_V1: &[u8] = b"dclutch:projected-hoard-context:v1";
/// PDA seed domain for the one typed Trading-capability caller.
pub const PROJECTED_CUSTODY_CALLER_PDA_DOMAIN_V1: &[u8] = b"dclutch:projected-custody-caller:v1";
/// Lock-and-close receipt magic.
pub const PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLPCL01";
const VERSION_V1: u16 = 1;

/// Stable refusal from projected pre-founding custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectedCustodyError {
    /// The exact wire width differed.
    InvalidLength,
    /// Magic or ABI version differed.
    InvalidHeader,
    /// Reserved or inactive fields were not canonical.
    NonCanonical,
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// Only the typed Trading-capability caller may act.
    CallerRole,
    /// The selected operation or phase was invalid.
    Phase,
    /// Revision did not advance exactly once.
    Revision,
    /// Expiry or current-slot relationship refused.
    Expiry,
    /// Exact token balance arithmetic refused.
    Balance,
    /// The ProjectFound receipt differed from persisted projection facts.
    Projection,
    /// The realized Core Market differed from the projected future Market.
    Market,
    /// Exact RentCredit closure coordinates differed.
    RentCredit,
    /// A terminal receipt did not bind the exact transition.
    Receipt,
}

/// Distinct typed caller; this is not generic `ExecutionRoleV1::Trading` bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProjectedCallerRoleV1 {
    /// A release-selected Trading capability rooted in one parent action.
    TradingCapability = 1,
}

/// Projected custody operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProjectedCustodyOperationV1 {
    /// Persist the immediate Core ProjectFound projection.
    Initialize = 0,
    /// Create the empty projected Hoard vault.
    OpenHoard = 1,
    /// Credit exact principal into the projected Hoard before Found.
    LockHoard = 2,
    /// After expiry, return principal and close vault/state to RentCredit.
    RefundAndClose = 3,
    /// After exact Found, rebind the credited Hoard without moving principal.
    RealizeAndClose = 4,
    /// After expiry, close an empty prepared Hoard and projection state.
    AbortOpenAndClose = 5,
    /// Credit the Hoard and atomically close the emptied source Vault/replay.
    LockHoardAndCloseSource = 6,
}

impl ProjectedCustodyOperationV1 {
    fn decode(value: u8) -> Result<Self, ProjectedCustodyError> {
        match value {
            0 => Ok(Self::Initialize),
            1 => Ok(Self::OpenHoard),
            2 => Ok(Self::LockHoard),
            3 => Ok(Self::RefundAndClose),
            4 => Ok(Self::RealizeAndClose),
            5 => Ok(Self::AbortOpenAndClose),
            6 => Ok(Self::LockHoardAndCloseSource),
            _ => Err(ProjectedCustodyError::NonCanonical),
        }
    }
}

/// Live projected-custody phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProjectedCustodyPhaseV1 {
    /// Projection state exists; vault is not created.
    Initialized = 1,
    /// Empty projected Hoard vault exists.
    HoardOpen = 2,
    /// Exact principal has been credited into the projected Hoard.
    HoardLocked = 3,
}

/// Canonical projected-state PDA seeds under Custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedCustodyStateSeedsV1 {
    market: [u8; 32],
    release_set: [u8; 32],
    context_digest: [u8; 32],
}

impl ProjectedCustodyStateSeedsV1 {
    /// Project exact state seeds from one request.
    pub const fn from_request(request: ProjectedCustodyRequestV1) -> Self {
        Self {
            market: request.market,
            release_set: request.release_set,
            context_digest: request.context_digest,
        }
    }

    /// Borrow exact ordered seed slices, excluding bump.
    pub fn as_slices(&self) -> [&[u8]; 4] {
        [
            crate::CUSTODY_REPLAY_PDA_DOMAIN_V1,
            &self.market,
            &self.release_set,
            &self.context_digest,
        ]
    }
}

/// Typed Trading-capability caller PDA seeds under the selected Trading program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedCustodyCallerSeedsV1 {
    release_set: [u8; 32],
    market: [u8; 32],
    parent_capability_root: [u8; 32],
    context_digest: [u8; 32],
    request_digest: [u8; 32],
}

impl ProjectedCustodyCallerSeedsV1 {
    /// Project exact typed caller seeds from one request and its complete digest.
    pub const fn new(request: ProjectedCustodyRequestV1, request_digest: [u8; 32]) -> Self {
        Self {
            release_set: request.release_set,
            market: request.market,
            parent_capability_root: request.parent_capability_root,
            context_digest: request.context_digest,
            request_digest,
        }
    }

    /// Borrow exact ordered seed slices, excluding bump.
    pub fn as_slices(&self) -> [&[u8]; 6] {
        [
            PROJECTED_CUSTODY_CALLER_PDA_DOMAIN_V1,
            &self.release_set,
            &self.market,
            &self.parent_capability_root,
            &self.context_digest,
            &self.request_digest,
        ]
    }
}

/// Canonical projected-custody action request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedCustodyRequestV1 {
    /// Selected operation.
    pub operation: ProjectedCustodyOperationV1,
    /// Distinct typed caller role.
    pub caller_role: ProjectedCallerRoleV1,
    /// Future Core Market PDA.
    pub market: [u8; 32],
    /// Market generation.
    pub generation: u64,
    /// Immutable Realm identity.
    pub realm: [u8; 32],
    /// Product finalized-record identity.
    pub product_record: [u8; 32],
    /// Semantic Product identity.
    pub product: [u8; 32],
    /// Source resolution-policy identity.
    pub source: [u8; 32],
    /// Selected release-set identity.
    pub release_set: [u8; 32],
    /// SHA-256 of the immediate ProjectFound receipt.
    pub projection_receipt_digest: [u8; 32],
    /// Immutable parent Trading capability root.
    pub parent_capability_root: [u8; 32],
    /// Exact parent action/context digest.
    pub context_digest: [u8; 32],
    /// Exact selected Trading program.
    pub caller_program: [u8; 32],
    /// Exact prepaid or permissionless creation payer.
    pub payer: [u8; 32],
    /// Exact immediate ProjectFound producer and selected Core program.
    pub core_program: [u8; 32],
    /// Immutable infrastructure-selected Rent program.
    pub rent_program: [u8; 32],
    /// Immutable token principal refund owner.
    pub refund_owner: [u8; 32],
    /// Permanent exact RentCredit receiving every close lamport.
    pub rent_credit: [u8; 32],
    /// Exact projected Hoard token vault.
    pub hoard_vault: [u8; 32],
    /// Exact already-custodied principal source vault.
    pub funding_source_vault: [u8; 32],
    /// Exact replay namespace of the already-custodied source vault.
    pub funding_source_context: [u8; 32],
    /// Economic compartment of the already-custodied source vault.
    pub funding_source_compartment: CompartmentV1,
    /// Realm-selected collateral Mint.
    pub mint: [u8; 32],
    /// Realm-selected token program.
    pub token_program: [u8; 32],
    /// Realm-selected immutable collateral-adapter release.
    pub collateral_release: [u8; 32],
    /// Last slot at which realization may occur before refund is permitted.
    pub expiry_slot: u64,
    /// Current optimistic revision.
    pub expected_revision: u64,
    /// Exact next optimistic revision.
    pub resulting_revision: u64,
    /// Exact principal for Lock/refund/realize; zero otherwise.
    pub amount: u64,
    /// Exact chain-derived projected replay rent.
    pub state_rent_lamports: u64,
    /// Exact chain-derived canonical Hoard-vault rent.
    pub vault_rent_lamports: u64,
    /// Exact normal source-replay revision consumed by atomic source closure.
    pub funding_source_replay_revision: u64,
    /// Exact normal source-replay rent returned to RentCredit.
    pub funding_source_state_rent_lamports: u64,
    /// Exact source-Vault rent returned to RentCredit.
    pub funding_source_vault_rent_lamports: u64,
}

impl ProjectedCustodyRequestV1 {
    /// Validate identities and the exact operation shape.
    pub fn validate(self) -> Result<(), ProjectedCustodyError> {
        for value in [
            self.market,
            self.realm,
            self.product_record,
            self.product,
            self.source,
            self.release_set,
            self.projection_receipt_digest,
            self.parent_capability_root,
            self.context_digest,
            self.caller_program,
            self.payer,
            self.core_program,
            self.rent_program,
            self.refund_owner,
            self.rent_credit,
            self.hoard_vault,
            self.funding_source_vault,
            self.funding_source_context,
            self.mint,
            self.token_program,
            self.collateral_release,
        ] {
            nonzero(value)?;
        }
        if self.expected_revision.checked_add(1) != Some(self.resulting_revision) {
            return Err(ProjectedCustodyError::Revision);
        }
        if self.state_rent_lamports == 0
            || self.vault_rent_lamports == 0
            || self.funding_source_replay_revision == 0
            || self.funding_source_state_rent_lamports == 0
            || self.funding_source_vault_rent_lamports == 0
            || self.context_digest == self.funding_source_context
            || matches!(
                self.funding_source_compartment,
                CompartmentV1::None | CompartmentV1::External | CompartmentV1::HoardPrincipal
            )
        {
            return Err(ProjectedCustodyError::NonCanonical);
        }
        let amount_active = matches!(
            self.operation,
            ProjectedCustodyOperationV1::LockHoard
                | ProjectedCustodyOperationV1::RefundAndClose
                | ProjectedCustodyOperationV1::RealizeAndClose
                | ProjectedCustodyOperationV1::LockHoardAndCloseSource
        );
        if amount_active != (self.amount > 0)
            || (self.operation == ProjectedCustodyOperationV1::Initialize
                && (self.expected_revision != 0 || self.resulting_revision != 1))
        {
            return Err(ProjectedCustodyError::NonCanonical);
        }
        Ok(())
    }

    /// Encode one exact request.
    pub fn encode(self) -> Result<[u8; PROJECTED_CUSTODY_REQUEST_BYTES_V1], ProjectedCustodyError> {
        self.validate()?;
        let mut output = [0; PROJECTED_CUSTODY_REQUEST_BYTES_V1];
        output[..8].copy_from_slice(&PROJECTED_CUSTODY_REQUEST_MAGIC_V1);
        put_u16(&mut output, 8, VERSION_V1)?;
        put_u8(&mut output, 10, self.operation as u8)?;
        put_u8(&mut output, 11, self.caller_role as u8)?;
        put_u8(&mut output, 12, self.funding_source_compartment.tag())?;
        write_identities(&mut output, 16, &self.identities())?;
        put_u64(&mut output, 688, self.generation)?;
        put_u64(&mut output, 696, self.expiry_slot)?;
        put_u64(&mut output, 704, self.expected_revision)?;
        put_u64(&mut output, 712, self.resulting_revision)?;
        put_u64(&mut output, 720, self.amount)?;
        put_u64(&mut output, 728, self.state_rent_lamports)?;
        put_u64(&mut output, 736, self.vault_rent_lamports)?;
        put_u64(&mut output, 744, self.funding_source_replay_revision)?;
        put_u64(&mut output, 752, self.funding_source_state_rent_lamports)?;
        put_u64(&mut output, 760, self.funding_source_vault_rent_lamports)?;
        Ok(output)
    }

    /// Hostile-decode one exact request.
    pub fn decode(input: &[u8]) -> Result<Self, ProjectedCustodyError> {
        header(
            input,
            &PROJECTED_CUSTODY_REQUEST_MAGIC_V1,
            PROJECTED_CUSTODY_REQUEST_BYTES_V1,
        )?;
        if any_nonzero(input, 13, 3)? {
            return Err(ProjectedCustodyError::NonCanonical);
        }
        let ids = read_identities::<21>(input, 16)?;
        let caller_role = match read_u8(input, 11)? {
            1 => ProjectedCallerRoleV1::TradingCapability,
            _ => return Err(ProjectedCustodyError::CallerRole),
        };
        let value = Self {
            operation: ProjectedCustodyOperationV1::decode(read_u8(input, 10)?)?,
            caller_role,
            market: ids[0],
            realm: ids[1],
            product_record: ids[2],
            product: ids[3],
            source: ids[4],
            release_set: ids[5],
            projection_receipt_digest: ids[6],
            parent_capability_root: ids[7],
            context_digest: ids[8],
            caller_program: ids[9],
            payer: ids[10],
            core_program: ids[11],
            rent_program: ids[12],
            refund_owner: ids[13],
            rent_credit: ids[14],
            hoard_vault: ids[15],
            funding_source_vault: ids[16],
            funding_source_context: ids[17],
            funding_source_compartment: CompartmentV1::decode(read_u8(input, 12)?)
                .map_err(|_| ProjectedCustodyError::NonCanonical)?,
            mint: ids[18],
            token_program: ids[19],
            collateral_release: ids[20],
            generation: read_u64(input, 688)?,
            expiry_slot: read_u64(input, 696)?,
            expected_revision: read_u64(input, 704)?,
            resulting_revision: read_u64(input, 712)?,
            amount: read_u64(input, 720)?,
            state_rent_lamports: read_u64(input, 728)?,
            vault_rent_lamports: read_u64(input, 736)?,
            funding_source_replay_revision: read_u64(input, 744)?,
            funding_source_state_rent_lamports: read_u64(input, 752)?,
            funding_source_vault_rent_lamports: read_u64(input, 760)?,
        };
        value.validate()?;
        Ok(value)
    }

    fn identities(self) -> [[u8; 32]; 21] {
        [
            self.market,
            self.realm,
            self.product_record,
            self.product,
            self.source,
            self.release_set,
            self.projection_receipt_digest,
            self.parent_capability_root,
            self.context_digest,
            self.caller_program,
            self.payer,
            self.core_program,
            self.rent_program,
            self.refund_owner,
            self.rent_credit,
            self.hoard_vault,
            self.funding_source_vault,
            self.funding_source_context,
            self.mint,
            self.token_program,
            self.collateral_release,
        ]
    }
}

/// Persisted authority for one exact future Market Hoard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedCustodyStateV1 {
    /// Live phase.
    pub phase: ProjectedCustodyPhaseV1,
    /// Exact projection/action coordinates.
    pub request: ProjectedCustodyRequestV1,
    /// Next required request revision.
    pub next_revision: u64,
    /// Locked principal; zero until Lock.
    pub locked_amount: u64,
    /// Digest of the last exact accepted request.
    pub last_request_digest: [u8; 32],
    /// PDA bump for exact self-authentication.
    pub bump: u8,
}

impl ProjectedCustodyStateV1 {
    /// Encode the sole persisted authority bytes.
    pub fn encode(self) -> Result<[u8; PROJECTED_CUSTODY_STATE_BYTES_V1], ProjectedCustodyError> {
        self.request.validate()?;
        nonzero(self.last_request_digest)?;
        let mut output = [0; PROJECTED_CUSTODY_STATE_BYTES_V1];
        output[..8].copy_from_slice(&PROJECTED_CUSTODY_STATE_MAGIC_V1);
        put_u16(&mut output, 8, VERSION_V1)?;
        put_u8(&mut output, 10, self.phase as u8)?;
        put_u8(&mut output, 11, self.bump)?;
        put_u8(
            &mut output,
            12,
            self.request.funding_source_compartment.tag(),
        )?;
        write_identities(&mut output, 32, &self.request.identities())?;
        put_u64(&mut output, 704, self.request.generation)?;
        put_u64(&mut output, 712, self.request.expiry_slot)?;
        put_u64(&mut output, 720, self.next_revision)?;
        put_u64(&mut output, 728, self.locked_amount)?;
        put_u64(&mut output, 736, self.request.state_rent_lamports)?;
        put_u64(&mut output, 744, self.request.vault_rent_lamports)?;
        put_u64(
            &mut output,
            752,
            self.request.funding_source_replay_revision,
        )?;
        put_u64(
            &mut output,
            760,
            self.request.funding_source_state_rent_lamports,
        )?;
        put_u64(
            &mut output,
            768,
            self.request.funding_source_vault_rent_lamports,
        )?;
        put(&mut output, 776, &self.last_request_digest)?;
        Ok(output)
    }

    /// Hostile-decode the sole persisted authority bytes.
    pub fn decode(input: &[u8]) -> Result<Self, ProjectedCustodyError> {
        header(
            input,
            &PROJECTED_CUSTODY_STATE_MAGIC_V1,
            PROJECTED_CUSTODY_STATE_BYTES_V1,
        )?;
        if any_nonzero(input, 13, 19)? {
            return Err(ProjectedCustodyError::NonCanonical);
        }
        let ids = read_identities::<21>(input, 32)?;
        let phase = match read_u8(input, 10)? {
            1 => ProjectedCustodyPhaseV1::Initialized,
            2 => ProjectedCustodyPhaseV1::HoardOpen,
            3 => ProjectedCustodyPhaseV1::HoardLocked,
            _ => return Err(ProjectedCustodyError::Phase),
        };
        let next_revision = read_u64(input, 720)?;
        let locked_amount = read_u64(input, 728)?;
        if next_revision == 0
            || (phase == ProjectedCustodyPhaseV1::HoardLocked) != (locked_amount > 0)
        {
            return Err(ProjectedCustodyError::Phase);
        }
        let operation = match phase {
            ProjectedCustodyPhaseV1::Initialized => ProjectedCustodyOperationV1::Initialize,
            ProjectedCustodyPhaseV1::HoardOpen => ProjectedCustodyOperationV1::OpenHoard,
            ProjectedCustodyPhaseV1::HoardLocked => ProjectedCustodyOperationV1::LockHoard,
        };
        let immutable = ProjectedCustodyRequestV1 {
            operation,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: ids[0],
            realm: ids[1],
            product_record: ids[2],
            product: ids[3],
            source: ids[4],
            release_set: ids[5],
            projection_receipt_digest: ids[6],
            parent_capability_root: ids[7],
            context_digest: ids[8],
            caller_program: ids[9],
            payer: ids[10],
            core_program: ids[11],
            rent_program: ids[12],
            refund_owner: ids[13],
            rent_credit: ids[14],
            hoard_vault: ids[15],
            funding_source_vault: ids[16],
            funding_source_context: ids[17],
            funding_source_compartment: CompartmentV1::decode(read_u8(input, 12)?)
                .map_err(|_| ProjectedCustodyError::NonCanonical)?,
            mint: ids[18],
            token_program: ids[19],
            collateral_release: ids[20],
            generation: read_u64(input, 704)?,
            expiry_slot: read_u64(input, 712)?,
            expected_revision: next_revision.saturating_sub(1),
            resulting_revision: next_revision,
            amount: if phase == ProjectedCustodyPhaseV1::HoardLocked {
                locked_amount
            } else {
                0
            },
            state_rent_lamports: read_u64(input, 736)?,
            vault_rent_lamports: read_u64(input, 744)?,
            funding_source_replay_revision: read_u64(input, 752)?,
            funding_source_state_rent_lamports: read_u64(input, 760)?,
            funding_source_vault_rent_lamports: read_u64(input, 768)?,
        };
        immutable.validate()?;
        Ok(Self {
            phase,
            request: immutable,
            next_revision,
            locked_amount,
            last_request_digest: slice(input, 776, 32)?
                .try_into()
                .map_err(|_| ProjectedCustodyError::InvalidLength)?,
            bump: read_u8(input, 11)?,
        })
    }

    /// Initialize from an immediate Core receipt; the Market must still be vacant.
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        request: ProjectedCustodyRequestV1,
        projection: ProjectFoundReceiptV1,
        projection_producer: [u8; 32],
        projection_receipt_digest: [u8; 32],
        request_digest: [u8; 32],
        current_slot: u64,
        market_vacant: bool,
        bump: u8,
    ) -> Result<Self, ProjectedCustodyError> {
        request.validate()?;
        if request.operation != ProjectedCustodyOperationV1::Initialize
            || !market_vacant
            || current_slot > request.expiry_slot
            || projection_receipt_digest != request.projection_receipt_digest
            || projection_producer != request.core_program
            || request.market != projection.market.to_bytes()
            || request.generation != projection.generation
            || request.realm != projection.realm.to_bytes()
            || request.product_record != projection.product_record.to_bytes()
            || request.product != projection.product.to_bytes()
            || request.source != projection.source.to_bytes()
            || request.release_set != projection.release_set.to_bytes()
            || request.mint != projection.collateral_mint.to_bytes()
            || request.token_program != projection.token_program.to_bytes()
            || request.collateral_release != projection.collateral_release.to_bytes()
            || request.rent_program != projection.rent_program.to_bytes()
        {
            return Err(ProjectedCustodyError::Projection);
        }
        nonzero(request_digest)?;
        Ok(Self {
            phase: ProjectedCustodyPhaseV1::Initialized,
            request,
            next_revision: 1,
            locked_amount: 0,
            last_request_digest: request_digest,
            bump,
        })
    }

    /// Open the exact empty projected Hoard vault.
    pub fn open_hoard(
        mut self,
        request: ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
        vault_balance: u64,
        market_vacant: bool,
    ) -> Result<Self, ProjectedCustodyError> {
        self.authenticate_next(&request, request_digest)?;
        if self.phase != ProjectedCustodyPhaseV1::Initialized
            || request.operation != ProjectedCustodyOperationV1::OpenHoard
            || vault_balance != 0
            || !market_vacant
        {
            return Err(ProjectedCustodyError::Phase);
        }
        self.phase = ProjectedCustodyPhaseV1::HoardOpen;
        self.advance(request, request_digest);
        Ok(self)
    }

    /// Credit exact principal into the projected Hoard before Found.
    pub fn lock_hoard(
        mut self,
        request: ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
        vault_before: u64,
        vault_after: u64,
        market_vacant: bool,
    ) -> Result<Self, ProjectedCustodyError> {
        self.authenticate_next(&request, request_digest)?;
        if self.phase != ProjectedCustodyPhaseV1::HoardOpen
            || request.operation != ProjectedCustodyOperationV1::LockHoard
            || vault_before != 0
            || vault_before.checked_add(request.amount) != Some(vault_after)
            || !market_vacant
        {
            return Err(ProjectedCustodyError::Balance);
        }
        self.phase = ProjectedCustodyPhaseV1::HoardLocked;
        self.locked_amount = request.amount;
        self.advance(request, request_digest);
        Ok(self)
    }

    /// Credit the projected Hoard from one fully consumed Custody source and
    /// authorize atomic closure of that source Vault and replay to RentCredit.
    #[allow(clippy::too_many_arguments)]
    pub fn lock_hoard_and_close_source(
        mut self,
        request: ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
        source_replay_key: [u8; 32],
        source_replay: CustodyReplayV1,
        source_before: u64,
        source_after: u64,
        hoard_before: u64,
        hoard_after: u64,
        source_vault_lamports: u64,
        source_replay_lamports: u64,
        rent_credit: [u8; 32],
        market_vacant: bool,
    ) -> Result<(Self, ProjectedCustodyLockReceiptV1), ProjectedCustodyError> {
        self.authenticate_next(&request, request_digest)?;
        nonzero(source_replay_key)?;
        if self.phase != ProjectedCustodyPhaseV1::HoardOpen
            || request.operation != ProjectedCustodyOperationV1::LockHoardAndCloseSource
            || source_before != request.amount
            || source_after != 0
            || hoard_before != 0
            || hoard_before.checked_add(request.amount) != Some(hoard_after)
            || source_vault_lamports != request.funding_source_vault_rent_lamports
            || source_replay_lamports != request.funding_source_state_rent_lamports
            || rent_credit != request.rent_credit
            || !market_vacant
            || source_replay.caller_role != ExecutionRoleV1::Trading
            || source_replay.release_set != request.release_set
            || source_replay.market != request.market
            || source_replay.realm != request.realm
            || source_replay.context != request.funding_source_context
            || source_replay.caller_program != request.caller_program
            || source_replay.rent_refund != request.rent_credit
            || source_replay.open_vault_count != 1
            || source_replay.next_revision != request.funding_source_replay_revision
            || source_replay.generation != request.generation
        {
            return Err(ProjectedCustodyError::Balance);
        }
        let receipt = ProjectedCustodyLockReceiptV1 {
            market: request.market,
            release_set: request.release_set,
            context_digest: request.context_digest,
            source_vault: request.funding_source_vault,
            source_replay: source_replay_key,
            hoard_vault: request.hoard_vault,
            rent_credit: request.rent_credit,
            request_digest,
            amount: request.amount,
            source_vault_rent_lamports: source_vault_lamports,
            source_replay_rent_lamports: source_replay_lamports,
            resulting_revision: request.resulting_revision,
        };
        receipt.encode()?;
        self.phase = ProjectedCustodyPhaseV1::HoardLocked;
        self.locked_amount = request.amount;
        self.advance(request, request_digest);
        Ok((self, receipt))
    }

    /// Authorize expiry refund and exact vault/state closure to RentCredit.
    #[allow(clippy::too_many_arguments)]
    pub fn refund_and_close(
        self,
        request: ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
        current_slot: u64,
        vault_before: u64,
        vault_after: u64,
        rent_credit: [u8; 32],
        market_vacant: bool,
    ) -> Result<ProjectedCustodyReceiptV1, ProjectedCustodyError> {
        self.authenticate_next(&request, request_digest)?;
        if self.phase != ProjectedCustodyPhaseV1::HoardLocked
            || request.operation != ProjectedCustodyOperationV1::RefundAndClose
            || current_slot <= self.request.expiry_slot
            || request.amount != self.locked_amount
            || vault_before != self.locked_amount
            || vault_after != 0
            || rent_credit != self.request.rent_credit
            || !market_vacant
        {
            return Err(ProjectedCustodyError::Expiry);
        }
        ProjectedCustodyReceiptV1::terminal(&self, &request, request_digest, false, false, [0; 32])
    }

    /// Authorize expiry cleanup of an empty prepared Hoard and projection state.
    #[allow(clippy::too_many_arguments)]
    pub fn abort_open_and_close(
        self,
        request: ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
        current_slot: u64,
        vault_balance: u64,
        rent_credit: [u8; 32],
        market_vacant: bool,
    ) -> Result<ProjectedCustodyReceiptV1, ProjectedCustodyError> {
        self.authenticate_next(&request, request_digest)?;
        if self.phase != ProjectedCustodyPhaseV1::HoardOpen
            || request.operation != ProjectedCustodyOperationV1::AbortOpenAndClose
            || request.amount != 0
            || current_slot <= self.request.expiry_slot
            || vault_balance != 0
            || rent_credit != self.request.rent_credit
            || !market_vacant
        {
            return Err(ProjectedCustodyError::Expiry);
        }
        ProjectedCustodyReceiptV1::terminal(&self, &request, request_digest, false, true, [0; 32])
    }

    /// Rebind the already-credited projected Hoard to the exact newly founded
    /// Market without moving principal, then authorize projection-state close.
    pub fn realize_and_close(
        self,
        request: ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
        market: CoreState,
        market_state_digest: [u8; 32],
        vault_balance: u64,
        rent_credit: [u8; 32],
    ) -> Result<ProjectedCustodyReceiptV1, ProjectedCustodyError> {
        self.realize_and_close_ref(
            &request,
            request_digest,
            &market,
            market_state_digest,
            vault_balance,
            rent_credit,
        )
    }

    /// Reference-preserving form of [`Self::realize_and_close`] for bounded
    /// SBF frames. It executes the identical semantic check without copying
    /// the 808-byte state, 768-byte request, and Core state through one frame.
    pub fn realize_and_close_ref(
        &self,
        request: &ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
        market: &CoreState,
        market_state_digest: [u8; 32],
        vault_balance: u64,
        rent_credit: [u8; 32],
    ) -> Result<ProjectedCustodyReceiptV1, ProjectedCustodyError> {
        self.authenticate_next(request, request_digest)?;
        if self.phase != ProjectedCustodyPhaseV1::HoardLocked
            || request.operation != ProjectedCustodyOperationV1::RealizeAndClose
            || request.amount != self.locked_amount
            || vault_balance != self.locked_amount
            || rent_credit != self.request.rent_credit
            || market.identity.market_id.to_bytes() != self.request.market
            || market.identity.generation != self.request.generation
            || market.identity.realm_id.to_bytes() != self.request.realm
            || market.identity.product_record.to_bytes() != self.request.product_record
            || market.identity.product_id.to_bytes() != self.request.product
            || market.identity.resolution_policy.to_bytes() != self.request.source
            || market.identity.selected_release_set.to_bytes() != self.request.release_set
            || !matches!(market.phase, Phase::Founding | Phase::Open)
        {
            return Err(ProjectedCustodyError::Market);
        }
        nonzero(market_state_digest)?;
        ProjectedCustodyReceiptV1::terminal(
            self,
            request,
            request_digest,
            true,
            false,
            market_state_digest,
        )
    }

    fn authenticate_next(
        &self,
        request: &ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
    ) -> Result<(), ProjectedCustodyError> {
        request.validate()?;
        nonzero(request_digest)?;
        if request.expected_revision != self.next_revision
            || request.resulting_revision
                != self
                    .next_revision
                    .checked_add(1)
                    .ok_or(ProjectedCustodyError::Revision)?
            || request.market != self.request.market
            || request.generation != self.request.generation
            || request.realm != self.request.realm
            || request.product_record != self.request.product_record
            || request.product != self.request.product
            || request.source != self.request.source
            || request.release_set != self.request.release_set
            || request.projection_receipt_digest != self.request.projection_receipt_digest
            || request.parent_capability_root != self.request.parent_capability_root
            || request.context_digest != self.request.context_digest
            || request.caller_program != self.request.caller_program
            || request.payer != self.request.payer
            || request.core_program != self.request.core_program
            || request.rent_program != self.request.rent_program
            || request.refund_owner != self.request.refund_owner
            || request.rent_credit != self.request.rent_credit
            || request.hoard_vault != self.request.hoard_vault
            || request.funding_source_vault != self.request.funding_source_vault
            || request.funding_source_context != self.request.funding_source_context
            || request.funding_source_compartment != self.request.funding_source_compartment
            || request.mint != self.request.mint
            || request.token_program != self.request.token_program
            || request.collateral_release != self.request.collateral_release
            || request.expiry_slot != self.request.expiry_slot
            || request.state_rent_lamports != self.request.state_rent_lamports
            || request.vault_rent_lamports != self.request.vault_rent_lamports
            || request.funding_source_replay_revision != self.request.funding_source_replay_revision
            || request.funding_source_state_rent_lamports
                != self.request.funding_source_state_rent_lamports
            || request.funding_source_vault_rent_lamports
                != self.request.funding_source_vault_rent_lamports
        {
            return Err(ProjectedCustodyError::Revision);
        }
        Ok(())
    }

    fn advance(&mut self, request: ProjectedCustodyRequestV1, request_digest: [u8; 32]) {
        self.next_revision = request.resulting_revision;
        self.last_request_digest = request_digest;
    }
}

/// Immediate evidence that principal reached the projected Hoard and its
/// fully consumed normal source Vault/replay were both closed to RentCredit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedCustodyLockReceiptV1 {
    /// Future Market.
    pub market: [u8; 32],
    /// Selected release set.
    pub release_set: [u8; 32],
    /// Domain-separated projected-Hoard replay context.
    pub context_digest: [u8; 32],
    /// Fully consumed source Vault.
    pub source_vault: [u8; 32],
    /// Fully consumed normal source replay.
    pub source_replay: [u8; 32],
    /// Credited projected Hoard Vault.
    pub hoard_vault: [u8; 32],
    /// Immutable RentCredit receiving both closures.
    pub rent_credit: [u8; 32],
    /// Exact complete projected request digest.
    pub request_digest: [u8; 32],
    /// Exact principal moved.
    pub amount: u64,
    /// Exact source-Vault rent closed.
    pub source_vault_rent_lamports: u64,
    /// Exact source-replay rent closed.
    pub source_replay_rent_lamports: u64,
    /// Projected replay revision committed.
    pub resulting_revision: u64,
}

impl ProjectedCustodyLockReceiptV1 {
    /// Encode exact lock-and-source-close evidence.
    pub fn encode(
        self,
    ) -> Result<[u8; PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1], ProjectedCustodyError> {
        for value in [
            self.market,
            self.release_set,
            self.context_digest,
            self.source_vault,
            self.source_replay,
            self.hoard_vault,
            self.rent_credit,
            self.request_digest,
        ] {
            nonzero(value)?;
        }
        if self.amount == 0
            || self.source_vault_rent_lamports == 0
            || self.source_replay_rent_lamports == 0
            || self.resulting_revision == 0
        {
            return Err(ProjectedCustodyError::Receipt);
        }
        let mut output = [0; PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1];
        output[..8].copy_from_slice(&PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1);
        put_u16(&mut output, 8, VERSION_V1)?;
        write_identities(
            &mut output,
            32,
            &[
                self.market,
                self.release_set,
                self.context_digest,
                self.source_vault,
                self.source_replay,
                self.hoard_vault,
                self.rent_credit,
                self.request_digest,
            ],
        )?;
        put_u64(&mut output, 288, self.amount)?;
        put_u64(&mut output, 296, self.source_vault_rent_lamports)?;
        put_u64(&mut output, 304, self.source_replay_rent_lamports)?;
        put_u64(&mut output, 312, self.resulting_revision)?;
        Ok(output)
    }

    /// Hostile-decode exact lock-and-source-close evidence.
    pub fn decode(input: &[u8]) -> Result<Self, ProjectedCustodyError> {
        header(
            input,
            &PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1,
            PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1,
        )?;
        if any_nonzero(input, 10, 22)? {
            return Err(ProjectedCustodyError::NonCanonical);
        }
        let ids = read_identities::<8>(input, 32)?;
        let value = Self {
            market: ids[0],
            release_set: ids[1],
            context_digest: ids[2],
            source_vault: ids[3],
            source_replay: ids[4],
            hoard_vault: ids[5],
            rent_credit: ids[6],
            request_digest: ids[7],
            amount: read_u64(input, 288)?,
            source_vault_rent_lamports: read_u64(input, 296)?,
            source_replay_rent_lamports: read_u64(input, 304)?,
            resulting_revision: read_u64(input, 312)?,
        };
        value.encode()?;
        Ok(value)
    }
}

/// Typed terminal evidence for refund or no-move Hoard realization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedCustodyReceiptV1 {
    /// True only for exact newly-founded Market realization.
    pub realized: bool,
    /// True only for expiry cleanup of an empty prepared Hoard.
    pub aborted_open: bool,
    /// Future/newly-founded Market.
    pub market: [u8; 32],
    /// Selected release set.
    pub release_set: [u8; 32],
    /// Parent capability root.
    pub parent_capability_root: [u8; 32],
    /// Parent action context digest.
    pub context_digest: [u8; 32],
    /// Projected Hoard vault.
    pub hoard_vault: [u8; 32],
    /// Exact principal credited before Found.
    pub amount: u64,
    /// Exact terminal request digest.
    pub request_digest: [u8; 32],
    /// Core state digest for realization; zero for refund.
    pub market_state_digest: [u8; 32],
    /// Exact permanent RentCredit close destination.
    pub rent_credit: [u8; 32],
    /// Final consumed revision.
    pub resulting_revision: u64,
}

impl ProjectedCustodyReceiptV1 {
    /// Encode the sole terminal acknowledgement bytes.
    pub fn encode(self) -> Result<[u8; PROJECTED_CUSTODY_RECEIPT_BYTES_V1], ProjectedCustodyError> {
        nonzero(self.market)?;
        nonzero(self.release_set)?;
        nonzero(self.parent_capability_root)?;
        nonzero(self.context_digest)?;
        nonzero(self.hoard_vault)?;
        nonzero(self.request_digest)?;
        nonzero(self.rent_credit)?;
        if self.resulting_revision == 0
            || (self.realized && self.aborted_open)
            || (self.aborted_open != (self.amount == 0))
            || self.realized == self.market_state_digest.iter().all(|byte| *byte == 0)
        {
            return Err(ProjectedCustodyError::Receipt);
        }
        let mut output = [0; PROJECTED_CUSTODY_RECEIPT_BYTES_V1];
        output[..8].copy_from_slice(&PROJECTED_CUSTODY_RECEIPT_MAGIC_V1);
        put_u16(&mut output, 8, VERSION_V1)?;
        put_u8(&mut output, 10, u8::from(self.realized))?;
        put_u8(&mut output, 11, u8::from(self.aborted_open))?;
        write_identities(
            &mut output,
            32,
            &[
                self.market,
                self.release_set,
                self.parent_capability_root,
                self.context_digest,
                self.hoard_vault,
                self.request_digest,
                self.market_state_digest,
                self.rent_credit,
            ],
        )?;
        put_u64(&mut output, 288, self.amount)?;
        put_u64(&mut output, 296, self.resulting_revision)?;
        Ok(output)
    }

    /// Hostile-decode one terminal acknowledgement.
    pub fn decode(input: &[u8]) -> Result<Self, ProjectedCustodyError> {
        header(
            input,
            &PROJECTED_CUSTODY_RECEIPT_MAGIC_V1,
            PROJECTED_CUSTODY_RECEIPT_BYTES_V1,
        )?;
        if any_nonzero(input, 12, 20)? || any_nonzero(input, 304, 16)? {
            return Err(ProjectedCustodyError::NonCanonical);
        }
        let realized = match read_u8(input, 10)? {
            0 => false,
            1 => true,
            _ => return Err(ProjectedCustodyError::NonCanonical),
        };
        let aborted_open = match read_u8(input, 11)? {
            0 => false,
            1 => true,
            _ => return Err(ProjectedCustodyError::NonCanonical),
        };
        let ids = read_identities::<8>(input, 32)?;
        let value = Self {
            realized,
            aborted_open,
            market: ids[0],
            release_set: ids[1],
            parent_capability_root: ids[2],
            context_digest: ids[3],
            hoard_vault: ids[4],
            request_digest: ids[5],
            market_state_digest: ids[6],
            rent_credit: ids[7],
            amount: read_u64(input, 288)?,
            resulting_revision: read_u64(input, 296)?,
        };
        value.encode()?;
        Ok(value)
    }

    fn terminal(
        state: &ProjectedCustodyStateV1,
        request: &ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
        realized: bool,
        aborted_open: bool,
        market_state_digest: [u8; 32],
    ) -> Result<Self, ProjectedCustodyError> {
        nonzero(request_digest)?;
        if realized == market_state_digest.iter().all(|byte| *byte == 0)
            || (realized && aborted_open)
            || (aborted_open != (state.locked_amount == 0))
        {
            return Err(ProjectedCustodyError::Receipt);
        }
        Ok(Self {
            realized,
            aborted_open,
            market: state.request.market,
            release_set: state.request.release_set,
            parent_capability_root: state.request.parent_capability_root,
            context_digest: state.request.context_digest,
            hoard_vault: state.request.hoard_vault,
            amount: state.locked_amount,
            request_digest,
            market_state_digest,
            rent_credit: state.request.rent_credit,
            resulting_revision: request.resulting_revision,
        })
    }
}

/// Convert an authenticated realization into the ordinary Custody replay
/// authority at the same canonical replay PDA. The projected Hoard remains
/// live, so the normal replay starts with exactly one open Vault.
pub fn normal_replay_from_realization_v1(
    state: ProjectedCustodyStateV1,
    receipt: ProjectedCustodyReceiptV1,
    poststate_commitment: [u8; 32],
) -> Result<CustodyReplayV1, ProjectedCustodyError> {
    if !receipt.realized
        || receipt.market != state.request.market
        || receipt.release_set != state.request.release_set
        || receipt.parent_capability_root != state.request.parent_capability_root
        || receipt.context_digest != state.request.context_digest
        || receipt.hoard_vault != state.request.hoard_vault
        || receipt.amount != state.locked_amount
        || receipt.rent_credit != state.request.rent_credit
        || receipt.resulting_revision
            != state
                .next_revision
                .checked_add(1)
                .ok_or(ProjectedCustodyError::Revision)?
        || poststate_commitment.iter().all(|byte| *byte == 0)
    {
        return Err(ProjectedCustodyError::Receipt);
    }
    Ok(CustodyReplayV1 {
        caller_role: ExecutionRoleV1::Trading,
        release_set: state.request.release_set,
        market: state.request.market,
        realm: state.request.realm,
        context: state.request.context_digest,
        caller_program: state.request.caller_program,
        rent_refund: state.request.rent_credit,
        open_vault_count: 1,
        next_revision: 1,
        generation: state.request.generation,
        last_request_digest: receipt.request_digest,
        last_poststate_commitment: poststate_commitment,
    })
}

fn nonzero(value: [u8; 32]) -> Result<(), ProjectedCustodyError> {
    if value.iter().all(|byte| *byte == 0) {
        Err(ProjectedCustodyError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn header(input: &[u8], magic: &[u8; 8], width: usize) -> Result<(), ProjectedCustodyError> {
    if input.len() != width || input.get(..8) != Some(magic.as_slice()) {
        return Err(ProjectedCustodyError::InvalidHeader);
    }
    if read_u16(input, 8)? != VERSION_V1 {
        return Err(ProjectedCustodyError::InvalidHeader);
    }
    Ok(())
}

fn write_identities<const N: usize>(
    output: &mut [u8],
    offset: usize,
    values: &[[u8; 32]; N],
) -> Result<(), ProjectedCustodyError> {
    for (index, value) in values.iter().enumerate() {
        let start = offset
            .checked_add(
                index
                    .checked_mul(32)
                    .ok_or(ProjectedCustodyError::InvalidLength)?,
            )
            .ok_or(ProjectedCustodyError::InvalidLength)?;
        put(output, start, value)?;
    }
    Ok(())
}

fn read_identities<const N: usize>(
    input: &[u8],
    offset: usize,
) -> Result<[[u8; 32]; N], ProjectedCustodyError> {
    let mut values = [[0; 32]; N];
    for (index, value) in values.iter_mut().enumerate() {
        let start = offset
            .checked_add(
                index
                    .checked_mul(32)
                    .ok_or(ProjectedCustodyError::InvalidLength)?,
            )
            .ok_or(ProjectedCustodyError::InvalidLength)?;
        *value = slice(input, start, 32)?
            .try_into()
            .map_err(|_| ProjectedCustodyError::InvalidLength)?;
    }
    Ok(values)
}

fn any_nonzero(input: &[u8], offset: usize, width: usize) -> Result<bool, ProjectedCustodyError> {
    Ok(slice(input, offset, width)?.iter().any(|byte| *byte != 0))
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8], ProjectedCustodyError> {
    input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(ProjectedCustodyError::InvalidLength)?,
        )
        .ok_or(ProjectedCustodyError::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), ProjectedCustodyError> {
    let target = output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(ProjectedCustodyError::InvalidLength)?,
        )
        .ok_or(ProjectedCustodyError::InvalidLength)?;
    target.copy_from_slice(value);
    Ok(())
}

fn put_u8(output: &mut [u8], offset: usize, value: u8) -> Result<(), ProjectedCustodyError> {
    *output
        .get_mut(offset)
        .ok_or(ProjectedCustodyError::InvalidLength)? = value;
    Ok(())
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) -> Result<(), ProjectedCustodyError> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) -> Result<(), ProjectedCustodyError> {
    put(output, offset, &value.to_le_bytes())
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8, ProjectedCustodyError> {
    input
        .get(offset)
        .copied()
        .ok_or(ProjectedCustodyError::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, ProjectedCustodyError> {
    Ok(u16::from_le_bytes(
        slice(input, offset, 2)?
            .try_into()
            .map_err(|_| ProjectedCustodyError::InvalidLength)?,
    ))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, ProjectedCustodyError> {
    Ok(u64::from_le_bytes(
        slice(input, offset, 8)?
            .try_into()
            .map_err(|_| ProjectedCustodyError::InvalidLength)?,
    ))
}

#[cfg(test)]
mod tests {
    use dclutch_market_core_codec::{Identity, MarketIdentity, Readiness};

    use super::*;

    fn id(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn request(
        operation: ProjectedCustodyOperationV1,
        revision: u64,
        amount: u64,
    ) -> ProjectedCustodyRequestV1 {
        ProjectedCustodyRequestV1 {
            operation,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: id(1),
            generation: 7,
            realm: id(2),
            product_record: id(3),
            product: id(4),
            source: id(5),
            release_set: id(6),
            projection_receipt_digest: id(7),
            parent_capability_root: id(8),
            context_digest: id(9),
            caller_program: id(10),
            payer: id(11),
            core_program: id(12),
            rent_program: id(13),
            refund_owner: id(14),
            rent_credit: id(15),
            hoard_vault: id(16),
            funding_source_vault: id(17),
            funding_source_context: id(18),
            funding_source_compartment: CompartmentV1::SeriesEscrow,
            mint: id(19),
            token_program: id(20),
            collateral_release: id(21),
            expiry_slot: 100,
            expected_revision: revision,
            resulting_revision: revision + 1,
            amount,
            state_rent_lamports: 1_000,
            vault_rent_lamports: 2_000,
            funding_source_replay_revision: 3,
            funding_source_state_rent_lamports: 3_000,
            funding_source_vault_rent_lamports: 4_000,
        }
    }

    fn source_replay() -> CustodyReplayV1 {
        CustodyReplayV1 {
            caller_role: ExecutionRoleV1::Trading,
            release_set: id(6),
            market: id(1),
            realm: id(2),
            context: id(18),
            caller_program: id(10),
            rent_refund: id(15),
            open_vault_count: 1,
            next_revision: 3,
            generation: 7,
            last_request_digest: id(40),
            last_poststate_commitment: id(41),
        }
    }

    fn projection() -> ProjectFoundReceiptV1 {
        ProjectFoundReceiptV1::new(
            Identity::new(id(1)).expect("id"),
            7,
            Identity::new(id(2)).expect("id"),
            Identity::new(id(19)).expect("id"),
            Identity::new(id(20)).expect("id"),
            Identity::new(id(21)).expect("id"),
            Identity::new(id(3)).expect("id"),
            Identity::new(id(4)).expect("id"),
            Identity::new(id(5)).expect("id"),
            Identity::new(id(6)).expect("id"),
            Identity::new(id(13)).expect("id"),
            id(20),
        )
        .expect("projection")
    }

    fn live_market() -> CoreState {
        let identity = |seed| Identity::new(id(seed)).expect("id");
        CoreState {
            phase: Phase::Founding,
            readiness: Readiness::Prepaid,
            terminal_winner: 0,
            identity: MarketIdentity {
                market_id: identity(1),
                realm_id: identity(2),
                product_record: identity(3),
                product_id: identity(4),
                resolution_policy: identity(5),
                capability_manifest: identity(20),
                selected_release_set: identity(6),
                registry_program: identity(21),
                generation: 7,
            },
            outstanding_capabilities: 0,
            rent_beneficiary: identity(22),
            terminal_receipt: None,
        }
    }

    #[test]
    fn projected_hoard_is_credited_before_found_then_realized_without_a_move() {
        let init = request(ProjectedCustodyOperationV1::Initialize, 0, 0);
        let bytes = init.encode().expect("bytes");
        assert_eq!(ProjectedCustodyRequestV1::decode(&bytes), Ok(init));
        let state = ProjectedCustodyStateV1::initialize(
            init,
            projection(),
            id(12),
            id(7),
            id(30),
            50,
            true,
            9,
        )
        .expect("initialize");
        let state = state
            .open_hoard(
                request(ProjectedCustodyOperationV1::OpenHoard, 1, 0),
                id(31),
                0,
                true,
            )
            .expect("open");
        let state = state
            .lock_hoard(
                request(ProjectedCustodyOperationV1::LockHoard, 2, 500),
                id(32),
                0,
                500,
                true,
            )
            .expect("lock");
        let realize = request(ProjectedCustodyOperationV1::RealizeAndClose, 3, 500);
        let market = live_market();
        let receipt = state
            .realize_and_close_ref(&realize, id(33), &market, id(34), 500, id(15))
            .expect("reference-preserving realize");
        assert_eq!(
            receipt,
            state
                .realize_and_close(realize, id(33), market, id(34), 500, id(15))
                .expect("compatibility realize")
        );
        let mut substituted = realize;
        substituted.release_set = id(44);
        assert_eq!(
            state.realize_and_close_ref(&substituted, id(33), &market, id(34), 500, id(15),),
            Err(ProjectedCustodyError::Revision)
        );
        assert!(receipt.realized);
        assert_eq!(receipt.amount, 500);
        assert_eq!(receipt.hoard_vault, id(16));
        let normal = normal_replay_from_realization_v1(state, receipt, id(35))
            .expect("normal Custody replay");
        assert_eq!(normal.open_vault_count, 1);
        assert_eq!(normal.context, id(9));
        assert_eq!(normal.rent_refund, id(15));
        assert_eq!(
            CustodyReplayV1::decode(&normal.to_bytes().expect("normal bytes")),
            Ok(normal)
        );
    }

    #[test]
    fn early_refund_foreign_market_and_hostile_reserved_bytes_refuse() {
        let init = request(ProjectedCustodyOperationV1::Initialize, 0, 0);
        let mut external_source = init;
        external_source.funding_source_compartment = CompartmentV1::External;
        assert_eq!(
            external_source.validate(),
            Err(ProjectedCustodyError::NonCanonical)
        );
        let mut colliding_context = init;
        colliding_context.context_digest = colliding_context.funding_source_context;
        assert_eq!(
            colliding_context.validate(),
            Err(ProjectedCustodyError::NonCanonical)
        );
        let mut bytes = init.encode().expect("bytes");
        bytes[13] = 1;
        assert_eq!(
            ProjectedCustodyRequestV1::decode(&bytes),
            Err(ProjectedCustodyError::NonCanonical)
        );
        let state = ProjectedCustodyStateV1::initialize(
            init,
            projection(),
            id(12),
            id(7),
            id(30),
            50,
            true,
            9,
        )
        .expect("initialize")
        .open_hoard(
            request(ProjectedCustodyOperationV1::OpenHoard, 1, 0),
            id(31),
            0,
            true,
        )
        .expect("open")
        .lock_hoard(
            request(ProjectedCustodyOperationV1::LockHoard, 2, 500),
            id(32),
            0,
            500,
            true,
        )
        .expect("lock");
        assert_eq!(
            state.refund_and_close(
                request(ProjectedCustodyOperationV1::RefundAndClose, 3, 500),
                id(33),
                100,
                500,
                0,
                id(15),
                true,
            ),
            Err(ProjectedCustodyError::Expiry)
        );
        let mut foreign = live_market();
        foreign.identity.market_id = Identity::new(id(99)).expect("id");
        assert_eq!(
            state.realize_and_close(
                request(ProjectedCustodyOperationV1::RealizeAndClose, 3, 500),
                id(33),
                foreign,
                id(34),
                500,
                id(15),
            ),
            Err(ProjectedCustodyError::Market)
        );
    }

    #[test]
    fn empty_open_can_abort_only_after_expiry() {
        let state = ProjectedCustodyStateV1::initialize(
            request(ProjectedCustodyOperationV1::Initialize, 0, 0),
            projection(),
            id(12),
            id(7),
            id(30),
            50,
            true,
            9,
        )
        .expect("initialize")
        .open_hoard(
            request(ProjectedCustodyOperationV1::OpenHoard, 1, 0),
            id(31),
            0,
            true,
        )
        .expect("open");
        let abort = request(ProjectedCustodyOperationV1::AbortOpenAndClose, 2, 0);
        assert_eq!(
            state.abort_open_and_close(abort, id(32), 100, 0, id(15), true),
            Err(ProjectedCustodyError::Expiry)
        );
        assert_eq!(
            state.abort_open_and_close(abort, id(32), 101, 1, id(15), true),
            Err(ProjectedCustodyError::Expiry)
        );
        let receipt = state
            .abort_open_and_close(abort, id(32), 101, 0, id(15), true)
            .expect("abort");
        assert!(!receipt.realized);
        assert!(receipt.aborted_open);
        assert_eq!(receipt.amount, 0);
        let bytes = receipt.encode().expect("receipt bytes");
        assert_eq!(ProjectedCustodyReceiptV1::decode(&bytes), Ok(receipt));
    }

    #[test]
    fn lock_can_atomically_exhaust_and_close_exact_normal_source() {
        let state = ProjectedCustodyStateV1::initialize(
            request(ProjectedCustodyOperationV1::Initialize, 0, 0),
            projection(),
            id(12),
            id(7),
            id(30),
            50,
            true,
            9,
        )
        .expect("initialize")
        .open_hoard(
            request(ProjectedCustodyOperationV1::OpenHoard, 1, 0),
            id(31),
            0,
            true,
        )
        .expect("open");
        let lock = request(ProjectedCustodyOperationV1::LockHoardAndCloseSource, 2, 500);
        assert_eq!(
            state.lock_hoard_and_close_source(
                lock,
                id(32),
                id(42),
                source_replay(),
                500,
                1,
                0,
                499,
                4_000,
                3_000,
                id(15),
                true,
            ),
            Err(ProjectedCustodyError::Balance)
        );
        let mut wrong_replay = source_replay();
        wrong_replay.context = id(43);
        assert_eq!(
            state.lock_hoard_and_close_source(
                lock,
                id(32),
                id(42),
                wrong_replay,
                500,
                0,
                0,
                500,
                4_000,
                3_000,
                id(15),
                true,
            ),
            Err(ProjectedCustodyError::Balance)
        );
        assert_eq!(
            state.lock_hoard_and_close_source(
                lock,
                id(32),
                id(42),
                source_replay(),
                500,
                0,
                0,
                500,
                3_999,
                3_000,
                id(15),
                true,
            ),
            Err(ProjectedCustodyError::Balance)
        );
        let (next, receipt) = state
            .lock_hoard_and_close_source(
                lock,
                id(32),
                id(42),
                source_replay(),
                500,
                0,
                0,
                500,
                4_000,
                3_000,
                id(15),
                true,
            )
            .expect("lock and close source");
        assert_eq!(next.phase, ProjectedCustodyPhaseV1::HoardLocked);
        assert_eq!(next.locked_amount, 500);
        assert_eq!(receipt.source_vault, id(17));
        assert_eq!(receipt.source_replay, id(42));
        let bytes = receipt.encode().expect("receipt bytes");
        assert_eq!(ProjectedCustodyLockReceiptV1::decode(&bytes), Ok(receipt));
    }
}
