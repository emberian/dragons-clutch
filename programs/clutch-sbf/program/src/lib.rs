#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

//! Bring-up native SBF program for Dragon's Clutch.
//!
//! ## What this is
//!
//! A deployable SBF program exposing **one** instruction, `Split`, so that the
//! account-facing half of the protocol can be executed by a real SVM rather
//! than only reasoned about offline.  It exists to produce bring-up evidence
//! for `docs/implementation/SBF_BRINGUP.md`.
//!
//! ## What this is not
//!
//! It is not a complete program, is not audited, carries no token or CPI code,
//! and is not a deployment authorization.  `Resolve` and `RedeemInternal`
//! refuse here exactly as they refuse in the offline reference adapter, and
//! every other instruction is refused as out of scope.  The PDA seed schema in
//! [`seeds`] is a **proposal**, not a frozen ABI.
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
//! ## `unsafe`
//!
//! First-party code in this crate is safe.  The only `unsafe` reaching this
//! crate is the expansion of the Anza `entrypoint!` macro, which is confined to
//! the `bpf` module below and compiled only for `target_os = "solana"`.

pub mod error;
pub mod processor;
pub mod seeds;

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
        crate::processor::process(program_id, accounts, instruction_data).map_err(Into::into)
    }
}
