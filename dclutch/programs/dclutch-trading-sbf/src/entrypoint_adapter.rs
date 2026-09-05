//! The named machine boundary: SBF loader entrypoint, input deserialization,
//! and the program heap allocator.
//!
//! This is the ONE module in the Trading executable that is exempt from the
//! workspace `unsafe_code` prohibition. Everything reachable from
//! [`crate::process_instruction`] is safe Rust; this module is the membrane
//! that turns the loader's raw input region into the safe values that
//! membrane consumes, and owns the allocator those values are measured
//! against. The exemption is granted for exactly this file, is recorded as a
//! single `#[allow(unsafe_code)]` on the `mod` declaration in `lib.rs`, and
//! covers no other path.
//!
//! # Why this module exists at all
//!
//! `solana_program::entrypoint!` builds a `Vec<AccountInfo>` on the 32 KiB SBF
//! bump heap before the program's first instruction runs. On the canonical
//! Registry-continuation Hot bundle that vector costs 3,744 bytes (78 slots x
//! 48) out of a heap the measured demand already exceeds. The SDK's own
//! `entrypoint_no_alloc!` removes exactly that vector by placing the slots in
//! the entrypoint's stack frame, but it is hard-capped at 64 accounts because
//! SBPF v0 gives every function a *static* 4,096-byte frame and 64 x 48 =
//! 3,072 is the largest round count that fits. Trading declares
//! [`crate::TRADING_MAX_INSTRUCTION_ACCOUNTS_V3`] (309), so the macro cannot be
//! adopted without regressing the bound.
//!
//! This adapter takes the SDK's stack-slot technique and adds the fallback the
//! macro lacks: up to [`ADAPTER_STACK_SLOTS_V1`] accounts are deserialized into
//! a stack-resident array and cost zero heap; beyond that the adapter falls
//! back to an exactly-sized heap buffer, so the declared bound is preserved and
//! never silently narrowed. The split point is a **measured-profile** bound
//! (see [`ADAPTER_STACK_SLOTS_V1`]), not a protocol fact.
//!
//! # What this is worth, measured
//!
//! On the canonical Registry-continuation Hot bundle (78 account slots, 65 of
//! them distinct), `--features hot-cu-profile`, `sbpf-solana-solana`, against
//! the same four sibling executables byte-for-byte:
//!
//! | checkpoint                | standard entrypoint | this adapter | reclaimed |
//! |---------------------------|--------------------:|-------------:|----------:|
//! | `start`                   |               8,425 |        4,696 |     3,729 |
//! | `root-product`            |              13,977 |       10,248 |     3,729 |
//! | `artifacts-strategy-effect` |            16,817 |       13,088 |     3,729 |
//! | `runtime-observations`    |              24,497 |       20,768 |     3,729 |
//!
//! The reclaim is flat because it is exactly one object: the entrypoint's
//! `Vec<AccountInfo>`, 78 x 48 = 3,744 bytes of allocation, against which this
//! allocator spends 16 bytes on its own header (and the profiler's probe
//! accounts for the remaining byte).
//!
//! **What is NOT reclaimed, and cannot be from here: 4,680 bytes of `Rc`
//! control blocks** - 65 distinct accounts x 2 `Rc<RefCell<..>>` x (32 + 40
//! bytes). `AccountInfo` holds its lamports and data behind `Rc<RefCell<..>>`
//! for aliasing semantics, `AccountInfo::new` calls `Rc::new` twice, and `Rc`
//! allocates. SBPF v0 offers no writable static memory to place a fixed arena
//! in, and fabricating an `RcInner` in borrowed memory would depend on an
//! `alloc` internal layout that carries no stability guarantee. Removing those
//! 4,680 bytes means moving `hot_v3` off `AccountInfo`, which is a protocol
//! change and not an adapter one.
//!
//! The compute price is +29,029 CU on that bundle (565,832 -> 594,861 for the
//! Trading program over the identical truncated run): bounds-checked cursor
//! reads, and an out-of-line allocator. See [`BumpHeapV1`] for why out of line.
//!
//! # Trust surface
//!
//! ## What the loader guarantees (assumed, not checked)
//!
//! These are properties of the Agave BPF loader's `serialize_parameters` and
//! of the SBPF memory map. They are the same assumptions
//! `solana_program_entrypoint::deserialize` makes; this adapter inherits them
//! rather than widening them.
//!
//! 1. `input` points at the base of the program input region, which the VM maps
//!    writable for exactly the serialized length. **The VM's mapping is the
//!    length authority.** The program is never told the length, so a read past
//!    the serialized end is an SBF access violation - an instruction failure
//!    with full rollback, not a read into another program's memory. This
//!    adapter therefore treats "how long is the buffer" as a runtime-enforced
//!    invariant and not a value it can validate; see
//!    `InputCursor::limit` for how the same code is bound-checked under test.
//! 2. The layout is exactly: `u64` account count; then per account either a
//!    `0xFF` non-duplicate marker followed by an account record, or a duplicate
//!    index byte followed by 7 padding bytes; then a `u64` instruction-data
//!    length, the instruction data, and the 32-byte program id.
//! 3. An account record is `is_signer: u8`, `is_writable: u8`, `executable: u8`,
//!    4 bytes the loader reserved for padding and which the *program* fills in
//!    with the original data length, `key: [u8; 32]`, `owner: [u8; 32]`,
//!    `lamports: u64`, `data_len: u64`, `data_len` bytes of data,
//!    `MAX_PERMITTED_DATA_INCREASE` bytes of realloc headroom, 8 unused bytes
//!    where the rent epoch used to live, then padding to the next multiple of
//!    [`BPF_ALIGN_OF_U128`].
//! 4. Duplicate indices are strictly less than the slot they appear in.
//! 5. The heap region begins at `HEAP_START_ADDRESS`, is zero-filled at the
//!    start of every invocation (including each CPI depth), and is at least
//!    [`ADAPTER_DEFAULT_HEAP_BYTES`] long. The SDK's own `BumpAllocator`
//!    already depends on the zero-fill: it reads its bump position out of the
//!    first heap word and treats zero as "not yet initialized".
//! 6. If the executing transaction carries a ComputeBudget `RequestHeapFrame(n)`
//!    instruction, the runtime has already validated `n` against
//!    `sanitize_requested_heap_size` and mapped a heap of exactly `n` bytes for
//!    every invocation in the transaction. A request the runtime would reject
//!    fails the transaction before this program runs at all, and a second
//!    request is a `DuplicateInstruction` transaction error. This is what makes
//!    reading the grant out of the instructions sysvar sound; see
//!    [`admitted_heap_frame_bytes_v1`].
//!
//! ## What this adapter checks anyway (fail-closed divergences from the SDK)
//!
//! The SDK's deserializer panics or reads uninitialized memory on shapes the
//! loader precludes. Panicking is *acceptable* there - a malformed input region
//! means the runtime is broken, and aborting the instruction is the correct
//! response - but this adapter returns explicit refusals instead, because a
//! refusal is cheaper to reason about and testable from the host harness:
//!
//! - account count above the destination capacity: SDK panics, adapter refuses
//!   with [`TradingSbfError::UnsupportedContent`];
//! - a duplicate index at or beyond the slot being filled: SDK reads an
//!   uninitialized `AccountInfo` and clones it (undefined behaviour), adapter
//!   refuses with [`TradingSbfError::Content`];
//! - a data length that does not fit in `u32`: SDK truncates it into the
//!   original-data-length field, adapter refuses with
//!   [`TradingSbfError::Content`];
//! - every cursor read is bounds-checked against `InputCursor::limit`, which
//!   production sets to `usize::MAX` (the VM is the authority, see assumption 1)
//!   and the adversarial tests set to the true buffer length, so the tests
//!   exercise the production arithmetic rather than a parallel path.
//!
//! ## What is deliberately identical to the SDK
//!
//! Field order, field widths, the `0xFF` duplicate marker, the 7 padding bytes
//! after a duplicate index, the write-back of `data_len` into the
//! original-data-length slot, the `data_len + MAX_PERMITTED_DATA_INCREASE + 8`
//! stride, the `align_offset(BPF_ALIGN_OF_U128)` padding computed on the
//! *offset* rather than the absolute address, and the `Rc` aliasing of a
//! duplicate (a duplicate slot clones the original's `Rc`s, so the two
//! `AccountInfo`s share one `RefCell` and the borrow checker still catches
//! aliased mutation at runtime). The differential tests assert this by running
//! the SDK's own `deserialize` and this adapter over byte-identical buffers and
//! comparing both the produced values and the mutated buffers.
//!
//! ## The heap ceiling, and what happens if a route lies
//!
//! [`BumpHeapV1`] bumps *upward* from the heap floor and compares against a
//! ceiling word it keeps in the heap header. The default ceiling is the
//! protocol's [`ADAPTER_DEFAULT_HEAP_BYTES`]. A route may lift it only through
//! [`admit_heap_frame_v1`], which does not take the route's word for anything:
//! it re-derives the grant from the instructions sysvar the runtime itself
//! built. Because the allocator bumps upward, lifting the ceiling is monotone
//! and order-independent - it invalidates no live allocation and may happen
//! after allocations have already been served.
//!
//! If that authentication were ever wrong and the ceiling were lifted above the
//! mapped heap, the failure is an SBF access violation on the first write past
//! the mapping: the instruction fails and the transaction rolls back in full.
//! Ugly, and not unsound. There is no path by which a lifted ceiling lets the
//! program write outside its own heap region.
//!
//! Policy: the Hot execution path is **not** on
//! [`declares_extended_heap_profile_v1`]'s list and keeps the 32 KiB structural
//! discipline. Adding a route to that list is the single visible act that lets
//! it off.

use core::{alloc::Layout, marker::PhantomData, mem::MaybeUninit, ptr::NonNull, slice};
use std::vec::Vec;

#[cfg(not(target_os = "solana"))]
use solana_program::instruction::Instruction;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::{BPF_ALIGN_OF_U128, MAX_PERMITTED_DATA_INCREASE},
    instruction::AccountMeta,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::TradingSbfError;

/// Read current CPI return data into a caller-owned allocation.
///
/// The SDK helper always allocates a new `Vec`. Common Hot's last child owns a
/// wire buffer whose bytes die at the CPI boundary; reusing that exact
/// allocation avoids a late, unreclaimable bump-heap allocation without
/// changing the syscall, producer, width, or receipt bytes being verified.
/// A return wider than the supplied capacity is refused rather than truncated.
pub(crate) fn get_return_data_into_v1(
    output: &mut Vec<u8>,
) -> Result<Option<Pubkey>, ProgramError> {
    let capacity = output.capacity();
    if capacity == 0 {
        return Err(TradingSbfError::Content.into());
    }
    output.clear();
    #[cfg(target_os = "solana")]
    {
        let mut producer = Pubkey::default();
        #[allow(deprecated)]
        let returned = unsafe {
            solana_program::syscalls::sol_get_return_data(
                output.as_mut_ptr(),
                u64::try_from(capacity).map_err(|_| TradingSbfError::Content)?,
                &mut producer,
            )
        };
        if returned == 0 {
            return Ok(None);
        }
        let returned = usize::try_from(returned).map_err(|_| TradingSbfError::Width)?;
        if returned > capacity {
            // THE RETURN IS WIDER THAN THE BORROWED BUFFER, which is a fact
            // about this allocation and NOT about the child. Refusing here read
            // "the checked data-defined transition refused" for a callee that
            // had just succeeded, and it cost a whole terminal redemption:
            // common Hot lends its CHILD REQUEST buffer to receive the child's
            // receipt, so the optimisation silently required
            // request_bytes >= receipt_bytes. That held at 648 against a
            // 592-byte representation receipt and stopped holding when physical
            // ABI v3 cut the terminal request to 508. Nothing was wrong with the
            // receipt; the buffer was too small, and only this function could
            // tell the difference.
            //
            // `sol_get_return_data` reports the FULL length whatever it managed
            // to copy, so the exact shortfall is known here and the fix is to
            // grow once and re-read. The optimisation still applies whenever the
            // request is the wider of the two, which is every other route.
            // `reserve` takes an amount ADDITIONAL TO THE LENGTH, and `clear`
            // above set the length to zero, so the argument is the whole width.
            // Passing the shortfall asks for a capacity the buffer already has
            // and the call does nothing at all.
            output.reserve(returned);
            let grown = output.capacity();
            let again = unsafe {
                solana_program::syscalls::sol_get_return_data(
                    output.as_mut_ptr(),
                    u64::try_from(grown).map_err(|_| TradingSbfError::Content)?,
                    &mut producer,
                )
            };
            let again = usize::try_from(again).map_err(|_| TradingSbfError::Width)?;
            if again != returned || again > grown {
                // The return data changed between two reads of the same
                // invocation, or the grown buffer still cannot hold it. Neither
                // is a width the caller chose, and both are genuinely a refusal.
                return Err(TradingSbfError::Transition.into());
            }
            // SAFETY: as below, over the grown allocation.
            unsafe { output.set_len(again) };
            return Ok(Some(producer));
        }
        // SAFETY: `sol_get_return_data` initialized exactly `returned` bytes
        // in this allocation, and the checked branch above proves that range
        // lies within its capacity.
        unsafe { output.set_len(returned) };
        Ok(Some(producer))
    }
    #[cfg(not(target_os = "solana"))]
    {
        let Some((producer, returned)) = solana_program::program::get_return_data() else {
            return Ok(None);
        };
        if returned.len() > capacity {
            return Err(TradingSbfError::Transition.into());
        }
        output.extend_from_slice(&returned);
        Ok(Some(producer))
    }
}

/// Accounts deserialized into the entrypoint's own stack frame at zero heap
/// cost.
///
/// **Measured-profile bound.** SBPF v0 (the `sbpf-solana-solana` ELF this
/// program is built for carries `e_flags = 0`) gives every function a static
/// 4,096-byte stack frame, so the slot array plus the frame's other locals
/// must fit in 4,096 bytes: `80 * size_of::<AccountInfo>()` is 3,840. The
/// lifting plan is not a larger number here - it is SBPF v2 dynamic frames,
/// after which the array can simply be
/// [`crate::TRADING_MAX_INSTRUCTION_ACCOUNTS_V3`] long and the heap fallback
/// below can be deleted. Until then, exceeding this count is not a refusal:
/// it costs the exactly-sized heap buffer the standard entrypoint always paid.
pub const ADAPTER_STACK_SLOTS_V1: usize = 80;

/// Heap bytes available to a program that requested no ComputeBudget heap frame.
///
/// The allocator's own `DEFAULT_HEAP_BYTES_V1`, re-exported under this module's
/// long-standing name rather than restated, so there is one author.
pub const ADAPTER_DEFAULT_HEAP_BYTES: usize = dclutch_sbf_runtime::DEFAULT_HEAP_BYTES_V1;

/// Largest heap frame the runtime will grant.
///
/// The allocator's own `MAX_HEAP_BYTES_V1`; see above.
pub const ADAPTER_MAX_HEAP_BYTES: usize = dclutch_sbf_runtime::MAX_HEAP_BYTES_V1;

/// Granularity the runtime requires of a ComputeBudget heap-frame request.
///
/// The allocator's own `HEAP_FRAME_GRANULARITY_BYTES_V1`; see above.
const HEAP_FRAME_GRANULARITY_BYTES: usize = dclutch_sbf_runtime::HEAP_FRAME_GRANULARITY_BYTES_V1;

/// Width of the eight-byte route magic every Trading instruction opens with.
const HOT_EXECUTION_MAGIC_BYTES_V1: usize = 8;

/// ComputeBudget program instruction discriminant for `RequestHeapFrame(u32)`.
///
/// Chain-derived: `solana_compute_budget_interface::ComputeBudgetInstruction`
/// is borsh-encoded, and `RequestHeapFrame` is its second variant.
const REQUEST_HEAP_FRAME_DISCRIMINANT: u8 = 1;

/// Bytes the bump heap reserves at its floor for the allocator's own state.
///
/// The allocator's own `HEAP_HEADER_BYTES`; the three word offsets it indexes
/// stay private to it, because nothing outside the allocator may address the
/// header. The loader's zero-fill of the heap (trust-surface assumption 5) is
/// load-bearing for all three.
const HEAP_HEADER_BYTES: usize = dclutch_sbf_runtime::HEAP_HEADER_BYTES;

/// Value the loader writes in place of an account record for a repeated account.
///
/// Chain-derived: `solana_program_entrypoint::NON_DUP_MARKER`.
const NON_DUP_MARKER: u8 = u8::MAX;

/// Padding bytes that follow a duplicate-account index.
const DUPLICATE_PADDING_BYTES: usize = 7;

/// Bytes the loader reserves after an account's data for the rent epoch it no
/// longer serializes.
const UNUSED_RENT_EPOCH_BYTES: usize = 8;

// ---------------------------------------------------------------------------
// The program heap
// ---------------------------------------------------------------------------

/// The program heap allocator, and its full documentation, now live in
/// `dclutch-sbf-runtime`.
///
/// It moved on 2026-09-01, unchanged, because a SECOND executable needs it:
/// `dclutch-general-accelerator-sbf` declared the `custom-heap` feature and
/// never implemented one, so it ran on the SDK's hardcoded 32 KiB allocator
/// while its transactions granted 65,536. That crate carries
/// `unsafe_code = "forbid"` and should keep it, so the allocator became a
/// shared adapter rather than a second copy of this file's `unsafe`.
///
/// What changed in the move and nothing else did: the two Trading refusal codes
/// the allocator used to raise directly are now a
/// `dclutch_sbf_runtime::HeapErrorV1`, mapped back to exactly the codes it
/// raised before by `heap_error_v1` below -- an allocator is not a protocol
/// surface and must not own registered discriminants. `lift_ceiling`,
/// `open_scratch`, `release_scratch` and `alloc_scratch` became `pub` so this
/// module can still reach them; their safety contracts are unchanged and are
/// stated on the items themselves.
pub use dclutch_sbf_runtime::{BumpHeapV1, HeapErrorV1};

/// Map an allocator refusal onto the Trading discriminant it always carried.
///
/// Byte-for-byte the codes this path raised before the allocator moved out:
/// a ceiling the allocator will not lift was and remains
/// `UnsupportedContent`, and a second scratch region was and remains
/// `Content`. The mapping is exhaustive, so a third allocator refusal does not
/// compile until someone decides which Trading code a reader should see.
const fn heap_error_v1(error: HeapErrorV1) -> TradingSbfError {
    match error {
        HeapErrorV1::UnsupportedCeiling => TradingSbfError::UnsupportedContent,
        HeapErrorV1::ScratchAlreadyOpen => TradingSbfError::Content,
    }
}

/// The Trading executable's program heap.
#[cfg(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
))]
#[global_allocator]
static PROGRAM_HEAP_V1: BumpHeapV1 = dclutch_sbf_runtime::program_heap_v1();

/// Bytes of program heap consumed so far, including the allocator's header.
#[cfg(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
))]
#[must_use]
pub fn program_heap_bytes_used_v1() -> usize {
    PROGRAM_HEAP_V1.bytes_used()
}

/// Bytes of program heap this invocation may consume in total.
#[cfg(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
))]
#[must_use]
pub fn program_heap_capacity_v1() -> usize {
    PROGRAM_HEAP_V1.bytes_capacity()
}

/// Refuse a route that needs the extended heap when the ceiling this adapter
/// lifted from the transaction's own sanitized `RequestHeapFrame` is still the
/// protocol default.
///
/// **The name is the whole point.** This function says the declared ceiling is
/// sanitized and above the default. It does NOT say the runtime granted it, and
/// it cannot: see "What this CANNOT tell you" below, where a request the runtime
/// accepted and did not apply faults at `0x30000fcf8` -- 776 bytes below the
/// REQUESTED ceiling -- while this check reports the extended heap present. It
/// was called `require_declared_heap_ceiling_above_default_v1` until 2026-09-01, and
/// `admitted` promised an observation no Solana program can make.
///
/// [`lift_declared_heap_profile_v1`] is best-effort BY CONSTRUCTION: a
/// transaction that declares the profile but carries no `RequestHeapFrame`, or
/// presents no instructions sysvar, keeps the protocol default and proceeds.
/// For a route whose peak is known to exceed that default, "proceeds" means it
/// allocates until it dies, and an out-of-memory abort names nothing at all --
/// not the route, not the budget, not the one instruction the caller left out.
/// That is the worst thing to hand the first person who integrates.
///
/// So the route that needs the frame asks here instead, and a caller who
/// forgot is told exactly what to add. This is a question about THIS
/// invocation's declared ceiling, not about the declaration list: a route can be
/// on the list and still arrive without a request, which is precisely the case
/// worth naming.
///
/// # What this CANNOT tell you, and the fault that proves it
///
/// It reads the ceiling this adapter lifted, and that ceiling came from the
/// `RequestHeapFrame` in the instructions sysvar. **That is what the transaction
/// ASKED FOR, never what the runtime GAVE**, and a Solana program cannot observe
/// the difference: `create_vm!` maps exactly
/// `invoke_context.get_compute_budget().heap_size` bytes
/// (`solana-program-runtime/src/vm.rs`, `heap.as_slice_mut().get_mut(..heap_size)`),
/// and every read or write above that is an access violation -- so there is no
/// probe, in either direction, that survives being wrong. Both sides of the
/// comparison here move together with the request.
///
/// The doctrine above this module says a request the runtime would reject fails
/// the transaction before this program runs. That covers a request the runtime
/// REJECTS. It does not cover a request the runtime never applied: such a
/// transaction runs, with the protocol default mapped, while this function
/// reports the extended ceiling as present.
///
/// The consequence is not a refusal a caller can handle. The scratch half bumps
/// DOWN from the ceiling, so its first block is written near the top of the
/// requested region -- above the mapped one -- and the Structured lane measured
/// exactly that on an ADMITTED, non-hostile path, at 203,408 CU inside a
/// Token-2022 `PermissionedBurnExtension` CPI:
///
/// ```text
/// request  65,536 -> Access violation writing 8 bytes at 0x30000fcf8
/// request 262,144 -> Access violation writing 8 bytes at 0x30003fcf8
/// ```
///
/// Both are exactly 776 bytes below the REQUESTED ceiling, and the address
/// tracks the request, so raising the request only moves the write further out.
///
/// MEASURED, so the mechanism itself is not broken: this harness does honour a
/// well-formed request. `direct_hot_top_level` passes on a route documented to
/// exhaust 32 KiB in finalization, carrying `DIRECT_HOT_HEAP_FRAME_BYTES_V1`
/// (65,536). What the two faults show is the case where the ceiling and the
/// mapping DISAGREE, which this function is structurally unable to detect.
///
/// Naming it `admitted` promised more than the platform can deliver, so as of
/// 2026-09-01 it does not. Whether the ceiling itself should change -- cap the
/// scratch region at the default, which makes an unhonoured request a named
/// refusal instead of an access violation, or keep 64 KiB and accept that a
/// dishonoured request is an abort -- is a ruling about what a program may
/// trust from its own instructions sysvar, and it is not made here.
///
/// What that ruling turns on is one measurement, and the capability-loss half
/// of it is now in doubt: `DCLTHOT3` was put on
/// [`declares_extended_heap_profile_v1`]'s list for two Registry
/// reauthentication CPIs that decision 0017 option B deleted the same day
/// (`hot_v3.rs`, `reauthenticate_top_level_root_roles_v3`: *"It stopped."*),
/// and the continuation route that never made them peaks at 29,895 of 32,768.
/// If the top-level route's peak now fits the default too, capping costs the
/// trade path nothing.
#[cfg(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
))]
pub fn require_declared_heap_ceiling_above_default_v1() -> Result<(), ProgramError> {
    if PROGRAM_HEAP_V1.bytes_capacity() > ADAPTER_DEFAULT_HEAP_BYTES {
        return Ok(());
    }
    Err(crate::TradingSbfError::HeapFrame.into())
}

/// Host builds allocate from the system allocator, which has no such ceiling,
/// so there is nothing to admit and nothing to refuse.
#[cfg(not(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
)))]
pub fn require_declared_heap_ceiling_above_default_v1() -> Result<(), ProgramError> {
    Ok(())
}

/// Bytes outstanding at the program heap's scratch end.
///
/// Zero whenever no [`HeapScratchRegionV1`] is open, which is every instant
/// outside the one region the Hot path opens.
#[cfg(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
))]
#[must_use]
pub fn program_heap_scratch_bytes_v1() -> usize {
    PROGRAM_HEAP_V1.scratch_bytes_used()
}

// ---------------------------------------------------------------------------
// The scratch region
// ---------------------------------------------------------------------------

/// Where a [`ScratchVecV1`] gets its bytes, and what happens when it is
/// dropped.
///
/// On chain this is the program heap's high end: [`BumpHeapV1::alloc_scratch`]
/// hands out blocks going DOWN from the ceiling and an individual release is a
/// no-op, exactly as the upward end's is, because [`HeapScratchRegionV1`]
/// returns the whole end in one store. Off chain -- the host test build, and
/// any build that is not the Trading executable -- there is no program heap to
/// bump, so a scratch block is an ordinary global allocation that is really
/// freed on drop. The host build therefore measures no heap and is not
/// evidence about one; it exists so the same code type-checks and runs in unit
/// tests.
#[cfg(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
))]
mod scratch_backing {
    use super::{Layout, PROGRAM_HEAP_V1, ProgramError};

    pub(super) fn open() -> Result<usize, ProgramError> {
        PROGRAM_HEAP_V1
            .open_scratch()
            .map_err(|error| super::heap_error_v1(error).into())
    }

    /// # Safety
    ///
    /// Every scratch block above `mark` must be dead.
    pub(super) unsafe fn close(mark: usize) {
        // SAFETY: forwarded from `HeapScratchRegionV1::drop`, which holds the
        // obligation; see that type.
        unsafe { PROGRAM_HEAP_V1.release_scratch(mark) };
    }

    /// # Safety
    ///
    /// `layout` must have a non-zero size.
    pub(super) unsafe fn alloc(layout: Layout) -> *mut u8 {
        // SAFETY: forwarded from `ScratchVecV1::with_capacity`, which refuses a
        // zero-sized layout before calling.
        unsafe { PROGRAM_HEAP_V1.alloc_scratch(layout) }
    }

    /// # Safety
    ///
    /// Unconditionally safe: the on-chain scratch end releases per region, not
    /// per block, so this is the same no-op the upward end's `dealloc` is.
    pub(super) const unsafe fn dealloc(_block: *mut u8, _layout: Layout) {}
}

#[cfg(not(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
)))]
mod scratch_backing {
    use super::{Layout, ProgramError};

    // `OPEN` mirrors the on-chain "exactly one region open" invariant so a host
    // test meets the same refusal a program would.
    //
    // Thread-local rather than a plain `static mut` because a host test binary
    // runs its tests concurrently -- and it is `cfg`-ed off on SBF for the
    // opposite reason: an SBF program has no thread-local storage at all, and
    // an ELF that asks for some fails to load, which is reported as
    // `UnsupportedProgramId` at the first invoke rather than as anything that
    // names TLS. That is not a lost check on chain: the Trading executable
    // takes the branch above, where the invariant lives in the heap header.
    // What is left here is every OTHER SBF build -- `no-entrypoint` libraries
    // linked into a test program, which have someone else's global allocator
    // and no program heap to bump.
    #[cfg(not(target_os = "solana"))]
    std::thread_local! {
        static OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    #[cfg(not(target_os = "solana"))]
    pub(super) fn open() -> Result<usize, ProgramError> {
        OPEN.with(|open| {
            if open.get() {
                Err(crate::TradingSbfError::Content.into())
            } else {
                open.set(true);
                Ok(0)
            }
        })
    }

    #[cfg(target_os = "solana")]
    pub(super) fn open() -> Result<usize, ProgramError> {
        Ok(0)
    }

    /// # Safety
    ///
    /// Unconditionally safe here: every scratch block on this backing was
    /// released by its own `dealloc` already.
    pub(super) unsafe fn close(_mark: usize) {
        #[cfg(not(target_os = "solana"))]
        OPEN.with(|open| open.set(false));
    }

    /// # Safety
    ///
    /// `layout` must have a non-zero size.
    pub(super) unsafe fn alloc(layout: Layout) -> *mut u8 {
        // SAFETY: forwarded from `ScratchVecV1::with_capacity`, which refuses a
        // zero-sized layout before calling.
        unsafe { std::alloc::alloc(layout) }
    }

    /// # Safety
    ///
    /// `block` must be a live allocation of exactly `layout`.
    pub(super) unsafe fn dealloc(block: *mut u8, layout: Layout) {
        // SAFETY: forwarded from `ScratchVecV1::drop`, which passes back the
        // pointer and layout its own `with_capacity` received.
        unsafe { std::alloc::dealloc(block, layout) };
    }
}

/// The one open scratch region, and the release of every block inside it.
///
/// # Why exactly one
///
/// The scratch end is a bump like the upward end, so a release is sound only
/// when it returns the TOP of that end -- here, the most recently opened
/// region. Two overlapping regions closed out of order would return a floor
/// above blocks still live under it, and the next scratch allocation would
/// hand those bytes out twice. Rather than track nesting, [`Self::open`]
/// refuses while a region is already open: the invariant becomes one
/// comparison, and any future caller that would have nested meets a refusal
/// instead of corruption.
///
/// # Why this is sound without an audit
///
/// A [`ScratchVecV1`] borrows the region it allocates from, so the borrow
/// checker refuses to let one outlive the release -- the obligation that a
/// mark/reset on the upward end would have carried across four hundred lines
/// and five calls is discharged here by a lifetime. Nothing else in the
/// program can allocate at this end: `GlobalAlloc::alloc`, and therefore
/// every `Vec`, `Box` and `String`, serves the upward end only.
pub struct HeapScratchRegionV1 {
    mark: usize,
    /// Not `Send`/`Sync`: the heap header is per-invocation interior state.
    not_send: PhantomData<*const ()>,
}

impl HeapScratchRegionV1 {
    /// Open the scratch region, or refuse because one is already open.
    pub fn open() -> Result<Self, ProgramError> {
        Ok(Self {
            mark: scratch_backing::open()?,
            not_send: PhantomData,
        })
    }
}

impl Drop for HeapScratchRegionV1 {
    fn drop(&mut self) {
        // SAFETY: every block served from this region lives in a
        // `ScratchVecV1<'_, _>` that borrows `self`, so no such block can
        // still be live at this point: the borrow checker rejects any program
        // in which one outlives the region. See this type's documentation.
        unsafe { scratch_backing::close(self.mark) };
    }
}

/// An exactly-sized bank in the scratch region.
///
/// It is a `Vec` with three differences that are the whole point: its capacity
/// is fixed at construction and a push past it refuses rather than reallocates
/// (so a bank in the scratch region can never strand a smaller copy of itself
/// there), its bytes come from the heap's high end, and it cannot outlive the
/// [`HeapScratchRegionV1`] it borrows.
pub struct ScratchVecV1<'region, T> {
    block: NonNull<T>,
    len: usize,
    capacity: usize,
    region: PhantomData<&'region HeapScratchRegionV1>,
}

impl<'region, T> ScratchVecV1<'region, T> {
    /// Reserve exactly `capacity` elements, refusing when the region cannot
    /// cover them.
    ///
    /// Refuses a zero-sized element type outright rather than growing a
    /// special case: nothing on this path has one, and the allocator's
    /// contract is stated for non-zero layouts.
    pub fn with_capacity(
        _region: &'region HeapScratchRegionV1,
        capacity: usize,
    ) -> Result<Self, ProgramError> {
        if core::mem::size_of::<T>() == 0 {
            return Err(TradingSbfError::Content.into());
        }
        let layout = Layout::array::<T>(capacity)
            .map_err(|_| ProgramError::from(TradingSbfError::Content))?;
        if layout.size() == 0 {
            // A zero-length bank owns no block at all; `dangling` is the
            // aligned non-null address `Vec` itself uses for this case.
            return Ok(Self {
                block: NonNull::dangling(),
                len: 0,
                capacity: 0,
                region: PhantomData,
            });
        }
        // SAFETY: `layout` has a non-zero size, which is this call's contract.
        let block = unsafe { scratch_backing::alloc(layout) };
        let Some(block) = NonNull::new(block.cast::<T>()) else {
            // THE HEAP RAN OUT, and it says so. `alloc_scratch` returns null
            // when the scratch end would cross the upward end -- the two bump
            // positions meeting in the middle -- which is the same wall the
            // upward end reports as an out-of-memory ABORT. Reporting it as
            // `Content` put "this execution is too wide for the heap frame it
            // paid for" among two thousand sites of "your bytes are wrong", and
            // the two have different readers and different remedies. The two
            // refusals above stay `Content` on purpose: a zero-sized element
            // type and a layout that overflows `usize` are defects in the
            // caller, not in the frame it was granted.
            return Err(TradingSbfError::ScratchExhausted.into());
        };
        Ok(Self {
            block,
            len: 0,
            capacity,
            region: PhantomData,
        })
    }

    /// Reserve `len` elements and fill them with clones of `value`.
    pub fn filled(
        region: &'region HeapScratchRegionV1,
        value: &T,
        len: usize,
    ) -> Result<Self, ProgramError>
    where
        T: Clone,
    {
        let mut bank = Self::with_capacity(region, len)?;
        while bank.len < len {
            bank.push(value.clone())?;
        }
        Ok(bank)
    }

    /// Append one element, refusing past the reserved capacity.
    pub fn push(&mut self, value: T) -> Result<(), ProgramError> {
        if self.len >= self.capacity {
            return Err(TradingSbfError::Content.into());
        }
        // SAFETY: `len < capacity`, so `block + len` is inside the block this
        // bank owns and is not yet initialized; the write initializes it and
        // the length below makes it visible.
        unsafe { self.block.as_ptr().add(self.len).write(value) };
        self.len = self.len.saturating_add(1);
        Ok(())
    }

    /// The initialized prefix.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: the first `len` elements were initialized by `push` and the
        // block is live for as long as this bank is.
        unsafe { slice::from_raw_parts(self.block.as_ptr(), self.len) }
    }

    /// The initialized prefix, mutably.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: as `as_slice`, and `&mut self` makes the borrow exclusive.
        unsafe { slice::from_raw_parts_mut(self.block.as_ptr(), self.len) }
    }
}

impl<T> Drop for ScratchVecV1<'_, T> {
    fn drop(&mut self) {
        // SAFETY: the first `len` elements are initialized and owned by this
        // bank; dropping them in place is exactly what `Vec` does.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.block.as_ptr(),
                self.len,
            ))
        };
        if self.capacity == 0 {
            return;
        }
        let Ok(layout) = Layout::array::<T>(self.capacity) else {
            return;
        };
        // SAFETY: `block` and `layout` are exactly what `with_capacity`
        // received from the backing allocator, and this runs once.
        unsafe { scratch_backing::dealloc(self.block.as_ptr().cast::<u8>(), layout) };
    }
}

impl<T> core::ops::Deref for ScratchVecV1<'_, T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> core::ops::DerefMut for ScratchVecV1<'_, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

// ---------------------------------------------------------------------------
// Heap-frame admission
// ---------------------------------------------------------------------------

/// Re-derive the heap frame the runtime granted this transaction.
///
/// The program cannot introspect its own memory map, so the grant is read back
/// out of the instructions sysvar the runtime itself serialized: if a
/// ComputeBudget `RequestHeapFrame(n)` instruction is present in the executing
/// transaction then the runtime already validated `n` and mapped `n` bytes of
/// heap for every invocation in that transaction (trust-surface assumption 6).
///
/// Returns `Ok(None)` when the transaction requested no frame, which is the
/// ordinary case and never an error. Refuses when the account is not the
/// canonical instructions sysvar, when the sysvar does not parse, or when a
/// request is present that the runtime's own
/// `sanitize_requested_heap_size` would have rejected - the last of these
/// cannot happen through a real runtime and refusing is the fail-closed
/// reading.
///
/// This function contains no `unsafe`: the sysvar is read through the safe
/// `AccountInfo` borrow and every index is checked.
pub fn admitted_heap_frame_bytes_v1(
    instructions: &AccountInfo<'_>,
) -> Result<Option<usize>, ProgramError> {
    if instructions.key != &solana_sdk_ids::sysvar::instructions::ID
        || instructions.is_signer
        || instructions.is_writable
        || instructions.executable
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    let data = instructions
        .try_borrow_data()
        .map_err(|_| TradingSbfError::NativeSignature)?;
    admitted_heap_frame_bytes_from_sysvar_v1(&data)
}

/// [`admitted_heap_frame_bytes_v1`] over the raw instructions-sysvar bytes.
///
/// Split out so the adversarial corpus can drive the parser directly. The
/// layout is the one `solana_instructions_sysvar` documents and constructs: a
/// `u16` instruction count, that many `u16` start offsets, and at each offset a
/// `u16` account count, that many 33-byte account metas, the 32-byte program
/// id, a `u16` data length, and the data. Every offset is checked against the
/// same slice and every width is checked arithmetic, so a truncated, overlong,
/// or self-referential sysvar is a refusal rather than a panic or a short read.
/// There is no `unsafe` here: the bytes arrive through a safe `AccountInfo`
/// borrow.
///
/// QUEUED CONVERGENCE: `native_signature` is growing a borrowed
/// `SysvarInstructionV1` reader for the continuation-admission path with the
/// same discipline. When it lands, this walk becomes a call into it and the
/// layout knowledge lives in one place. It is written out here rather than
/// taken as a dependency because that reader is another lane's in-flight work
/// and this module must not be coupled to uncommitted code.
fn admitted_heap_frame_bytes_from_sysvar_v1(data: &[u8]) -> Result<Option<usize>, ProgramError> {
    /// Bytes per serialized account meta: one privilege byte and an address.
    const META_BYTES: usize = 33;

    let refusal = || ProgramError::from(TradingSbfError::NativeSignature);
    let count = read_u16(data, 0).ok_or_else(refusal)?;
    let mut granted: Option<usize> = None;
    for index in 0..count {
        let offset_at = usize::from(index)
            .checked_mul(2)
            .and_then(|scaled| scaled.checked_add(2))
            .ok_or_else(refusal)?;
        let start = usize::from(read_u16(data, offset_at).ok_or_else(refusal)?);
        let accounts = usize::from(read_u16(data, start).ok_or_else(refusal)?);
        let program_id_at = accounts
            .checked_mul(META_BYTES)
            .and_then(|scaled| scaled.checked_add(start))
            .and_then(|scaled| scaled.checked_add(2))
            .ok_or_else(refusal)?;
        let data_len_at = program_id_at.checked_add(32).ok_or_else(refusal)?;
        let program_id = data.get(program_id_at..data_len_at).ok_or_else(refusal)?;
        let data_len = usize::from(read_u16(data, data_len_at).ok_or_else(refusal)?);
        let data_at = data_len_at.checked_add(2).ok_or_else(refusal)?;
        let data_end = data_at.checked_add(data_len).ok_or_else(refusal)?;
        let instruction_data = data.get(data_at..data_end).ok_or_else(refusal)?;
        if program_id != solana_sdk_ids::compute_budget::ID.as_ref() {
            continue;
        }
        // Agave decodes ComputeBudget instructions with borsh's
        // `try_from_slice_unchecked`, which ignores trailing bytes. Mirror
        // that exactly: a longer payload with this discriminant is still a
        // heap-frame request to the runtime, so it must be one here too.
        let Some(&REQUEST_HEAP_FRAME_DISCRIMINANT) = instruction_data.first() else {
            continue;
        };
        let bytes = read_u32(instruction_data, 1).ok_or_else(refusal)?;
        let bytes = usize::try_from(bytes).map_err(|_| refusal())?;
        if !(ADAPTER_DEFAULT_HEAP_BYTES..=ADAPTER_MAX_HEAP_BYTES).contains(&bytes)
            || !bytes.is_multiple_of(HEAP_FRAME_GRANULARITY_BYTES)
        {
            return Err(refusal());
        }
        if granted.is_some() {
            // The runtime rejects a transaction carrying two of these with
            // `DuplicateInstruction`, so this is unreachable through a real
            // runtime; refuse rather than pick one.
            return Err(refusal());
        }
        granted = Some(bytes);
    }
    Ok(granted)
}

/// Authenticate the transaction's heap-frame grant and lift the allocator's
/// ceiling to it.
///
/// Returns the ceiling now in force. Calling this is monotone and
/// order-independent: the allocator bumps upward, so raising the ceiling
/// invalidates nothing already handed out and may happen at any point in the
/// invocation.
#[cfg(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
))]
pub fn admit_heap_frame_v1(instructions: &AccountInfo<'_>) -> Result<usize, ProgramError> {
    match admitted_heap_frame_bytes_v1(instructions)? {
        Some(bytes) => PROGRAM_HEAP_V1
            .lift_ceiling(bytes)
            .map_err(|error| heap_error_v1(error).into()),
        None => Ok(PROGRAM_HEAP_V1.bytes_capacity()),
    }
}

/// Whether the diagnostic profile build lifts the ceiling for every route.
///
/// **Not a decision, and it cannot reach a shipped executable.** The Hot tail
/// past the 32 KiB wall -- the six lifecycle creates and the child role CPIs --
/// cannot be measured at all while the run refuses at `pf-enter`, and two
/// separate lanes have now rebuilt the same throwaway patch to see it. This is
/// that patch, named, so the third one does not.
///
/// `hot-cu-profile` is documented as "diagnostic-only phase checkpoints for
/// measured SBF compute profiling": a build carrying it logs a labelled line
/// and a compute reading at thirty points inside one instruction, so it is not
/// a thing anyone ships by accident. If one ever were shipped,
/// `program-test/tests/hot_heap_frame_is_inert.rs` fails on it, because that
/// test asserts the shipped Hot path still refuses a granted frame.
///
/// The 32 KiB discipline is UNCHANGED for every executable that is not this
/// one. Hot is still absent from the list below, and putting it there is still
/// the single visible act that would take it off.
const fn hot_cu_profile_lifts_every_route_v1() -> bool {
    cfg!(feature = "hot-cu-profile")
}

/// Routes permitted to run on a runtime-granted heap frame larger than the
/// protocol default.
///
/// Exhaustive and adapter-owned. Adding a route here is the single visible act
/// that takes it off the 32 KiB discipline, and it must be an instruction whose
/// transaction has the packet room to actually carry `RequestHeapFrame` and to
/// present the instructions sysvar - without both, the declaration is inert and
/// the route keeps the default ceiling.
///
/// The first entries are the one-time, ALT-backed founding transactions:
///
/// - `DCLTGMF3`, the composed Lock/Found/Realize/Claims/Open route;
/// - `DCLTGFP1`, the split founding's stage 1 — the same frame and the same
///   child allocation profile minus only the Open window, so it inherits the
///   same declaration for the same measured reason;
/// - `DCLTPCB2`, projected-Custody bootstrap, which commit `328fead` measured
///   dying out of memory and diagnosed precisely: it "holds three stages' worth
///   of allocations live [...] against an allocator that never frees, so its
///   peak is the sum. Either it allocates less, or it supplies its own global
///   allocator over the granted heap." This module is that allocator, and this
///   is the declaration that lets the grant reach it.
pub fn declares_extended_heap_profile_v1(instruction_data: &[u8]) -> bool {
    if hot_cu_profile_lifts_every_route_v1() {
        return true;
    }
    // `DCLTHOT3`, Hot execution. Added 2026-08-30, and the reason is the route
    // rather than the tail W2p closed: a caller who invokes Trading DIRECTLY --
    // which is how every public caller sends a Direct trade -- makes two
    // Registry reauthentication CPIs that a Registry continuation never makes,
    // and holds their frames and receipts against an allocator that never
    // frees. THE CONTINUATION ROUTE ASKS FOR ONE TOO now, since 2026-09-03: it
    // stopped fitting the 32 KiB default (29,895 measured at `365304c2d`,
    // about 33,020 now) and its packet stopped having four spare bytes (it has
    // sixty-five, and a `RequestHeapFrame` costs eight). Both halves of the
    // claim that used to stand here expired independently and nothing said so,
    // because the row that would have reported it was red for the whole
    // interval. Declaring here is what makes a grant ADMISSIBLE, not required:
    // the route that needs it asks for it, and asks
    // `require_declared_heap_ceiling_above_default_v1` to refuse by name if it did not
    // arrive -- which BOTH arms of `authenticate_root_against_market_boxed_v3`
    // now do, the continuation arm having carried no such call until the same
    // day.
    if instruction_data.get(..HOT_EXECUTION_MAGIC_BYTES_V1)
        == Some(dclutch_market::capability_program::hot_v3::HOT_EXECUTION_MAGIC_V3.as_slice())
    {
        return true;
    }
    // `DCLTSEL1`, the validated-artifact seal outer. Added 2026-08-31 (CLOSESEAL)
    // because the entry above left it DEAD, not merely expensive.
    //
    // `process_capability_seal_v1` authenticates its Market and root exactly as a
    // hot action does, through `reauthenticate_top_level_root_roles_v3` -- and
    // that function's first act is `require_declared_heap_ceiling_above_default_v1`. The Hot
    // arm was declared here; the seal outer, which is the same prologue, was not.
    // So every seal write refused `TradingSbfError::HeapFrame` unconditionally,
    // and no NEW capability seal could be written on chain by this release: a
    // fresh `(descriptor, action, release, Registry)` tuple had no sealed
    // execution path at all. `registry_hot_continuation`'s seal cases are the
    // ones that carried the red.
    //
    // The declaration only makes a grant ADMISSIBLE. A seal transaction that
    // sends no `RequestHeapFrame` still keeps the default ceiling and still
    // refuses by name -- which is the right shape for a caller who forgot,
    // rather than an unnamed abort. The seal outer's own peak is NOT measured
    // here; declaring is the conservative direction, and weakening the guard so
    // this route skips it would have been the other one.
    if dclutch_vm::capability_seal::is_capability_seal_request_v1(instruction_data) {
        return true;
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if crate::generic_market_founding_v1::is_generic_market_founding_v3(instruction_data)
        || crate::generic_founding_stages_v1::is_generic_found_and_permit_v1(instruction_data)
        || crate::projected_custody_bootstrap_v1::is_projected_custody_bootstrap_v2(
            instruction_data,
        )
        || crate::projected_custody_bootstrap_v1::is_controller_funding_prepare_v1(instruction_data)
    {
        return true;
    }
    let _ = instruction_data;
    false
}

/// Lift the heap ceiling for a route that declares an extended heap profile.
///
/// Best effort by construction: a transaction that declared the profile but
/// presented no instructions sysvar, or presented one carrying no grant, keeps
/// the protocol default ceiling and proceeds. Only an actively malformed
/// sysvar refuses, and it refuses by leaving the ceiling alone.
#[cfg(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
))]
fn lift_declared_heap_profile_v1(accounts: &[AccountInfo<'_>], instruction_data: &[u8]) {
    if !declares_extended_heap_profile_v1(instruction_data) {
        return;
    }
    let Some(instructions) = accounts
        .iter()
        .find(|account| account.key == &solana_sdk_ids::sysvar::instructions::ID)
    else {
        return;
    };
    let _ = admit_heap_frame_v1(instructions);
}

// ---------------------------------------------------------------------------
// Child invocation
// ---------------------------------------------------------------------------

/// Invoke a child program from an instruction this program ALREADY OWNS.
///
/// # Why this is here rather than a call to `invoke_signed`
///
/// `solana_cpi::invoke_signed_unchecked` builds the runtime's stable layout
/// with `StableInstruction::from(instruction.clone())`. It clones because it
/// only holds a `&Instruction` and `StableInstruction::from` MOVES both `Vec`s
/// out of the instruction it is given; an owner therefore pays nothing, and
/// every caller in this executable owns its metas and its wire outright. On the
/// SBF bump allocator, whose `dealloc` is a no-op, that clone is not a
/// transient: **2,322 bytes for the two child CPIs of the canonical Direct
/// bundle** stay charged for the rest of the instruction, measured at the point
/// the heap is scarcest.
///
/// So this takes the two buffers by `&mut`, moves them into the SDK's OWN
/// `StableInstruction` through the SDK's OWN `From<Vec<T>> for StableVec<T>`,
/// makes the syscall the SDK makes with the arguments the SDK passes, and moves
/// them back out through the SDK's OWN `From<StableVec<T>> for Vec<T>`. The
/// caller's buffers come back with their allocation, length and capacity
/// unchanged, ready to be cleared and refilled for the next invocation. Nothing
/// about the layout the runtime reads is restated here.
///
/// # What is reproduced, and why it is reproduced rather than skipped
///
/// `invoke_signed` is `invoke_signed_unchecked` plus a `RefCell` consistency
/// pre-check, and that pre-check is a runtime guarantee, not an optimisation:
/// it is what turns a callee's write to an account this program is holding
/// borrowed into a returned [`ProgramError::AccountBorrowFailed`] instead of
/// undefined behaviour. [`require_cpi_borrowable_v1`] is that loop, conjunct
/// for conjunct, over the metas that are about to become the instruction's
/// account list. `child_invocation_borrow_check_matches_the_sdk` in this
/// module's corpus runs both against the same frames and asserts they agree, on
/// acceptance and on every borrow conflict.
///
/// The `_unchecked` variants are NOT what this replaces and their bargain is
/// not taken: this is `invoke_signed`, minus a clone the caller does not need.
///
/// `#[inline(never)]`, and both halves below with it: the 80-byte
/// `StableInstruction` and the syscall's five arguments belong to THIS frame,
/// not to a composition's. Inlined, it put `execute_resolution_route_v3` 512
/// bytes over the SBPF v0 4,096-byte static frame bound.
#[inline(never)]
pub fn invoke_signed_owned_v1(
    program_id: &Pubkey,
    metas: &mut Vec<AccountMeta>,
    data: &mut Vec<u8>,
    account_infos: &[AccountInfo<'_>],
    signers_seeds: &[&[&[u8]]],
) -> Result<(), ProgramError> {
    require_cpi_borrowable_v1(metas, account_infos)?;
    invoke_signed_owned_unchecked_v1(program_id, metas, data, account_infos, signers_seeds)
}

/// Refuse a child invocation whose account frame this program still holds
/// borrowed in a way the callee's access would violate.
///
/// Sourced to `solana_cpi::invoke_signed` (3.1.0, `src/lib.rs:252`), whose
/// whole body ahead of the syscall is this loop. It is reproduced rather than
/// called because `invoke_signed` takes a `&Instruction` -- the very thing this
/// module exists not to build -- and the loop reads only the account list.
///
/// The semantics that must survive the reproduction, all of them observable:
///
/// - the metas are walked in ORDER, and the FIRST `AccountInfo` whose key
///   matches is the one checked (`break`), so a frame carrying the same account
///   twice is checked once per meta against its first occurrence;
/// - a writable meta demands BOTH mutable borrows, a readonly meta both shared
///   borrows, and the borrow is released immediately (`let _ =`, never a
///   binding that would hold it across the syscall);
/// - a meta naming an account absent from `account_infos` is NOT an error here.
///   The runtime refuses that, and refusing it earlier would be a different
///   program.
#[inline(never)]
fn require_cpi_borrowable_v1(
    metas: &[AccountMeta],
    account_infos: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    for account_meta in metas.iter() {
        for account_info in account_infos.iter() {
            if account_meta.pubkey == *account_info.key {
                if account_meta.is_writable {
                    let _ = account_info.try_borrow_mut_lamports()?;
                    let _ = account_info.try_borrow_mut_data()?;
                } else {
                    let _ = account_info.try_borrow_lamports()?;
                    let _ = account_info.try_borrow_data()?;
                }
                break;
            }
        }
    }
    Ok(())
}

/// The syscall half of [`invoke_signed_owned_v1`], on the SBF target.
///
/// Sourced to `solana_cpi::invoke_signed_unchecked` (3.1.0, `src/lib.rs:301`).
/// The four pointer/length arguments are formed exactly as that function forms
/// them, and the success discriminant is its `_SUCCESS`.
#[cfg(target_os = "solana")]
#[inline(never)]
fn invoke_signed_owned_unchecked_v1(
    program_id: &Pubkey,
    metas: &mut Vec<AccountMeta>,
    data: &mut Vec<u8>,
    account_infos: &[AccountInfo<'_>],
    signers_seeds: &[&[&[u8]]],
) -> Result<(), ProgramError> {
    #[allow(deprecated)]
    use solana_program::{
        stable_layout::{stable_instruction::StableInstruction, stable_vec::StableVec},
        syscalls::sol_invoke_signed_rust,
    };

    let account_info_count =
        u64::try_from(account_infos.len()).map_err(|_| TradingSbfError::Content)?;
    let signers_seeds_count =
        u64::try_from(signers_seeds.len()).map_err(|_| TradingSbfError::Content)?;
    // `StableVec::from` is `Vec::into_raw_parts` under a `ManuallyDrop`: the
    // caller's allocation, length and capacity, reinterpreted. No copy, no
    // allocation, and the buffers are handed back below through the inverse
    // conversion the same crate defines.
    let stable = StableInstruction {
        accounts: StableVec::from(core::mem::take(metas)),
        data: StableVec::from(core::mem::take(data)),
        program_id: *program_id,
    };
    // SAFETY: this is `solana_cpi::invoke_signed_unchecked`'s own call, with
    // its own arguments, and the obligations it discharges are discharged here:
    //
    // 1. `&stable` points at a live `StableInstruction` -- the runtime's pinned
    //    `#[repr(C)]` layout, built by the SDK's own conversions -- which is a
    //    local binding of this function and so outlives the call.
    // 2. `account_infos as *const _ as *const u8` is the slice's DATA address,
    //    which is what the runtime reads `account_info_count` elements from;
    //    the slice is borrowed for the whole call, so those elements are live.
    // 3. `signers_seeds` likewise: the address of the first `&[&[u8]]`, and the
    //    borrow keeps the seeds and everything they point at alive.
    // 4. The `RefCell` consistency pre-check `invoke_signed` performs has
    //    already run (`require_cpi_borrowable_v1`), so the runtime's writes
    //    through these accounts cannot alias a borrow this program is holding.
    // 5. The counts are the slices' own lengths, converted losslessly above.
    let result = unsafe {
        sol_invoke_signed_rust(
            &stable as *const _ as *const u8,
            account_infos as *const _ as *const u8,
            account_info_count,
            signers_seeds as *const _ as *const u8,
            signers_seeds_count,
        )
    };
    // Unconditionally, ahead of the result: on the error path these buffers are
    // still the caller's, and `StableInstruction` carries no `Drop` of its own,
    // so destructuring hands each `StableVec`'s release obligation to the `Vec`
    // it came from.
    let StableInstruction {
        accounts,
        data: invoked_data,
        program_id: _,
    } = stable;
    *metas = Vec::from(accounts);
    *data = Vec::from(invoked_data);
    if result == CPI_SUCCESS_V1 {
        Ok(())
    } else {
        Err(ProgramError::from(result))
    }
}

/// The syscall half of [`invoke_signed_owned_v1`], off the SBF target.
///
/// Host builds have no syscall to make, so this is
/// `solana_program::program::invoke_signed_unchecked` -- the stub path -- with
/// the buffers moved in and back out so the two targets agree on what the
/// caller's `metas` and `data` hold when the call returns.
#[cfg(not(target_os = "solana"))]
#[inline(never)]
fn invoke_signed_owned_unchecked_v1(
    program_id: &Pubkey,
    metas: &mut Vec<AccountMeta>,
    data: &mut Vec<u8>,
    account_infos: &[AccountInfo<'_>],
    signers_seeds: &[&[&[u8]]],
) -> Result<(), ProgramError> {
    let instruction = Instruction {
        program_id: *program_id,
        accounts: core::mem::take(metas),
        data: core::mem::take(data),
    };
    let result = solana_program::program::invoke_signed_unchecked(
        &instruction,
        account_infos,
        signers_seeds,
    );
    let Instruction {
        program_id: _,
        accounts,
        data: invoked_data,
    } = instruction;
    *metas = accounts;
    *data = invoked_data;
    result
}

/// Syscall return value the CPI syscalls use for success.
///
/// Chain-derived: `solana_cpi`'s `_SUCCESS`, itself a copy of
/// `solana_program_entrypoint::SUCCESS`.
#[cfg_attr(not(target_os = "solana"), allow(dead_code))]
const CPI_SUCCESS_V1: u64 = 0;

// ---------------------------------------------------------------------------
// Loader input deserialization
// ---------------------------------------------------------------------------

/// A bounds-checked walk over the loader's input region.
///
/// Production constructs this with `limit == usize::MAX`, because the program
/// is never told the serialized length and the VM's mapping is the enforcing
/// authority (trust-surface assumption 1). The adversarial tests construct it
/// with the true length, so the truncation corpus exercises exactly the
/// arithmetic production runs rather than a parallel checked path.
struct InputCursor {
    /// Base of the input region.
    base: *mut u8,
    /// Bytes consumed so far.
    offset: usize,
    /// Bytes readable from `base`.
    limit: usize,
}

impl InputCursor {
    /// Bind a cursor to `base`, permitting reads of the first `limit` bytes.
    ///
    /// # Safety
    ///
    /// `base` must point at a writable region of at least `limit` bytes that
    /// outlives `'a` in every reference this cursor hands out.
    #[inline(always)]
    const unsafe fn new(base: *mut u8, limit: usize) -> Self {
        Self {
            base,
            offset: 0,
            limit,
        }
    }

    /// Reserve `bytes` and return the offset they start at.
    #[inline(always)]
    fn take(&mut self, bytes: usize) -> Result<usize, ProgramError> {
        let start = self.offset;
        let end = start.checked_add(bytes).ok_or(TradingSbfError::Content)?;
        if end > self.limit {
            return Err(TradingSbfError::Content.into());
        }
        self.offset = end;
        Ok(start)
    }

    /// Skip `bytes` without reading them.
    #[inline(always)]
    fn skip(&mut self, bytes: usize) -> Result<(), ProgramError> {
        self.take(bytes).map(|_| ())
    }

    /// Align the *offset* up to `BPF_ALIGN_OF_U128`.
    ///
    /// The SDK computes this padding as
    /// `(offset as *const u8).align_offset(BPF_ALIGN_OF_U128)` - on the offset
    /// value reinterpreted as a pointer, not on the absolute address. For a
    /// `u8` pointer that is exactly `offset.wrapping_neg() % 8`, and the input
    /// region's base is itself 8-aligned, so the two agree; the arithmetic form
    /// is used here because it cannot return the `usize::MAX` that
    /// `align_offset` is permitted to return for an unknown provenance.
    #[inline(always)]
    fn pad_to_alignment(&mut self) -> Result<(), ProgramError> {
        let padding = self.offset.wrapping_neg() % BPF_ALIGN_OF_U128;
        self.skip(padding)
    }

    /// Read a little-endian `u64`.
    ///
    /// # Safety
    ///
    /// The cursor's region must satisfy `InputCursor::new`'s contract.
    #[inline(always)]
    unsafe fn read_u64(&mut self) -> Result<u64, ProgramError> {
        let at = self.take(8)?;
        // SAFETY: `take` proved `[at, at + 8)` is inside the cursor's region,
        // which the construction contract says is readable. `read_unaligned`
        // imposes no alignment requirement, so the loader's 8-byte field
        // alignment is not relied on here.
        Ok(unsafe { self.base.add(at).cast::<u64>().read_unaligned() })
    }

    /// Read a single byte.
    ///
    /// # Safety
    ///
    /// The cursor's region must satisfy `InputCursor::new`'s contract.
    #[inline(always)]
    unsafe fn read_u8(&mut self) -> Result<u8, ProgramError> {
        let at = self.take(1)?;
        // SAFETY: `take` proved `[at, at + 1)` is inside the cursor's readable
        // region.
        Ok(unsafe { *self.base.add(at) })
    }

    /// Borrow a 32-byte address in place.
    ///
    /// # Safety
    ///
    /// The cursor's region must satisfy `InputCursor::new`'s contract, and
    /// the returned reference must not outlive the region.
    #[inline(always)]
    unsafe fn read_address<'a>(&mut self) -> Result<&'a Pubkey, ProgramError> {
        let at = self.take(32)?;
        // SAFETY: `take` proved `[at, at + 32)` is inside the readable region.
        // `Pubkey` is a `#[repr(transparent)]` 32-byte array with alignment 1,
        // so any address in the region is correctly aligned for it, and every
        // 32-byte pattern is a valid inhabitant. The caller's lifetime bound
        // keeps the reference inside the region's lifetime.
        Ok(unsafe { &*self.base.add(at).cast::<Pubkey>() })
    }

    /// Borrow the mutable lamport cell in place.
    ///
    /// # Safety
    ///
    /// The cursor's region must satisfy `InputCursor::new`'s contract, the
    /// returned reference must not outlive it, and this cell must not be
    /// borrowed twice - which the loader guarantees by giving every
    /// non-duplicate account a distinct record.
    #[inline(always)]
    unsafe fn read_lamports<'a>(&mut self) -> Result<&'a mut u64, ProgramError> {
        let at = self.take(8)?;
        // SAFETY: `take` proved `[at, at + 8)` is inside the region, which the
        // construction contract says is writable. The loader places this field
        // at an 8-aligned offset from an 8-aligned base (trust-surface
        // assumption 3), so the pointer is aligned for `u64`. Uniqueness of
        // the borrow follows from each account record being visited once: a
        // duplicate slot clones the original's `Rc` instead of re-reading.
        Ok(unsafe { &mut *self.base.add(at).cast::<u64>() })
    }

    /// Borrow `len` bytes of account data in place.
    ///
    /// # Safety
    ///
    /// As `InputCursor::read_lamports`.
    #[inline(always)]
    unsafe fn read_data<'a>(&mut self, len: usize) -> Result<&'a mut [u8], ProgramError> {
        let at = self.take(len)?;
        // SAFETY: `take` proved `[at, at + len)` is inside the writable region,
        // and `len` is bounded by the cursor limit so it cannot exceed
        // `isize::MAX`. `u8` has alignment 1. Uniqueness follows as in
        // `read_lamports`.
        Ok(unsafe { slice::from_raw_parts_mut(self.base.add(at), len) })
    }

    /// Borrow `len` bytes of instruction data in place.
    ///
    /// # Safety
    ///
    /// As `InputCursor::read_address`.
    #[inline(always)]
    unsafe fn read_bytes<'a>(&mut self, len: usize) -> Result<&'a [u8], ProgramError> {
        let at = self.take(len)?;
        // SAFETY: as `read_data`, and shared rather than exclusive.
        Ok(unsafe { slice::from_raw_parts(self.base.add(at), len) })
    }

    /// Overwrite the loader's 4-byte padding with the account's data length.
    ///
    /// The SDK does this so `AccountInfo::original_data_len` can recover the
    /// pre-invocation length from the 4 bytes preceding the key; the runtime's
    /// realloc validation depends on it, so it is not optional.
    ///
    /// # Safety
    ///
    /// `at` must be an offset this cursor already reserved for the
    /// original-data-length field, and the region must be writable.
    #[inline(always)]
    unsafe fn write_original_data_len(&mut self, at: usize, len: u32) {
        // SAFETY: `at` was reserved by a prior `take(4)`, so `[at, at + 4)` is
        // inside the writable region. `write_unaligned` imposes no alignment
        // requirement.
        unsafe { self.base.add(at).cast::<u32>().write_unaligned(len) };
    }
}

/// Deserialize the loader input region into `slots`.
///
/// Mirrors `solana_program_entrypoint::deserialize_into` field for field, with
/// the fail-closed divergences named in this module's trust surface. Returns
/// the program id, the number of slots written, and the instruction data.
///
/// # Safety
///
/// `input` must be the base of a loader-serialized input region of at least
/// `limit` bytes that stays mapped for `'a`. On success the first `count`
/// entries of `slots` are initialized; on failure some prefix of them may be
/// initialized and the caller must not read any of them.
pub unsafe fn deserialize_into_v1<'a>(
    input: *mut u8,
    limit: usize,
    slots: &mut [MaybeUninit<AccountInfo<'a>>],
) -> Result<(&'a Pubkey, usize, &'a [u8]), ProgramError> {
    // SAFETY: forwarded from this function's contract.
    let mut cursor = unsafe { InputCursor::new(input, limit) };

    // SAFETY: the cursor was constructed over a region satisfying its
    // contract; every read below inherits that and is bounds-checked.
    let count = unsafe { cursor.read_u64()? };
    let count = usize::try_from(count).map_err(|_| TradingSbfError::UnsupportedContent)?;
    if count > slots.len() {
        return Err(TradingSbfError::UnsupportedContent.into());
    }

    for index in 0..count {
        // SAFETY: as above.
        let marker = unsafe { cursor.read_u8()? };
        if marker == NON_DUP_MARKER {
            // SAFETY: as above.
            let account = unsafe { deserialize_account_v1(&mut cursor)? };
            slots
                .get_mut(index)
                .ok_or(TradingSbfError::UnsupportedContent)?
                .write(account);
        } else {
            cursor.skip(DUPLICATE_PADDING_BYTES)?;
            let source = usize::from(marker);
            if source >= index {
                // The loader never emits a forward or self reference. The SDK
                // would clone an uninitialized `AccountInfo` here; refuse.
                return Err(TradingSbfError::Content.into());
            }
            let original = slots.get(source).ok_or(TradingSbfError::Content)?;
            // SAFETY: `source < index` and every slot below `index` was
            // written by an earlier iteration of this loop, so this slot is
            // initialized.
            let clone = unsafe { original.assume_init_ref() }.clone();
            slots
                .get_mut(index)
                .ok_or(TradingSbfError::UnsupportedContent)?
                .write(clone);
        }
    }

    // SAFETY: as above.
    let data_len = unsafe { cursor.read_u64()? };
    let data_len = usize::try_from(data_len).map_err(|_| TradingSbfError::Content)?;
    // SAFETY: as above.
    let instruction_data = unsafe { cursor.read_bytes(data_len)? };
    // SAFETY: as above.
    let program_id = unsafe { cursor.read_address()? };

    Ok((program_id, count, instruction_data))
}

/// Deserialize one non-duplicate account record.
///
/// # Safety
///
/// `cursor` must satisfy `InputCursor::new`'s contract and be positioned
/// immediately after a [`NON_DUP_MARKER`].
#[inline(always)]
unsafe fn deserialize_account_v1<'a>(
    cursor: &mut InputCursor,
) -> Result<AccountInfo<'a>, ProgramError> {
    // SAFETY: forwarded from this function's contract; every read is
    // bounds-checked by the cursor.
    unsafe {
        let is_signer = cursor.read_u8()? != 0;
        let is_writable = cursor.read_u8()? != 0;
        let executable = cursor.read_u8()? != 0;
        let original_data_len_at = cursor.take(4)?;
        let key = cursor.read_address()?;
        let owner = cursor.read_address()?;
        let lamports = cursor.read_lamports()?;
        let data_len = cursor.read_u64()?;
        let data_len = usize::try_from(data_len).map_err(|_| TradingSbfError::Content)?;
        let narrowed = u32::try_from(data_len).map_err(|_| TradingSbfError::Content)?;
        cursor.write_original_data_len(original_data_len_at, narrowed);
        let data = cursor.read_data(data_len)?;
        cursor.skip(MAX_PERMITTED_DATA_INCREASE)?;
        cursor.skip(UNUSED_RENT_EPOCH_BYTES)?;
        cursor.pad_to_alignment()?;
        Ok(AccountInfo::new(
            key,
            is_signer,
            is_writable,
            lamports,
            data,
            owner,
            executable,
        ))
    }
}

// ---------------------------------------------------------------------------
// The entrypoint
// ---------------------------------------------------------------------------

/// The SBF loader's entrypoint symbol.
///
/// # Safety
///
/// Called only by the SBF loader, with `input` pointing at the region described
/// by this module's trust surface.
#[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
    // SAFETY: the loader's contract, forwarded.
    let count = unsafe { input.cast::<u64>().read_unaligned() };
    if count <= ADAPTER_STACK_SLOTS_V1 as u64 {
        // SAFETY: as above.
        unsafe { entrypoint_on_stack(input) }
    } else {
        // SAFETY: as above.
        unsafe { entrypoint_on_heap(input) }
    }
}

/// Deserialize into the entrypoint's own stack frame. No heap is touched.
///
/// # Safety
///
/// As [`entrypoint`].
#[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
#[inline(never)]
unsafe fn entrypoint_on_stack(input: *mut u8) -> u64 {
    #[allow(clippy::declare_interior_mutable_const)]
    const UNINIT: MaybeUninit<AccountInfo<'_>> = MaybeUninit::uninit();
    let mut slots = [UNINIT; ADAPTER_STACK_SLOTS_V1];
    // SAFETY: forwarded from this function's contract; `usize::MAX` is the
    // limit because the VM's input mapping is the length authority
    // (trust-surface assumption 1).
    match unsafe { deserialize_into_v1(input, usize::MAX, &mut slots) } {
        Ok((program_id, count, instruction_data)) => {
            let Some(written) = slots.get(..count) else {
                return u64::from(ProgramError::from(TradingSbfError::UnsupportedContent));
            };
            // SAFETY: `deserialize_into_v1` returned `Ok`, so exactly the first
            // `count` slots are initialized. `MaybeUninit<AccountInfo>` has the
            // same layout as `AccountInfo`, so the reinterpretation is sound;
            // this is the same step `entrypoint_no_alloc!` performs.
            let accounts = unsafe {
                &*(core::ptr::from_ref(written) as *const [MaybeUninit<AccountInfo<'_>>]
                    as *const [AccountInfo<'_>])
            };
            dispatch(program_id, accounts, instruction_data)
        }
        Err(error) => u64::from(error),
    }
}

/// Deserialize into an exactly-sized heap buffer for frames wider than
/// [`ADAPTER_STACK_SLOTS_V1`].
///
/// # Safety
///
/// As [`entrypoint`].
#[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
#[inline(never)]
unsafe fn entrypoint_on_heap(input: *mut u8) -> u64 {
    use std::vec::Vec;

    // SAFETY: forwarded from this function's contract.
    let count = unsafe { input.cast::<u64>().read_unaligned() };
    let Ok(count) = usize::try_from(count) else {
        return u64::from(ProgramError::from(TradingSbfError::UnsupportedContent));
    };
    if count > crate::TRADING_MAX_INSTRUCTION_ACCOUNTS_V3 {
        // Refuse before reserving the buffer rather than after, so an
        // over-wide frame cannot exhaust the heap on its way to the same
        // refusal `require_instruction_account_bound_v3` already returns.
        return u64::from(ProgramError::from(TradingSbfError::UnsupportedContent));
    }
    let mut slots: Vec<MaybeUninit<AccountInfo<'_>>> = Vec::with_capacity(count);
    // SAFETY: `Vec::with_capacity(count)` reserved `count` elements, and
    // `MaybeUninit<AccountInfo>` needs no initialization to be a valid
    // inhabitant, so every element in `0..count` is a live `MaybeUninit`.
    unsafe { slots.set_len(count) };
    // SAFETY: forwarded; `usize::MAX` as in `entrypoint_on_stack`.
    match unsafe { deserialize_into_v1(input, usize::MAX, &mut slots) } {
        Ok((program_id, written, instruction_data)) => {
            let Some(filled) = slots.get(..written) else {
                return u64::from(ProgramError::from(TradingSbfError::UnsupportedContent));
            };
            // SAFETY: as in `entrypoint_on_stack`.
            let accounts = unsafe {
                &*(core::ptr::from_ref(filled) as *const [MaybeUninit<AccountInfo<'_>>]
                    as *const [AccountInfo<'_>])
            };
            dispatch(program_id, accounts, instruction_data)
        }
        Err(error) => u64::from(error),
    }
}

/// Lift the heap ceiling if the route declares it, then run the program.
#[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
#[inline(never)]
fn dispatch(program_id: &Pubkey, accounts: &[AccountInfo<'_>], instruction_data: &[u8]) -> u64 {
    #[cfg(not(feature = "custom-heap"))]
    lift_declared_heap_profile_v1(accounts, instruction_data);
    match crate::process_instruction(program_id, accounts, instruction_data) {
        Ok(()) => solana_program::entrypoint::SUCCESS,
        Err(error) => u64::from(error),
    }
}

#[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
solana_program::custom_panic_default!();

/// Read a little-endian `u16` at `at`, or `None` if it does not fit.
fn read_u16(data: &[u8], at: usize) -> Option<u16> {
    let end = at.checked_add(2)?;
    let bytes: [u8; 2] = data.get(at..end)?.try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

/// Read a little-endian `u32` at `at`, or `None` if it does not fit.
fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    let end = at.checked_add(4)?;
    let bytes: [u8; 4] = data.get(at..end)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use std::{boxed::Box, vec, vec::Vec};

    use solana_program::entrypoint::deserialize as sdk_deserialize;

    use super::*;

    // -----------------------------------------------------------------
    // A byte-exact mirror of Agave's `serialize_parameters_aligned`
    // (solana-program-runtime 3.1.4, src/serialization.rs:474) for the
    // non-direct-mapping path the pinned program-test runtime uses.
    // -----------------------------------------------------------------

    #[derive(Clone)]
    struct Acct {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
        is_signer: bool,
        is_writable: bool,
        executable: bool,
    }

    enum SlotSpec {
        Fresh(Acct),
        Dup(u8),
    }

    fn as_u64(value: usize) -> u64 {
        u64::try_from(value).expect("usize fits u64")
    }

    fn account(seed: u8) -> Acct {
        Acct {
            key: Pubkey::new_from_array([seed; 32]),
            owner: Pubkey::new_from_array([seed.wrapping_add(128); 32]),
            lamports: u64::from(seed).wrapping_mul(7).wrapping_add(1),
            data: vec![seed.wrapping_add(3); usize::from(seed) % 11],
            is_signer: seed.is_multiple_of(2),
            is_writable: seed.is_multiple_of(3),
            executable: seed.is_multiple_of(5),
        }
    }

    fn serialize(slots: &[SlotSpec], instruction_data: &[u8], program_id: &Pubkey) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(&as_u64(slots.len()).to_le_bytes());
        for slot in slots {
            match slot {
                SlotSpec::Fresh(acct) => {
                    out.push(NON_DUP_MARKER);
                    out.push(u8::from(acct.is_signer));
                    out.push(u8::from(acct.is_writable));
                    out.push(u8::from(acct.executable));
                    out.extend_from_slice(&[0_u8; 4]);
                    out.extend_from_slice(acct.key.as_ref());
                    out.extend_from_slice(acct.owner.as_ref());
                    out.extend_from_slice(&acct.lamports.to_le_bytes());
                    out.extend_from_slice(&as_u64(acct.data.len()).to_le_bytes());
                    out.extend_from_slice(&acct.data);
                    let padding = acct.data.len().wrapping_neg() % BPF_ALIGN_OF_U128;
                    out.resize(out.len() + MAX_PERMITTED_DATA_INCREASE + padding, 0);
                    // Agave masks the rent epoch out with u64::MAX; nothing
                    // deserializes it, which is exactly what must be proven.
                    out.extend_from_slice(&u64::MAX.to_le_bytes());
                }
                SlotSpec::Dup(index) => {
                    out.push(*index);
                    out.extend_from_slice(&[0_u8; 7]);
                }
            }
        }
        out.extend_from_slice(&as_u64(instruction_data.len()).to_le_bytes());
        out.extend_from_slice(instruction_data);
        out.extend_from_slice(program_id.as_ref());
        out
    }

    /// An input buffer with the 8-byte alignment the loader's region has.
    struct AlignedInput {
        words: Vec<u64>,
        len: usize,
    }

    impl AlignedInput {
        fn new(bytes: &[u8]) -> Self {
            let words = bytes.len().div_ceil(8);
            let mut backing: Vec<u64> = vec![0; words];
            {
                let raw = unsafe {
                    slice::from_raw_parts_mut(backing.as_mut_ptr().cast::<u8>(), words * 8)
                };
                raw.get_mut(..bytes.len())
                    .expect("backing covers bytes")
                    .copy_from_slice(bytes);
            }
            Self {
                words: backing,
                len: bytes.len(),
            }
        }

        fn as_mut_ptr(&mut self) -> *mut u8 {
            self.words.as_mut_ptr().cast::<u8>()
        }

        fn bytes(&self) -> &[u8] {
            unsafe { slice::from_raw_parts(self.words.as_ptr().cast::<u8>(), self.len) }
        }
    }

    fn assert_account_equal(sdk: &AccountInfo<'_>, mine: &AccountInfo<'_>, at: usize) {
        assert_eq!(sdk.key, mine.key, "key at {at}");
        assert_eq!(sdk.owner, mine.owner, "owner at {at}");
        assert_eq!(sdk.is_signer, mine.is_signer, "is_signer at {at}");
        assert_eq!(sdk.is_writable, mine.is_writable, "is_writable at {at}");
        assert_eq!(sdk.executable, mine.executable, "executable at {at}");
        assert_eq!(sdk.lamports(), mine.lamports(), "lamports at {at}");
        assert_eq!(
            *sdk.try_borrow_data().expect("sdk data"),
            *mine.try_borrow_data().expect("adapter data"),
            "data at {at}"
        );
    }

    /// Run the SDK entrypoint deserializer and this adapter over two
    /// byte-identical copies of the same loader buffer and prove they agree on
    /// every produced value, on the duplicate aliasing, and on the bytes they
    /// write back into the buffer.
    fn differential(slots: &[SlotSpec], instruction_data: &[u8]) {
        let program_id = Pubkey::new_from_array([0xAB; 32]);
        let bytes = serialize(slots, instruction_data, &program_id);
        let mut reference = AlignedInput::new(&bytes);
        let mut subject = AlignedInput::new(&bytes);

        let (sdk_program_id, sdk_accounts, sdk_data) =
            unsafe { sdk_deserialize(reference.as_mut_ptr()) };

        let mut destination: Vec<MaybeUninit<AccountInfo<'_>>> = Vec::new();
        destination.resize_with(slots.len().max(1), MaybeUninit::uninit);
        let limit = subject.len;
        let (mine_program_id, count, mine_data) =
            unsafe { deserialize_into_v1(subject.as_mut_ptr(), limit, &mut destination) }
                .expect("adapter deserialization");
        let written = destination.get(..count).expect("written slots");
        let mine_accounts = unsafe {
            &*(core::ptr::from_ref::<[MaybeUninit<AccountInfo<'_>>]>(written)
                as *const [AccountInfo<'_>])
        };

        assert_eq!(count, slots.len(), "slot count");
        assert_eq!(sdk_accounts.len(), count, "sdk slot count");
        assert_eq!(sdk_program_id, mine_program_id, "program id");
        assert_eq!(sdk_data, mine_data, "instruction data");
        for (at, (sdk, mine)) in sdk_accounts.iter().zip(mine_accounts).enumerate() {
            assert_account_equal(sdk, mine, at);
        }

        // Duplicate slots must alias one `Rc` in both, and non-duplicates must
        // not alias at all.
        for (at, slot) in slots.iter().enumerate() {
            if let SlotSpec::Dup(source) = slot {
                let source = usize::from(*source);
                let original = mine_accounts.get(source).expect("dup source");
                let clone = mine_accounts.get(at).expect("dup slot");
                assert!(
                    std::rc::Rc::ptr_eq(&original.lamports, &clone.lamports),
                    "dup {at} must alias lamports of {source}"
                );
                assert!(
                    std::rc::Rc::ptr_eq(&original.data, &clone.data),
                    "dup {at} must alias data of {source}"
                );
                let sdk_original = sdk_accounts.get(source).expect("sdk dup source");
                let sdk_clone = sdk_accounts.get(at).expect("sdk dup slot");
                assert!(
                    std::rc::Rc::ptr_eq(&sdk_original.lamports, &sdk_clone.lamports),
                    "sdk dup {at} must alias"
                );
            }
        }

        // The original-data-length write-back is the only mutation either
        // deserializer performs; byte equality proves the adapter performs it
        // at exactly the same offsets with exactly the same values.
        assert_eq!(
            reference.bytes(),
            subject.bytes(),
            "buffers must be identical after both deserializers ran"
        );
    }

    #[test]
    fn differential_zero_accounts() {
        differential(&[], &[]);
    }

    #[test]
    fn differential_one_account() {
        differential(&[SlotSpec::Fresh(account(1))], &[9, 8, 7]);
    }

    #[test]
    fn differential_sixty_four_accounts() {
        let slots: Vec<SlotSpec> = (0..64)
            .map(|index| SlotSpec::Fresh(account(u8::try_from(index).expect("seed"))))
            .collect();
        differential(&slots, &[1, 2, 3, 4]);
    }

    #[test]
    fn differential_canonical_seventy_eight_accounts() {
        let slots: Vec<SlotSpec> = (0..78)
            .map(|index| SlotSpec::Fresh(account(u8::try_from(index).expect("seed"))))
            .collect();
        differential(&slots, &[0xDC; 1_224]);
    }

    #[test]
    fn differential_declared_maximum_account_frame() {
        let slots: Vec<SlotSpec> = (0..crate::TRADING_MAX_INSTRUCTION_ACCOUNTS_V3)
            .map(|index| SlotSpec::Fresh(account(u8::try_from(index % 251).expect("seed"))))
            .collect();
        differential(&slots, &[7; 32]);
    }

    #[test]
    fn differential_duplicates_in_every_position() {
        // first, middle, last, and the same source repeated.
        let mut slots: Vec<SlotSpec> = Vec::new();
        slots.push(SlotSpec::Fresh(account(1)));
        slots.push(SlotSpec::Dup(0));
        for seed in 2..10_u8 {
            slots.push(SlotSpec::Fresh(account(seed)));
        }
        slots.push(SlotSpec::Dup(0));
        slots.push(SlotSpec::Dup(5));
        slots.push(SlotSpec::Fresh(account(20)));
        slots.push(SlotSpec::Dup(12));
        differential(&slots, &[42]);
    }

    #[test]
    fn differential_zero_length_and_long_data() {
        let mut empty = account(3);
        empty.data = Vec::new();
        let mut long = account(4);
        long.data = vec![0x5A; 10_240];
        let mut odd = account(5);
        odd.data = vec![0x11; 1];
        differential(
            &[
                SlotSpec::Fresh(empty),
                SlotSpec::Fresh(long),
                SlotSpec::Fresh(odd),
            ],
            &[],
        );
    }

    #[test]
    fn differential_every_privilege_combination() {
        let mut slots: Vec<SlotSpec> = Vec::new();
        for mask in 0..8_u8 {
            let mut acct = account(mask.wrapping_add(60));
            acct.is_signer = mask & 1 != 0;
            acct.is_writable = mask & 2 != 0;
            acct.executable = mask & 4 != 0;
            slots.push(SlotSpec::Fresh(acct));
        }
        differential(&slots, &[0xFF, 0x00]);
    }

    // -----------------------------------------------------------------
    // Adversarial corpus. Every case runs with the cursor limit set to the
    // true buffer length, so a refusal is proof the parser stopped inside the
    // region rather than proof the test got lucky.
    // -----------------------------------------------------------------

    fn refuse(bytes: &[u8], capacity: usize) -> ProgramError {
        let mut input = AlignedInput::new(bytes);
        let mut destination: Vec<MaybeUninit<AccountInfo<'_>>> = Vec::new();
        destination.resize_with(capacity, MaybeUninit::uninit);
        let limit = input.len;
        unsafe { deserialize_into_v1(input.as_mut_ptr(), limit, &mut destination) }
            .expect_err("must refuse")
    }

    #[test]
    fn refuses_more_accounts_than_capacity() {
        let slots: Vec<SlotSpec> = (0..4)
            .map(|index| SlotSpec::Fresh(account(u8::try_from(index).expect("seed"))))
            .collect();
        let bytes = serialize(&slots, &[], &Pubkey::new_from_array([1; 32]));
        assert_eq!(
            refuse(&bytes, 3),
            TradingSbfError::UnsupportedContent.into()
        );
    }

    #[test]
    fn refuses_absurd_account_count() {
        let mut bytes = serialize(&[], &[], &Pubkey::new_from_array([1; 32]));
        bytes
            .get_mut(..8)
            .expect("count")
            .copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            refuse(&bytes, 8),
            TradingSbfError::UnsupportedContent.into()
        );
    }

    #[test]
    fn refuses_self_referential_duplicate() {
        let bytes = serialize(
            &[SlotSpec::Fresh(account(1)), SlotSpec::Dup(1)],
            &[],
            &Pubkey::new_from_array([1; 32]),
        );
        assert_eq!(refuse(&bytes, 8), TradingSbfError::Content.into());
    }

    #[test]
    fn refuses_forward_duplicate() {
        let bytes = serialize(
            &[SlotSpec::Dup(0), SlotSpec::Fresh(account(1))],
            &[],
            &Pubkey::new_from_array([1; 32]),
        );
        assert_eq!(refuse(&bytes, 8), TradingSbfError::Content.into());
    }

    #[test]
    fn refuses_out_of_range_duplicate() {
        let bytes = serialize(
            &[SlotSpec::Fresh(account(1)), SlotSpec::Dup(200)],
            &[],
            &Pubkey::new_from_array([1; 32]),
        );
        assert_eq!(refuse(&bytes, 8), TradingSbfError::Content.into());
    }

    #[test]
    fn refuses_every_truncation_of_a_real_buffer() {
        let slots = [
            SlotSpec::Fresh(account(1)),
            SlotSpec::Dup(0),
            SlotSpec::Fresh(account(2)),
        ];
        let bytes = serialize(&slots, &[3, 1, 4], &Pubkey::new_from_array([1; 32]));
        // Cut at every field boundary the parser can be inside, plus a dense
        // sweep of the head where the account records live.
        let mut cuts: Vec<usize> = (0..200).collect();
        cuts.extend([
            bytes.len() - 1,
            bytes.len() - 32,
            bytes.len() - 33,
            bytes.len() - 40,
            bytes.len() / 2,
        ]);
        for cut in cuts {
            let truncated = bytes.get(..cut).expect("cut inside buffer");
            let error = refuse(truncated, 8);
            assert!(
                error == TradingSbfError::Content.into()
                    || error == TradingSbfError::UnsupportedContent.into(),
                "truncation at {cut} must refuse, got {error:?}"
            );
        }
    }

    #[test]
    fn refuses_absurd_account_data_length() {
        let mut bytes = serialize(
            &[SlotSpec::Fresh(account(1))],
            &[],
            &Pubkey::new_from_array([1; 32]),
        );
        // count(8) + dup(1) + flags(3) + pad(4) + key(32) + owner(32) + lamports(8)
        let data_len_at = 8 + 1 + 3 + 4 + 32 + 32 + 8;
        bytes
            .get_mut(data_len_at..data_len_at + 8)
            .expect("data len")
            .copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(refuse(&bytes, 8), TradingSbfError::Content.into());
    }

    #[test]
    fn refuses_account_data_length_above_u32() {
        let mut bytes = serialize(
            &[SlotSpec::Fresh(account(1))],
            &[],
            &Pubkey::new_from_array([1; 32]),
        );
        let data_len_at = 8 + 1 + 3 + 4 + 32 + 32 + 8;
        bytes
            .get_mut(data_len_at..data_len_at + 8)
            .expect("data len")
            .copy_from_slice(&(u64::from(u32::MAX) + 1).to_le_bytes());
        assert_eq!(refuse(&bytes, 8), TradingSbfError::Content.into());
    }

    #[test]
    fn refuses_absurd_instruction_data_length() {
        let mut bytes = serialize(&[], &[1, 2, 3], &Pubkey::new_from_array([1; 32]));
        bytes
            .get_mut(8..16)
            .expect("instruction data len")
            .copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(refuse(&bytes, 8), TradingSbfError::Content.into());
    }

    // -----------------------------------------------------------------
    // The allocator
    // -----------------------------------------------------------------

    #[repr(align(16))]
    struct HeapBacking([u8; ADAPTER_MAX_HEAP_BYTES]);

    struct TestHeap {
        backing: Box<HeapBacking>,
    }

    impl TestHeap {
        fn new() -> Self {
            Self {
                backing: Box::new(HeapBacking([0; ADAPTER_MAX_HEAP_BYTES])),
            }
        }

        fn allocator(&mut self) -> BumpHeapV1 {
            unsafe { BumpHeapV1::with_base(self.backing.0.as_mut_ptr() as usize) }
        }
    }

    // `BumpHeapV1::alloc` is a `GlobalAlloc` method, not an inherent one. The
    // trait used to be in scope for the whole file; the extraction to
    // `dclutch-sbf-runtime` narrowed that import to `Layout`, which is right
    // for the module and left these tests unable to name `alloc`. Scoped to the
    // tests that call it rather than restored file-wide.
    use core::{alloc::GlobalAlloc, ptr::null_mut};

    fn layout(size: usize, align: usize) -> Layout {
        Layout::from_size_align(size, align).expect("layout")
    }

    #[test]
    fn allocator_starts_after_its_header_and_bumps_upward() {
        let mut heap = TestHeap::new();
        let allocator = heap.allocator();
        assert_eq!(allocator.bytes_used(), HEAP_HEADER_BYTES);
        assert_eq!(allocator.bytes_capacity(), ADAPTER_DEFAULT_HEAP_BYTES);
        let first = unsafe { allocator.alloc(layout(24, 8)) };
        let second = unsafe { allocator.alloc(layout(24, 8)) };
        assert!(!first.is_null() && !second.is_null());
        assert!(second as usize > first as usize, "must bump upward");
        assert_eq!(allocator.bytes_used(), HEAP_HEADER_BYTES + 48);
    }

    #[test]
    fn allocator_honours_alignment() {
        let mut heap = TestHeap::new();
        let allocator = heap.allocator();
        let _ = unsafe { allocator.alloc(layout(1, 1)) };
        for align in [1_usize, 2, 4, 8, 16, 32, 64, 128] {
            let block = unsafe { allocator.alloc(layout(3, align)) };
            assert!(!block.is_null(), "align {align}");
            assert_eq!(block as usize % align, 0, "align {align}");
        }
    }

    #[test]
    fn allocator_refuses_past_the_default_ceiling() {
        let mut heap = TestHeap::new();
        let allocator = heap.allocator();
        let fits = ADAPTER_DEFAULT_HEAP_BYTES - HEAP_HEADER_BYTES;
        let block = unsafe { allocator.alloc(layout(fits, 8)) };
        assert!(!block.is_null(), "the exact remainder must fit");
        assert!(unsafe { allocator.alloc(layout(1, 1)) }.is_null());
    }

    #[test]
    fn allocator_dealloc_is_a_no_op_exactly_as_the_sdk_is() {
        // Deliberate, and measured: see `BumpHeapV1`'s documentation. A
        // last-in-first-out rewind here reclaims every dropped temporary but
        // cannot be optimized away at its call sites, and
        // `process_hot_execution_v3` has no frame left to pay for that. This
        // test exists so reinstating it is a visible decision rather than a
        // silent behaviour change.
        let mut heap = TestHeap::new();
        let allocator = heap.allocator();
        let block = unsafe { allocator.alloc(layout(4_096, 8)) };
        let high_water = allocator.bytes_used();
        assert_eq!(high_water, HEAP_HEADER_BYTES + 4_096);
        unsafe { allocator.dealloc(block, layout(4_096, 8)) };
        assert_eq!(
            allocator.bytes_used(),
            high_water,
            "the bump position must not move on release"
        );
        let next = unsafe { allocator.alloc(layout(8, 8)) };
        assert!(next as usize > block as usize + 4_095, "no block is reused");
    }

    #[test]
    fn allocator_reallocation_goes_through_the_default_allocate_copy_release() {
        // `realloc` is deliberately NOT overridden; see `BumpHeapV1`, which
        // records the measurement that withdrew the in-place version. This
        // pins the consequence, so reinstating it is a visible decision: a
        // grown block always moves, and its contents survive the move.
        let mut heap = TestHeap::new();
        let allocator = heap.allocator();
        let block = unsafe { allocator.alloc(layout(64, 8)) };
        unsafe { slice::from_raw_parts_mut(block, 64) }.fill(0xC7);
        let grown = unsafe { allocator.realloc(block, layout(64, 8), 4_096) };
        assert!(!grown.is_null());
        assert_ne!(grown, block, "the default realloc relocates");
        assert!(
            unsafe { slice::from_raw_parts(grown, 64) }
                .iter()
                .all(|byte| *byte == 0xC7),
            "reallocation must preserve the block's contents"
        );
        assert_eq!(allocator.bytes_used(), HEAP_HEADER_BYTES + 64 + 4_096);
        // Growing past the ceiling refuses rather than handing back memory the
        // runtime never mapped.
        assert_eq!(
            unsafe { allocator.realloc(grown, layout(4_096, 8), ADAPTER_DEFAULT_HEAP_BYTES) },
            null_mut()
        );
    }

    #[test]
    fn allocator_ceiling_lifts_only_to_an_authenticated_grant() {
        let mut heap = TestHeap::new();
        let allocator = heap.allocator();
        assert_eq!(allocator.bytes_capacity(), ADAPTER_DEFAULT_HEAP_BYTES);
        // Below the protocol default, above the runtime maximum, and not a
        // multiple of the runtime's granularity are all refusals.
        assert!(
            allocator
                .lift_ceiling(ADAPTER_DEFAULT_HEAP_BYTES - 1_024)
                .is_err()
        );
        assert!(
            allocator
                .lift_ceiling(ADAPTER_MAX_HEAP_BYTES + 1_024)
                .is_err()
        );
        assert!(allocator.lift_ceiling(33_000).is_err());
        assert_eq!(allocator.bytes_capacity(), ADAPTER_DEFAULT_HEAP_BYTES);
        assert_eq!(allocator.lift_ceiling(65_536), Ok(65_536));
        assert_eq!(allocator.bytes_capacity(), 65_536);
        // Monotone: it never comes back down.
        assert!(allocator.lift_ceiling(ADAPTER_DEFAULT_HEAP_BYTES).is_err());
        assert_eq!(allocator.bytes_capacity(), 65_536);
    }

    #[test]
    fn allocator_ceiling_lift_is_order_independent() {
        let mut heap = TestHeap::new();
        let allocator = heap.allocator();
        let fits = ADAPTER_DEFAULT_HEAP_BYTES - HEAP_HEADER_BYTES;
        let live = unsafe { allocator.alloc(layout(fits, 8)) };
        assert!(!live.is_null());
        assert!(unsafe { allocator.alloc(layout(1, 1)) }.is_null());
        assert_eq!(allocator.lift_ceiling(65_536), Ok(65_536));
        let after = unsafe { allocator.alloc(layout(16_384, 8)) };
        assert!(!after.is_null(), "the lift must open the extra region");
        assert_eq!(
            unsafe { allocator.alloc(layout(fits, 8)) },
            null_mut(),
            "and only the extra region"
        );
        // The pre-lift allocation is untouched.
        assert_eq!(
            live as usize,
            allocator_base(&allocator) + HEAP_HEADER_BYTES
        );
    }

    fn allocator_base(allocator: &BumpHeapV1) -> usize {
        allocator.base()
    }

    // -----------------------------------------------------------------
    // The scratch end
    // -----------------------------------------------------------------

    #[test]
    fn scratch_end_bumps_downward_from_the_ceiling() {
        let mut heap = TestHeap::new();
        let allocator = heap.allocator();
        assert_eq!(allocator.scratch_bytes_used(), 0);
        let first = unsafe { allocator.alloc_scratch(layout(24, 8)) };
        let second = unsafe { allocator.alloc_scratch(layout(24, 8)) };
        assert!(!first.is_null() && !second.is_null());
        assert!(
            (second as usize) < first as usize,
            "the scratch end must bump downward"
        );
        assert_eq!(
            first as usize + 24,
            allocator_base(&allocator) + ADAPTER_DEFAULT_HEAP_BYTES,
            "the first scratch block ends exactly at the ceiling"
        );
        assert_eq!(allocator.scratch_bytes_used(), 48);
        // The upward end is untouched by any of it.
        assert_eq!(allocator.bytes_used(), HEAP_HEADER_BYTES);
    }

    #[test]
    fn the_two_ends_refuse_to_cross_each_other() {
        let mut heap = TestHeap::new();
        let allocator = heap.allocator();
        // Just under half the default heap each, so the two together leave a
        // gap smaller than either.
        let half = 16_000_usize;
        let low = unsafe { allocator.alloc(layout(half, 8)) };
        let high = unsafe { allocator.alloc_scratch(layout(half, 8)) };
        assert!(!low.is_null() && !high.is_null());
        assert!((low as usize) + half <= high as usize, "disjoint");
        assert_eq!(allocator.bytes_used(), HEAP_HEADER_BYTES + half);
        assert_eq!(allocator.scratch_bytes_used(), half);
        // BOTH ends now refuse rather than handing out the other's bytes.
        assert!(unsafe { allocator.alloc(layout(half, 8)) }.is_null());
        assert!(unsafe { allocator.alloc_scratch(layout(half, 8)) }.is_null());
    }

    #[test]
    fn releasing_the_scratch_end_returns_every_block_in_it_at_once() {
        let mut heap = TestHeap::new();
        let allocator = heap.allocator();
        let mark = allocator.open_scratch().expect("no region open yet");
        assert_eq!(mark, ADAPTER_DEFAULT_HEAP_BYTES);
        let first = unsafe { allocator.alloc_scratch(layout(4_096, 8)) };
        let _second = unsafe { allocator.alloc_scratch(layout(1_024, 8)) };
        assert_eq!(allocator.scratch_bytes_used(), 5_120);
        // A second region cannot be opened while this one is.
        assert!(allocator.open_scratch().is_err());
        unsafe { allocator.release_scratch(mark) };
        assert_eq!(allocator.scratch_bytes_used(), 0);
        assert!(allocator.open_scratch().is_ok(), "and reopening works");
        // The next region starts back at the ceiling: the released bytes are
        // handed out again, which is the whole point and also why a live
        // reference into them would be a use-after-free. `ScratchVecV1`
        // borrows the region so the borrow checker forbids one.
        let again = unsafe { allocator.alloc_scratch(layout(4_096, 8)) };
        assert_eq!(again, first);
    }

    #[test]
    fn a_scratch_bank_refuses_past_its_reserved_capacity() {
        let region = HeapScratchRegionV1::open().expect("region");
        let mut bank = ScratchVecV1::<u32>::with_capacity(&region, 2).expect("bank");
        assert!(bank.push(7).is_ok());
        assert!(bank.push(9).is_ok());
        // A `Vec` would reallocate here and strand the smaller copy; a scratch
        // bank has one exact width and refuses instead.
        assert!(bank.push(11).is_err());
        assert_eq!(bank.as_slice(), &[7, 9]);
        assert_eq!(&bank[..], &[7, 9]);
        *bank
            .as_mut_slice()
            .first_mut()
            .expect("the bank holds two elements") = 5;
        assert_eq!(bank.as_slice(), &[5, 9]);
    }

    #[test]
    fn a_scratch_bank_drops_the_elements_it_holds() {
        use std::rc::Rc;

        let region = HeapScratchRegionV1::open().expect("region");
        let witness = Rc::new(());
        {
            let mut bank = ScratchVecV1::with_capacity(&region, 3).expect("bank");
            bank.push(Rc::clone(&witness)).expect("push");
            bank.push(Rc::clone(&witness)).expect("push");
            assert_eq!(Rc::strong_count(&witness), 3);
        }
        assert_eq!(
            Rc::strong_count(&witness),
            1,
            "the bank must run its elements' destructors"
        );
    }

    #[test]
    fn exactly_one_scratch_region_may_be_open() {
        let region = HeapScratchRegionV1::open().expect("first");
        assert!(
            HeapScratchRegionV1::open().is_err(),
            "a second region would be released out of order and hand live \
             bytes out twice; it refuses instead"
        );
        drop(region);
        assert!(HeapScratchRegionV1::open().is_ok());
    }

    #[test]
    fn a_zero_length_scratch_bank_owns_no_block() {
        let region = HeapScratchRegionV1::open().expect("region");
        let bank = ScratchVecV1::<u64>::with_capacity(&region, 0).expect("bank");
        assert!(bank.as_slice().is_empty());
        let filled = ScratchVecV1::filled(&region, &0_u8, 4).expect("filled");
        assert_eq!(filled.as_slice(), &[0, 0, 0, 0]);
    }

    // -----------------------------------------------------------------
    // Heap-frame admission out of the instructions sysvar
    // -----------------------------------------------------------------
    mod admission {
        use super::*;

        fn request_heap_frame_data(bytes: u32) -> Vec<u8> {
            let mut data = vec![REQUEST_HEAP_FRAME_DISCRIMINANT];
            data.extend_from_slice(&bytes.to_le_bytes());
            data
        }

        /// Build the sysvar bytes the runtime would serialize for a transaction
        /// carrying `instructions`, using the runtime's own constructor.
        fn sysvar_bytes(instructions: &[(Pubkey, Vec<u8>)]) -> Vec<u8> {
            let borrowed: Vec<solana_program::sysvar::instructions::BorrowedInstruction<'_>> =
                instructions
                    .iter()
                    .map(|(program_id, data)| {
                        solana_program::sysvar::instructions::BorrowedInstruction {
                            program_id,
                            accounts: Vec::new(),
                            data,
                        }
                    })
                    .collect();
            solana_instructions_sysvar::construct_instructions_data(&borrowed)
        }

        fn admitted(instructions: &[(Pubkey, Vec<u8>)]) -> Result<Option<usize>, ProgramError> {
            admitted_heap_frame_bytes_from_sysvar_v1(&sysvar_bytes(instructions))
        }

        fn compute_budget() -> Pubkey {
            Pubkey::new_from_array(solana_sdk_ids::compute_budget::ID.to_bytes())
        }

        fn some_other_program() -> Pubkey {
            Pubkey::new_from_array([0x7E; 32])
        }

        #[test]
        fn admits_a_real_runtime_heap_grant() {
            for bytes in [32_768_u32, 65_536, 131_072, 262_144] {
                assert_eq!(
                    admitted(&[
                        (some_other_program(), vec![1, 2, 3]),
                        (compute_budget(), request_heap_frame_data(bytes)),
                    ]),
                    Ok(Some(usize::try_from(bytes).expect("grant"))),
                    "grant {bytes}"
                );
            }
        }

        #[test]
        fn admits_nothing_when_the_transaction_requested_nothing() {
            assert_eq!(admitted(&[]), Ok(None));
            assert_eq!(admitted(&[(some_other_program(), vec![9; 40])]), Ok(None));
            // A ComputeBudget instruction that is not a heap-frame request.
            let mut set_limit = vec![2_u8];
            set_limit.extend_from_slice(&1_400_000_u32.to_le_bytes());
            assert_eq!(admitted(&[(compute_budget(), set_limit)]), Ok(None));
            // An empty ComputeBudget payload is not a heap-frame request either.
            assert_eq!(admitted(&[(compute_budget(), Vec::new())]), Ok(None));
        }

        #[test]
        fn admits_a_grant_with_trailing_bytes_exactly_as_borsh_unchecked_does() {
            // Agave decodes with `try_from_slice_unchecked`, which ignores trailing
            // bytes, so the runtime would have granted 65,536 here. Refusing would
            // make this adapter disagree with the heap it was actually given.
            let mut data = request_heap_frame_data(65_536);
            data.extend_from_slice(&[0xAA; 7]);
            assert_eq!(admitted(&[(compute_budget(), data)]), Ok(Some(65_536)));
        }

        #[test]
        fn refuses_a_grant_the_runtime_would_have_rejected() {
            // Below MIN_HEAP_FRAME_BYTES, above MAX_HEAP_FRAME_BYTES, and not a
            // multiple of 1,024 are exactly agave's `sanitize_requested_heap_size`
            // rejections. None can reach an executing program, so refuse.
            for bytes in [0_u32, 1_024, 31_744, 33_000, 263_168, u32::MAX] {
                assert!(
                    admitted(&[(compute_budget(), request_heap_frame_data(bytes))]).is_err(),
                    "unsanitized request {bytes} must refuse"
                );
            }
        }

        #[test]
        fn refuses_two_heap_grants() {
            assert!(
                admitted(&[
                    (compute_budget(), request_heap_frame_data(65_536)),
                    (compute_budget(), request_heap_frame_data(131_072)),
                ])
                .is_err(),
                "the runtime rejects this transaction outright; refuse rather than pick"
            );
        }

        #[test]
        fn refuses_a_truncated_sysvar() {
            let bytes = sysvar_bytes(&[(compute_budget(), request_heap_frame_data(65_536))]);
            // The last two bytes are the current-instruction index, which this
            // scanner never reads; every byte before them is load-bearing.
            let body_end = bytes.len() - 2;
            for cut in 0..bytes.len() {
                let truncated = bytes.get(..cut).expect("cut inside sysvar");
                let outcome = admitted_heap_frame_bytes_from_sysvar_v1(truncated);
                assert!(
                    outcome.is_err() || outcome == Ok(Some(65_536)),
                    "truncation at {cut} must refuse or reproduce the grant exactly, got {outcome:?}"
                );
                if cut < body_end {
                    assert!(
                        outcome.is_err(),
                        "truncation at {cut} cuts the instruction and must refuse"
                    );
                }
            }
        }

        #[test]
        fn refuses_a_sysvar_account_that_is_not_the_sysvar() {
            let mut data = sysvar_bytes(&[(compute_budget(), request_heap_frame_data(65_536))]);
            let mut lamports = 1_u64;
            let owner = Pubkey::new_from_array([0x5D; 32]);
            let impostor = Pubkey::new_from_array([0x11; 32]);
            let account = AccountInfo::new(
                &impostor,
                false,
                false,
                &mut lamports,
                data.as_mut_slice(),
                &owner,
                false,
            );
            assert!(admitted_heap_frame_bytes_v1(&account).is_err());
        }

        #[test]
        fn admits_through_the_real_sysvar_account() {
            let mut data = sysvar_bytes(&[(compute_budget(), request_heap_frame_data(131_072))]);
            let mut lamports = 1_u64;
            let owner = Pubkey::new_from_array([0x5D; 32]);
            let account = AccountInfo::new(
                &solana_sdk_ids::sysvar::instructions::ID,
                false,
                false,
                &mut lamports,
                data.as_mut_slice(),
                &owner,
                false,
            );
            assert_eq!(admitted_heap_frame_bytes_v1(&account), Ok(Some(131_072)));
        }

        // -----------------------------------------------------------------
        // The extended-heap policy list
        // -----------------------------------------------------------------

        /// The SHIPPED policy: only the founding routes may declare it.
        ///
        /// `hot-cu-profile` deliberately suspends this policy --
        /// [`hot_cu_profile_lifts_every_route_v1`] says so and says why -- so
        /// this test is about the build that ships and its negative assertions
        /// belong to that build. Without the gate below it is RED under
        /// `--features hot-cu-profile` and therefore under `--all-features`, at
        /// its very first line, and a lane running the suite that way reads a
        /// policy regression that is not there. Measured 2026-09-02: green with
        /// default features at every commit in the range, red at
        /// `assert!(!declares_extended_heap_profile_v1(&[]))` with the profile
        /// feature on, at every one of them.
        ///
        /// The feature's own contract is pinned beside it rather than skipped,
        /// because a test that merely disappears under a feature says nothing
        /// about what that feature does.
        #[cfg(not(feature = "hot-cu-profile"))]
        #[test]
        fn only_the_founding_routes_declare_an_extended_heap_profile() {
            assert!(!declares_extended_heap_profile_v1(&[]));
            // The Hot execution discriminator must never be on the list.
            assert!(!declares_extended_heap_profile_v1(&[0xDC; 96]));
            #[cfg(any(
                feature = "families",
                feature = "series-family",
                feature = "dealer-family"
            ))]
            {
                let mut founding = vec![
                    0_u8;
                    crate::generic_market_founding_v1::GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3
                ];
                founding[..8].copy_from_slice(
                    &crate::generic_market_founding_v1::GENERIC_MARKET_FOUNDING_MAGIC_V3,
                );
                let mut stage1 = vec![
                    0_u8;
                    crate::generic_founding_stages_v1::GENERIC_FOUND_AND_PERMIT_INSTRUCTION_BYTES_V1
                ];
                stage1[..8].copy_from_slice(
                    &crate::generic_founding_stages_v1::GENERIC_FOUND_AND_PERMIT_MAGIC_V1,
                );
                for magic in [
                    founding,
                    stage1,
                    crate::projected_custody_bootstrap_v1::PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V2
                        .to_vec(),
                ] {
                    assert!(declares_extended_heap_profile_v1(&magic));
                    let mut nearly = magic.clone();
                    nearly[7] = nearly[7].wrapping_add(1);
                    assert!(!declares_extended_heap_profile_v1(&nearly));
                    assert!(!declares_extended_heap_profile_v1(
                        magic.get(..7).expect("prefix")
                    ));
                }
                // The split founding's stage 2 stays on the 32 KiB discipline:
                // its frame is two raw accounts and Core's 21-account Open
                // window, and keeping it off this list is a deliberate
                // property, not an omission.
                let mut open = vec![
                    0_u8;
                    crate::generic_founding_stages_v1::GENERIC_MARKET_OPEN_INSTRUCTION_BYTES_V1
                ];
                open[..8].copy_from_slice(
                    &crate::generic_founding_stages_v1::GENERIC_MARKET_OPEN_MAGIC_V1,
                );
                assert!(crate::generic_founding_stages_v1::is_generic_market_open_v1(&open));
                assert!(!declares_extended_heap_profile_v1(&open));
            }
        }

        /// The diagnostic build's contract, stated where its suspension is.
        ///
        /// `hot-cu-profile` lifts the ceiling for EVERY route, which is the
        /// whole reason the phase table can be taken past the 32 KiB wall at
        /// all. That is not a policy this program ships -- `hot_heap_frame_is_inert`
        /// fails on an ELF carrying it -- but while the feature is on it is the
        /// behaviour, and asserting it is what makes the sibling test's
        /// `cfg(not(...))` a statement rather than a way to stop a red.
        #[cfg(feature = "hot-cu-profile")]
        #[test]
        fn the_diagnostic_profile_lifts_every_route_including_the_empty_one() {
            assert!(hot_cu_profile_lifts_every_route_v1());
            assert!(declares_extended_heap_profile_v1(&[]));
            assert!(declares_extended_heap_profile_v1(&[0xDC; 96]));
        }
    }

    // -----------------------------------------------------------------
    // The child-invocation membrane
    // -----------------------------------------------------------------

    /// One account's backing storage, so an `AccountInfo` can hold `&mut` into
    /// it for the whole of a case.
    struct Backing {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: [u8; 4],
    }

    impl Backing {
        fn new(seed: u8) -> Self {
            Self {
                key: Pubkey::new_from_array([seed; 32]),
                owner: Pubkey::new_from_array([seed.wrapping_add(64); 32]),
                lamports: u64::from(seed),
                data: [seed; 4],
            }
        }

        fn info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                false,
                true,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                false,
            )
        }
    }

    /// What both paths say about one frame: the membrane's answer and the
    /// SDK's, from the SAME metas and the SAME accounts.
    ///
    /// On a host build `solana_program::program::invoke_signed` is exactly the
    /// `RefCell` consistency pre-check followed by the default syscall stub,
    /// which logs and returns `Ok(())`. So this comparison is precisely the
    /// comparison that matters: the reproduced pre-check against the one it was
    /// copied from, on acceptance and on refusal alike.
    fn both_paths(
        metas: &[AccountMeta],
        infos: &[AccountInfo<'_>],
    ) -> (Result<(), ProgramError>, Result<(), ProgramError>) {
        let program_id = Pubkey::new_from_array([0xEE; 32]);
        let mut membrane_metas = metas.to_vec();
        let mut membrane_data = vec![7_u8, 8, 9];
        let membrane = invoke_signed_owned_v1(
            &program_id,
            &mut membrane_metas,
            &mut membrane_data,
            infos,
            &[],
        );
        // The buffers come back, whatever the answer was.
        assert_eq!(membrane_metas, metas);
        assert_eq!(membrane_data, vec![7_u8, 8, 9]);

        let sdk = solana_program::program::invoke_signed(
            &Instruction {
                program_id,
                accounts: metas.to_vec(),
                data: vec![7_u8, 8, 9],
            },
            infos,
            &[],
        );
        (membrane, sdk)
    }

    #[test]
    fn child_invocation_borrow_check_matches_the_sdk() {
        let borrow_failed: Result<(), ProgramError> = Err(ProgramError::AccountBorrowFailed);

        // 1. Nothing borrowed: both admit, under either privilege.
        {
            let (mut a, mut b) = (Backing::new(1), Backing::new(2));
            let infos = [a.info(), b.info()];
            for writable in [false, true] {
                let metas = [
                    AccountMeta {
                        pubkey: infos[0].key.to_bytes().into(),
                        is_signer: false,
                        is_writable: writable,
                    },
                    AccountMeta {
                        pubkey: infos[1].key.to_bytes().into(),
                        is_signer: false,
                        is_writable: writable,
                    },
                ];
                let (membrane, sdk) = both_paths(&metas, &infos);
                assert_eq!(membrane, Ok(()));
                assert_eq!(membrane, sdk);
            }
        }

        // 2. A data borrow this program is still holding, against every
        //    combination of borrow kind and declared privilege. The two that
        //    conflict must refuse; the one that does not must not.
        for (hold_exclusive, writable, expected) in [
            (true, true, borrow_failed.clone()),
            (true, false, borrow_failed.clone()),
            (false, true, borrow_failed.clone()),
            (false, false, Ok(())),
        ] {
            let mut a = Backing::new(3);
            let infos = [a.info()];
            let metas = [AccountMeta {
                pubkey: infos[0].key.to_bytes().into(),
                is_signer: false,
                is_writable: writable,
            }];
            let answered = if hold_exclusive {
                let guard = infos[0].try_borrow_mut_data().expect("exclusive");
                let answered = both_paths(&metas, &infos);
                drop(guard);
                answered
            } else {
                let guard = infos[0].try_borrow_data().expect("shared");
                let answered = both_paths(&metas, &infos);
                drop(guard);
                answered
            };
            let (membrane, sdk) = answered;
            assert_eq!(
                membrane, expected,
                "exclusive={hold_exclusive} writable={writable}"
            );
            assert_eq!(
                membrane, sdk,
                "exclusive={hold_exclusive} writable={writable}"
            );
        }

        // 3. The LAMPORTS cell is checked too, and separately from the data
        //    cell: a held mutable lamports borrow refuses a writable meta whose
        //    data cell is entirely free.
        {
            let mut a = Backing::new(4);
            let infos = [a.info()];
            let metas = [AccountMeta {
                pubkey: infos[0].key.to_bytes().into(),
                is_signer: false,
                is_writable: true,
            }];
            let guard = infos[0].try_borrow_mut_lamports().expect("exclusive");
            let (membrane, sdk) = both_paths(&metas, &infos);
            assert_eq!(membrane, borrow_failed);
            assert_eq!(membrane, sdk);
            drop(guard);
        }

        // 4. A meta naming an account the frame does not carry is NOT an error
        //    here. The runtime refuses that, and refusing it earlier would be a
        //    different program.
        {
            let mut a = Backing::new(5);
            let infos = [a.info()];
            let metas = [AccountMeta {
                pubkey: Pubkey::new_from_array([0x5A; 32]),
                is_signer: false,
                is_writable: true,
            }];
            let (membrane, sdk) = both_paths(&metas, &infos);
            assert_eq!(membrane, Ok(()));
            assert_eq!(membrane, sdk);
        }

        // 5. The FIRST match wins, and the loop stops there. A frame carrying
        //    the same account twice, with the SECOND occurrence borrowed, is
        //    admitted -- because the first occurrence answered for the meta.
        //    This is the `break`, and it is observable.
        {
            let mut a = Backing::new(6);
            let clean = a.info();
            let duplicate = clean.clone();
            let infos = [clean, duplicate];
            let metas = [AccountMeta {
                pubkey: infos[0].key.to_bytes().into(),
                is_signer: false,
                is_writable: true,
            }];
            // Both `AccountInfo`s share one `RefCell`, so borrowing through the
            // second is borrowing through the first: the point of the case is
            // the ORDER of the walk, which is why the answer is a refusal and
            // both paths must give the same one.
            let guard = infos[1].try_borrow_mut_data().expect("exclusive");
            let (membrane, sdk) = both_paths(&metas, &infos);
            assert_eq!(membrane, borrow_failed);
            assert_eq!(membrane, sdk);
            drop(guard);
        }
    }

    /// The membrane hands the caller's buffers back with the allocation they
    /// arrived with, which is the whole reason it takes them by `&mut`.
    #[test]
    fn the_membrane_returns_the_buffers_it_was_lent() {
        let program_id = Pubkey::new_from_array([0xEE; 32]);
        let mut a = Backing::new(7);
        let infos = [a.info()];
        let mut metas = Vec::with_capacity(8);
        metas.push(AccountMeta {
            pubkey: infos[0].key.to_bytes().into(),
            is_signer: false,
            is_writable: false,
        });
        let mut data = Vec::with_capacity(64);
        data.extend_from_slice(b"child wire");
        let (metas_ptr, metas_cap) = (metas.as_ptr(), metas.capacity());
        let (data_ptr, data_cap) = (data.as_ptr(), data.capacity());

        invoke_signed_owned_v1(&program_id, &mut metas, &mut data, &infos, &[]).expect("stub");

        assert_eq!(metas.as_ptr(), metas_ptr);
        assert_eq!(metas.capacity(), metas_cap);
        assert_eq!(metas.len(), 1);
        assert_eq!(data.as_ptr(), data_ptr);
        assert_eq!(data.capacity(), data_cap);
        assert_eq!(data, b"child wire");
    }
}
