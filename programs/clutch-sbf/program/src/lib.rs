#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

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
//! | [`native_window`] | bounded sealed-archive occupation fold persisted only by Resolution v4 |
//! | [`token`] | Token-2022 observation, admission, and CPI construction |
//!
//! The per-lane ownership boundaries are tabulated in
//! `docs/implementation/SBF_BRINGUP.md`.
//!
//! ## `unsafe`
//!
//! First-party code in this crate is safe.  The only `unsafe` reaching this
//! crate is the expansion of the Anza `entrypoint!` macro, which is confined to
//! the `bpf` module below and compiled only for `target_os = "solana"`.

pub mod accounts;
pub mod claim_truth;
pub mod dispatch;
pub mod error;
pub mod instructions;
pub mod native_window;
pub mod seeds;
pub mod source;
pub mod source_archive;
pub mod token;

#[cfg(target_os = "solana")]
mod bpf {
    use solana_account_info::AccountInfo;
    use solana_program_entrypoint::{entrypoint, ProgramResult};
    use solana_pubkey::Pubkey;

    entrypoint!(process_instruction);

    fn process_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        crate::dispatch::process(program_id, accounts, instruction_data).map_err(Into::into)
    }
}
