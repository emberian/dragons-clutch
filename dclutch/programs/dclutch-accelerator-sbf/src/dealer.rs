//! Readonly stateless admitted-AOT accelerator for Dealer execution.
//!
//! Common Trading authenticates the release, action descriptor, Product,
//! execution artifacts, exact request, Profile13 account expansion, and input
//! register bank. This program independently rejoins every Dealer semantic
//! account through that public view, evaluates the selected LP-lifecycle or
//! junior-equity transition, and returns one canonical candidate-bank
//! chunk. It never writes an account, invokes a child, or owns protocol state.

use alloc::vec;

use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_HEADER_BYTES_V2, ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2,
    ACCELERATOR_OUTPUT_PAGE_ACK_BYTES_V3, AcceleratorAckV2, AcceleratorOutputPageAckV3,
    AdmittedAcceleratorRequestV2,
};
use dclutch_trading_sbf::{
    TradingSbfError,
    dealer::{
        equity_request::DEALER_EQUITY_REQUEST_MAGIC_V3,
        lp_request::DEALER_MULTI_LP_REQUEST_MAGIC_V3,
        equity_accelerator::{
            DealerEquityAcceleratorErrorV4, evaluate_authenticated_dealer_equity_v4,
        },
        lp_accelerator::{
            DealerLpAcceleratorErrorV4, evaluate_authenticated_dealer_lp_v4,
        },
    },
    hot_v3::{AuthenticatedAcceleratorInvocationV4, authenticate_accelerator_invocation_v4},
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};

/// Stable physical refusal from the Dealer accelerator boundary.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerAcceleratorSbfErrorV4 {
    /// AcceleratorRequestV2 transport or candidate-bank width differed.
    InvalidRequest = 0xC100,
    /// Common Trading could not authenticate the release/artifact/runtime view.
    InvalidInvocation = 0xC101,
    /// A canonical acknowledgement could not be constructed.
    InvalidAcknowledgement = 0xC102,
    /// The account frame or request transport this callback was handed.
    ///
    /// One of four conjuncts split out of [`Self::InvalidInvocation`], which
    /// published a single code for everything common Trading authenticates on
    /// behalf of this program. Measured 2026-09-02: the honest Dealer equity
    /// Add and a hostile Position substitution both refused `0xC101` here, so
    /// the campaign's hostile assertion had no code it could name that the
    /// honest route did not also produce -- a universal donor manufactured by
    /// a `map_err` that discarded a cause the callee had already computed.
    InvalidFrame = 0xC103,
    /// The release waist this callback rejoined: Market or Rent.
    InvalidRelease = 0xC104,
    /// The Registry records, selected descriptor or AdmittedAot strategy.
    InvalidArtifact = 0xC105,
    /// The AccountProfile-derived runtime view a candidate is computed against:
    /// tail width, span widths, logical account count, geometry, transcript.
    InvalidRuntimeView = 0xC106,
    /// The transaction's granted heap frame could not be admitted.
    ///
    /// Its own accusation, not a frame or a runtime view: the caller granted a
    /// frame this program could not lift its allocator's ceiling to, or the
    /// account at the Instructions sysvar coordinate is not that sysvar. An
    /// out-of-memory ABORT names none of that -- it is `ProgramFailedToComplete`
    /// with no code at all -- which is exactly what this route produced on
    /// 2026-09-02 before the ceiling was lifted.
    HeapCeilingNotLifted = 0xC107,
    /// The output page this program was handed is not one it can write.
    ///
    /// Wrong owner, not marked writable, executable, a signer, or a data
    /// borrow this program could not take. All five are one accusation --
    /// *the page is not mine to write* -- and a program that faulted on them
    /// instead would publish `ProgramFailedToComplete` with no code at all,
    /// which is what an out-of-bounds account index already cost this route
    /// once.
    OutputPageUnwritable = 0xC108,
    /// The output page repeats another account in this CPI frame.
    ///
    /// Its own code because it is its own hazard: **CPI privileges union on a
    /// repeated key**, so a page whose key equals a read-only frame account
    /// would carry write authority into that account. Trading refuses this
    /// before the CPI; refusing it again here is not ceremony, it is the
    /// program that would DO the writing declining to be the instrument.
    OutputPageAliasesFrame = 0xC109,
    /// The candidate bank is wider than the page provisioned for it.
    ///
    /// A provisioning fact, not an authentication one, and it is the only one
    /// of the three that a correct caller can hit by growing a Product.
    OutputPageTooNarrow = 0xC10A,
}

dclutch_refusal_registry::pin_refusal_band!(
    DealerAcceleratorSbfErrorV4,
    dclutch_refusal_registry::ACCELERATOR_REFUSAL_BASE + 0x100,
    [
        InvalidRequest,
        InvalidInvocation,
        InvalidAcknowledgement,
        InvalidFrame,
        InvalidRelease,
        InvalidArtifact,
        InvalidRuntimeView,
        HeapCeilingNotLifted,
        OutputPageUnwritable,
        OutputPageAliasesFrame,
        OutputPageTooNarrow
    ]
);

/// Diagnostic-only compute checkpoint at this program's own boundaries.
///
/// The `acc-*` trail inside `authenticate_accelerator_invocation_v4` decomposes
/// the prelude; nothing decomposed what happens after it returns. These close
/// that gap: the split between the authentication that is byte-identical across
/// chunks, the family evaluation that recomputes the same bank, and the
/// slice-and-acknowledge that is the only part a chunk index changes. Never
/// enabled in a shipped artifact; the `cu-profile` feature also turns on
/// Trading's `hot-cu-profile`, because the two trails are one measurement.
#[cfg(feature = "cu-profile")]
fn cu_checkpoint(label: &str) {
    solana_program::log::sol_log(label);
    solana_program::log::sol_log_compute_units();
}

macro_rules! cu_checkpoint {
    ($label:literal) => {
        #[cfg(feature = "cu-profile")]
        cu_checkpoint(concat!("dclutch-dealer-cu:", $label));
    };
}

/// Evaluate one authenticated Dealer candidate chunk.
///
/// Physical authentication failures return a program error with no return
/// data. A fully authenticated invocation whose Dealer semantics refuse emits
/// the canonical refused acknowledgement; common Trading therefore retains
/// sole authority over effects and write-last commitment.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    lift_admitted_heap_frame_v4(accounts)?;
    cu_checkpoint!("entry");
    let request = AdmittedAcceleratorRequestV2::decode(instruction_data)
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidRequest)?;
    let bank_bytes = usize::try_from(request.total_bank_bytes())
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidRequest)?;
    let invocation = authenticate_accelerator_invocation_v4(program_id, accounts, instruction_data)
        .map_err(accelerator_invocation_refusal_v4)?;
    cu_checkpoint!("authenticated");
    // Over the request, not over the whole instruction data: the prelude
    // witness rides after it, outside the acknowledged prefix. Trading takes
    // the same prefix on the other side, so an acknowledgement digest over
    // anything else would simply not match.
    let request_digest = content(
        instruction_data
            .get(
                ..request
                    .acknowledged_prefix_len()
                    .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidRequest)?,
            )
            .ok_or(DealerAcceleratorSbfErrorV4::InvalidRequest)?,
    )?;
    match request {
        AdmittedAcceleratorRequestV2::ChunkedBankV2(chunked) => {
            let mut candidate = vec![0_u8; bank_bytes];
            let accepted = evaluate_selected_family_v4(&invocation, &mut candidate);
            cu_checkpoint!("evaluated");
            let acknowledgement = if accepted {
                let bank_digest = content(&candidate)?;
                let start = usize::try_from(chunked.chunk_offset())
                    .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
                let remaining = candidate
                    .len()
                    .checked_sub(start)
                    .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
                let payload_bytes = remaining.min(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2);
                let end = start
                    .checked_add(payload_bytes)
                    .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
                let payload = candidate
                    .get(start..end)
                    .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
                AcceleratorAckV2::accepted(chunked, request_digest, bank_digest, payload)
                    .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?
            } else {
                AcceleratorAckV2::refused(chunked, request_digest)
            };
            let output_bytes = ACCELERATOR_ACK_HEADER_BYTES_V2
                .checked_add(acknowledgement.payload().len())
                .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
            let mut output = vec![0_u8; output_bytes];
            acknowledgement
                .encode_into(&mut output)
                .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
            cu_checkpoint!("acknowledged");
            set_return_data(&output);
        }
        AdmittedAcceleratorRequestV2::OutputPageV3(page_request) => {
            // The page is admitted BEFORE the family runs, so a page this
            // program cannot write costs the caller the authentication it
            // already paid and not the evaluation on top of it.
            let page = admit_output_page_v4(program_id, accounts, &invocation, bank_bytes)?;
            cu_checkpoint!("page-admitted");
            let acknowledgement = {
                // THE EVALUATOR WRITES THE ACCOUNT, with no candidate buffer in
                // between. That is the transport: the bank is 1,392 bytes for
                // an equity Add and there is no reason for this program to hold
                // a second copy of it on a heap whose `dealloc` is a no-op.
                let mut data = page
                    .try_borrow_mut_data()
                    .map_err(|_| DealerAcceleratorSbfErrorV4::OutputPageUnwritable)?;
                let window = data
                    .get_mut(..bank_bytes)
                    .ok_or(DealerAcceleratorSbfErrorV4::OutputPageTooNarrow)?;
                if evaluate_selected_family_v4(&invocation, window) {
                    // Hashed from the account, not from what this program
                    // believes it wrote. Trading hashes the same bytes out of
                    // the same account after the CPI returns, so a digest taken
                    // over anything else would simply not match.
                    AcceleratorOutputPageAckV3::accepted(
                        page_request,
                        request_digest,
                        content(window)?,
                    )
                } else {
                    // A refusal writes no digest, so whatever the evaluator left
                    // in the page is bytes nothing can bind. Trading refuses on
                    // the disposition before it ever reads them.
                    AcceleratorOutputPageAckV3::refused(page_request, request_digest)
                }
            };
            cu_checkpoint!("evaluated");
            let mut output = vec![0_u8; ACCELERATOR_OUTPUT_PAGE_ACK_BYTES_V3];
            acknowledgement
                .encode_into(&mut output)
                .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
            cu_checkpoint!("acknowledged");
            set_return_data(&output);
        }
    }
    Ok(())
}

/// Admit the one account this program is ever handed write authority over.
///
/// Three refusals, one accusation each, and none of them is authentication:
/// common Trading already refused a page it did not build. This is the program
/// that would do the writing declining to do it blind -- the ownership it
/// asserts is its OWN (a page owned by anyone else is not this program's to
/// write), the aliasing is the privilege union a repeated key would create, and
/// the width is a provisioning fact a growing Product can reach honestly.
fn admit_output_page_v4<'a, 'info>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, 'a, 'info>,
    bank_bytes: usize,
) -> Result<&'a AccountInfo<'info>, DealerAcceleratorSbfErrorV4> {
    let page = invocation
        .output_page()
        .ok_or(DealerAcceleratorSbfErrorV4::OutputPageUnwritable)?;
    if page.owner != program_id || !page.is_writable || page.is_signer || page.executable {
        return Err(DealerAcceleratorSbfErrorV4::OutputPageUnwritable);
    }
    if accounts
        .iter()
        .filter(|account| account.key == page.key)
        .count()
        != 1
    {
        return Err(DealerAcceleratorSbfErrorV4::OutputPageAliasesFrame);
    }
    if page.data_len() < bank_bytes {
        return Err(DealerAcceleratorSbfErrorV4::OutputPageTooNarrow);
    }
    Ok(page)
}

/// Run the one family the authenticated selection names, naming its refusal.
///
/// Lifted out of `process` unchanged when the second transport
/// arrived, because a dispatch written twice is a dispatch that can disagree
/// with itself about which family a request selects.
fn evaluate_selected_family_v4(
    invocation: &AuthenticatedAcceleratorInvocationV4<'_, '_, '_>,
    candidate: &mut [u8],
) -> bool {
    let family_magic = invocation.family_request().get(..8);
    let selected_action = invocation.selected_action();
    // Dispatch only after common Hot authenticated the top-level request,
    // ProgramSet selection and invocation context. Both magic and selected
    // action must name the same family wire; a cross-family combination is a
    // semantic refusal rather than an alternate decoder choice.
    if family_magic == Some(DEALER_MULTI_LP_REQUEST_MAGIC_V3.as_slice())
        && matches!(selected_action, 7 | 8)
    {
        accepted_or_named_v4(
            evaluate_authenticated_dealer_lp_v4(invocation, candidate)
                .map_err(DealerLpAcceleratorErrorV4::refusal_name),
        )
    } else if family_magic == Some(DEALER_EQUITY_REQUEST_MAGIC_V3.as_slice())
        && matches!(selected_action, 1..=6)
    {
        accepted_or_named_v4(
            evaluate_authenticated_dealer_equity_v4(invocation, candidate)
                .map_err(DealerEquityAcceleratorErrorV4::refusal_name),
        )
    } else {
        solana_program::log::sol_log(FAMILY_REFUSAL_LOG_PREFIX_V4);
        solana_program::log::sol_log("dispatch:NoFamily");
        false
    }
}

/// The line a reader greps for the family evaluator's own word.
///
/// Kept as a constant so the test that asserts a refusal by name and the
/// program that emits it cannot drift apart by a typo.
pub const FAMILY_REFUSAL_LOG_PREFIX_V4: &str = "dclutch-accelerator dealer refused:";

/// Whether the selected family accepted, naming its cause when it did not.
///
/// A REFUSED acknowledgement is a legitimate disposition -- common Trading
/// stays the sole effect projector and commit-last writer, so a semantic
/// refusal here is an answer, not an error -- but `.is_ok()` made every one of
/// them the same answer. The equity family alone distinguishes seven causes and
/// the boundary published none of them, so `accepted.rs` had no discriminant it
/// could assert that the honest route did not also produce. The disposition is
/// unchanged; what changes is that the cause reaches the log.
fn accepted_or_named_v4<T>(outcome: Result<T, &'static str>) -> bool {
    match outcome {
        Ok(_) => true,
        Err(name) => {
            solana_program::log::sol_log(FAMILY_REFUSAL_LOG_PREFIX_V4);
            solana_program::log::sol_log(name);
            false
        }
    }
}

/// Lift this program's allocator ceiling to the frame the transaction granted.
///
/// INSTALLING AN ALLOCATOR IS NOT LIFTING ITS CEILING, and the two were one
/// mistake here. `program_heap_v1()` starts at the protocol default -- the same
/// 32,768 the SDK's allocator enforces -- and stays there until something
/// authenticates the grant and raises it. Measured on real ELFs 2026-09-02:
/// with the runtime transcript and the tail conjunct repaired, the Dealer
/// equity Add finally reached this program's allocations and died of
/// `memory allocation failed, out of memory` after 331,928 CU, an ABORT that
/// carries no refusal code, while its transaction had granted 65,536.
///
/// The grant is not taken on the caller's word.
/// [`admitted_heap_frame_bytes_v1`] reads it back out of the Instructions
/// sysvar the RUNTIME serialized, and refuses unless the account at that
/// coordinate is the canonical sysvar -- so a frame nobody requested lifts
/// nothing, and a substituted account refuses rather than being believed.
#[cfg(all(
    target_os = "solana",
    feature = "custom-heap",
    not(feature = "no-entrypoint")
))]
fn lift_admitted_heap_frame_v4(accounts: &[AccountInfo<'_>]) -> ProgramResult {
    use dclutch_capability_program_contract::hot_v3::HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3;
    use dclutch_trading_sbf::{
        admitted_composition_v3::ADMITTED_ACCELERATOR_HOT_FIXED_START_V4,
        entrypoint_adapter::admitted_heap_frame_bytes_v1,
    };
    let instructions = accounts
        .get(ADMITTED_ACCELERATOR_HOT_FIXED_START_V4 + HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
        .ok_or(DealerAcceleratorSbfErrorV4::HeapCeilingNotLifted)?;
    let Some(bytes) = admitted_heap_frame_bytes_v1(instructions)
        .map_err(|_| DealerAcceleratorSbfErrorV4::HeapCeilingNotLifted)?
    else {
        return Ok(());
    };
    crate::PROGRAM_HEAP_V1
        .lift_ceiling(bytes)
        .map(|_| ())
        .map_err(|_| DealerAcceleratorSbfErrorV4::HeapCeilingNotLifted.into())
}

/// See the `target_os = "solana"` arm: there is no ceiling of ours to raise.
#[cfg(not(all(
    target_os = "solana",
    feature = "custom-heap",
    not(feature = "no-entrypoint")
)))]
fn lift_admitted_heap_frame_v4(_accounts: &[AccountInfo<'_>]) -> ProgramResult {
    Ok(())
}

/// Translate common Trading's callback refusal into this program's band.
///
/// A DISCARDED CAUSE IS A SEARCH. Trading already decided which conjunct
/// refused and says so in its own band; `map_err(|_| InvalidInvocation)` threw
/// that away at the one line that knew it, and the four conjuncts arrived on
/// the wire as one code. Each of the four now has its own discriminant here,
/// derived from `TradingSbfError` rather than written as a literal, so the
/// wire carries the distinction and decision 0007 still owns both bands.
///
/// [`DealerAcceleratorSbfErrorV4::InvalidInvocation`] survives as the residual
/// for the refusals Trading raises that are ALREADY distinct in its own
/// vocabulary -- `Release`, `ReleaseSuperseded`, `Root`, `NativeSignature`,
/// `UnsupportedContent` -- and for a runtime builtin. Those keep their exact
/// inner code in the log rather than being folded into a conjunct they do not
/// belong to.
fn accelerator_invocation_refusal_v4(error: ProgramError) -> DealerAcceleratorSbfErrorV4 {
    let ProgramError::Custom(code) = error else {
        solana_program::msg!("dclutch-accelerator dealer: inner builtin ProgramError");
        return DealerAcceleratorSbfErrorV4::InvalidInvocation;
    };
    match code {
        _ if code == TradingSbfError::AcceleratorFrame as u32 => {
            DealerAcceleratorSbfErrorV4::InvalidFrame
        }
        _ if code == TradingSbfError::AcceleratorRelease as u32 => {
            DealerAcceleratorSbfErrorV4::InvalidRelease
        }
        _ if code == TradingSbfError::AcceleratorArtifact as u32 => {
            DealerAcceleratorSbfErrorV4::InvalidArtifact
        }
        _ if code == TradingSbfError::AcceleratorRuntimeView as u32 => {
            DealerAcceleratorSbfErrorV4::InvalidRuntimeView
        }
        _ => {
            solana_program::msg!("dclutch-accelerator dealer: inner refusal code");
            solana_program::log::sol_log_64(0, 0, 0, 0, u64::from(code));
            DealerAcceleratorSbfErrorV4::InvalidInvocation
        }
    }
}

fn content(bytes: &[u8]) -> Result<ContentId, DealerAcceleratorSbfErrorV4> {
    ContentId::new(hash(bytes).to_bytes())
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)
}
