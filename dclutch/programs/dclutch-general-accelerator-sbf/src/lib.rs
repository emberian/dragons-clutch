#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Readonly stateless admitted-AOT accelerator for General clearing.
//!
//! The program receives the canonical admitted frame from Trading, rebuilds
//! the complete authenticated input bank from Trading-owned scratch pages,
//! evaluates one General successor transition, and returns exactly one typed
//! candidate chunk. It never writes an account, invokes a child, or owns any
//! protocol state; common Trading remains the sole effect and commit authority.

extern crate alloc;
extern crate std;

use alloc::vec;

use dclutch_capability_program_contract::hot_v3::{
    DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_RUNTIME_CONFIG_COORDINATE_V3,
    HOT_RUNTIME_PRODUCT_COORDINATE_V3, HotExecutionEnvelopeV3,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, hot_v3::HOT_RUNTIME_ROOT_COORDINATE_V3,
};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::{
    admitted_v3::{
        ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3, ADMITTED_INSTRUCTIONS_ACCOUNT_V3,
        ADMITTED_OUTPUT_PAGE_ACCOUNT_V3, ADMITTED_TRADING_PROGRAM_ACCOUNT_V3,
        admitted_runtime_accounts_start_v3,
    },
    v2::{
        ACCELERATOR_ACK_HEADER_BYTES_V2, ACCELERATOR_OUTPUT_PAGE_ACK_BYTES_V3, AcceleratorAckV2,
        AcceleratorOutputPageAckV3, AcceleratorTransportProfileV2, AdmittedAcceleratorRequestV2,
        AuthenticatedScratchPageV2, RequestTransportV2,
    },
};
use dclutch_general_adapter_contract::{
    account_rules_v3::general_account_profile_fixed_count_v3,
    admitted_accelerator_v3::authenticate_frozen_selection_v3,
    artifacts_v3::{GeneralDecodedRequestV3, decode_general_request_v3},
    candidate_v1::{CandidateVerifyRowViewV1, GeneralCandidateV1, candidate_verifier_len_v1},
    collection_v1::GeneralBatchV1,
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, authenticate_general_close_candidate_v3,
        general_hot_candidate_bank_len_v3, general_hot_environment_from_bank_v3,
        general_hot_scalar_count_v3, project_general_cancel_order_candidate_in_place_v3,
        project_general_close_batch_candidate_in_place_v3,
        project_general_hot_candidate_in_place_v3,
        project_general_initialize_candidate_in_place_v3,
        project_general_open_batch_candidate_in_place_v3,
        project_general_place_order_candidate_in_place_v3,
        project_general_release_order_candidate_in_place_v3,
        project_general_selection_candidate_in_place_v3,
        project_general_submit_candidate_in_place_v3,
        project_general_verify_candidate_workspace_v3,
    },
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateV3},
    runtime_manifest::SettlementManifestV2,
    runtime_selection::{
        RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionCursorV2,
        consider_verified_candidate_v2, freeze_selection_v2,
    },
    runtime_settlement::{
        RuntimeSettlementActionV2, RuntimeSettlementViewV2,
        evaluate_runtime_settlement_in_place_v2, initialize_runtime_settlement_in_place_v2,
        runtime_settlement_effect_len_v2,
    },
    runtime_verify::RuntimeCandidateVerifierV2,
    runtime_width::{VerifiedCandidateV2, settlement_cursor_len},
    state_artifacts_v3::{
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GENERAL_TERMINAL_STATE_ACCOUNT_V3,
        GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3, GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3,
        GeneralReadonlyEvidenceKindV3, general_readonly_evidence_count_v3,
        general_readonly_evidence_v3,
    },
};
use dclutch_general_codec::{
    Action, SelectionPolicyV1, successor_request_v2::CONTROLLER_REQUEST_BYTES_V2,
};
use dclutch_general_config_contract::v3::GeneralConfigV3;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, log::sol_log,
    program::set_return_data, program_error::ProgramError, pubkey::Pubkey,
};
use solana_sdk_ids::{compute_budget, sysvar};

/// Stable physical refusal from the General accelerator boundary.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAcceleratorSbfErrorV3 {
    /// Accelerator request transport or register geometry differed.
    InvalidRequest = 0xC000,
    /// The fixed admitted frame or readonly runtime frame differed.
    InvalidFrame = 0xC001,
    /// The current top-level instruction could not be read from the sysvar.
    ///
    /// This variant used to carry every one of `authenticate_top_level`'s
    /// causes -- eight conjuncts behind one code, so a validator log could say
    /// only that the top level was wrong and never which part of it. It is
    /// narrowed here to the sysvar access itself and the five distinct causes
    /// below are split out; its numeric value is unchanged, so a code already
    /// seen in a log still means a subset of what it meant.
    InvalidTopLevelInstruction = 0xC002,
    /// The request's declared scratch-bank geometry could not be used.
    ///
    /// Narrowed the way `InvalidTopLevelInstruction` was: this used to carry
    /// every cause in `assemble_input_bank`, so a refusal could not say whether
    /// a page was mis-privileged, mis-decoded, foreign to the request, out of
    /// order, or simply summed to the wrong bank. It now means the declared
    /// counts and offsets, or the admitted Trading identity they are keyed by,
    /// did not fit. Its numeric value is unchanged, so a code already seen in a
    /// log still means a subset of what it meant.
    InvalidScratchBank = 0xC003,
    /// The exact acknowledgement could not be encoded.
    InvalidAcknowledgement = 0xC004,
    /// An instruction ahead of this one did not belong to ComputeBudget.
    ForeignInstructionBeforeTrading = 0xC005,
    /// No exact `RequestHeapFrame` preceded the current instruction.
    ///
    /// The REQUEST, which is all a program can read. A program cannot observe
    /// its granted heap -- `create_vm!` maps exactly the compute budget's
    /// `heap_size` and every access above it faults -- so this code says the
    /// caller did not ASK, never that the runtime did not give. It was called
    /// `HeapFrameNotRequested` until 2026-09-01, and its numeric value is
    /// unchanged, so a code already seen in a log still means what it meant.
    ///
    /// It was ALSO documented here, for most of that same day, as harmless
    /// because this program ran on the stock `entrypoint!` allocator and could
    /// not use an extended heap either way. That stopped being true later the
    /// same day: `custom-heap` is implemented now, the request this scan finds
    /// is what raises this program's own ceiling, and a caller who does not ask
    /// gets 32,768 rather than the 65,536 the route needs.
    HeapFrameNotRequested = 0xC006,
    /// The current top-level instruction was not the admitted Trading program's.
    TopLevelProgramNotTrading = 0xC007,
    /// The top-level data was not a canonical Hot execution envelope.
    InvalidHotEnvelope = 0xC008,
    /// The carried family request was not the exact width, or did not decode.
    InvalidFamilyRequest = 0xC009,
    /// The supplied account was not the readonly instructions sysvar.
    InstructionsSysvarAccount = 0xC00A,
    /// The runtime could not report which top-level instruction is executing.
    CurrentInstructionIndexUnreadable = 0xC00B,
    /// An instruction ahead of the current one could not be read.
    PrecedingInstructionUnreadable = 0xC00C,
    /// A scratch page was not a Trading-owned readonly unsigned data account.
    ScratchPagePrivileges = 0xC00D,
    /// A scratch page's bytes were not a canonical authenticated page.
    ScratchPageDecode = 0xC00E,
    /// A scratch page did not belong to this caller and this request.
    ScratchPageRequestBinding = 0xC00F,
    /// A scratch page arrived out of its streamed chunk index or offset.
    ScratchPageOrder = 0xC010,
    /// The reassembled bank's bytes differed from the digest declared.
    ScratchBankDigest = 0xC011,
    /// The pages did not sum to the bank length the request declared.
    ///
    /// Split from `ScratchBankDigest` because the two say different things: a
    /// length mismatch means the pages do not add up to the declared bank, and
    /// a digest mismatch means they add up exactly and carry different bytes.
    /// One is a transport arithmetic fault and the other is a content fault.
    ScratchBankLength = 0xC012,
    /// The declared heap frame could not be installed as this program's ceiling.
    ///
    /// Unreachable by construction and refused by name anyway. The only value
    /// ever offered is `DIRECT_HOT_HEAP_FRAME_BYTES_V1`, a compile-time
    /// constant the assertion beside this enum pins inside the allocator's
    /// accepted range and on its 1,024-byte granularity, and the ceiling starts
    /// at the protocol default which is below it. A refusal here would mean one
    /// of those facts has changed under a later edit, and the alternative to
    /// naming it is an allocation that dies of an out-of-memory abort naming
    /// nothing at all -- which is the exact failure this whole change removes.
    HeapCeilingNotLifted = 0xC013,
    /// The output page this program was handed is not one it can write.
    ///
    /// Wrong owner, not marked writable, executable, a signer, or a data borrow
    /// this program could not take. One accusation -- *the page is not mine to
    /// write* -- and a fault instead of a refusal would publish
    /// `ProgramFailedToComplete` with no code at all.
    OutputPageUnwritable = 0xC014,
    /// The output page repeats another account in this CPI frame.
    ///
    /// Its own code because it is its own hazard: **CPI privileges union on a
    /// repeated key**, so a page whose key equals a read-only frame account
    /// would carry write authority into that account. Trading refuses this
    /// before the CPI; the program that would DO the writing declines to be
    /// the instrument.
    OutputPageTooNarrow = 0xC015,
    /// The candidate bank is wider than the page provisioned for it.
    OutputPageAliasesFrame = 0xC016,
}

impl GeneralAcceleratorSbfErrorV3 {
    /// Every refusal this program can raise, in discriminant order.
    ///
    /// This is what the band assertions below read. It is kept honest by
    /// [`GeneralAcceleratorSbfErrorV3::ordinal`], whose match is exhaustive: a variant added to the
    /// enum does not compile until its author writes an arm here, and the only
    /// arm that satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 23] = [
        Self::InvalidRequest,
        Self::InvalidFrame,
        Self::InvalidTopLevelInstruction,
        Self::InvalidScratchBank,
        Self::InvalidAcknowledgement,
        Self::ForeignInstructionBeforeTrading,
        Self::HeapFrameNotRequested,
        Self::TopLevelProgramNotTrading,
        Self::InvalidHotEnvelope,
        Self::InvalidFamilyRequest,
        Self::InstructionsSysvarAccount,
        Self::CurrentInstructionIndexUnreadable,
        Self::PrecedingInstructionUnreadable,
        Self::ScratchPagePrivileges,
        Self::ScratchPageDecode,
        Self::ScratchPageRequestBinding,
        Self::ScratchPageOrder,
        Self::ScratchBankDigest,
        Self::ScratchBankLength,
        Self::HeapCeilingNotLifted,
        Self::OutputPageUnwritable,
        Self::OutputPageTooNarrow,
        Self::OutputPageAliasesFrame,
    ];

    /// This refusal's position in [`GeneralAcceleratorSbfErrorV3::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism:
    /// a sixth variant is a COMPILE ERROR here rather than a discriminant no
    /// assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::InvalidRequest => 0,
            Self::InvalidFrame => 1,
            Self::InvalidTopLevelInstruction => 2,
            Self::InvalidScratchBank => 3,
            Self::InvalidAcknowledgement => 4,
            Self::ForeignInstructionBeforeTrading => 5,
            Self::HeapFrameNotRequested => 6,
            Self::TopLevelProgramNotTrading => 7,
            Self::InvalidHotEnvelope => 8,
            Self::InvalidFamilyRequest => 9,
            Self::InstructionsSysvarAccount => 10,
            Self::CurrentInstructionIndexUnreadable => 11,
            Self::PrecedingInstructionUnreadable => 12,
            Self::ScratchPagePrivileges => 13,
            Self::ScratchPageDecode => 14,
            Self::ScratchPageRequestBinding => 15,
            Self::ScratchPageOrder => 16,
            Self::ScratchBankDigest => 17,
            Self::ScratchBankLength => 18,
            Self::HeapCeilingNotLifted => 19,
            Self::OutputPageUnwritable => 20,
            Self::OutputPageTooNarrow => 21,
            Self::OutputPageAliasesFrame => 22,
        }
    }
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
//
// WHY THIS IS A LIST AND NOT TWO ENDPOINTS. The ceiling assertion used to name
// one variant BY HAND as "the last one". A hand-named ceiling says nothing
// about the variants after it and goes stale silently every single time the
// enum grows -- the failure is not that the name is wrong, it is that nothing
// can notice. Claims proved it the expensive way: its bound went on naming
// `ReleaseSuperseded` after a later variant landed, so for as long as that
// stood, the newest refusal in the program was checked by nothing.
//
// So the band is now checked over `ALL`, element by element, and `ALL` is
// welded to the enum by the exhaustive `ordinal` match. A new variant cannot
// join quietly: it does not compile until its author answers for it, and the
// answer they must give is its index here.
const _: () = {
    assert!(
        GeneralAcceleratorSbfErrorV3::ALL[0] as u32
            == dclutch_refusal_registry::GENERAL_ACCELERATOR_REFUSAL_BASE,
        "GeneralAcceleratorSbfErrorV3 must start at its registered refusal band base"
    );
    let mut index = 0;
    while index < GeneralAcceleratorSbfErrorV3::ALL.len() {
        let variant = GeneralAcceleratorSbfErrorV3::ALL[index];
        assert!(
            variant.ordinal() == index,
            "GeneralAcceleratorSbfErrorV3::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32
                == dclutch_refusal_registry::GENERAL_ACCELERATOR_REFUSAL_BASE + index as u32,
            "GeneralAcceleratorSbfErrorV3 discriminants are not the contiguous run from the band base that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::GENERAL_ACCELERATOR_REFUSAL_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "GeneralAcceleratorSbfErrorV3 must not run past its registered refusal band"
        );
        index += 1;
    }
};

impl From<GeneralAcceleratorSbfErrorV3> for ProgramError {
    fn from(value: GeneralAcceleratorSbfErrorV3) -> Self {
        Self::Custom(value as u32)
    }
}

/// The requested frame in the allocator's unit.
///
/// `ComputeBudgetInstruction::request_heap_frame` takes a `u32` and a heap
/// ceiling is a `usize`; on every target this program is built for that is a
/// widening.
#[cfg(all(
    target_os = "solana",
    feature = "custom-heap",
    not(feature = "no-entrypoint")
))]
#[allow(clippy::cast_possible_truncation)]
const DIRECT_HOT_HEAP_FRAME_BYTES_V1_USIZE: usize = DIRECT_HOT_HEAP_FRAME_BYTES_V1 as usize;

// What makes `HeapCeilingNotLifted` unreachable rather than merely unlikely.
// `lift_ceiling` refuses a value below the protocol default, above the runtime
// maximum, or off the 1,024-byte granularity the runtime sanitizes requests to,
// and the only value this program ever offers it is the constant below. These
// are the three conjuncts, checked where the constant is, so a later edit to
// the frame size is a compile error rather than a refusal in production.
#[cfg(all(
    target_os = "solana",
    feature = "custom-heap",
    not(feature = "no-entrypoint")
))]
const _: () = {
    assert!(
        DIRECT_HOT_HEAP_FRAME_BYTES_V1_USIZE >= dclutch_sbf_bump_heap::DEFAULT_HEAP_BYTES_V1,
        "the declared heap frame is below the protocol default the allocator starts at"
    );
    assert!(
        DIRECT_HOT_HEAP_FRAME_BYTES_V1_USIZE <= dclutch_sbf_bump_heap::MAX_HEAP_BYTES_V1,
        "the declared heap frame is above the largest frame the runtime will grant"
    );
    assert!(
        DIRECT_HOT_HEAP_FRAME_BYTES_V1_USIZE
            .is_multiple_of(dclutch_sbf_bump_heap::HEAP_FRAME_GRANULARITY_BYTES_V1),
        "the declared heap frame is not on the granularity the runtime sanitizes requests to"
    );
};

/// Log the program allocator's outstanding bytes against the ceiling it enforces.
///
/// WHICH QUESTION THIS ANSWERS, because a heap probe answers only one. It does
/// NOT measure the granted heap frame -- only a raw write into the heap region
/// does that, and this crate forbids `unsafe`. It measures what THIS PROGRAM'S
/// ALLOCATOR has handed out and the ceiling that allocator will refuse past,
/// which is exactly the pair that decides whether an allocation fails.
///
/// It used to answer it by ALLOCATING a one-byte probe and reading its address
/// against the SDK allocator's hardcoded `HEAP_LENGTH`. Two things were wrong
/// with that once this program installed its own allocator: the ceiling it
/// printed was a constant rather than the lifted one, and the instrument moved
/// what it measured. Both readings now come from the allocator itself, which
/// keeps them in its heap header and allocates nothing to report them.
///
/// The three logged words are: bytes outstanding, the ceiling actually
/// enforced, and the frame the transport requests. Before 2026-09-01 the middle
/// word was always 32,768 no matter what the transaction paid for; after the
/// lift it is 65,536, and the gap between the second and third is what the
/// program was throwing away.
#[cfg(feature = "heap-profile")]
fn heap_mark(label: &str) {
    #[cfg(all(target_os = "solana", feature = "custom-heap"))]
    {
        sol_log(label);
        solana_program::log::sol_log_64(
            u64::try_from(PROGRAM_HEAP_V1.bytes_used()).unwrap_or(u64::MAX),
            u64::try_from(PROGRAM_HEAP_V1.bytes_capacity()).unwrap_or(u64::MAX),
            u64::from(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
            0,
            0,
        );
    }
    #[cfg(all(target_os = "solana", not(feature = "custom-heap")))]
    {
        // The SDK's allocator keeps no readable state, so the probe is the only
        // instrument available on this build -- and it measures against the
        // hardcoded ceiling, which is the point of the build.
        let probe = vec![0_u8; 1];
        let ceiling = usize::try_from(solana_program::entrypoint::HEAP_START_ADDRESS)
            .unwrap_or(0)
            .saturating_add(solana_program::entrypoint::HEAP_LENGTH);
        let used = ceiling.saturating_sub(probe.as_ptr() as usize);
        sol_log(label);
        solana_program::log::sol_log_64(
            u64::try_from(used).unwrap_or(u64::MAX),
            u64::try_from(solana_program::entrypoint::HEAP_LENGTH).unwrap_or(u64::MAX),
            u64::from(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
            0,
            0,
        );
    }
    #[cfg(not(target_os = "solana"))]
    {
        let _ = label;
    }
}

macro_rules! heap_mark {
    ($label:literal) => {
        #[cfg(feature = "heap-profile")]
        heap_mark(concat!("dclutch-general-heap:", $label));
    };
}

/// Semantic refusal after the complete physical frame has authenticated.
///
/// This is NOT a protocol-visible refusal code -- it carries no `#[repr]` and
/// is never converted to a `ProgramError`. Every value here becomes the same
/// canonical refused acknowledgement on the wire, by design, so that Trading
/// can tell a transport fault from a failure-atomic semantic refusal. Which
/// means the validator log is the only place the cause can live, and
/// [`GeneralAcceleratorSemanticErrorV3::log_line`] is how it gets there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAcceleratorSemanticErrorV3 {
    /// A runtime account coordinate was unresolvable, out of frame, or borrowed.
    RuntimeAccount,
    /// The action's readonly-evidence table carries no entry of the kind asked for.
    EvidenceCoordinate,
    /// The General config account's bytes were not a canonical config.
    ConfigDecode,
    /// The config account's digest differed from the one the input bank declares.
    ConfigIdentity,
    /// The config refused the generation or semantic basis the bank declares.
    ConfigMarket,
    /// The Product account's digest differed from the one the input bank declares.
    ProductIdentity,
    /// Action-selected state or evidence was absent, extraneous, or malformed.
    ///
    /// Narrowed the way `InvalidTopLevelInstruction` and `InvalidScratchBank`
    /// were: this used to carry the six causes above as well, so every action
    /// in the program -- all fifteen call `authenticated_general_domain` before
    /// anything else -- refused domain authentication under the same word it
    /// uses for its own state accounts. It now means the action's own selected
    /// state or evidence, and the domain says which conjunct of itself failed.
    State,
    /// The pure General transition refused its authenticated inputs.
    Transition,
    /// Candidate-bank projection refused.
    Candidate,
}

impl GeneralAcceleratorSemanticErrorV3 {
    /// The exact line this refusal writes to the validator log.
    ///
    /// A `&'static str` per variant rather than a `{:?}` format: the refusing
    /// path is inside a `no_std` program whose peak heap is already the binding
    /// constraint at runtime width 258, and `sol_log` takes a `&str` with no
    /// allocation at all. The match is exhaustive, so a tenth variant does not
    /// compile until its author says what a reader should see.
    const fn log_line(self) -> &'static str {
        match self {
            Self::RuntimeAccount => "general: refused, runtime account coordinate unreadable",
            Self::EvidenceCoordinate => "general: refused, no readonly evidence of that kind",
            Self::ConfigDecode => "general: refused, config account did not decode",
            Self::ConfigIdentity => "general: refused, config digest is not the bank's config id",
            Self::ConfigMarket => "general: refused, config rejects the bank's generation or basis",
            Self::ProductIdentity => {
                "general: refused, product digest is not the bank's product id"
            }
            Self::State => "general: refused, action state or evidence",
            Self::Transition => "general: refused, the pure transition",
            Self::Candidate => "general: refused, candidate projection",
        }
    }
}

/// This program's heap, over the frame its transaction actually grants.
///
/// `solana_program::entrypoint!` expands `custom_heap_default!`, which installs
/// the SDK's `BumpAllocator` over `HEAP_START_ADDRESS .. + HEAP_LENGTH` -- a
/// hardcoded 32,768 -- and bumps DOWN from that compile-time ceiling, so
/// `ComputeBudget::RequestHeapFrame` is inert against it. The macro elides that
/// allocator exactly when the calling crate declares a feature named
/// `custom-heap`. This crate declared it and, until 2026-09-01, implemented
/// nothing, so the program ran on 32 KiB while every transport that reaches it
/// requests 65,536: at runtime width 258 it died of an out-of-memory abort with
/// 26,515 outstanding, and an abort names nothing -- not the route, not the
/// budget, not the instruction the caller did supply.
///
/// [`dclutch_sbf_bump_heap::program_heap_v1`] is Trading's allocator, which
/// bumps UPWARD so the ceiling is a comparison rather than an origin and can be
/// raised mid-invocation. It is a shared crate rather than a second copy
/// precisely so this one can keep `#![forbid(unsafe_code)]`: the constructor is
/// safe because the address it binds is the platform's, not a caller's.
///
/// The ceiling is still the protocol default until
/// [`authenticate_top_level`] proves the request and raises it.
///
/// WHAT MAKES THIS SOUND ACROSS THE CPI BOUNDARY, and it is the one fact the
/// whole change rests on. This program is invoked by Trading, not top-level,
/// so it needs two guarantees that Trading's own adapter already states as
/// trust-surface assumptions 5 and 6
/// (`dclutch-trading-sbf/src/entrypoint_adapter.rs`): the heap region is
/// zero-filled at the start of EVERY invocation *including each CPI depth* --
/// so this allocator reads its own fresh header rather than the caller's bump
/// position -- and a validated `RequestHeapFrame(n)` maps a heap of exactly `n`
/// bytes for EVERY invocation in the transaction, so the frame the transport
/// requested is mapped under this frame too. Without the first, the header is
/// somebody else's; without the second, raising the ceiling would just move the
/// fault. Both are the runtime's, not this program's, and both are what the
/// scan in `authenticate_top_level` is entitled to conclude from.
#[cfg(all(
    target_os = "solana",
    feature = "custom-heap",
    not(feature = "no-entrypoint")
))]
#[global_allocator]
static PROGRAM_HEAP_V1: dclutch_sbf_bump_heap::BumpHeapV1 =
    dclutch_sbf_bump_heap::program_heap_v1();

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    process_instruction(program_id, accounts, instruction_data)
}

/// Evaluate one admitted chunk and publish a canonical V2 acknowledgement.
///
/// Physical authentication errors return a program error with no return data.
/// A well-formed request whose General semantics refuse returns the canonical
/// refused acknowledgement, allowing Trading to distinguish transport failure
/// from a failure-atomic semantic refusal.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    heap_mark!("entry");
    let request = AdmittedAcceleratorRequestV2::decode(instruction_data)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidRequest)?;
    // THE GEOMETRY IS VALIDATED AFTER THE ACTION IS KNOWN, and the order is the
    // point: `validate_request_geometry` decides how wide the bank this
    // instruction assembles must be, and that width is per-action. It ran one
    // line earlier until 2026-09-02, which was correct only while every action
    // declared the same stride. `authenticate_top_level` does not read
    // `request`, so the move costs nothing.
    let (family_request, controller) = authenticate_top_level(accounts)?;
    validate_request_geometry(controller.action, request)?;
    let fixed_count = usize::from(
        general_account_profile_fixed_count_v3(controller.action)
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidFrame)?,
    );
    // The runtime slice begins one account later under the output-page
    // transport, because the page is APPENDED to the frame the chunked
    // transport already had. Both coordinates come from `admitted_v3`.
    let runtime_start = admitted_runtime_accounts_start_v3(request.profile())
        .ok_or(GeneralAcceleratorSbfErrorV3::InvalidFrame)?;
    validate_frame(accounts, request, fixed_count, runtime_start)?;
    heap_mark!("frame-validated");
    let bank_len = usize::try_from(request.total_bank_bytes())
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidRequest)?;
    let input_page_start = runtime_start
        .checked_add(fixed_count)
        .ok_or(GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
    let request_digest = content(instruction_data)?;
    if let AdmittedAcceleratorRequestV2::OutputPageV3(page_request) = request {
        // The page is admitted BEFORE anything is assembled into it, so a page
        // this program cannot write costs the caller the authentication it
        // already paid and not the input walk on top of it.
        let page = admit_output_page_v3(program_id, accounts, bank_len)?;
        let ack = {
            // THE PAGE IS THE WORKING BANK. General's input arrives on Trading
            // scratch pages and its candidate replaces it in place, so on this
            // transport there is no reason for a heap copy of either -- the
            // input is assembled straight into the account, evaluated there,
            // and hashed there. That matters more here than anywhere: this
            // program's heap peak is set by exactly this bank, and its
            // allocator never frees.
            let mut data = page
                .try_borrow_mut_data()
                .map_err(|_| GeneralAcceleratorSbfErrorV3::OutputPageUnwritable)?;
            let window = data
                .get_mut(..bank_len)
                .ok_or(GeneralAcceleratorSbfErrorV3::OutputPageTooNarrow)?;
            assemble_input_bank_into(accounts, request, input_page_start, window)?;
            heap_mark!("input-bank");
            match evaluate_candidate(
                controller,
                &family_request,
                request.tail_count(),
                runtime_accounts(accounts, runtime_start, fixed_count)?,
                window,
            ) {
                Ok(()) => AcceleratorOutputPageAckV3::accepted(
                    page_request,
                    request_digest,
                    content(window)?,
                ),
                Err(cause) => {
                    sol_log(cause.log_line());
                    AcceleratorOutputPageAckV3::refused(page_request, request_digest)
                }
            }
        };
        heap_mark!("evaluated");
        let mut output = [0_u8; ACCELERATOR_OUTPUT_PAGE_ACK_BYTES_V3];
        ack.encode_into(&mut output)
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
        heap_mark!("acknowledged");
        set_return_data(&output);
        return Ok(());
    }
    let AdmittedAcceleratorRequestV2::ChunkedBankV2(chunked) = request else {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidRequest.into());
    };
    let mut candidate = vec![0_u8; bank_len];
    assemble_input_bank_into(accounts, request, input_page_start, &mut candidate)?;
    heap_mark!("input-bank");
    let evaluation = evaluate_candidate(
        controller,
        &family_request,
        request.tail_count(),
        runtime_accounts(accounts, runtime_start, fixed_count)?,
        &mut candidate,
    );
    heap_mark!("evaluated");
    let bank_digest = content(&candidate)?;
    let ack = match evaluation {
        Ok(()) => {
            let start = usize::try_from(chunked.chunk_offset())
                .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
            let remaining = candidate
                .len()
                .checked_sub(start)
                .ok_or(GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
            let payload_len = remaining
                .min(dclutch_execution_strategy_contract::v2::ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2);
            let payload = candidate
                .get(
                    start
                        ..start
                            .checked_add(payload_len)
                            .ok_or(GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?,
                )
                .ok_or(GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
            AcceleratorAckV2::accepted(chunked, request_digest, bank_digest, payload)
                .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?
        }
        Err(cause) => {
            // The wire cannot carry this distinction and should not: the
            // refused acknowledgement is one canonical shape so Trading can
            // separate a transport fault from a failure-atomic semantic
            // refusal. That makes the log the only reader, and until this line
            // there was none -- ninety-six refusal sites collapsed into three
            // variants collapsed into one `Refused` ack that named nothing.
            sol_log(cause.log_line());
            AcceleratorAckV2::refused(chunked, request_digest)
        }
    };
    let ack_len = ACCELERATOR_ACK_HEADER_BYTES_V2
        .checked_add(ack.payload().len())
        .ok_or(GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
    // A stack buffer, deliberately. The SBF bump allocator never frees, so a
    // heap acknowledgement here would sit on top of the peak the runtime-width
    // input bank already sets -- and the GEN-SEVEN register widening (+648
    // bytes across the bank) pushed exactly that peak past the 32KiB heap at
    // Close N=258. The whole acknowledgement is bounded at 1,024 bytes
    // (header 144 + chunk payload 880), which one stack frame carries with
    // room; the frame-diagnostic gate is what checks that claim on every
    // build.
    let mut output = [0_u8;
        ACCELERATOR_ACK_HEADER_BYTES_V2
            + dclutch_execution_strategy_contract::v2::ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2];
    let output = output
        .get_mut(..ack_len)
        .ok_or(GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
    ack.encode_into(output)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
    heap_mark!("acknowledged");
    set_return_data(output);
    Ok(())
}

/// Refuse a request whose geometry is not the one THIS ACTION declares.
///
/// The action is a parameter, and the call moved below `authenticate_top_level`
/// to get it. Both widths this checks are per-action now, so computing them
/// before the action was known would have pinned every request to the stride of
/// the actions that still have a per-outcome tail -- refusing `OpenBatch` and
/// `CloseBatch` outright, and doing it in the one place the accelerator decides
/// how wide the bank it is about to assemble is.
fn validate_request_geometry(
    action: Action,
    request: AdmittedAcceleratorRequestV2<'_>,
) -> ProgramResult {
    let tail_count = request.tail_count();
    if request.transport() != RequestTransportV2::ScratchPages
        || tail_count == 0
        || request.scalar_count()
            != general_hot_scalar_count_v3(action, tail_count)
                .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidRequest)?
        || request.identity_count() != GENERAL_HOT_COMMON_IDENTITIES_V3
        || usize::try_from(request.total_bank_bytes())
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidRequest)?
            != general_hot_candidate_bank_len_v3(action, tail_count)
                .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidRequest)?
        || !request.inline_bank().is_empty()
    {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidRequest.into());
    }
    Ok(())
}

fn validate_frame(
    accounts: &[AccountInfo<'_>],
    request: AdmittedAcceleratorRequestV2<'_>,
    fixed_count: usize,
    runtime_start: usize,
) -> ProgramResult {
    // Derived from the bank width, which is what the INPUT page count always
    // was. The chunked request's `chunk_count` happened to equal it because
    // input and output banks share a width; the output-page request has no
    // such field to misread.
    let pages = usize::try_from(
        request
            .input_page_count()
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidFrame)?,
    )
    .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidFrame)?;
    let expected = runtime_start
        .checked_add(fixed_count)
        .and_then(|value| value.checked_add(pages))
        .ok_or(GeneralAcceleratorSbfErrorV3::InvalidFrame)?;
    if accounts.len() != expected {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidFrame.into());
    }
    // EXACTLY ONE WRITABLE ACCOUNT, and only under the transport that has one.
    // This loop used to read "no account is writable", which was the honest
    // statement of the invariant while the accelerator owned nothing; the
    // output page is the one account it does own, and it sits at the one
    // coordinate `admitted_v3` names for it.
    let writable = match request.profile() {
        AcceleratorTransportProfileV2::OutputPageV3 => Some(ADMITTED_OUTPUT_PAGE_ACCOUNT_V3),
        AcceleratorTransportProfileV2::ChunkedBankV2
        | AcceleratorTransportProfileV2::ShadowTranscriptV3 => None,
    };
    for (index, account) in accounts.iter().enumerate() {
        if account.is_writable != (writable == Some(index))
            || account.is_signer != (index == ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3)
        {
            return Err(GeneralAcceleratorSbfErrorV3::InvalidFrame.into());
        }
    }
    let trading = account(accounts, ADMITTED_TRADING_PROGRAM_ACCOUNT_V3)?;
    if !trading.executable {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidFrame.into());
    }
    Ok(())
}

/// Raise this program's heap ceiling to the frame the transaction requested.
///
/// A no-op on any build without the custom heap -- the host test build, and the
/// build that hands the heap back to the SDK's allocator -- because there is no
/// ceiling of ours to raise there.
#[cfg(all(
    target_os = "solana",
    feature = "custom-heap",
    not(feature = "no-entrypoint")
))]
fn lift_declared_heap_frame_v1() -> ProgramResult {
    PROGRAM_HEAP_V1
        .lift_ceiling(DIRECT_HOT_HEAP_FRAME_BYTES_V1_USIZE)
        .map(|_| ())
        .map_err(|_| GeneralAcceleratorSbfErrorV3::HeapCeilingNotLifted.into())
}

/// See the `target_os = "solana"` arm: there is no ceiling of ours to raise.
#[cfg(not(all(
    target_os = "solana",
    feature = "custom-heap",
    not(feature = "no-entrypoint")
)))]
fn lift_declared_heap_frame_v1() -> ProgramResult {
    Ok(())
}

fn authenticate_top_level(
    accounts: &[AccountInfo<'_>],
) -> Result<([u8; CONTROLLER_REQUEST_BYTES_V2], GeneralDecodedRequestV3), ProgramError> {
    let instructions = account(accounts, ADMITTED_INSTRUCTIONS_ACCOUNT_V3)?;
    if instructions.key != &solana_instructions_sysvar::ID
        || instructions.owner != &sysvar::ID
        || instructions.is_writable
        || instructions.is_signer
    {
        return Err(GeneralAcceleratorSbfErrorV3::InstructionsSysvarAccount.into());
    }
    let index = load_current_index_checked(instructions)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::CurrentInstructionIndexUnreadable)?;
    // Two laws live here: the heap was REQUESTED, and the caller is Trading. The
    // POSITION was never one of them. Pinning Trading to index 1 also forbade
    // every instruction ahead of it except the heap request -- and a General
    // action needs more compute than the per-instruction default, measured at
    // 516,162 CU for OpenBatch at N=2, which takes a `set_compute_unit_limit`
    // instruction, which moves Trading to index 2. The two requirements were
    // jointly unsatisfiable, so NO transaction could execute a General action
    // through this program: with the limit instruction removed to satisfy the
    // position, the transaction is granted 202,850 CU and dies at 202,842.
    //
    // The position is the accident and it is dropped. Both laws are kept, and
    // the first is now STRICTER than the pinned index made it: every
    // instruction ahead of this one must belong to the ComputeBudget program,
    // which can only price a transaction and can neither move value nor touch
    // an account, and one of them must be the exact heap request. Every shape
    // the pinned index admitted is still admitted -- index 1 with it at zero
    // satisfies both conjuncts -- and every newly admitted shape differs from
    // one of those only by additional ComputeBudget instructions.
    let mut heap_frame_requested = false;
    let mut earlier_index = 0_u16;
    while earlier_index < index {
        let earlier = load_instruction_at_checked(usize::from(earlier_index), instructions)
            .map_err(|_| GeneralAcceleratorSbfErrorV3::PrecedingInstructionUnreadable)?;
        if earlier.program_id != compute_budget::ID {
            return Err(GeneralAcceleratorSbfErrorV3::ForeignInstructionBeforeTrading.into());
        }
        if earlier == ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1) {
            heap_frame_requested = true;
        }
        earlier_index = earlier_index
            .checked_add(1)
            .ok_or(GeneralAcceleratorSbfErrorV3::PrecedingInstructionUnreadable)?;
    }
    // A caller that never REQUESTED the heap is refused here by its own name.
    // It used to share `InvalidTopLevelInstruction` with seven other causes,
    // so the `declare_general_heap: false` hostile could not tell the heap it
    // was testing from the seven things it was not.
    if !heap_frame_requested {
        return Err(GeneralAcceleratorSbfErrorV3::HeapFrameNotRequested.into());
    }
    // AND THE REQUEST IS NOW WORTH SOMETHING. Until 2026-09-01 this scan proved
    // a request that nothing in the program could spend: the allocator's
    // ceiling was the SDK's hardcoded 32,768 and stayed there. The same
    // instruction that satisfies the conjunct above now raises this program's
    // own ceiling to what it asks for, and it happens HERE because everything
    // expensive -- `validate_frame`, `assemble_input_bank`, the transition --
    // runs after `authenticate_top_level` returns, while the decode above it
    // fits the default with room to spare.
    //
    // What this does NOT claim: that the runtime GRANTED the frame. A program
    // cannot observe its granted heap -- `create_vm!` maps exactly
    // `heap_size` bytes and every access above faults -- so this is the
    // sanitized REQUEST, exactly as `HeapFrameNotRequested` says. A request the
    // runtime accepted and did not apply still faults rather than refusing;
    // that is a platform property, not something this line can fix.
    lift_declared_heap_frame_v1()?;
    let instruction = load_instruction_at_checked(usize::from(index), instructions)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidTopLevelInstruction)?;
    let trading = account(accounts, ADMITTED_TRADING_PROGRAM_ACCOUNT_V3)?;
    if instruction.program_id != *trading.key {
        return Err(GeneralAcceleratorSbfErrorV3::TopLevelProgramNotTrading.into());
    }
    let (_, family) = HotExecutionEnvelopeV3::split_instruction(&instruction.data)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidHotEnvelope)?;
    if family.len() != CONTROLLER_REQUEST_BYTES_V2 {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidFamilyRequest.into());
    }
    let request = decode_general_request_v3(family)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidFamilyRequest)?;
    let family_copy = family
        .try_into()
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidFamilyRequest)?;
    Ok((family_copy, request))
}

/// Reassemble the Trading-owned input pages into a caller-supplied bank.
///
/// The destination is a parameter because the two transports put the bank in
/// different places -- a heap vector under the chunked one, the accelerator's
/// own page under the output-page one -- and the walk that authenticates the
/// pages is the same walk either way. It was a `-> Vec<u8>` until the second
/// transport arrived; making the caller own the destination is what stops
/// there being two copies of this authentication.
fn assemble_input_bank_into(
    accounts: &[AccountInfo<'_>],
    request: AdmittedAcceleratorRequestV2<'_>,
    page_start: usize,
    output: &mut [u8],
) -> ProgramResult {
    let page_count = usize::try_from(
        request
            .input_page_count()
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?,
    )
    .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
    let trading = account(accounts, ADMITTED_TRADING_PROGRAM_ACCOUNT_V3)?;
    let trading_id = ContentId::new(trading.key.to_bytes())
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
    let bank_len = usize::try_from(request.total_bank_bytes())
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
    if output.len() != bank_len {
        return Err(GeneralAcceleratorSbfErrorV3::ScratchBankLength.into());
    }
    let mut cursor = 0_usize;
    for page_index in 0..page_count {
        let page_account = account(
            accounts,
            page_start
                .checked_add(page_index)
                .ok_or(GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?,
        )?;
        if page_account.owner != trading.key
            || page_account.is_signer
            || page_account.is_writable
            || page_account.executable
        {
            return Err(GeneralAcceleratorSbfErrorV3::ScratchPagePrivileges.into());
        }
        let data = page_account
            .try_borrow_data()
            .map_err(|_| GeneralAcceleratorSbfErrorV3::ScratchPagePrivileges)?;
        let page = AuthenticatedScratchPageV2::decode(&data)
            .map_err(|_| GeneralAcceleratorSbfErrorV3::ScratchPageDecode)?;
        page.validate_request_input(trading_id, request)
            .map_err(|_| GeneralAcceleratorSbfErrorV3::ScratchPageRequestBinding)?;
        if usize::try_from(page.chunk_index())
            .map_err(|_| GeneralAcceleratorSbfErrorV3::ScratchPageOrder)?
            != page_index
            || usize::try_from(page.chunk_offset())
                .map_err(|_| GeneralAcceleratorSbfErrorV3::ScratchPageOrder)?
                != cursor
        {
            return Err(GeneralAcceleratorSbfErrorV3::ScratchPageOrder.into());
        }
        let end = cursor
            .checked_add(page.payload().len())
            .ok_or(GeneralAcceleratorSbfErrorV3::ScratchBankLength)?;
        output
            .get_mut(cursor..end)
            .ok_or(GeneralAcceleratorSbfErrorV3::ScratchBankLength)?
            .copy_from_slice(page.payload());
        cursor = end;
    }
    if cursor != output.len() {
        return Err(GeneralAcceleratorSbfErrorV3::ScratchBankLength.into());
    }
    if content(output)? != request.input_bank_digest() {
        return Err(GeneralAcceleratorSbfErrorV3::ScratchBankDigest.into());
    }
    Ok(())
}

/// Admit the one account this program is ever handed write authority over.
///
/// Three refusals, one accusation each, and none of them is authentication:
/// common Trading refuses a page it did not build before it ever makes the CPI.
/// This is the program that would do the writing declining to do it blind --
/// the ownership it asserts is its OWN, the aliasing is the privilege union a
/// repeated key would create, and the width is a provisioning fact a growing
/// batch can reach honestly.
fn admit_output_page_v3<'a, 'info>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    bank_len: usize,
) -> Result<&'a AccountInfo<'info>, GeneralAcceleratorSbfErrorV3> {
    let page = accounts
        .get(ADMITTED_OUTPUT_PAGE_ACCOUNT_V3)
        .ok_or(GeneralAcceleratorSbfErrorV3::OutputPageUnwritable)?;
    if page.owner != program_id || !page.is_writable || page.is_signer || page.executable {
        return Err(GeneralAcceleratorSbfErrorV3::OutputPageUnwritable);
    }
    if accounts
        .iter()
        .filter(|account| account.key == page.key)
        .count()
        != 1
    {
        return Err(GeneralAcceleratorSbfErrorV3::OutputPageAliasesFrame);
    }
    if page.data_len() < bank_len {
        return Err(GeneralAcceleratorSbfErrorV3::OutputPageTooNarrow);
    }
    Ok(page)
}

fn runtime_accounts<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    runtime_start: usize,
    fixed_count: usize,
) -> Result<&'a [AccountInfo<'info>], ProgramError> {
    accounts
        .get(
            runtime_start
                ..runtime_start
                    .checked_add(fixed_count)
                    .ok_or(GeneralAcceleratorSbfErrorV3::InvalidFrame)?,
        )
        .ok_or_else(|| GeneralAcceleratorSbfErrorV3::InvalidFrame.into())
}

fn evaluate_candidate(
    request: GeneralDecodedRequestV3,
    family_request: &[u8],
    outcome_count: u32,
    runtime: &[AccountInfo<'_>],
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    if family_request
        != request
            .to_bytes()
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?
    {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let environment =
        general_hot_environment_from_bank_v3(request.action, candidate, outcome_count)
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    match request.action {
        Action::OpenBatch => {
            evaluate_open_batch(request, runtime, outcome_count, environment, candidate)
        }
        Action::CloseBatch => {
            evaluate_close_batch(request, runtime, outcome_count, environment, candidate)
        }
        Action::PlaceOrder => {
            evaluate_place_order(request, runtime, outcome_count, environment, candidate)
        }
        Action::CancelOrder => {
            evaluate_cancel_order(request, runtime, outcome_count, environment, candidate)
        }
        Action::ReleaseOrder => {
            evaluate_release_order(request, runtime, outcome_count, environment, candidate)
        }
        Action::SubmitCandidate => {
            evaluate_submit_candidate(request, runtime, outcome_count, environment, candidate)
        }
        Action::VerifyCandidateRow => {
            evaluate_verify_candidate(request, runtime, outcome_count, candidate)
        }
        Action::CloseCandidate => evaluate_close_candidate(
            request,
            family_request,
            runtime,
            outcome_count,
            environment,
            candidate,
        ),
        Action::Consider | Action::Freeze => {
            evaluate_selection(request, runtime, outcome_count, environment, candidate)
        }
        Action::InitializeSettlement => {
            evaluate_initialize(request, runtime, outcome_count, environment, candidate)
        }
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close => {
            evaluate_settlement(request, runtime, outcome_count, environment, candidate)
        }
    }
}

fn evaluate_close_candidate(
    request: GeneralDecodedRequestV3,
    family_request: &[u8],
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate_bank: &[u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let _ = authenticated_general_domain(runtime, environment)?;
    let candidate_data = data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?;
    let candidate_state = GeneralLocalStateV3::decode(&candidate_data)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if candidate_state.header().kind != GeneralLocalStateKindV3::Candidate {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let submission = GeneralCandidateV1::decode(candidate_state.body())
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let batch_data = data(
        runtime,
        evidence_coordinate(request.action, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
    )?;
    let batch_state = GeneralLocalStateV3::decode(&batch_data)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if batch_state.header().kind != GeneralLocalStateKindV3::Batch {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let batch = GeneralBatchV1::decode(batch_state.body())
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    authenticate_general_close_candidate_v3(
        family_request,
        batch,
        submission,
        outcome_count,
        environment,
        candidate_bank,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)?;
    Ok(())
}

fn evaluate_submit_candidate(
    request: GeneralDecodedRequestV3,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let (config, _) = authenticated_general_domain(runtime, environment)?;
    if !data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?.is_empty() {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let closed_batch = data(
        runtime,
        evidence_coordinate(request.action, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
    )?;
    let batch = GeneralLocalStateV3::decode(&closed_batch)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if batch.header().kind != GeneralLocalStateKindV3::Batch {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let candidate_image = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::CandidateImage,
        )?,
    )?;
    let submitted_candidate = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::SubmittedCandidate,
        )?,
    )?;
    let root_coordinate = u16::try_from(HOT_RUNTIME_ROOT_COORDINATE_V3)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let root_data = data(runtime, root_coordinate)?;
    let root_tail = root_data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(GeneralAcceleratorSemanticErrorV3::State)?;
    project_general_submit_candidate_in_place_v3(
        root_tail,
        batch.body(),
        config,
        &candidate_image,
        &submitted_candidate,
        outcome_count,
        environment,
        request.candidate_id,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evaluate_verify_candidate(
    request: GeneralDecodedRequestV3,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    candidate_bank: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let _ = authenticated_general_domain(
        runtime,
        general_hot_environment_from_bank_v3(
            Action::VerifyCandidateRow,
            candidate_bank,
            outcome_count,
        )
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?,
    )?;

    let submission_data = data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?;
    let submission_state = GeneralLocalStateV3::decode(&submission_data)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if submission_state.header().kind != GeneralLocalStateKindV3::Candidate {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let submission = GeneralCandidateV1::decode(submission_state.body())
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let verifier_len = candidate_verifier_len_v1(submission)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;

    let verifier_data = data(runtime, GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3)?;
    let verifier_state = if verifier_data.is_empty() {
        None
    } else {
        let state = GeneralLocalStateV3::decode(&verifier_data)
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
        if state.header().kind != GeneralLocalStateKindV3::Verifier {
            return Err(GeneralAcceleratorSemanticErrorV3::State);
        }
        RuntimeCandidateVerifierV2::decode(state.body())
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
        Some(state)
    };
    let cursor_before = verifier_state
        .as_ref()
        .map_or([].as_slice(), |state| state.body());

    let result_data = data(runtime, GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3)?;
    if !result_data.is_empty() {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let verified_before: &[u8] = &[];

    let batch_data = data(
        runtime,
        evidence_coordinate(request.action, GeneralReadonlyEvidenceKindV3::ClosedBatch)?,
    )?;
    let batch_state = GeneralLocalStateV3::decode(&batch_data)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if batch_state.header().kind != GeneralLocalStateKindV3::Batch {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let batch = GeneralBatchV1::decode(batch_state.body())
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let candidate_data = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::CandidateImage,
        )?,
    )?;
    let page_data = data(
        runtime,
        evidence_coordinate(request.action, GeneralReadonlyEvidenceKindV3::CandidatePage)?,
    )?;
    let order_data = data(
        runtime,
        evidence_coordinate(request.action, GeneralReadonlyEvidenceKindV3::EscrowedOrder)?,
    )?;
    let order_state = GeneralLocalStateV3::decode(&order_data)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if order_state.header().kind != GeneralLocalStateKindV3::Order {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let manifest_data = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::SettlementManifest,
        )?,
    )?;
    let view = CandidateVerifyRowViewV1 {
        batch,
        submission,
        candidate: &candidate_data,
        page: &page_data,
        order: order_state.body(),
        cursor_before,
        verified_before,
        expected_page_index: request.page_index,
        expected_row_index: u32::from(request.execution_index),
        expected_revision: request.expected_revision,
    };
    let mut cursor_workspace = vec![0_u8; verifier_len];
    project_general_verify_candidate_workspace_v3(
        view,
        outcome_count,
        candidate_bank,
        &mut cursor_workspace,
        &manifest_data,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)?;
    Ok(())
}

fn evaluate_release_order(
    request: GeneralDecodedRequestV3,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let (config, _) = authenticated_general_domain(runtime, environment)?;
    let primary = data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?;
    let order = GeneralLocalStateV3::decode(&primary)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if order.header().kind != GeneralLocalStateKindV3::Order {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let root_coordinate = u16::try_from(HOT_RUNTIME_ROOT_COORDINATE_V3)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let root_data = data(runtime, root_coordinate)?;
    let root_tail = root_data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(GeneralAcceleratorSemanticErrorV3::State)?;
    project_general_release_order_candidate_in_place_v3(
        root_tail,
        order.body(),
        config,
        outcome_count,
        environment,
        request.candidate_id,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evaluate_cancel_order(
    request: GeneralDecodedRequestV3,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let (config, _) = authenticated_general_domain(runtime, environment)?;
    let primary = data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?;
    let batch = GeneralLocalStateV3::decode(&primary)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let terminal = data(runtime, GENERAL_TERMINAL_STATE_ACCOUNT_V3)?;
    let order = GeneralLocalStateV3::decode(&terminal)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if batch.header().kind != GeneralLocalStateKindV3::Batch
        || order.header().kind != GeneralLocalStateKindV3::Order
    {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let root_coordinate = u16::try_from(HOT_RUNTIME_ROOT_COORDINATE_V3)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let root_data = data(runtime, root_coordinate)?;
    let root_tail = root_data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(GeneralAcceleratorSemanticErrorV3::State)?;
    project_general_cancel_order_candidate_in_place_v3(
        root_tail,
        batch.body(),
        order.body(),
        config,
        outcome_count,
        environment,
        request.candidate_id,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evaluate_place_order(
    request: GeneralDecodedRequestV3,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let (config, _) = authenticated_general_domain(runtime, environment)?;
    let primary = data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?;
    let state = GeneralLocalStateV3::decode(&primary)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if state.header().kind != GeneralLocalStateKindV3::Batch
        || !data(runtime, GENERAL_TERMINAL_STATE_ACCOUNT_V3)?.is_empty()
    {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let root_coordinate = u16::try_from(HOT_RUNTIME_ROOT_COORDINATE_V3)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let root_data = data(runtime, root_coordinate)?;
    let root_tail = root_data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(GeneralAcceleratorSemanticErrorV3::State)?;
    let signed_order_terms = data(
        runtime,
        evidence_coordinate(request.action, GeneralReadonlyEvidenceKindV3::OrderTerms)?,
    )?;
    project_general_place_order_candidate_in_place_v3(
        root_tail,
        state.body(),
        config,
        outcome_count,
        environment,
        request.candidate_id,
        &signed_order_terms,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evaluate_close_batch(
    request: GeneralDecodedRequestV3,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let (config, _) = authenticated_general_domain(runtime, environment)?;
    let primary = data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?;
    let state = GeneralLocalStateV3::decode(&primary)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if state.header().kind != GeneralLocalStateKindV3::Batch {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let root_coordinate = u16::try_from(HOT_RUNTIME_ROOT_COORDINATE_V3)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let root_data = data(runtime, root_coordinate)?;
    let root_tail = root_data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(GeneralAcceleratorSemanticErrorV3::State)?;
    project_general_close_batch_candidate_in_place_v3(
        root_tail,
        state.body(),
        config,
        outcome_count,
        environment,
        request.expected_revision,
        request.candidate_id,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evaluate_open_batch(
    request: GeneralDecodedRequestV3,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let (config, _) = authenticated_general_domain(runtime, environment)?;
    if !data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?.is_empty() {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let root_coordinate = u16::try_from(HOT_RUNTIME_ROOT_COORDINATE_V3)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let root_data = data(runtime, root_coordinate)?;
    let root_tail = root_data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(GeneralAcceleratorSemanticErrorV3::State)?;
    project_general_open_batch_candidate_in_place_v3(
        root_tail,
        config,
        outcome_count,
        environment,
        request.expected_revision,
        request.candidate_id,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evaluate_selection(
    request: GeneralDecodedRequestV3,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let (config, product_record_digest) = authenticated_general_domain(runtime, environment)?;
    let primary = data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?;
    let vacant = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let before = if primary.is_empty() {
        &vacant[..]
    } else {
        let state = GeneralLocalStateV3::decode(&primary)
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
        if state.header().kind != GeneralLocalStateKindV3::Selection {
            return Err(GeneralAcceleratorSemanticErrorV3::State);
        }
        state.body()
    };
    let mut scratch = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut output = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    match request.action {
        Action::Consider => {
            let policy_coordinate = evidence_coordinate(
                request.action,
                GeneralReadonlyEvidenceKindV3::SelectionPolicy,
            )?;
            let verified_coordinate = evidence_coordinate(
                request.action,
                GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate,
            )?;
            let policy_data = data(runtime, policy_coordinate)?;
            let verified_data = data(runtime, verified_coordinate)?;
            let policy = SelectionPolicyV1::decode(&policy_data)
                .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
            let verified = VerifiedCandidateV2::decode(&verified_data)
                .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
            if policy.policy_id != config.selection_policy_id()
                || request.candidate_id != Some(verified.header().candidate_id)
                || request.page_index != verified.header().candidate_coordinate
                || verified.header().outcome_count != outcome_count
                || verified.header().product_id != product_record_digest
                || verified.header().price_scale != config.price_scale()
            {
                return Err(GeneralAcceleratorSemanticErrorV3::State);
            }
            consider_verified_candidate_v2(
                policy,
                before,
                &verified_data,
                request.expected_revision,
                &mut scratch,
                &mut output,
            )
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::Transition)?;
        }
        Action::Freeze => {
            let selected = RuntimeSelectionCursorV2::decode(before)
                .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
            let header = selected.header();
            if header.outcome_count != outcome_count
                || header.policy_id != config.selection_policy_id()
                || header.product_id != product_record_digest
                || header.price_scale != config.price_scale()
            {
                return Err(GeneralAcceleratorSemanticErrorV3::State);
            }
            freeze_selection_v2(before, request.expected_revision, &mut scratch, &mut output)
                .map_err(|_| GeneralAcceleratorSemanticErrorV3::Transition)?
        }
        _ => return Err(GeneralAcceleratorSemanticErrorV3::State),
    }
    project_general_selection_candidate_in_place_v3(
        request.action,
        &output,
        outcome_count,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evaluate_initialize(
    request: GeneralDecodedRequestV3,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let (config, product_record_digest) = authenticated_general_domain(runtime, environment)?;
    if !data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?.is_empty() {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let frozen = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::FrozenSelection,
        )?,
    )?;
    let verifier = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::RuntimeVerifier,
        )?,
    )?;
    let verified = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
        )?,
    )?;
    authenticate_frozen_selection_v3(
        config.selection_policy_id(),
        product_record_digest,
        config.price_scale(),
        request.candidate_id,
        outcome_count,
        &frozen,
        &verified,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let cursor_bytes = settlement_cursor_len(outcome_count)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let mut cursor_output = vec![0_u8; cursor_bytes];
    initialize_runtime_settlement_in_place_v2(
        &verifier,
        &verified,
        request.expected_revision,
        &mut cursor_output,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Transition)?;
    project_general_initialize_candidate_in_place_v3(
        &cursor_output,
        outcome_count,
        environment,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evaluate_settlement(
    request: GeneralDecodedRequestV3,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let (config, product_record_digest) = authenticated_general_domain(runtime, environment)?;
    let primary = data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?;
    let state = GeneralLocalStateV3::decode(&primary)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if state.header().kind != GeneralLocalStateKindV3::Settlement {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let verified = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
        )?,
    )?;
    let verified_value = VerifiedCandidateV2::decode(&verified)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if request.candidate_id != Some(verified_value.header().candidate_id)
        || verified_value.header().outcome_count != outcome_count
        || verified_value.header().product_id != product_record_digest
        || verified_value.header().price_scale != config.price_scale()
    {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let action = match request.action {
        Action::Collect => RuntimeSettlementActionV2::Collect,
        Action::Materialize => RuntimeSettlementActionV2::Materialize,
        Action::Distribute => RuntimeSettlementActionV2::Distribute,
        Action::Close => RuntimeSettlementActionV2::Close,
        _ => return Err(GeneralAcceleratorSemanticErrorV3::State),
    };
    let manifest_data = match action {
        RuntimeSettlementActionV2::Collect | RuntimeSettlementActionV2::Distribute => Some(data(
            runtime,
            evidence_coordinate(
                request.action,
                GeneralReadonlyEvidenceKindV3::SettlementManifest,
            )?,
        )?),
        RuntimeSettlementActionV2::Materialize | RuntimeSettlementActionV2::Close => None,
    };
    if let Some(manifest_bytes) = manifest_data.as_deref() {
        let manifest = SettlementManifestV2::decode(manifest_bytes)
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
        let selected = manifest
            .order(u32::from(request.manifest_order_index))
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
        if selected.header().source_page_index != request.page_index
            || selected.header().source_execution_index != u32::from(request.execution_index)
        {
            return Err(GeneralAcceleratorSemanticErrorV3::State);
        }
    }
    let cursor_bytes = state.body().len();
    let effect_bytes = runtime_settlement_effect_len_v2(outcome_count)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let inventory_bytes = usize::try_from(outcome_count)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?
        .checked_mul(8)
        .ok_or(GeneralAcceleratorSemanticErrorV3::State)?;
    let mut cursor_workspace = vec![0_u8; cursor_bytes];
    let mut inventory_workspace = vec![0_u8; inventory_bytes];
    let mut effect_workspace = vec![0_u8; effect_bytes];
    evaluate_runtime_settlement_in_place_v2(
        RuntimeSettlementViewV2 {
            action,
            cursor_before: state.body(),
            verified: &verified,
            manifest: manifest_data.as_deref(),
            manifest_order_index: u32::from(request.manifest_order_index),
            expected_revision: request.expected_revision,
            surplus_beneficiary: if action == RuntimeSettlementActionV2::Close {
                Some(config.quote_surplus_beneficiary())
            } else {
                None
            },
        },
        &mut cursor_workspace,
        &mut inventory_workspace,
        &mut effect_workspace,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Transition)?;
    project_general_hot_candidate_in_place_v3(
        request.action,
        &effect_workspace,
        &cursor_workspace,
        outcome_count,
        environment,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn authenticated_general_domain(
    runtime: &[AccountInfo<'_>],
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
) -> Result<(GeneralConfigV3, [u8; 32]), GeneralAcceleratorSemanticErrorV3> {
    let config_coordinate = u16::try_from(HOT_RUNTIME_CONFIG_COORDINATE_V3)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::RuntimeAccount)?;
    let config_data = data(runtime, config_coordinate)?;
    let config = GeneralConfigV3::decode(&config_data)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::ConfigDecode)?;
    let config_id = hash(&config_data).to_bytes();
    if config_id != environment.general_config_id {
        return Err(GeneralAcceleratorSemanticErrorV3::ConfigIdentity);
    }
    if config
        .require_market(environment.generation, environment.semantic_basis_id)
        .is_err()
    {
        // `require_market` stays the sole authority for the comparison -- it is
        // called, not reimplemented, because a second copy of a guard is how
        // the two stop agreeing. What is added is the READER: the code alone
        // says "generation or basis", which is two conjuncts and no values, and
        // a log that names a disagreement without printing either side sends
        // whoever reads it back to the fixture to guess.
        sol_log("general: config/bank generation (config, bank)");
        solana_program::log::sol_log_64(config.generation(), environment.generation, 0, 0, 0);
        sol_log("general: config/bank semantic basis (config, bank)");
        solana_program::log::sol_log_data(&[
            &config.claim_basis_id(),
            &environment.semantic_basis_id,
        ]);
        return Err(GeneralAcceleratorSemanticErrorV3::ConfigMarket);
    }
    let product_coordinate = u16::try_from(HOT_RUNTIME_PRODUCT_COORDINATE_V3)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::RuntimeAccount)?;
    let product_data = data(runtime, product_coordinate)?;
    let product_record_digest = hash(&product_data).to_bytes();
    if product_record_digest != environment.product_record_digest {
        return Err(GeneralAcceleratorSemanticErrorV3::ProductIdentity);
    }
    Ok((config, product_record_digest))
}

fn evidence_coordinate(
    action: Action,
    kind: GeneralReadonlyEvidenceKindV3,
) -> Result<u16, GeneralAcceleratorSemanticErrorV3> {
    let mut index = 0_u16;
    while index < general_readonly_evidence_count_v3(action) {
        let evidence = general_readonly_evidence_v3(action, index)
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::EvidenceCoordinate)?;
        if evidence.kind == kind {
            return Ok(evidence.coordinate);
        }
        index = index
            .checked_add(1)
            .ok_or(GeneralAcceleratorSemanticErrorV3::EvidenceCoordinate)?;
    }
    Err(GeneralAcceleratorSemanticErrorV3::EvidenceCoordinate)
}

fn data<'a>(
    runtime: &'a [AccountInfo<'_>],
    coordinate: u16,
) -> Result<core::cell::Ref<'a, [u8]>, GeneralAcceleratorSemanticErrorV3> {
    let borrowed = runtime
        .get(usize::from(coordinate))
        .ok_or(GeneralAcceleratorSemanticErrorV3::RuntimeAccount)?
        .try_borrow_data()
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::RuntimeAccount)?;
    Ok(core::cell::Ref::map(borrowed, |value| &**value))
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| GeneralAcceleratorSbfErrorV3::InvalidFrame.into())
}

fn content(bytes: &[u8]) -> Result<ContentId, ProgramError> {
    ContentId::new(hash(bytes).to_bytes())
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement.into())
}

#[cfg(test)]
mod tests {
    use dclutch_execution_strategy_contract::v2::AcceleratorRequestV2;

    use super::*;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero test identity")
    }

    fn request_for(
        action: Action,
        outcome_count: u32,
        transport: RequestTransportV2,
    ) -> AdmittedAcceleratorRequestV2<'static> {
        AcceleratorRequestV2::new(
            transport,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            outcome_count,
            general_hot_scalar_count_v3(action, outcome_count).expect("scalar count"),
            GENERAL_HOT_COMMON_IDENTITIES_V3,
            0,
            match transport {
                RequestTransportV2::Inline => {
                    let bytes = general_hot_candidate_bank_len_v3(Action::Consider, outcome_count)
                        .expect("bank bytes");
                    alloc::boxed::Box::leak(vec![0; bytes].into_boxed_slice())
                }
                RequestTransportV2::ScratchPages => &[],
            },
        )
        .map(AdmittedAcceleratorRequestV2::ChunkedBankV2)
        .expect("request")
    }

    #[test]
    fn scratch_geometry_accepts_product_widths_one_and_258() {
        for outcome_count in [1_u32, 258] {
            let request = request_for(
                Action::Consider,
                outcome_count,
                RequestTransportV2::ScratchPages,
            );
            validate_request_geometry(Action::Consider, request)
                .expect("runtime-width scratch transport");
            assert_eq!(
                usize::try_from(request.total_bank_bytes()).expect("bank bytes"),
                general_hot_candidate_bank_len_v3(Action::Consider, outcome_count)
                    .expect("General bank")
            );
            assert!(request.input_page_count().expect("page count") > 1);
        }
    }

    /// A REQUEST BUILT FOR ANOTHER ACTION'S STRIDE REFUSES, AND THE GATE HAS TO
    /// KNOW THE ACTION TO SAY SO.
    ///
    /// This is why `validate_request_geometry` moved below
    /// `authenticate_top_level`. It decides the width of the bank this
    /// instruction is about to assemble, and that width is per-action since
    /// `OpenBatch` and `CloseBatch` stopped declaring a per-outcome tail. Run
    /// before the action was known it could only have pinned one stride, which
    /// would have refused those two outright.
    ///
    /// Both directions, each with the matching-action control beside it, so a
    /// gate that accepted everything and a gate that refused everything both
    /// fail.
    #[test]
    fn a_request_built_for_another_actions_stride_refuses() {
        for (built_for, read_as) in [
            (Action::Consider, Action::OpenBatch),
            (Action::OpenBatch, Action::Consider),
        ] {
            let request = request_for(built_for, 258, RequestTransportV2::ScratchPages);
            validate_request_geometry(built_for, request)
                .expect("the action it was built for accepts it");
            assert_eq!(
                validate_request_geometry(read_as, request),
                Err(GeneralAcceleratorSbfErrorV3::InvalidRequest.into()),
                "a {built_for:?} request validated as {read_as:?} must refuse"
            );
        }
    }

    #[test]
    fn inline_and_zero_width_requests_refuse() {
        assert_eq!(
            validate_request_geometry(
                Action::Consider,
                request_for(Action::Consider, 1, RequestTransportV2::Inline)
            ),
            Err(GeneralAcceleratorSbfErrorV3::InvalidRequest.into())
        );
        let zero = AcceleratorRequestV2::new(
            RequestTransportV2::ScratchPages,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            0,
            87,
            GENERAL_HOT_COMMON_IDENTITIES_V3,
            0,
            &[],
        )
        .expect("transport permits syntactic zero");
        assert_eq!(
            validate_request_geometry(
                Action::Consider,
                AdmittedAcceleratorRequestV2::ChunkedBankV2(zero)
            ),
            Err(GeneralAcceleratorSbfErrorV3::InvalidRequest.into())
        );
    }
}
