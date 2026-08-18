#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

//! Bring-up native SBF program for Dragon's Clutch.
//!
//! ## What this is
//!
//! A deployable SBF program with a routed instruction set of which exactly
//! **one** instruction, `Split`, is implemented, so that the account-facing half
//! of the protocol can be executed by a real SVM rather than only reasoned
//! about offline.  It exists to produce bring-up evidence for
//! `docs/implementation/SBF_BRINGUP.md`.
//!
//! ## What this is not
//!
//! It is not a complete program, is not audited, carries no token or CPI code,
//! and is not a deployment authorization.  Four of the five instruction-family
//! modules are honest stubs that read no account, write no byte, and refuse.
//! `Resolve` and `RedeemInternal` refuse here exactly as they refuse in the
//! offline reference adapter, and `CreateMarket` refuses because no authority
//! model exists.  The PDA seed schema in [`seeds`] is a **proposal**, not a
//! frozen ABI.
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
//! | [`accounts`] | hostile-metadata authentication, address comparison, and every account decoder |
//! | [`dispatch`] | request decoding and routing to exactly one instruction family |
//! | [`instructions`] | one module per instruction family; only `split` is implemented |
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
pub mod dispatch;
pub mod error;
pub mod instructions;
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
        crate::dispatch::process(program_id, accounts, instruction_data).map_err(Into::into)
    }
}
