#![no_std]
#![deny(unsafe_code)]
#![deny(missing_docs)]

//! Canonical data-driven SBF adapter for the fixed Trading execution role.
//!
//! The executable capability lifecycle path authenticates the common Core
//! envelope, current Registry releases, finalized
//! descriptor/config/profile/effect records, and interpreted
//! transition/effects. Activation commits one composite root and its selected
//! FundingLedgerV2 row. Native close refunds the exact remaining principal,
//! root/ledger Rent, and separately classified surplus to the Market's one
//! RentCredit before making the root and selected ledger vacant. Realm/token
//! custody remains explicitly unsupported. The boundary has no family
//! discriminator.

// The kernel crates this adapter calls are `no_std`; the adapter layer itself
// allocates, so the executable links `std` for `Vec`/`Box` and for the
// `GlobalAlloc` implementation `entrypoint_adapter` installs.
extern crate std;

use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

/// One author for the child-CPI caller authority, derived once per execution.
pub mod child_authority_v4;

/// Ephemeral exact prior-child receipt retention for the common Hot executor.
mod child_receipt_v3;

/// Family-neutral authoritative admitted-AOT candidate CPI.
pub mod admitted_composition_v3;

/// Family-neutral EffectProgram V3 composition for canonical Claims CPIs.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family",
    feature = "outer-only"
))]
pub mod claims_composition_v3;
/// Family-neutral EffectProgram V3 composition for canonical Core CPIs.
pub mod core_composition_v3;
/// Family-neutral EffectProgram V3 composition for canonical Custody CPIs.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family",
    feature = "outer-only"
))]
pub mod custody_composition_v3;
/// Dealer family projection behind the common data-defined Trading boundary.
#[cfg(any(feature = "families", feature = "dealer-family"))]
pub mod dealer;
/// Lock-bounded durable Dealer scenario checkpoint lifecycle.
#[cfg(any(feature = "families", feature = "dealer-family"))]
pub mod dealer_scenario_checkpoint_v1;
/// Direct family projection behind the common data-defined Trading boundary.
#[cfg(feature = "families")]
pub mod direct;
/// Permissionless, release-authenticated Direct Open-to-Retiring transition.
#[cfg(feature = "families")]
pub mod direct_begin_retiring_v1;
/// Permissionless, release-authenticated close of one drained maker replay.
#[cfg(feature = "families")]
pub mod direct_close_maker_v1;
/// Permissionless settlement of one Direct fee, in a transaction of its own.
#[cfg(feature = "families")]
pub mod direct_fee_settlement_v1;
/// Permissionless first-use Direct Custody replay setup.
#[cfg(feature = "families")]
pub mod direct_replay_setup_v1;
/// Permissionless dust-tolerant setup of Direct's Token-2022 destinations.
#[cfg(feature = "families")]
pub mod direct_token_setup_v1;
/// Manifest-, root-, release-, and descriptor-authenticated generic dispatch.
pub mod dispatch;
/// V3 descriptor joins for independently finalized runtime-tail artifacts.
pub mod dispatch_v3;
/// Profile13 physical representative expansion shared by prefix and continuation.
mod dynamic_accounts_v4;
/// The named machine boundary: SBF entrypoint, input deserialization, heap.
///
/// The ONE `unsafe` exemption in this executable. Everything reachable from
/// `process_instruction` is safe Rust; this module converts the loader's raw
/// input region into the safe values that boundary consumes and owns the bump
/// allocator they are measured against. Its module documentation carries the
/// full trust surface. No other module in this crate may carry this attribute.
#[allow(unsafe_code)]
pub mod entrypoint_adapter;
/// Registry-authenticated family-neutral Execution Strategy V2 admission.
pub mod execution_strategy_v2;
/// Two-stage generic Market founding: Found-and-permit, then commit-last Open.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
pub mod generic_founding_stages_v1;
/// Generic atomic Custody→Core→Claims Market founding and commit-last Open.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
pub mod generic_market_founding_v1;
/// Family-neutral authenticated V3 hot execution outer.
pub mod hot_v3;
/// Family-neutral native-signature evidence authentication and register seeding.
pub mod native_signature;
/// Family-neutral executable Core-to-Trading boundary.
pub mod outer;
/// Exact Claims Founding route and ordered projected-receipt join.
// Composed entirely of `crate::series` routes and constants, so it is gated
// exactly as `series` is: one feature wider and `--features dealer-family`
// alone builds this module against a module that is not there.
#[cfg(any(feature = "families", feature = "series-family"))]
#[allow(dead_code)]
mod projected_claims_composition_v4;
/// Exact current-Core Found route and acknowledgment join for projected Markets.
// Composed entirely of `crate::series` routes and constants, so it is gated
// exactly as `series` is: one feature wider and `--features dealer-family`
// alone builds this module against a module that is not there.
#[cfg(any(feature = "families", feature = "series-family"))]
#[allow(dead_code)]
mod projected_core_composition_v4;
/// Family-neutral creation of the projected-Custody prestate founding needs.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
pub mod projected_custody_bootstrap_v1;
/// Family-neutral projected-Custody route-zero execution and receipt join.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[allow(dead_code)]
mod projected_custody_composition_v4;
/// Compact projected-Market execution and persisted-state reconstruction.
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
pub mod projected_market_v2;
/// Final Core Open and Trading replay commit-last boundary.
// Composed entirely of `crate::series` routes and constants, so it is gated
// exactly as `series` is: one feature wider and `--features dealer-family`
// alone builds this module against a module that is not there.
#[cfg(any(feature = "families", feature = "series-family"))]
#[allow(dead_code)]
mod projected_open_composition_v4;
/// Exact projected-Hoard realization route and receipt join.
// Composed entirely of `crate::series` routes and constants, so it is gated
// exactly as `series` is: one feature wider and `--features dealer-family`
// alone builds this module against a module that is not there.
#[cfg(any(feature = "families", feature = "series-family"))]
#[allow(dead_code)]
mod projected_realize_composition_v4;
/// Family-neutral EffectProgram V3 composition for canonical Resolution CPIs.
pub mod resolution_composition_v3;
/// Series family projection behind the common data-defined Trading boundary.
#[cfg(any(feature = "families", feature = "series-family"))]
pub mod series;
/// Family-neutral read-only Shadow-AOT comparison CPI.
pub mod shadow_composition_v3;
/// Wallet-authorized caller for one canonical Claims User Position.
pub mod user_position_admission_v1;

/// Stable refusal from the canonical Trading SBF boundary.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TradingSbfError {
    /// The instruction is not supported by an admitted content profile.
    UnsupportedContent = 0x4000,
    /// The Registry receipt did not authenticate this Program as current Trading.
    Release = 0x4001,
    /// The immutable Trading child root or its PDA refused.
    Root = 0x4002,
    /// Manifest, selected entry, descriptor, or config content refused.
    Content = 0x4003,
    /// The checked data-defined transition refused.
    Transition = 0x4004,
    /// A projected physical mutation or account write could not commit.
    Commit = 0x4005,
    /// Instructions-sysvar or native-signature evidence was not exact.
    NativeSignature = 0x4006,
    /// The release's pinned deployment slot moved: the substrate was upgraded.
    /// Every open market on the superseded release generation refuses until a
    /// re-release re-authenticates the new deployment and re-pins its slot.
    ///
    /// Decision 0012. Not a corrupted account and not an attack: the exact
    /// upgrade authority the release names shipped new bytes, so the cached
    /// authentication no longer describes what is deployed.
    ReleaseSuperseded = 0x4007,
    /// This route needs the extended heap and the transaction did not grant it.
    ///
    /// The route declares an extended heap profile, but the runtime handed it
    /// the protocol default anyway, because the transaction carried no
    /// ComputeBudget `RequestHeapFrame` or presented no instructions sysvar.
    /// That grant is best-effort by construction, so without this refusal the
    /// route would allocate until it died -- and an out-of-memory abort names
    /// nothing: not the route, not the budget, not what the caller omitted.
    ///
    /// The remedy is always the caller's and always the same: add
    /// `ComputeBudgetInstruction::request_heap_frame` to the transaction and
    /// keep the instructions sysvar in the account frame.
    HeapFrame = 0x4008,
    /// The account offered to `CloseSeal` is not a live canonical seal.
    ///
    /// Omission `P-006`. It is absent, System-owned, the wrong width, not
    /// writable, not rent-exempt, carries a body that is not a canonical
    /// artifact-profile-1 seal, or sits at an address its own body does not
    /// reproduce. The ordinary reading is the first one: `CloseSeal` is
    /// permissionless and racing is expected, so **a second close of the same
    /// seal refuses here, by absence**, and that is the whole of the
    /// double-close story.
    CloseSealAccount = 0x4009,
    /// `CloseSeal` was aimed at a seal the live Trading release still addresses.
    ///
    /// Omission `P-006`, and the conjunct the whole route rests on. A seal's
    /// fourth PDA seed is the Trading semantic release that wrote it, so a
    /// release stops addressing a seal rather than invalidating it (decision
    /// 0005). The close is allowed only once the seal has fallen out of the
    /// live release's address space: the closer exhibits a Registry-owned
    /// activation cache that still authenticates its Trading role against this
    /// deployed Program and its ProgramData — which a superseded generation
    /// cannot do, because the Loader moved its pinned slot (decision 0012) —
    /// and the semantic release that cache names must differ from the seal's.
    ///
    /// Refusing here is also what keeps the seal write-once: the writer derives
    /// the seal address from that same live semantic release, so an address
    /// this code protects is an address the live executable could rewrite.
    CloseSealLiveRelease = 0x400A,
    /// The `CloseSeal` frame was not the exact permissionless closing shape.
    ///
    /// Omission `P-006`. The beneficiary did not sign, is not a writable empty
    /// System wallet, or aliases the seal; the Registry account is not the one
    /// the seal's own key names; the Trading program account is not this
    /// executable; or the Rent sysvar is not the Rent sysvar. Distinct from
    /// [`TradingSbfError::CloseSealAccount`] because it says the *caller's*
    /// frame is wrong rather than that there is nothing here to close.
    CloseSealFrame = 0x400B,
    /// The maker replay this fee settlement names records no obligation.
    ///
    /// `FEE_SECOND_TRANSACTION_V1` section 2.4 invariant 3: tx1 sets
    /// `fee_owed := combined_fee` and tx2 sets it to zero. Zero is therefore
    /// both "never owed" and "already settled", and the route cannot and need
    /// not distinguish them -- in both states there is nothing to move and the
    /// maker is not blocked. A replay at the pre-`fee_owed` width reads zero
    /// and refuses here too.
    FeeNotOwed = 0x400C,
    /// The fee destination is not a token account of the configured recipient.
    ///
    /// Pinned by OWNER and mint, never by address: a recipient token account
    /// closed between the fill and its settlement must not strand the fee, so
    /// any account of that owner will do (design section 3). "Any account of
    /// that owner" is not "any account", and this is where the difference is
    /// refused.
    FeeDestination = 0x400D,
    /// The fee source is not a token account of the debtor.
    ///
    /// Custody checks `source.key == request.source` and the source's mint, and
    /// never `semantic.source_owner` -- so without this pin one maker could
    /// settle their own obligation out of another maker's standing delegation,
    /// clearing themselves for free and stranding the account they spent. The
    /// design's section 1.4 refusal table does not enumerate this row; it is
    /// added here rather than left to the reader.
    FeeSource = 0x400E,
    /// The `CloseMakerReplay` frame was not the exact permissionless shape.
    ///
    /// Wall 22. The account count, a privilege, or a duplicate address is
    /// wrong; the Trading program account is not this executable; the Rent
    /// sysvar is not the Rent sysvar; or the rent destination is not the plain
    /// System wallet the replay's own `rent_owner` field records. The remedy
    /// is the caller's: rebuild the 22-account frame from the replay's own
    /// recorded facts.
    CloseMakerFrame = 0x400F,
    /// The account offered as the maker replay is not the canonical one.
    ///
    /// Wall 22. It is absent, System-owned, carries a body that is not a
    /// canonical replay for the request's market/generation/maker coordinate,
    /// or sits at an address its own persisted bump does not reproduce.
    /// `CloseMakerReplay` is permissionless and racing is expected, so **a
    /// second close of the same replay refuses here, by absence** -- the same
    /// double-close story as [`TradingSbfError::CloseSealAccount`].
    CloseMakerReplayAccount = 0x4010,
    /// The maker replay still owes its recorded Direct fee.
    ///
    /// Cohort-9 review item 1, amendment 2. This account is the SOLE record of
    /// the FEE-TX2 receivable -- `fee_settlement_v1` reads the amount off it
    /// and nothing else -- so a close that ignored `fee_owed` would erase a
    /// debt with no residue. The remedy is always available: fee settlement is
    /// deliberately phase-free, so settle (permissionlessly), then close.
    CloseMakerFeeOutstanding = 0x4011,
    /// The maker replay still has registered live intents.
    ///
    /// Wall 22. `live_count` is nonzero: registered records under this replay
    /// have not all closed. Close them first -- older ones are permissionlessly
    /// closable once `cancel-through` passes them -- then close the replay.
    CloseMakerLiveIntents = 0x4012,
    /// The activation descriptor's own effect program refused its projection.
    ///
    /// Raised at exactly one site --
    /// `outer::ActivationRuntimeV2::prepare_effects`' call to
    /// `project_with_aliases_and_requests_atomic` -- and reachable only from
    /// `process_activation`, which is the only caller of `prepare_effects`.
    /// This is the family's DECLARED effect program executing against the
    /// seeded register bank: a `RequireLamportsEq` that the transfer above it
    /// falsified, an alias or permission the profile does not grant, a request
    /// write outside the tail. The content was admitted; running it is what
    /// refused.
    ///
    /// Split out of [`TradingSbfError::Content`] because it could not be
    /// distinguished from it. `late_effect_refusal_rolls_back_the_projected_
    /// transfer` spent the whole baseline refusing at `seed_common_registers`
    /// -- nine hundred lines upstream, before the effect program ran at all --
    /// and no assertion available to it could tell: both carried `Content`, and
    /// so do roughly twenty other sites on this one route. The repair
    /// (`d969d8f7`) moved its refusal 4,799 CU downstream into the projection,
    /// which is a thing a test should be able to *say*, not a thing a lane has
    /// to re-measure to believe.
    ActivationEffect = 0x4013,
}

impl TradingSbfError {
    /// Every refusal this boundary can raise, in discriminant order.
    ///
    /// This is what the band assertions below read. It is kept honest by
    /// [`TradingSbfError::ordinal`], whose match is exhaustive: a variant added
    /// to the enum does not compile until its author writes an arm there, and
    /// the only arm that satisfies the assertions is its own index here.
    pub const ALL: [Self; 20] = [
        Self::UnsupportedContent,
        Self::Release,
        Self::Root,
        Self::Content,
        Self::Transition,
        Self::Commit,
        Self::NativeSignature,
        Self::ReleaseSuperseded,
        Self::HeapFrame,
        Self::CloseSealAccount,
        Self::CloseSealLiveRelease,
        Self::CloseSealFrame,
        Self::FeeNotOwed,
        Self::FeeDestination,
        Self::FeeSource,
        Self::CloseMakerFrame,
        Self::CloseMakerReplayAccount,
        Self::CloseMakerFeeOutstanding,
        Self::CloseMakerLiveIntents,
        Self::ActivationEffect,
    ];

    /// This refusal's position in [`TradingSbfError::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a
    /// sixteenth variant is a COMPILE ERROR here rather than a discriminant no
    /// assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::UnsupportedContent => 0,
            Self::Release => 1,
            Self::Root => 2,
            Self::Content => 3,
            Self::Transition => 4,
            Self::Commit => 5,
            Self::NativeSignature => 6,
            Self::ReleaseSuperseded => 7,
            Self::HeapFrame => 8,
            Self::CloseSealAccount => 9,
            Self::CloseSealLiveRelease => 10,
            Self::CloseSealFrame => 11,
            Self::FeeNotOwed => 12,
            Self::FeeDestination => 13,
            Self::FeeSource => 14,
            Self::CloseMakerFrame => 15,
            Self::CloseMakerReplayAccount => 16,
            Self::CloseMakerFeeOutstanding => 17,
            Self::CloseMakerLiveIntents => 18,
            Self::ActivationEffect => 19,
        }
    }
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
//
// WHY THIS IS A LIST AND NOT TWO ENDPOINTS. The ceiling assertion used to name
// one variant BY HAND as "the last one", and it stopped bounding the enum the
// moment a variant was appended past the one it named. That happened TWICE in
// one week -- `CloseSeal`'s three arrived after `HeapFrame` on main, and the
// fee lane's three arrived after those -- and neither went red.
//
// A variant COUNT was the first repair, and it was a real improvement: bumping
// the count without moving the named variant refused. But the count was still a
// number a human typed, and the case it could not see is the one that keeps
// happening -- a sixteenth variant appended while the count and the named
// ceiling are both left alone. `FeeSource` is still the fourteenth index, the
// count still reads 15, both assertions still pass, and the new refusal is
// checked by nothing. The count did not close the hole; it moved it.
//
// So the band is now checked over `ALL`, element by element, and `ALL` is
// welded to the enum by the exhaustive `ordinal` match. A new variant cannot
// join quietly: it does not compile until its author answers for it, and the
// answer they must give is its index here. The hand-typed count retires into
// `ALL.len()`, which is the same number with nobody left to mistype it.
const _: () = {
    assert!(
        TradingSbfError::ALL[0] as u32 == dclutch_refusal_registry::TRADING_REFUSAL_BASE,
        "TradingSbfError must start at its registered refusal band base"
    );
    let mut index = 0;
    while index < TradingSbfError::ALL.len() {
        let variant = TradingSbfError::ALL[index];
        assert!(
            variant.ordinal() == index,
            "TradingSbfError::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32 == dclutch_refusal_registry::TRADING_REFUSAL_BASE + index as u32,
            "TradingSbfError discriminants are not the contiguous run from the band base that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::TRADING_REFUSAL_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "TradingSbfError must not run past its registered refusal band"
        );
        index += 1;
    }
};

impl From<TradingSbfError> for ProgramError {
    fn from(value: TradingSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

use crate::admitted_composition_v3::ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4;
use dclutch_capability_program_contract::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3;

/// Bank pages the widest common heap profile carries.
///
/// **Measured-profile headroom, not a protocol fact** -- the same kind of
/// number as `entrypoint_adapter::ADAPTER_STACK_SLOTS_V1`, and named for the
/// same reason: so the bound it feeds is a sum of things that can be checked
/// one at a time. Each page carries
/// `dclutch_execution_strategy_contract::generated_v2::ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2`
/// (880) bytes of accelerator input bank.
pub const TRADING_MAX_HOT_BANK_PAGES_V3: usize = 10;

/// Non-injected physical runtime representatives the widest common heap
/// profile carries.
///
/// **Measured-profile headroom, not a protocol fact.** The canonical
/// Registry-continuation Hot bundle uses 78 account slots; this is the ceiling
/// the entrypoint will deserialize, not a width anything reaches.
pub const TRADING_MAX_HOT_PHYSICAL_REPRESENTATIVES_V3: usize = 251;

/// Maximum bounded account vector accepted by the canonical Trading entrypoint.
///
/// This covers the Registry-continuation Hot frame at the common heap-profile
/// maxima, and is now the SUM OF ITS TERMS rather than a literal that has to be
/// re-added by hand:
///
/// | term | value | authority |
/// |---|---:|---|
/// | common fixed frame | [`HOT_FIXED_ACCOUNT_COUNT_V3`] | capability-program contract |
/// | Registry continuation admission | 1 | `hot_v3::authenticate_hot_invocation_v3` |
/// | admitted-AOT strategy evidence | [`ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4`] | `admitted_composition_v3` |
/// | bank pages | [`TRADING_MAX_HOT_BANK_PAGES_V3`] | measured profile |
/// | physical runtime representatives | [`TRADING_MAX_HOT_PHYSICAL_REPRESENTATIVES_V3`] | measured profile |
///
/// It was written as `308` on 2026-08-26 with `38 fixed accounts` in its prose,
/// and the fixed frame had ALREADY become 39 -- `ca5e5f14` appended
/// `HOT_CAPABILITY_SEAL_ACCOUNT_V3` at index 38. So the declared bound was one
/// account short of the very shape its own comment enumerated, and would have
/// refused a maximal frame of that shape. This is the third copy of that stale
/// `38` to be found (see `ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4` for the first
/// two), which is why it is a sum now and not a number.
///
/// The Registry continuation's six-account OUTER prefix is deliberately absent
/// from this arithmetic, and that resolves an open question rather than
/// forgetting one: the prefix lives in the top-level instruction's sysvar
/// record, never in the account vector this bound measures.
/// `hot_v3::authenticate_hot_invocation_v3` proves it, comparing
/// `observed.metas_from(REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1).len()`
/// against `accounts.len()` -- the nested vector begins after the prefix.
///
/// `entrypoint_adapter` deserializes this vector without the obsolete
/// 64-account fixed-array limit, on the stack up to
/// `entrypoint_adapter::ADAPTER_STACK_SLOTS_V1` and in an exactly-sized heap
/// buffer above it; larger frames refuse before any family or mutation path
/// executes.
pub const TRADING_MAX_INSTRUCTION_ACCOUNTS_V3: usize = HOT_FIXED_ACCOUNT_COUNT_V3
    + 1
    + ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4
    + TRADING_MAX_HOT_BANK_PAGES_V3
    + TRADING_MAX_HOT_PHYSICAL_REPRESENTATIVES_V3;

/// Execute the family-neutral authenticated capability lifecycle route.
///
/// Activation and native closure share the common descriptor/profile boundary.
/// Realm/token closure remains fail-closed until its ordered-vault adapter
/// lands in this same authority boundary.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    require_instruction_account_bound_v3(accounts.len())?;
    #[cfg(any(feature = "families", feature = "dealer-family"))]
    if dealer_scenario_checkpoint_v1::is_dealer_scenario_checkpoint_create_v1(instruction_data) {
        return dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_create_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(feature = "families", feature = "dealer-family"))]
    if dealer_scenario_checkpoint_v1::is_dealer_scenario_checkpoint_page_v1(instruction_data) {
        return dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_page_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(feature = "families", feature = "dealer-family"))]
    if dealer_scenario_checkpoint_v1::is_dealer_scenario_checkpoint_evaluate_v1(instruction_data) {
        return dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_evaluate_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(feature = "families", feature = "dealer-family"))]
    if dealer_scenario_checkpoint_v1::is_dealer_scenario_checkpoint_reserve_v1(instruction_data) {
        return dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_reserve_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(feature = "families", feature = "dealer-family"))]
    if dealer_scenario_checkpoint_v1::is_dealer_scenario_checkpoint_rollback_v1(instruction_data) {
        return dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_rollback_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(feature = "families", feature = "dealer-family"))]
    if dealer_scenario_checkpoint_v1::is_dealer_scenario_checkpoint_commit_v1(instruction_data) {
        return dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_commit_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(feature = "families", feature = "dealer-family"))]
    if dealer_scenario_checkpoint_v1::is_dealer_scenario_checkpoint_cleanup_v1(instruction_data) {
        return dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_cleanup_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    if dclutch_user_position_admission_contract::is_user_position_admission_v1(instruction_data) {
        return user_position_admission_v1::process_user_position_admission_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(feature = "families")]
    if dclutch_direct_codec::retirement_v1::is_direct_begin_retiring_v1(instruction_data) {
        return direct_begin_retiring_v1::process_direct_begin_retiring_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(feature = "families")]
    if dclutch_direct_codec::close_maker_v1::is_direct_close_maker_v1(instruction_data) {
        return direct_close_maker_v1::process_direct_close_maker_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(feature = "families")]
    if dclutch_direct_codec::replay_setup_v1::is_direct_replay_setup_v1(instruction_data) {
        return direct_replay_setup_v1::process_direct_replay_setup_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(feature = "families")]
    if dclutch_direct_codec::token_setup_v1::is_direct_token_setup_v1(instruction_data) {
        return direct_token_setup_v1::process_direct_token_setup_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    // The fee leg, in the transaction of its own that
    // `docs/design/FEE_SECOND_TRANSACTION_V1.md` moved it into.
    #[cfg(feature = "families")]
    if dclutch_direct_codec::fee_settlement_v1::is_direct_fee_settlement_v1(instruction_data) {
        return direct_fee_settlement_v1::process_direct_fee_settlement_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if generic_market_founding_v1::is_generic_market_founding_v3(instruction_data) {
        return generic_market_founding_v1::process_generic_market_founding_v3(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if generic_founding_stages_v1::is_generic_found_and_permit_v1(instruction_data) {
        return generic_founding_stages_v1::process_generic_found_and_permit_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if generic_founding_stages_v1::is_generic_market_open_v1(instruction_data) {
        return generic_founding_stages_v1::process_generic_market_open_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if projected_custody_bootstrap_v1::is_projected_custody_bootstrap_v2(instruction_data) {
        return projected_custody_bootstrap_v1::process_projected_custody_bootstrap_v2(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if projected_custody_bootstrap_v1::is_controller_funding_prepare_v1(instruction_data) {
        return projected_custody_bootstrap_v1::process_controller_funding_prepare_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    // The way back out of a staged prestate whose founding never happened. A
    // projection that reached `SourceFunded` and did not found before its
    // expiry slot holds collateral that the forward direction can no longer
    // move, because Core's Found and Open stages both refuse an expired
    // artifact. Without this route that collateral is stranded permanently.
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if projected_custody_bootstrap_v1::is_projected_custody_abort_v1(instruction_data) {
        return projected_custody_bootstrap_v1::process_projected_custody_abort_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if projected_custody_bootstrap_v1::is_controller_funding_cleanup_step1_v1(instruction_data) {
        return projected_custody_bootstrap_v1::process_controller_funding_cleanup_step1_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    if projected_custody_bootstrap_v1::is_controller_funding_cleanup_step2_v1(instruction_data) {
        return projected_custody_bootstrap_v1::process_controller_funding_cleanup_step2_v1(
            program_id,
            accounts,
            instruction_data,
        );
    }
    // Decision 0005. The validated-artifact seal is written by its own outer,
    // once per (descriptor, action, Trading interpreter release, Registry).
    // It creates one PDA under this Program, signs nothing else, and can only
    // ever persist this executable's own verdict about immutable public bytes.
    if dclutch_capability_seal_contract::is_capability_seal_request_v1(instruction_data) {
        return hot_v3::process_capability_seal_v1(program_id, accounts, instruction_data);
    }
    // Omission P-006. The other end of the seal's life: a seal the live release
    // no longer addresses is 968 bytes of rent nobody can reach, and the class
    // grows once per Trading release rather than once per Market. The closer
    // signs, keeps exactly the rent the close liberates, and races nobody --
    // the second attempt refuses by absence. It creates nothing, signs for
    // nothing, and touches exactly one account this Program owns.
    if dclutch_capability_seal_contract::is_capability_seal_close_request_v1(instruction_data) {
        return hot_v3::process_capability_seal_close_v1(program_id, accounts, instruction_data);
    }
    if hot_v3::is_hot_execution_v3(instruction_data) {
        hot_v3::process_hot_execution_v3(program_id, accounts, instruction_data)
    } else {
        outer::process_capability_lifecycle(program_id, accounts, instruction_data)
    }
}

fn require_instruction_account_bound_v3(account_count: usize) -> ProgramResult {
    if account_count <= TRADING_MAX_INSTRUCTION_ACCOUNTS_V3 {
        Ok(())
    } else {
        Err(TradingSbfError::UnsupportedContent.into())
    }
}

#[cfg(test)]
mod entrypoint_tests {
    use super::*;

    #[test]
    fn canonical_bound_exceeds_legacy_limit_and_refuses_overflow() {
        // Deliberately a compile-time fact: the canonical bound must exceed
        // the legacy 64-account limit for the runtime checks below to mean
        // anything, so let the compiler enforce it.
        const { assert!(TRADING_MAX_INSTRUCTION_ACCOUNTS_V3 > 64) };
        assert_eq!(require_instruction_account_bound_v3(65), Ok(()));
        assert_eq!(
            require_instruction_account_bound_v3(TRADING_MAX_INSTRUCTION_ACCOUNTS_V3),
            Ok(())
        );
        assert_eq!(
            require_instruction_account_bound_v3(TRADING_MAX_INSTRUCTION_ACCOUNTS_V3 + 1),
            Err(TradingSbfError::UnsupportedContent.into())
        );
    }

    #[test]
    fn canonical_bound_admits_a_maximal_frame_and_moves_when_its_terms_move() {
        // NOT a restatement of the definition. The bound is a sum now, so the
        // only thing left to assert is that its VALUE is pinned: any term
        // changing lands here as a diff on a number a reviewer has to look at,
        // the same discipline this repo's identity tables use. The reason it
        // needs one is on the record -- the bound read `308` with `38 fixed
        // accounts` in its prose while the contract already said 39, so it
        // would have refused a maximal frame of the exact shape it enumerated.
        assert_eq!(HOT_FIXED_ACCOUNT_COUNT_V3, 39);
        assert_eq!(TRADING_MAX_INSTRUCTION_ACCOUNTS_V3, 309);

        // The property the pin exists to protect: a frame of the widest
        // enumerated shape is ADMITTED, and one account wider is refused.
        let widest = HOT_FIXED_ACCOUNT_COUNT_V3
            + 1
            + ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4
            + TRADING_MAX_HOT_BANK_PAGES_V3
            + TRADING_MAX_HOT_PHYSICAL_REPRESENTATIVES_V3;
        assert_eq!(require_instruction_account_bound_v3(widest), Ok(()));
        assert_eq!(
            require_instruction_account_bound_v3(widest + 1),
            Err(TradingSbfError::UnsupportedContent.into())
        );
    }
}
