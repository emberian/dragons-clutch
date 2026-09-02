//! The SBF program-heap bump allocator, and nothing else.
//!
//! # Why this is its own crate
//!
//! Two dClutch executables need the heap frame their transaction PAYS FOR, and
//! the SDK cannot give it to either of them. `solana_program::entrypoint!`
//! installs `BumpAllocator::with_fixed_address_range(HEAP_START_ADDRESS,
//! HEAP_LENGTH)` -- a hardcoded 32,768 -- and bumps DOWN from that compile-time
//! ceiling, so `ComputeBudget::RequestHeapFrame` is inert against it no matter
//! what the runtime granted. The macro elides that allocator only when the
//! CALLING crate enables a feature named `custom-heap`, and then the crate owes
//! an allocator of its own.
//!
//! Trading has had one since `328fead`. `dclutch-general-accelerator-sbf`
//! DECLARED the feature and never implemented one, so it ran on the stock
//! 32 KiB allocator while its transactions granted 65,536 -- half the frame it
//! paid for was never addressable, and at runtime width 258 it died of an
//! out-of-memory abort with 26,515 outstanding.
//!
//! A second implementation there was not available: that crate carries
//! `unsafe_code = "forbid"`, and it should keep it. So the allocator moves here
//! and both consume it.
//!
//! # Kernel policy
//!
//! This is a NAMED ADAPTER, not kernel code. AGENTS.md puts the Solana SDK,
//! account memory and the loader boundary outside the first-party kernel in
//! explicitly named adapters, and this is the whole `unsafe` surface of the
//! program heap in one auditable file. Its safety contract is stated on
//! [BumpHeapV1::with_base] and discharged for the ordinary case by
//! [program_heap_v1], which needs no `unsafe` at a call site.
//!
//! # Where the numbers in [BumpHeapV1]'s documentation come from
//!
//! Every measurement in the type's own documentation below -- the withdrawn
//! last-in-first-out `dealloc`, the withdrawn in-place `realloc`, the bump
//! positions, the 42,784 peak against a 32,768 ceiling, the hole that no mark
//! can reclaim -- was taken on `dclutch-trading-sbf`'s canonical Direct and
//! Registry-continuation Hot bundles, which is where this allocator was written
//! and is still the only route that opens a scratch region. They are kept with
//! the type rather than left behind because they are the record of what was
//! tried and refused, and a second consumer reaching for a free list should
//! read them first. They are NOT claims about any other program's profile.

#![no_std]

use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
};

use solana_program::entrypoint::{HEAP_LENGTH, HEAP_START_ADDRESS};

/// Heap bytes available to a program that requested no ComputeBudget heap frame.
///
/// Chain-derived: `solana_program_entrypoint::HEAP_LENGTH`, which is also
/// Agave's `MIN_HEAP_FRAME_BYTES` and the heap size every transaction gets
/// without asking.
pub const DEFAULT_HEAP_BYTES_V1: usize = HEAP_LENGTH;

/// Largest heap frame the runtime will grant.
///
/// Chain-derived: Agave's `MAX_HEAP_FRAME_BYTES`
/// (`solana_program_runtime::execution_budget`), 256 KiB.
pub const MAX_HEAP_BYTES_V1: usize = 256 * 1024;

/// Granularity the runtime requires of a ComputeBudget heap-frame request.
///
/// Chain-derived: `ComputeBudgetInstructionDetails::sanitize_requested_heap_size`
/// requires the request to be a multiple of 1,024.
pub const HEAP_FRAME_GRANULARITY_BYTES_V1: usize = 1_024;

/// Bytes the bump heap reserves at its floor for the allocator's own state.
///
/// Word 0 is the bump position as an offset from the heap floor, word 1 is the
/// admitted ceiling in bytes, word 2 is the scratch floor -- the offset the
/// high-end scratch allocator has bumped DOWN to. Zero means "not yet written"
/// for all three, which is why the loader's zero-fill of the heap is
/// load-bearing.
pub const HEAP_HEADER_BYTES: usize = 24;

/// Byte offset of the bump-position word within the heap header.
const HEAP_POSITION_WORD: usize = 0;

/// Byte offset of the admitted-ceiling word within the heap header.
const HEAP_CEILING_WORD: usize = 8;

/// Byte offset of the scratch-floor word within the heap header.
const HEAP_SCRATCH_WORD: usize = 16;

/// What this allocator can refuse.
///
/// Deliberately NOT a `ProgramError` and deliberately NOT `#[repr]`: an
/// allocator is not a protocol surface, and a consumer maps these onto its own
/// registered refusal codes. Trading has always mapped them onto
/// `UnsupportedContent` and `Content` respectively, and still does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeapErrorV1 {
    /// The stated ceiling was below the protocol default, above
    /// [MAX_HEAP_BYTES_V1], not a multiple of
    /// [HEAP_FRAME_GRANULARITY_BYTES_V1], or below one already admitted.
    UnsupportedCeiling,
    /// A scratch region was already open. Exactly one may be.
    ScratchAlreadyOpen,
}

/// The SBF program heap, bound to the region the loader maps.
///
/// The one safe constructor, and the reason a consumer needs no `unsafe` of its
/// own: `HEAP_START_ADDRESS` is not a caller-supplied address but the floor the
/// VM maps writable, 8-aligned, zero-filled and at least [DEFAULT_HEAP_BYTES_V1]
/// bytes long for the whole of every invocation. The obligation
/// [BumpHeapV1::with_base] states is therefore discharged by the platform here
/// rather than by the caller.
///
/// The remaining obligation is one a `#[global_allocator]` carries anyway and
/// that the type system cannot express: nothing else may write to that region,
/// which means a program installs exactly one of these and does not also let
/// the SDK install its own. Enabling the `custom-heap` feature is what
/// guarantees the second half.
#[must_use]
// `HEAP_START_ADDRESS` is a `u64` and the base is a `usize`, so the narrowing is
// real on a target with pointers narrower than the VM's addresses. It is not
// real HERE, and the assertion below is the proof rather than the claim: a
// target where the floor does not fit a pointer refuses to compile this crate at
// all, which is the honest answer for an allocator that could not address its
// own heap. `try_from` is not available in a `const fn`, and this one must stay
// const -- every caller is a `static` initializer.
#[expect(
    clippy::cast_possible_truncation,
    reason = "HEAP_START_USIZE_FITS_V1 refuses the build on a target where it would truncate"
)]
pub const fn program_heap_v1() -> BumpHeapV1 {
    // SAFETY: see this function's documentation -- the address is the
    // platform's, not a caller's.
    unsafe { BumpHeapV1::with_base(HEAP_START_ADDRESS as usize) }
}

/// The floor is representable as a pointer on every target this crate builds for.
const HEAP_START_USIZE_FITS_V1: () = assert!(
    HEAP_START_ADDRESS <= usize::MAX as u64,
    "the VM heap floor does not fit a pointer on this target"
);
const _: () = HEAP_START_USIZE_FITS_V1;

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
///
/// # A third reclaim, asked for by name, answered NO at one end and YES at the
/// other: the region reset
///
/// **What follows is W2o's measurement and it still stands: a mark/reset of
/// the UPWARD end cannot reclaim the observation bank.** What changed (W2p) is
/// the conclusion drawn from it. The blocking fact is not that the bank is
/// short-lived but that it is short-lived *underneath* eighteen kilobytes that
/// are not, and the fix for that is not a mark -- it is to allocate it at the
/// other end. This allocator now has one: [`Self::alloc_scratch`] bumps DOWN
/// from the ceiling, the two ends refuse to cross, and
/// `HeapScratchRegionV1` releases the whole high end in one store. A block
/// served there is reclaimed no matter what was allocated at the upward end
/// while it was live, so the reorder the paragraphs below ask for is not
/// needed -- and the release's obligation, which on the upward end would have
/// spanned four hundred lines and five calls, becomes a lexical scope the
/// borrow checker enforces.
///
/// The original question and its measurement:
///
/// The question put to this allocator (W2o, 2026-08-27) was whether a
/// mark/reset at a phase boundary should be reinstated, on the ground that
/// `hot_v3::process_hot_execution_v3` does
/// `drop(observations); drop(runtime_data);` immediately before `before-commit`
/// and releases about 5,968 bytes at exactly the point the child walk begins
/// allocating. The measurement says no, and the reason is a fact about the
/// SHAPE of that drop rather than about its size.
///
/// A bump allocator can release only its TOP block, and a region reset is that
/// same release under a named scope: it is sound exactly when nothing allocated
/// after the mark is live at the reset. Measured on the canonical Direct
/// bundle, at the instruction where those two drops run, the bump position is
/// **35,299** and the two dead blocks lie at roughly `[10,456, 11,920)` and
/// `[12,656, 17,160)`. Between them and the top sit about **18,000 bytes that
/// are all LIVE**: the alias table, the register pair the projection kept and
/// the two the preplan arena rents from it, the preplan output pair, the
/// account-input / permission / lamport / request / write-range / discipline
/// banks the effect projection built, the decoded privilege bytes, the boxed
/// Claims composition and the resolved role programs. Every one of them is read
/// by the child walk or by the commit that follow.
///
/// So the drop pattern's shape is a **hole, not a region**. No mark can be
/// placed that both precedes the observation bank and follows nothing live, and
/// last-in-first-out will not fire on it for the same reason. Reclaiming it
/// needs a free list -- the standing hazard recorded above -- or a reordering
/// that moves every surviving allocation BELOW the observation bank, which
/// means pre-allocating the projection's output banks before its input exists.
/// That is a restructure of the middle of `process_hot_execution_v3`, not an
/// allocator change, and it is where the next lane should look.
///
/// What a region reset WOULD reclaim on this path, provably, is the preflight
/// walk: its account frame and its child wire are locals of
/// `preflight_child_routes_v3`, nothing outside can hold a pointer into them,
/// and they are the top block when it returns. That is about **720 bytes** as
/// of this measurement. It is not carried, because 720 bytes does not buy a
/// general release primitive in a module whose whole discipline is that the
/// unsafe surface stays small enough to audit.
///
/// And the arithmetic that sizes the prize, so the next lane spends its effort
/// where the bytes are. The peak is **42,784** against a 32,768 ceiling, and
/// 35,299 of it is standing when the two drops run. A reclaim that returned
/// the observation bank, the runtime-data guards and the preflight walk in
/// full -- 6,689 bytes -- would leave 28,610 standing for the child walk and
/// the commit to build their 7,485 on top of, for a peak of **36,095**: still
/// about **3,300 bytes over**, but within reach of the structural cuts, which
/// it is not today. So the reclaim is worth roughly **6,700 bytes and is the
/// single largest lever left** -- and it is a FREE LIST, with everything that
/// implies, or the reordering above. Neither is a mark and neither is
/// last-in-first-out; do not reach for those again on the strength of the drop
/// site alone.
///
/// W2p reached for neither. It took the option the hole's SHAPE leaves once
/// the shape rather than the size is read as the problem: **stop putting the
/// short-lived bank underneath the long-lived ones.** The scratch end does
/// that without moving one other allocation, and the reclaim it measures is
/// the one the paragraph above sized.
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
    /// [`DEFAULT_HEAP_BYTES_V1`] bytes that is zero-filled on entry, is
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
    /// `offset` must be `HEAP_POSITION_WORD`, `HEAP_CEILING_WORD` or
    /// `HEAP_SCRATCH_WORD`.
    #[inline(always)]
    unsafe fn header_word(&self, offset: usize) -> *mut usize {
        // SAFETY: `self.base` is the floor of a writable region of at least
        // DEFAULT_HEAP_BYTES_V1 (32,768) bytes by this type's construction
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
            DEFAULT_HEAP_BYTES_V1
        } else {
            stored
        }
    }

    /// Raise the ceiling to an authenticated grant.
    ///
    /// Refuses anything below the protocol default, above
    /// [`MAX_HEAP_BYTES_V1`], not a multiple of
    /// [HEAP_FRAME_GRANULARITY_BYTES_V1], or below a ceiling already admitted.
    /// The caller is responsible for having authenticated `bytes` against the
    /// instructions sysvar; `admit_heap_frame_v1` is the only route that
    /// does so, which is why this stays private.
    ///
    /// Only the `target_os = "solana"` admission path calls it; the host build
    /// reaches it from the allocator corpus alone.
    pub fn lift_ceiling(&self, bytes: usize) -> Result<usize, HeapErrorV1> {
        if !(DEFAULT_HEAP_BYTES_V1..=MAX_HEAP_BYTES_V1).contains(&bytes)
            || !bytes.is_multiple_of(HEAP_FRAME_GRANULARITY_BYTES_V1)
        {
            return Err(HeapErrorV1::UnsupportedCeiling);
        }
        let current = self.ceiling();
        if bytes < current {
            return Err(HeapErrorV1::UnsupportedCeiling);
        }
        // SAFETY: HEAP_CEILING_WORD is a header offset; see `header_word`.
        unsafe { *self.header_word(HEAP_CEILING_WORD) = bytes };
        Ok(bytes)
    }

    /// Absolute address of the heap floor this allocator is bound to.
    ///
    /// In a program this is `HEAP_START_ADDRESS` and reveals nothing a consumer
    /// could not name itself. It is public because a test that binds the
    /// allocator to its OWN backing buffer has no other way to say where a
    /// block should have landed, and computing that from a returned pointer
    /// would make the test agree with the allocator by construction. The field
    /// stays private: this hands out the address, not the ability to move it.
    #[must_use]
    pub const fn base(&self) -> usize {
        self.base
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

    /// Lowest offset the scratch end has bumped down to.
    ///
    /// Equal to the ceiling exactly when no scratch region is open, which is
    /// also what the loader's zero-fill means.
    #[inline(always)]
    fn scratch_floor(&self) -> usize {
        // SAFETY: HEAP_SCRATCH_WORD is a header offset; see `header_word`.
        let stored = unsafe { *self.header_word(HEAP_SCRATCH_WORD) };
        if stored == 0 { self.ceiling() } else { stored }
    }

    /// Bytes outstanding at the scratch end.
    #[must_use]
    pub fn scratch_bytes_used(&self) -> usize {
        self.ceiling().saturating_sub(self.scratch_floor())
    }

    /// Open the one scratch region.
    ///
    /// Refuses when a region is already open. See `HeapScratchRegionV1` for
    /// why exactly one may be open at a time.
    pub fn open_scratch(&self) -> Result<usize, HeapErrorV1> {
        let ceiling = self.ceiling();
        if self.scratch_floor() != ceiling {
            return Err(HeapErrorV1::ScratchAlreadyOpen);
        }
        Ok(ceiling)
    }

    /// Return the scratch end to `mark`.
    ///
    /// # Safety
    ///
    /// Every block this allocator served from the scratch end above `mark`
    /// must be dead: no live reference may point into `[mark, ceiling)`.
    /// `HeapScratchRegionV1` is the only caller and discharges this by
    /// construction -- a `ScratchVecV1` borrows the region, so the borrow
    /// checker refuses to let one outlive the release.
    pub unsafe fn release_scratch(&self, mark: usize) {
        if mark < self.scratch_floor() {
            return;
        }
        // SAFETY: HEAP_SCRATCH_WORD is a header offset; see `header_word`.
        unsafe { *self.header_word(HEAP_SCRATCH_WORD) = mark };
    }

    /// Hand out `layout` from the scratch end, bumping DOWNWARD.
    ///
    /// The two ends meet in the middle and each refuses rather than crossing
    /// the other: an upward allocation is bounded by [`Self::scratch_floor`]
    /// and this one is bounded by [`Self::position`], so a block from either
    /// end is disjoint from every block outstanding at the other.
    ///
    /// # Safety
    ///
    /// Same contract as [`GlobalAlloc::alloc`]: `layout` must have a non-zero
    /// size.
    pub unsafe fn alloc_scratch(&self, layout: Layout) -> *mut u8 {
        let floor = self.scratch_floor();
        let Some(unaligned) = self
            .base
            .checked_add(floor)
            .and_then(|address| address.checked_sub(layout.size()))
        else {
            return null_mut();
        };
        // Align DOWN: the block starts at the highest correctly aligned
        // address whose whole extent still lies below the current floor.
        let start_address = unaligned & !(layout.align().wrapping_sub(1));
        let Some(start) = start_address.checked_sub(self.base) else {
            return null_mut();
        };
        if start < self.position() {
            return null_mut();
        }
        // SAFETY: HEAP_SCRATCH_WORD is a header offset; see `header_word`.
        unsafe { *self.header_word(HEAP_SCRATCH_WORD) = start };
        // SAFETY: `start` is at least `position` and at most `floor <=
        // ceiling`, and `[base, base + ceiling)` is mapped writable by the
        // construction contract.
        unsafe { (self.base as *mut u8).add(start) }
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
        // The bound is the scratch floor, not the ceiling: it IS the ceiling
        // whenever no scratch region is open, and while one is it is the
        // lowest byte the scratch end has handed out.
        if end > self.scratch_floor() {
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
