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
//! [`crate::TRADING_MAX_INSTRUCTION_ACCOUNTS_V3`] = 308, so the macro cannot be
//! adopted without regressing the bound.
//!
//! This adapter takes the SDK's stack-slot technique and adds the fallback the
//! macro lacks: up to [`ADAPTER_STACK_SLOTS_V1`] accounts are deserialized into
//! a stack-resident array and cost zero heap; beyond that the adapter falls
//! back to an exactly-sized heap buffer, so the 308 bound is preserved and
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

use core::{
    alloc::{GlobalAlloc, Layout},
    mem::MaybeUninit,
    ptr::null_mut,
    slice,
};

use solana_program::{
    account_info::AccountInfo,
    entrypoint::{BPF_ALIGN_OF_U128, HEAP_LENGTH, MAX_PERMITTED_DATA_INCREASE},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::TradingSbfError;

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
/// Chain-derived: `solana_program_entrypoint::HEAP_LENGTH`, which is also
/// Agave's `MIN_HEAP_FRAME_BYTES` and the heap size every transaction gets
/// without asking.
pub const ADAPTER_DEFAULT_HEAP_BYTES: usize = HEAP_LENGTH;

/// Largest heap frame the runtime will grant.
///
/// Chain-derived: Agave's `MAX_HEAP_FRAME_BYTES`
/// (`solana_program_runtime::execution_budget`), 256 KiB.
pub const ADAPTER_MAX_HEAP_BYTES: usize = 256 * 1024;

/// Granularity the runtime requires of a ComputeBudget heap-frame request.
///
/// Chain-derived: `ComputeBudgetInstructionDetails::sanitize_requested_heap_size`
/// requires the request to be a multiple of 1,024.
const HEAP_FRAME_GRANULARITY_BYTES: usize = 1_024;

/// ComputeBudget program instruction discriminant for `RequestHeapFrame(u32)`.
///
/// Chain-derived: `solana_compute_budget_interface::ComputeBudgetInstruction`
/// is borsh-encoded, and `RequestHeapFrame` is its second variant.
const REQUEST_HEAP_FRAME_DISCRIMINANT: u8 = 1;

/// Bytes the bump heap reserves at its floor for the allocator's own state.
///
/// Word 0 is the bump position as an offset from the heap floor, word 1 is the
/// admitted ceiling in bytes. Zero means "not yet written" for both, which is
/// why the loader's zero-fill of the heap (trust-surface assumption 5) is
/// load-bearing.
const HEAP_HEADER_BYTES: usize = 16;

/// Byte offset of the bump-position word within the heap header.
const HEAP_POSITION_WORD: usize = 0;

/// Byte offset of the admitted-ceiling word within the heap header.
const HEAP_CEILING_WORD: usize = 8;

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

/// Upward bump allocator over the SBF program heap region.
///
/// It differs from the SDK's `BumpAllocator` in exactly one way, and that way
/// exists to reclaim heap rather than to go faster: **it bumps upward.** The
/// SDK bumps down from a ceiling fixed at compile time, which is why
/// `ComputeBudget::RequestHeapFrame` is inert against it - the granted size is
/// never read, as commit `328fead` measured and recorded. Bumping upward makes
/// the ceiling a comparison instead of an origin, so it can be raised at any
/// point in the invocation without moving or invalidating a single live
/// allocation.
///
/// # Two heap reclaims that belong here and are NOT here
///
/// 1. **Last-in-first-out `dealloc`.** A bump allocator can return its top
///    block for one comparison, reclaiming every dropped temporary. It has now
///    been written, built, run against the canonical Registry-continuation Hot
///    bundle, and withdrawn **on its own measurement**, which is the only
///    reason worth recording:
///
///    - The frame objection is gone. `hot_v3::process_hot_execution_v3` used
///      the whole 4,096-byte SBPF v0 frame, so a non-empty `dealloc` - which
///      cannot be folded away at its drop sites - pushed it to 4,288 and made
///      the build report 255 calls overwriting the caller frame. Splitting that
///      function into its authentication and execution halves took it to 3,008,
///      and the last-in-first-out release then builds with zero diagnostics.
///    - It is worth **44 bytes**. Measured through the profile checkpoints on a
///      256 KiB diagnostic heap, against a byte-identical run without it: 4,696
///      / 6,800 / 9,632 / 17,304 / 25,908 / 27,504 / 36,060 with it, against
///      4,696 / 6,808 / 9,648 / 17,328 / 25,940 / 27,536 / 36,104 without. The
///      whole difference is the eight-byte probe allocation each checkpoint
///      itself makes and drops. **Not one temporary of this path is the top
///      block when it is dropped**, so last-in-first-out never fires on it.
///
///    Reclaiming those temporaries needs an allocator that can free a block
///    that is not the top one, which is a free list, not a bump. Forty-four
///    bytes does not buy the standing hazard: with a no-op `dealloc` a
///    use-after-free is inert, and with any real release it is corruption.
///    Reinstate this only together with the measurement that justifies it.
/// 2. **In-place `realloc` of the top block**, so `Vec` growth stops stranding
///    every previous buffer. Written, built, measured, and withdrawn on the
///    measurement: worth **zero bytes** at every checkpoint of the canonical
///    Registry-continuation Hot bundle, because nothing on that path grows a
///    `Vec` it did not reserve. It is not carried on the chance that some
///    other route would like it. `GlobalAlloc`'s default allocate-copy-release
///    is used instead, exactly as the SDK does.
///
/// Neither omission is a claim that this allocator is finished. Both are
/// recorded so that reinstating either is a decision someone makes with the
/// number in front of them.
pub struct BumpHeapV1 {
    /// Absolute address of the heap floor.
    base: usize,
}

impl BumpHeapV1 {
    /// Bind an allocator to a heap region beginning at `base`.
    ///
    /// # Safety
    ///
    /// `base` must be the floor of a writable region of at least
    /// [`ADAPTER_DEFAULT_HEAP_BYTES`] bytes that is zero-filled on entry, is
    /// aligned to at least `align_of::<usize>()`, stays mapped for the whole
    /// lifetime of this allocator, and is not written by anything else.
    #[must_use]
    pub const unsafe fn with_base(base: usize) -> Self {
        Self { base }
    }

    /// Address of a header word.
    ///
    /// # Safety
    ///
    /// `offset` must be `HEAP_POSITION_WORD` or `HEAP_CEILING_WORD`.
    #[inline(always)]
    unsafe fn header_word(&self, offset: usize) -> *mut usize {
        // SAFETY: `self.base` is the floor of a writable region of at least
        // ADAPTER_DEFAULT_HEAP_BYTES (32,768) bytes by this type's construction
        // contract, and the caller passes an offset inside the 16-byte header,
        // so `base + offset` is in bounds. The base is `usize`-aligned by the
        // same contract and both offsets are multiples of 8, so the resulting
        // pointer is aligned for `usize`.
        unsafe { (self.base as *mut u8).add(offset).cast::<usize>() }
    }

    /// Current bump position as an offset from the heap floor.
    #[inline(always)]
    fn position(&self) -> usize {
        // SAFETY: HEAP_POSITION_WORD is a header offset, and the word is
        // either zero (the loader's zero-fill, never yet written) or a value
        // this allocator itself stored.
        let stored = unsafe { *self.header_word(HEAP_POSITION_WORD) };
        if stored == 0 {
            HEAP_HEADER_BYTES
        } else {
            stored
        }
    }

    /// Record a new bump position.
    #[inline(always)]
    fn set_position(&self, position: usize) {
        // SAFETY: HEAP_POSITION_WORD is a header offset; see `header_word`.
        unsafe { *self.header_word(HEAP_POSITION_WORD) = position };
    }

    /// Bytes of heap this allocator is permitted to hand out.
    #[inline(always)]
    fn ceiling(&self) -> usize {
        // SAFETY: HEAP_CEILING_WORD is a header offset; see `header_word`.
        let stored = unsafe { *self.header_word(HEAP_CEILING_WORD) };
        if stored == 0 {
            ADAPTER_DEFAULT_HEAP_BYTES
        } else {
            stored
        }
    }

    /// Raise the ceiling to an authenticated grant.
    ///
    /// Refuses anything below the protocol default, above
    /// [`ADAPTER_MAX_HEAP_BYTES`], not a multiple of
    /// [`HEAP_FRAME_GRANULARITY_BYTES`], or below a ceiling already admitted.
    /// The caller is responsible for having authenticated `bytes` against the
    /// instructions sysvar; [`admit_heap_frame_v1`] is the only route that
    /// does so, which is why this stays private.
    ///
    /// Only the `target_os = "solana"` admission path calls it; the host build
    /// reaches it from the allocator corpus alone.
    #[cfg_attr(not(target_os = "solana"), allow(dead_code))]
    fn lift_ceiling(&self, bytes: usize) -> Result<usize, ProgramError> {
        if !(ADAPTER_DEFAULT_HEAP_BYTES..=ADAPTER_MAX_HEAP_BYTES).contains(&bytes)
            || !bytes.is_multiple_of(HEAP_FRAME_GRANULARITY_BYTES)
        {
            return Err(TradingSbfError::UnsupportedContent.into());
        }
        let current = self.ceiling();
        if bytes < current {
            return Err(TradingSbfError::UnsupportedContent.into());
        }
        // SAFETY: HEAP_CEILING_WORD is a header offset; see `header_word`.
        unsafe { *self.header_word(HEAP_CEILING_WORD) = bytes };
        Ok(bytes)
    }

    /// Bytes handed out so far, including the allocator's own header.
    #[must_use]
    pub fn bytes_used(&self) -> usize {
        self.position()
    }

    /// Bytes this allocator may hand out in total.
    #[must_use]
    pub fn bytes_capacity(&self) -> usize {
        self.ceiling()
    }
}

// SAFETY: SBF programs are single-threaded, so the interior mutation through
// the heap header words is never concurrent. Every pointer this implementation
// returns is inside `[base + HEAP_HEADER_BYTES, base + ceiling)`, which is
// mapped writable for the whole invocation by the type's construction
// contract, is aligned as the caller's `Layout` demands, and is disjoint from
// every block still outstanding: the bump position only moves back past a
// block when `dealloc` is told that block is being released.
unsafe impl GlobalAlloc for BumpHeapV1 {
    // `alloc` is inlined; it was not, and the two functions that stopped it now
    // have the frame for it. `process_hot_execution_v3` used its whole
    // 4,096-byte SBPF v0 frame and reported 47 calls overwriting the caller
    // frame with `alloc` inlined; split into its authentication and execution
    // halves it uses 3,008 and reports none. `authenticate_collateral` reported
    // the other 8 and now mints its record through one out-of-line constructor.
    // A call per allocation was the whole of this adapter's compute regression,
    // and it is paid back here. `realloc` stays out of line: it is the SDK's
    // allocate-copy-release, and nothing on the canonical path grows a `Vec` it
    // did not reserve, so inlining it would cost frame for no measured caller.
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let position = self.position();
        let Some(start) = self
            .base
            .checked_add(position)
            .and_then(|address| address.checked_next_multiple_of(layout.align()))
            .and_then(|address| address.checked_sub(self.base))
        else {
            return null_mut();
        };
        let Some(end) = start.checked_add(layout.size()) else {
            return null_mut();
        };
        if end > self.ceiling() {
            return null_mut();
        }
        self.set_position(end);
        // SAFETY: `start` is at most `ceiling`, and the region
        // `[base, base + ceiling)` is mapped writable by the construction
        // contract, so `base + start` is a valid address inside it.
        unsafe { (self.base as *mut u8).add(start) }
    }

    #[inline]
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // A no-op, exactly as the SDK's. See this type's documentation for the
        // measured reason the last-in-first-out release is not here.
    }
}

/// The Trading executable's program heap.
#[cfg(all(
    target_os = "solana",
    not(feature = "custom-heap"),
    not(feature = "no-entrypoint")
))]
#[global_allocator]
// SAFETY: HEAP_START_ADDRESS is the floor of the SBF program heap region. The
// VM maps it writable, 8-aligned, zero-filled, and at least HEAP_LENGTH bytes
// long for the whole of every invocation, and nothing but this allocator
// writes to it.
static PROGRAM_HEAP_V1: BumpHeapV1 =
    unsafe { BumpHeapV1::with_base(solana_program::entrypoint::HEAP_START_ADDRESS as usize) };

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
        Some(bytes) => PROGRAM_HEAP_V1.lift_ceiling(bytes),
        None => Ok(PROGRAM_HEAP_V1.bytes_capacity()),
    }
}

/// Routes permitted to run on a runtime-granted heap frame larger than the
/// protocol default.
///
/// Exhaustive and adapter-owned. The Hot execution path is deliberately absent:
/// its 1,224-byte continuation packet has no room to carry a ComputeBudget
/// instruction and its heap demand is being closed structurally. Adding a route
/// here is the single visible act that takes it off the 32 KiB discipline, and
/// it must be an instruction whose transaction has the packet room to actually
/// carry `RequestHeapFrame` and to present the instructions sysvar - without
/// both, the declaration is inert and the route keeps the default ceiling.
///
/// The two entries are the one-time, ALT-backed founding transactions:
///
/// - `DCLTGMF1`, the atomic Lock/Found/Realize/Claims/Open route;
/// - `DCLTPCB1`, projected-Custody bootstrap, which commit `328fead` measured
///   dying out of memory and diagnosed precisely: it "holds three stages' worth
///   of allocations live [...] against an allocator that never frees, so its
///   peak is the sum. Either it allocates less, or it supplies its own global
///   allocator over the granted heap." This module is that allocator, and this
///   is the declaration that lets the grant reach it.
pub fn declares_extended_heap_profile_v1(instruction_data: &[u8]) -> bool {
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if crate::generic_market_founding_v1::is_generic_market_founding_v1(instruction_data)
        || crate::projected_custody_bootstrap_v1::is_projected_custody_bootstrap_v1(
            instruction_data,
        )
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
    fn differential_declared_maximum_three_hundred_eight_accounts() {
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
        allocator.base
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
                for magic in [
                    crate::generic_market_founding_v1::GENERIC_MARKET_FOUNDING_MAGIC_V1,
                    crate::projected_custody_bootstrap_v1::PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V1,
                ] {
                    assert!(declares_extended_heap_profile_v1(&magic));
                    let mut nearly = magic;
                    nearly[7] = nearly[7].wrapping_add(1);
                    assert!(!declares_extended_heap_profile_v1(&nearly));
                    assert!(!declares_extended_heap_profile_v1(
                        magic.get(..7).expect("prefix")
                    ));
                }
            }
        }
    }
}
