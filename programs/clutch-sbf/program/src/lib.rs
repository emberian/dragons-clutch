#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(
    not(feature = "profile-full"),
    allow(dead_code, unreachable_code, unused_imports)
)]

//! Bring-up native SBF program for Dragon's Clutch.
//!
//! ## What this is
//!
//! A deployable SBF program with a routed instruction set (see
//! [`instructions`] for exactly what is implemented), so that the
//! account-facing half of the protocol can be executed by a real SVM rather
//! than only reasoned about offline.  It exists to produce bring-up evidence
//! for `docs/implementation/SBF_BRINGUP.md`.
//!
//! ## What this is not
//!
//! It is not a complete program, is not audited, and is not a deployment
//! authorization.  The current executable subset has real local-bank evidence
//! for permissionless PDA construction, backed pooled-custody `Endow`,
//! `Split`/`Merge`, Token-2022 materialization and dematerialization, categorical
//! and native degree-1--3 point or quantized-occupation resolution, internal
//! and categorical/native bearer redemption, free-cash withdrawal, funded
//! order placement/cancellation, and
//! one deliberately narrow coupled-settlement slice.  `SettlePage` is therefore
//! no longer an unconditional stub: it executes only a same-page, full-fill,
//! direct single-Egg, zero-fee, exactly divisible pair whose selected candidate,
//! candidate feed, frozen receipt, orders, and ACTIVE reservations all bind.
//! Every broader settlement form still refuses.  See
//! `docs/implementation/COUPLED_SETTLEMENT_V1.md`.
//!
//! Resolve authenticates and consumes the canonical sealed source archive,
//! including its pinned adapter-release identity.  Provider ingestion and
//! public archive construction are not routed: the current bank evidence
//! installs canonical archive bytes at genesis and is not a provider-ingestion
//! claim.  Candidate selection, entitlement creation, general
//! partial/portfolio settlement, and the full blank-bank venue lifecycle also
//! remain incomplete.  Evidence labels and the current dependency order live
//! in `CURRENT_TRUTH.md` and `docs/V1_BACKLOG.md`; older bring-up documents are
//! historical rather than the source of present tense truth.
//!
//! ## Layering
//!
//! Economic and transition semantics live in `clutch-kernel`.  Byte ownership
//! lives in `clutch-solana-layout` and in the reference-only codecs of
//! `clutch-solana-reference`.  This crate adds only what those crates cannot
//! have: runtime account authentication, program-address derivation, and
//! write-back.  Neither the kernel nor the layout crate is modified by this
//! lane.
//!
//! ## Module map
//!
//! | module | owns |
//! | --- | --- |
//! | [`error`] | the stable numeric refusal codes |
//! | [`seeds`] | the proposed PDA seed schema for all 15 protocol accounts plus the 3 reference-only ones |
//! | [`source`] | fail-closed source-spec and authenticated price-admission kernel; not yet joined to an instruction |
//! | [`accounts`] | hostile-metadata authentication, address comparison, and every account decoder |
//! | [`dispatch`] | request decoding and routing to exactly one instruction family |
//! | [`instructions`] | one module per instruction family; see each module's status |
//! | [`instructions_sysvar`] | pinned Instructions-sysvar decode for the R2 immediate-post join; routed by nothing |
//! | [`loader_state`] | pinned Upgradeable Loader ProgramData decode for the R2 deployment-slot pin; routed by nothing |
//! | [`native_window`] | bounded sealed-archive occupation fold persisted only by Resolution v4 |
//! | [`pyth_receiver`] | pinned Pyth `PriceUpdateV2` decode and conservative price normalization |
//! | [`source_identity`] | **the one-const boundary**: every identity byte the R2 pull profile pins |
//! | [`source_v2`] | the R2 pull source plane: spec generation 2, the crossing rule, the authentication join |
//! | [`source_archive_v2`] | the v2 source-spec account and the v2 sealed archive page |
//! | [`token`] | Token-2022 observation, admission, and CPI construction |
//!
//! The per-lane ownership boundaries are tabulated in
//! `docs/implementation/SBF_BRINGUP.md`.
//!
//! ## `unsafe`
//!
//! First-party `unsafe` in this crate is confined to two places, both in the
//! `bpf` module below and both compiled only for `target_os = "solana"`: the
//! expansion of the Anza `entrypoint!` macro, and the requestable-heap bump
//! allocator that replaces the macro's 32-KiB default (`bpf::GrowableBump`).
//! Neither has a rustdoc page: `bpf` is private and compiled only under
//! `target_os = "solana"`, so no host doc build sees it.

#[cfg(not(any(
    feature = "profile-full",
    feature = "profile-direct-v3-source-v2-point",
    feature = "profile-general-source-v2-point",
    feature = "profile-non-production-general-v2-empty-book-identity-lab"
)))]
compile_error!("select exactly one Dragon's Clutch capability profile");
#[cfg(any(
    all(
        feature = "profile-full",
        feature = "profile-direct-v3-source-v2-point"
    ),
    all(feature = "profile-full", feature = "profile-general-source-v2-point"),
    all(
        feature = "profile-direct-v3-source-v2-point",
        feature = "profile-general-source-v2-point"
    ),
    all(
        feature = "profile-full",
        feature = "profile-non-production-general-v2-empty-book-identity-lab"
    ),
    all(
        feature = "profile-direct-v3-source-v2-point",
        feature = "profile-non-production-general-v2-empty-book-identity-lab"
    ),
    all(
        feature = "profile-general-source-v2-point",
        feature = "profile-non-production-general-v2-empty-book-identity-lab"
    )
))]
compile_error!("Dragon's Clutch capability profiles are mutually exclusive");

pub mod accounts;
pub mod capabilities;
pub mod claim_truth;
pub mod dispatch;
pub mod error;
pub mod instructions;
pub mod instructions_sysvar;
pub mod loader_state;
pub mod native_window;
pub mod pyth_receiver;
pub mod seeds;
pub mod source;
pub mod source_archive;
pub mod source_archive_v2;
pub mod source_generation;
pub mod source_identity;
pub mod source_v2;
pub mod token;

#[cfg(target_os = "solana")]
mod bpf {
    use solana_account_info::AccountInfo;
    use solana_program_entrypoint::{entrypoint, ProgramResult, HEAP_START_ADDRESS};
    use solana_pubkey::Pubkey;

    entrypoint!(process_instruction);

    /// The widest heap a transaction can request with
    /// `ComputeBudgetInstruction::request_heap_frame`.
    const HEAP_CEILING: usize = 256 * 1024;

    /// An upward bump allocator over the whole requestable heap region.
    ///
    /// The `entrypoint!` default (suppressed by this crate's `custom-heap`
    /// feature) allocates *downward* from `HEAP_START + 32 KiB`, so a
    /// transaction-requested larger frame is unreachable: the clearing walk's
    /// boxed `ClearWorkV1` (~48.7 KiB) needs the requestable region.  This
    /// allocator bumps *upward* from the region's base instead — small
    /// allocations land in the always-mapped first 32 KiB exactly as before,
    /// and an allocation past the mapped frame is only reachable by an
    /// instruction whose transaction requested one (without the request, the
    /// first write beyond the mapping is an access violation that aborts the
    /// transaction — a refusal, not a corruption).  Like the default, it
    /// never frees: per-instruction heaps die with the instruction.
    struct GrowableBump;

    // The first 8 bytes of the heap region hold the bump cursor, exactly as
    // the Anza default uses them for its own position.
    unsafe impl std::alloc::GlobalAlloc for GrowableBump {
        #[inline]
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            let start = HEAP_START_ADDRESS as usize;
            let cursor = start as *mut usize;
            let mut position = *cursor;
            if position == 0 {
                position = start + core::mem::size_of::<usize>();
            }
            let align = layout.align().max(core::mem::size_of::<usize>());
            let aligned = match position.checked_add(align - 1) {
                Some(bumped) => bumped & !(align - 1),
                None => return core::ptr::null_mut(),
            };
            let end = match aligned.checked_add(layout.size()) {
                Some(end) => end,
                None => return core::ptr::null_mut(),
            };
            if end > start + HEAP_CEILING {
                return core::ptr::null_mut();
            }
            *cursor = end;
            aligned as *mut u8
        }
        #[inline]
        unsafe fn dealloc(&self, _: *mut u8, _: std::alloc::Layout) {
            // A bump allocator frees nothing; the region dies with the
            // instruction.
        }
    }

    #[global_allocator]
    static ALLOCATOR: GrowableBump = GrowableBump;

    fn process_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        crate::dispatch::process(program_id, accounts, instruction_data).map_err(Into::into)
    }
}
