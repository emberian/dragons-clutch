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
//! authorization.  It now carries token and CPI code: [`token`] observes and
//! admits Token-2022 mints and token accounts and builds the CPIs, and
//! `Materialize`/`Dematerialize` drive a real `MintTo`/`Burn` **when the
//! optional token leg is present in the account list**.  That optionality is a
//! transitional hole named in [`instructions::split`] and in
//! `docs/implementation/TOKEN2022_PLAN.md`, not a design.  `SettlePage` is the
//! one honest refusal, with a recorded finding (the batch relation awaits the
//! on-chain streaming verifier); `PlaceOrder` and `CancelOrder` (page-v4
//! tombstone retirement) are implemented with host tests only — no reference
//! oracle and no SVM leg.  `Resolve` and `RedeemInternal` are evidence-gated
//! exactly as in the offline reference adapter, and `CreateMarket` is a
//! permissionless founding transition that creates the outcome mints and
//! the Hoard token account via CPI and writes the founding plane; the
//! genesis module creates the remaining accounts via system CPI.
//! [`instructions::genesis`] is the one family that **creates** accounts,
//! through a real system-program CPI: it too has host tests only, its CPI has
//! never run on a bank, and its `Endow` credits internal position cash that no
//! collateral backs — the internal-ledger half of a deposit whose value leg is
//! still unwired.  The PDA seed schema in [`seeds`] is a **proposal**, not a
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
//! | [`source`] | fail-closed source-spec and authenticated price-admission kernel; not yet joined to an instruction |
//! | [`accounts`] | hostile-metadata authentication, address comparison, and every account decoder |
//! | [`dispatch`] | request decoding and routing to exactly one instruction family |
//! | [`instructions`] | one module per instruction family; see each module's status |
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
pub mod dispatch;
pub mod error;
pub mod instructions;
pub mod seeds;
pub mod source;
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
