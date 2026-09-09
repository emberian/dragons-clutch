//! Typed browser/CLI intent and native planning results; no wire encoding lives here.

use dclutch_claims::rational_lifecycle::LifecycleActionV2;
use solana_program::instruction::Instruction;

/// Human selection. A missing coordinate requests authenticated support discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleIntentV1 {
    /// Selected Market account.
    pub market: [u8; 32],
    /// Wallet paying transaction fees and any explicitly previewed preparation.
    pub payer: [u8; 32],
    /// Existing Claims lifecycle action.
    pub action: LifecycleActionV2,
    /// One representation coordinate; receipt actions carry none.
    pub coordinate: Option<u32>,
    /// Optional check against the canonically derived backing Position.
    pub expected_position: Option<[u8; 32]>,
    /// Authenticated capability choice when the Market has multiple candidates.
    pub selected_capability: Option<[u8; 32]>,
    /// Receipt descriptor chosen from authenticated candidates for this Market.
    pub representation_descriptor: Option<[u8; 32]>,
}

/// Deployment coordinates remain untrusted until joined to the selected release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleProgramsV1 {
    /// Market Core program.
    pub core: [u8; 32],
    /// Immutable Registry program.
    pub registry: [u8; 32],
    /// Claims program.
    pub claims: [u8; 32],
    /// Trading Hot entrypoint.
    pub trading: [u8; 32],
    /// dClutch Rent program, distinct from Solana's Rent sysvar.
    pub rent_program: [u8; 32],
    /// Custody program.
    pub custody: [u8; 32],
    /// Exact activated release cache selected by deployment discovery.
    pub activation_cache: [u8; 32],
    /// Canonical checked multiprogram binary from the selected deployment manifest.
    pub checked_execution_release_set: Vec<u8>,
}

/// Explicit native-requested account data window, used for Loader headers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleDataSliceV1 {
    /// Byte offset within the account.
    pub offset: u32,
    /// Number of requested bytes.
    pub length: u32,
}

/// One canonical native-selected RPC coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleAccountRequestV1 {
    /// Canonical address to acquire.
    pub address: [u8; 32],
    /// None requests complete data; a slice never masquerades as complete state.
    pub data_slice: Option<StructuredLifecycleDataSliceV1>,
}

/// Native-authored byte filter for discovering immutable descriptor candidates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleMemcmpV1 {
    /// Canonical semantic-owner field offset.
    pub offset: u32,
    /// Exact field bytes.
    pub bytes: Vec<u8>,
}

/// Bounded candidate inventory request; matches still require native authentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleProgramScanV1 {
    /// Program owning candidate records.
    pub program: [u8; 32],
    /// Exact account width when the selected native profile knows it.
    pub data_size: Option<u32>,
    /// Canonical field filters authored by the native semantic owner.
    pub memcmp: Vec<StructuredLifecycleMemcmpV1>,
    /// Optional bounded header projection for inventory only.
    pub data_slice: Option<StructuredLifecycleDataSliceV1>,
}

/// An untrusted inventory of candidate keys, never authority for a transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleScanResultV1 {
    /// Exact native query answered by the transport.
    pub request: StructuredLifecycleProgramScanV1,
    /// Finalized query observation slot.
    pub slot: u64,
    /// Candidate addresses; each requires full point acquisition and authentication.
    pub addresses: Vec<[u8; 32]>,
}

/// Actual account data and owner, without caller-authored transaction privileges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleAccountValueV1 {
    /// Runtime owner.
    pub owner: [u8; 32],
    /// Observed balance.
    pub lamports: u64,
    /// Executable status.
    pub executable: bool,
    /// Complete allocated account width, including when data is sliced.
    pub space: u64,
    /// Complete data or the explicitly requested window.
    pub data: Vec<u8>,
}

/// One observation; absent and present-empty are different states.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleAccountV1 {
    /// Address/window that was acquired.
    pub request: StructuredLifecycleAccountRequestV1,
    /// Actual value, or authenticated RPC absence.
    pub value: Option<StructuredLifecycleAccountValueV1>,
}

/// All supplied observations come from one finalized slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleSnapshotV1 {
    /// Common finalized observation slot.
    pub slot: u64,
    /// Bounded observed corpus, with no duplicate addresses.
    pub accounts: Vec<StructuredLifecycleAccountV1>,
    /// Prior native-requested inventories; they do not establish record validity.
    pub scans: Vec<StructuredLifecycleScanResultV1>,
}

/// Authenticated nonzero representation support, offered as a user choice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleSupportV1 {
    /// Representation coordinate, not an index into a caller-authored array.
    pub coordinate: u32,
    /// Exact descriptor coefficient numerator.
    pub coefficient: u64,
}

/// One eligible selected capability with its representation descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleCapabilityV1 {
    /// Canonical capability identity.
    pub capability: [u8; 32],
    /// Canonical representation descriptor identity.
    pub descriptor: [u8; 32],
    /// Canonical receipt Mint address.
    pub receipt_mint: [u8; 32],
    /// Common denominator for the descriptor coefficients.
    pub denominator: u64,
    /// Canonically ordered nonzero support.
    pub support: Vec<StructuredLifecycleSupportV1>,
}

/// One authenticated per-Market Structured capability before receipt selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleRootChoiceV1 {
    /// Canonical Trading capability-root PDA.
    pub capability: [u8; 32],
    /// Immutable selected ProgramSet identity.
    pub program_set: [u8; 32],
    /// Exact manifest entry index.
    pub entry_index: u16,
}

/// One transaction boundary in the retained native preparation/execution path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredLifecycleStepKindV1 {
    /// Seal one selected immutable artifact for the Hot entrypoint.
    SealArtifact,
    /// Supply the exact native-derived rent preparation.
    FundRent,
    /// Execute the selected Claims lifecycle transition through Trading.
    ExecuteLifecycle,
}

/// Account image expected after the transaction, authored by the native planner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleExpectedAccountV1 {
    /// Exact account address.
    pub address: [u8; 32],
    /// Expected complete image or account absence after closure.
    pub value: Option<StructuredLifecycleAccountValueV1>,
    /// Subtract the finalized transaction fee from the expected image's balance.
    /// Only the exact fee payer may carry this flag; native verification checks it.
    pub deduct_transaction_fee: bool,
}

/// User-facing economic/resource consequences of this native step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecyclePreviewV1 {
    /// Selected representation receipt.
    pub receipt_mint: [u8; 32],
    /// Selected coordinate, if this is a coordinate action.
    pub coordinate: Option<u32>,
    /// Derived backing Position, if relevant.
    pub position: Option<[u8; 32]>,
    /// Wallet lamports used to prepare this step, excluding transaction fees.
    pub preparation_lamports: u64,
    /// Rent returned by this step.
    pub returned_rent_lamports: u64,
    /// Exact rent recipient, when rent is returned.
    pub rent_recipient: Option<[u8; 32]>,
    /// Receipt supply observed before the step.
    pub receipt_supply_before: u64,
    /// Receipt supply after the step.
    pub receipt_supply_after: u64,
}

/// One complete unsigned transaction step; future steps require fresh discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredLifecyclePlanV1 {
    /// Complete original user selection.
    pub intent: StructuredLifecycleIntentV1,
    /// Native-defined identity of this exact step and prestate.
    pub step_id: [u8; 32],
    /// Kind of next transaction.
    pub step_kind: StructuredLifecycleStepKindV1,
    /// Canonical selected capability identity.
    pub selected_capability: [u8; 32],
    /// Observation floor used by the constructor.
    pub finalized_slot: u64,
    /// Complete canonical instructions, including required transaction preparation.
    pub instructions: Vec<Instruction>,
    /// Wallet signers required by the compiled packet.
    pub required_wallet_signers: Vec<[u8; 32]>,
    /// Concrete effect and rent preview.
    pub preview: StructuredLifecyclePreviewV1,
    /// Native-owned exact finalized poststate checks, including protected accounts.
    pub expected_poststates: Vec<StructuredLifecycleExpectedAccountV1>,
}

/// Iterative native planning result. Discovery never signs or sends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StructuredLifecyclePlanningV1 {
    /// Acquire these account coordinates, then call the same planner again.
    Discover {
        /// Native-selected requests not yet present in the corpus.
        requests: Vec<StructuredLifecycleAccountRequestV1>,
        /// Native-requested bounded candidate inventories.
        scans: Vec<StructuredLifecycleProgramScanV1>,
    },
    /// Choose an authenticated capability before discovering its receipt actions.
    SelectCapability {
        /// Per-Market capabilities admitting the requested selector.
        capabilities: Vec<StructuredLifecycleRootChoiceV1>,
    },
    /// Choose among authenticated capabilities/support before constructing a step.
    Select {
        /// Eligible immutable selections.
        capabilities: Vec<StructuredLifecycleCapabilityV1>,
    },
    /// Review and execute exactly one next transaction.
    Ready {
        /// Next native step.
        plan: StructuredLifecyclePlanV1,
    },
    /// Current finalized account state already satisfies the requested lifecycle.
    Complete {
        /// Selected immutable capability identity.
        selected_capability: [u8; 32],
        /// State description expressed as concrete resources and quantities.
        preview: StructuredLifecyclePreviewV1,
    },
}
