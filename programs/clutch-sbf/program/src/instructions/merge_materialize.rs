//! `Intent::Merge`, `Intent::Materialize`, `Intent::Dematerialize`.
//!
//! All three are **implemented** here.
//!
//! All three share the ten-account seam plane of [`super::split`]: the account
//! list, the check order, the CLO-DELTA-V1 closure obligations, the kernel step,
//! and the write-back are [`super::split::seam`], because the offline reference
//! adapter routes all four seam intents through one `TransitionMetadata` /
//! `StateBytes` / `ExpectedBindings` triple and a second copy of that list here
//! would be a second place for the seam's writable set to drift.  What this
//! module owns is the mapping from a routed [`Request`] to a
//! [`super::split::SeamOp`].
//!
//! ## External-shadow handling
//!
//! `Materialize` and `Dematerialize` name a destination or a source in the
//! intent itself.  The offline adapter compares it against
//! `metadata.external.key`; this program keeps exactly that comparison and adds
//! what an on-chain program can add, namely that the account it compares
//! against has itself been proved equal to
//! [`crate::seeds::external_pda`].  The two together are obligation 1 of
//! `docs/implementation/SOLANA_REFERENCE_ADAPTER.md` discharged rather than
//! assumed: a caller-named destination is accepted only when it is the
//! canonically derived shadow account.
//!
//! `ExternalAccount` itself is untouched.  `docs/implementation/TOKEN2022_PLAN.md`
//! proposes collapsing the reference-only shadow into a single source of truth
//! with a real token account; that decision is **queued, not taken**, so this
//! lane moves balances through the shadow exactly as the reference adapter does
//! and restructures nothing.
//!
//! ## `Merge` is the exact inverse of `Split`
//!
//! `Merge` used to be **refused** here, and the refusal was mirrored from the
//! offline reference adapter, which had no `Intent::Merge` arm at all: the
//! intent fell through to `_ => Err(Error::UnsupportedIntent)` despite
//! `clutch_kernel::MarketState::merge` existing and despite PROJECT.md's
//! central promise that "the complete set can always be recombined into its
//! collateral before resolution".  The differential test in this module was
//! written as an alarm designed to fail the day the reference grew the arm.
//! The reference grew the arm; the alarm fired; this is what replaced it.
//!
//! The seam plane needed no new shape.  What the reference named, and what this
//! module now mirrors, is the direction of every term:
//!
//! | | `Split` | `Merge` |
//! | --- | --- | --- |
//! | `PositionAccount::cash_atoms` | debited `quantity` (checked sub) | credited `quantity` (checked add) |
//! | `HoardAccount::collateral_atoms` | raised by the kernel | lowered by the kernel |
//! | every internal balance, per outcome | `+quantity` | `-quantity` |
//! | `MarketAccount::collateral_cap` | checked | **not** checked |
//! | phase discipline | `lifecycle == 0 && close_state == 0` | identical |
//!
//! Two of those rows are decisions rather than sign flips.
//!
//! **The cap is not checked on the way down.**  A merge lowers the hoard
//! (`MarketState::merge` refuses `InsufficientCollateral` and then subtracts),
//! so the post-state collateral is strictly below the pre-state collateral and
//! cannot cross a ceiling the pre-state was under.  Checking it anyway would be
//! worse than redundant: a market somehow already above its cap would be unable
//! to unwind, and unwinding is the one direction that always has to stay open.
//!
//! **The cash credit lands after the kernel step, not in the per-intent arm.**
//! `Split`'s debit is a precondition — you pay, then the claims are minted — so
//! it sits with the cap check above the kernel.  A merge's credit is the
//! consequence of a burn that already succeeded, so it sits below, which is
//! both where the reference adapter credits it and where `RedeemInternal`
//! already credits a payout.  [`super::split::seam`] carries the credit at that
//! point for exactly this reason.
//!
//! ## An asymmetry observed in the oracle, mirrored rather than corrected
//!
//! `Split` and `Merge` refuse when `MarketAccount::lifecycle != 0` **or**
//! `PositionAccount::close_state != 0`.  `Materialize` and `Dematerialize`
//! check neither in the offline reference adapter: the market's lifecycle is
//! still covered indirectly, because `validate_links` ties it to the kernel
//! phase and `MarketState::require_active` refuses a resolved market, but a
//! position whose `close_state` is non-zero can still move claims across the
//! internal/external boundary on both adapters.  Whether that is intended —
//! a closing position arguably should still be able to dematerialize — is a
//! semantics question for the reference-adapter lane.  This lane mirrors it
//! rather than silently adding a check the oracle does not have, and names it
//! here so the decision is visible instead of inherited.
//!
//! ## Differential coverage
//!
//! The host differential in this module runs this program's processor path and
//! `clutch_solana_reference::apply` over identical fixture bytes and compares
//! byte-identical post-state and refusal class.  The **SVM-side** differential
//! for these instructions is a named follow-up wave: `harness/` is frozen this
//! wave and still emits a nine-account `Split` transaction, so no emitted
//! transaction exercises the ten-account seam plane yet.

use crate::accounts::Outcome;
use crate::instructions::split;
use clutch_solana_reference::Request;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Apply exactly one `Merge`, `Materialize`, or `Dematerialize` request.
///
/// The request is converted to a [`split::SeamOp`] and handed to the shared
/// seam plane; nothing about the account list, the check order, or the
/// write-back differs from `Split`, because in the reference adapter nothing
/// about them differs either.  `Merge` differs from `Split` only in the sign of
/// every term it moves, and in the two decisions the module docs name.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    let op = split::seam_op(request)?;
    split::seam(program_id, accounts, request.sequence, &op, |ids| {
        split::derive_bindings(program_id, ids)
    })
}

#[cfg(test)]
mod tests {
    use crate::error::ClutchError;
    use crate::instructions::split::tests::{
        edit_position, fixture, h, ids, key, layout_request, second_position, split_request, Case,
        Class,
    };
    use crate::instructions::split::tests::{token_leg, TokenLegCase};
    use crate::instructions::split::{IX_ACTOR, IX_EXTERNAL, IX_MARKET, IX_POSITION, IX_REPLAY};
    use crate::token;
    use clutch_kernel::Error as KernelError;
    use clutch_solana_layout::{
        Hash32, HoardAccount, Intent, PositionAccount, SupplyLedgerAccount,
    };
    use clutch_solana_reference::{ExternalAccount, ReplayAccount};

    /// The fixture's external-shadow account key, as the intent names it.
    fn shadow(case: &Case) -> Hash32 {
        Hash32::from_bytes(case.keys[IX_EXTERNAL].to_bytes())
    }

    fn materialize(case: &Case, sequence: u64, outcome: u8, quantity: u64) -> Vec<u8> {
        let ids = ids();
        layout_request(
            sequence,
            Intent::Materialize {
                market: ids.market,
                owner: ids.owner,
                destination: shadow(case),
                outcome,
                quantity,
            },
        )
    }

    fn dematerialize(case: &Case, sequence: u64, outcome: u8, quantity: u64) -> Vec<u8> {
        let ids = ids();
        layout_request(
            sequence,
            Intent::Dematerialize {
                market: ids.market,
                owner: ids.owner,
                source: shadow(case),
                outcome,
                quantity,
            },
        )
    }

    fn merge(sequence: u64, quantity: u64) -> Vec<u8> {
        let ids = ids();
        layout_request(
            sequence,
            Intent::Merge {
                market: ids.market,
                owner: ids.owner,
                quantity,
            },
        )
    }

    /// A case that has already split 20 complete sets, so the seam has claims
    /// to move in both directions.
    fn funded() -> Case {
        let mut case = fixture();
        case.advance(&split_request(0, 20), "split 20");
        case
    }

    #[test]
    fn materialize_and_dematerialize_agree_byte_for_byte_with_the_reference() {
        let mut case = funded();
        let request = materialize(&case, 1, 1, 7);
        case.advance(&request, "materialize 7 of outcome 1");

        let position = PositionAccount::decode(&case.state.position).expect("position decodes");
        let external = ExternalAccount::decode(&case.state.external).expect("shadow decodes");
        let supply = SupplyLedgerAccount::decode(&case.state.supply).expect("ledger decodes");
        assert_eq!(position.internal[1], 13);
        assert_eq!(external.balances[1], 7);
        /* Materialization is supply-neutral: it moves one claim between the
         * ledger's two terms and changes neither their sum nor the kernel
         * aggregate. */
        assert_eq!(supply.internal_supply[1], 13);
        assert_eq!(supply.external_supply[1], 7);
        assert_eq!(supply.internal_supply[0], 20);
        assert_eq!(supply.external_supply[0], 0);

        let request = dematerialize(&case, 2, 1, 3);
        case.advance(&request, "dematerialize 3 of outcome 1");
        let supply = SupplyLedgerAccount::decode(&case.state.supply).expect("ledger decodes");
        assert_eq!(supply.internal_supply[1], 16);
        assert_eq!(supply.external_supply[1], 4);
        let external = ExternalAccount::decode(&case.state.external).expect("shadow decodes");
        assert_eq!(external.balances[1], 4);
    }

    #[test]
    fn materialize_refusals_agree_with_the_reference_adapter() {
        let case = funded();

        // Insufficient balance: more claims than the position holds.
        case.refuses(
            &materialize(&case, 1, 0, 21),
            "materialize beyond the position",
            Class::Kernel(KernelError::InsufficientBalance),
        );

        /* An outcome outside the market is a kernel refusal.  A quantity of
         * zero is deliberately *not* tested here: the frozen intent codec
         * refuses to encode one at all (`CodecError::ZeroValue`), so
         * `KernelError::ZeroQuantity` is unreachable through the layout
         * request envelope on either adapter. */
        case.refuses(
            &materialize(&case, 1, 5, 1),
            "materialize an outcome the market has not got",
            Class::Kernel(KernelError::InvalidPayoutIndex),
        );

        // The caller names a destination that is not the external shadow.
        let ids = ids();
        case.refuses(
            &layout_request(
                1,
                Intent::Materialize {
                    market: ids.market,
                    owner: ids.owner,
                    destination: h(0xab),
                    outcome: 0,
                    quantity: 1,
                },
            ),
            "destination is not the shadow account",
            Class::Binding,
        );
        case.refuses(
            &layout_request(
                1,
                Intent::Materialize {
                    market: ids.market,
                    owner: ids.owner,
                    destination: Hash32::from_bytes(case.keys[IX_POSITION].to_bytes()),
                    outcome: 0,
                    quantity: 1,
                },
            ),
            "destination is the position account",
            Class::Binding,
        );

        // Wrong signer, and no signer at all.
        let mut stranger = funded();
        stranger.keys[IX_ACTOR] = key(0x9e);
        let request = materialize(&stranger, 1, 0, 1);
        stranger.refuses(&request, "stranger materializes", Class::Actor);

        let mut unsigned = funded();
        unsigned.signer = false;
        let request = materialize(&unsigned, 1, 0, 1);
        unsigned.refuses(&request, "unsigned materialize", Class::Signature);

        // Aliased accounts.
        let mut aliased = funded();
        aliased.keys[IX_REPLAY] = aliased.keys[IX_EXTERNAL];
        aliased.bindings.replay = aliased.bindings.external;
        let request = materialize(&aliased, 1, 0, 1);
        aliased.refuses(&request, "replay aliases the shadow", Class::Alias);

        // Stale replay.
        case.refuses(
            &materialize(&case, 9, 0, 1),
            "stale materialize sequence",
            Class::Replay,
        );
    }

    #[test]
    fn dematerialize_refusals_agree_with_the_reference_adapter() {
        let case = funded();

        // Nothing has been materialized, so the shadow holds nothing.
        case.refuses(
            &dematerialize(&case, 1, 0, 1),
            "dematerialize an empty shadow",
            Class::Kernel(KernelError::InsufficientBalance),
        );

        let ids = ids();
        case.refuses(
            &layout_request(
                1,
                Intent::Dematerialize {
                    market: ids.market,
                    owner: ids.owner,
                    source: h(0xab),
                    outcome: 0,
                    quantity: 1,
                },
            ),
            "source is not the shadow account",
            Class::Binding,
        );

        let mut stranger = funded();
        stranger.keys[IX_ACTOR] = key(0x9e);
        let request = dematerialize(&stranger, 1, 0, 1);
        stranger.refuses(&request, "stranger dematerializes", Class::Actor);

        let mut unsigned = funded();
        unsigned.signer = false;
        let request = dematerialize(&unsigned, 1, 0, 1);
        unsigned.refuses(&request, "unsigned dematerialize", Class::Signature);

        let mut aliased = funded();
        aliased.keys[IX_REPLAY] = aliased.keys[IX_EXTERNAL];
        aliased.bindings.replay = aliased.bindings.external;
        let request = dematerialize(&aliased, 1, 0, 1);
        aliased.refuses(&request, "replay aliases the shadow", Class::Alias);

        case.refuses(
            &dematerialize(&case, 9, 0, 1),
            "stale dematerialize sequence",
            Class::Replay,
        );
    }

    #[test]
    fn a_counterfeit_materialize_is_refused_by_both_adapters() {
        /* The regression `clutch_solana_reference` names
         * `forged_position_cannot_materialize_claims_absent_from_aggregate`:
         * a position claiming one internal atom that the kernel aggregate and
         * the supply ledger do not carry.  Under CLO-DELTA-V1 the C2
         * representation bound refuses it before anything is written, so the
         * counterfeit claim can never be moved into the external shadow, where
         * it would become a bearer asset. */
        let mut forged = fixture();
        edit_position(&mut forged.state, |position| position.internal[0] = 1);
        let request = materialize(&forged, 0, 0, 1);
        forged.refuses(&request, "internal 1 against aggregate 0", Class::Closure);

        let (result, post) = forged.program(&request);
        assert_eq!(result, Err(ClutchError::AggregateClosureMismatch.into()));
        assert_eq!(post, forged.state, "a refused materialize writes nothing");
        let external = ExternalAccount::decode(&post.external).expect("shadow decodes");
        assert_eq!(external.balances[0], 0, "no counterfeit reached the shadow");
        let supply = SupplyLedgerAccount::decode(&post.supply).expect("ledger decodes");
        assert_eq!(supply.internal_supply[0], 0);
        assert_eq!(supply.external_supply[0], 0);
    }

    /* -------------------------------------------------------------------- */
    /* The optional Token-2022 leg                                           */
    /* -------------------------------------------------------------------- */

    /// A funded case plus a well-formed token leg for one outcome.
    ///
    /// The injected outcome-mint binding stands in for the derivation
    /// `crate::seeds::find` deliberately does not compile off-chain; on-chain
    /// `derive_bindings` computes the same address from the same seeds.
    fn token_case(outcome: u8, mint_supply: u64, holder_amount: u64) -> (Case, TokenLegCase) {
        let mut case = funded();
        let leg = token_leg(&case, outcome, mint_supply, holder_amount);
        case.bindings.outcome_mint = Some((leg.mint, 11));
        (case, leg)
    }

    /// Refuse with exactly one adapter code, and write nothing at all.
    ///
    /// Every refusal reachable *before* the CPI leaves all nine program
    /// accounts byte-identical, because steps 1-3 of `TOKEN2022_PLAN.md` §3.3
    /// run before the first write.  The one refusal that does not is the
    /// post-CPI delta check, which has [`refuses_after_the_kernel_ran`].
    fn refuses_with(
        case: &Case,
        request: &[u8],
        leg: &TokenLegCase,
        expected: ClutchError,
        label: &str,
    ) {
        let (result, post) = case.program_with_token_leg(request, leg);
        assert_eq!(result, Err(expected.into()), "{label}");
        assert_eq!(
            post, case.state,
            "{label}: a pre-CPI refusal writes nothing"
        );
    }

    /// Refuse after the kernel and the ledger have already been written.
    ///
    /// **This is the one deviation from `TOKEN2022_PLAN.md` §3.3, stated as a
    /// test rather than as a footnote.**  The plan's step order is validate,
    /// admit, snapshot, compute, token effects, verify, *commit*.  This program
    /// runs the compute step through [`super::split::seam`]'s existing
    /// `kernel_step` and `ledger_step`, and those two **write their accounts as
    /// they compute**: splitting them into compute-and-commit halves would mean
    /// carrying a decoded `KernelAccount` — most of an SBF frame on its own —
    /// across two frames, which is the constraint `SBF_BRINGUP.md` records.
    ///
    /// So a refusal raised by the post-CPI delta check leaves the kernel
    /// aggregate and the supply ledger already written, and the Hoard,
    /// position, external shadow and replay accounts untouched.  On chain that
    /// is invisible: SVM transaction semantics discard every byte of it.  Off
    /// chain, in this test harness, there is no rollback, so the writes are
    /// observable — and asserting them here is what keeps the deviation a
    /// measured fact instead of a claim.
    fn refuses_after_the_kernel_ran(
        case: &Case,
        request: &[u8],
        leg: &TokenLegCase,
        expected: ClutchError,
        label: &str,
    ) {
        let (result, post) = case.program_with_token_leg(request, leg);
        assert_eq!(result, Err(expected.into()), "{label}");
        assert_eq!(post.hoard, case.state.hoard, "{label}: hoard untouched");
        assert_eq!(
            post.position, case.state.position,
            "{label}: position untouched"
        );
        assert_eq!(
            post.external, case.state.external,
            "{label}: shadow untouched"
        );
        assert_eq!(post.replay, case.state.replay, "{label}: replay untouched");
        assert_eq!(post.market, case.state.market, "{label}: market untouched");
        assert_ne!(
            post.supply, case.state.supply,
            "{label}: the ledger write is the deviation this test exists to pin"
        );
    }

    #[test]
    fn the_account_count_selects_the_plane_and_nothing_else_does() {
        let (case, leg) = token_case(1, 0, 0);

        /* Ten accounts is the plane that existed before this lane, and it is
         * unchanged: the shadow moves and no token is touched. */
        let request = materialize(&case, 1, 1, 7);
        let mut ten = case.clone();
        ten.advance(&request, "ten-account materialize still works");
        let external = ExternalAccount::decode(&ten.state.external).expect("shadow decodes");
        assert_eq!(external.balances[1], 7);

        /* Eleven or twelve is neither plane.  A partial token leg is a caller
         * error and not a plane this program will guess at. */
        let (result, _) = case.program(&request);
        assert!(result.is_ok(), "the ten-account plane is still accepted");

        /* `Split` and `Merge` move collateral, whose leg is unwired, so a
         * thirteen-account collateral intent is refused rather than having its
         * suffix ignored. */
        let (result, post) = case.program_with_token_leg(&merge(1, 1), &leg);
        assert_eq!(result, Err(ClutchError::AccountCount.into()));
        assert_eq!(post, case.state);
        let (result, post) = case.program_with_token_leg(&split_request(1, 1), &leg);
        assert_eq!(result, Err(ClutchError::AccountCount.into()));
        assert_eq!(post, case.state);
    }

    #[test]
    fn an_off_chain_token_leg_refuses_the_delta_it_could_not_move() {
        /* `solana_cpi::invoke_signed` compiles to `Ok(())` under
         * `not(target_os = "solana")`.  Without the exact-delta check of
         * `TOKEN2022_PLAN.md` §3.3 step 6 this materialize would report success
         * having moved nothing -- which is precisely the failure mode the check
         * exists for, and precisely why the check is not "defence in depth".
         *
         * This is the one property of the CPI that can be established on the
         * host.  The CPI actually working is `programs/clutch-sbf/svm-tests`. */
        let (case, leg) = token_case(1, 0, 0);
        refuses_after_the_kernel_ran(
            &case,
            &materialize(&case, 1, 1, 7),
            &leg,
            ClutchError::TokenDeltaMismatch,
            "off-chain mint_to moves nothing",
        );
        /* The same for a burn, and note the fixture's holder does hold enough:
         * the refusal is about the delta, not about the balance. */
        let (case, leg) = token_case(1, 7, 7);
        let mut funded_shadow = case.clone();
        funded_shadow.advance(&materialize(&case, 1, 1, 7), "materialize without the leg");
        funded_shadow.bindings.outcome_mint = case.bindings.outcome_mint;
        refuses_after_the_kernel_ran(
            &funded_shadow,
            &dematerialize(&funded_shadow, 2, 1, 3),
            &leg,
            ClutchError::TokenDeltaMismatch,
            "off-chain burn moves nothing",
        );
    }

    #[test]
    fn the_token_program_role_is_authenticated_before_anything_is_read() {
        let (case, base) = token_case(1, 0, 0);
        let request = materialize(&case, 1, 1, 7);

        let mut impostor = base.clone();
        impostor.token_program = key(0x4d);
        refuses_with(
            &case,
            &request,
            &impostor,
            ClutchError::WrongTokenProgram,
            "another program at the token-program role",
        );

        /* The right key on an account that cannot be invoked is not the token
         * program either. */
        let mut inert = base.clone();
        inert.token_program_executable = false;
        refuses_with(
            &case,
            &request,
            &inert,
            ClutchError::WrongTokenProgram,
            "a non-executable token program",
        );

        let mut writable = base.clone();
        writable.token_program_writable = true;
        refuses_with(
            &case,
            &request,
            &writable,
            ClutchError::UnexpectedWritable,
            "a writable token program",
        );

        /* The mint and the holder are written by the CPI, so a caller that did
         * not declare them writable is refused before the CPI, not by it. */
        let mut read_only_mint = base.clone();
        read_only_mint.mint_writable = false;
        refuses_with(
            &case,
            &request,
            &read_only_mint,
            ClutchError::NotWritable,
            "a read-only outcome mint",
        );
        let mut read_only_holder = base;
        read_only_holder.holder_writable = false;
        refuses_with(
            &case,
            &request,
            &read_only_holder,
            ClutchError::NotWritable,
            "a read-only holder account",
        );
    }

    #[test]
    fn the_outcome_mint_is_derived_and_never_believed() {
        let (case, base) = token_case(1, 0, 0);
        let request = materialize(&case, 1, 1, 7);

        // A perfectly well-formed mint at an address the seeds do not produce.
        let mut elsewhere = base.clone();
        elsewhere.mint = key(0x5e);
        elsewhere.holder_data = token::fixtures::account_bytes(
            elsewhere.mint.to_bytes(),
            case.keys[IX_ACTOR].to_bytes(),
            0,
        );
        refuses_with(
            &case,
            &request,
            &elsewhere,
            ClutchError::WrongPda,
            "a mint at an underived address",
        );

        // The mint of another outcome, presented for this one.
        let mut other_outcome = base;
        other_outcome.mint = crate::instructions::split::tests::outcome_mint_key(0);
        refuses_with(
            &case,
            &request,
            &other_outcome,
            ClutchError::WrongPda,
            "outcome 0's mint presented for outcome 1",
        );
    }

    #[test]
    fn the_extension_refusal_runs_at_the_instruction_and_not_only_at_init() {
        /* `TOKEN2022_PLAN.md` §3.4: a mint address is not a stable description
         * of a mint's behaviour, because `MintCloseAuthority` lets a zero-supply
         * mint be closed and reinitialized with a different extension set.  So
         * the refusal runs over the mint as loaded in *this* transaction. */
        let (case, base) = token_case(1, 0, 0);
        let request = materialize(&case, 1, 1, 7);
        let authority = case.keys[IX_MARKET].to_bytes();

        for (discriminant, label) in [
            (1_u16, "TransferFeeConfig"),
            (3, "MintCloseAuthority"),
            (6, "DefaultAccountState"),
            (9, "NonTransferable"),
            (12, "PermanentDelegate"),
            (14, "TransferHook"),
            (26, "Pausable"),
        ] {
            let mut hostile = base.clone();
            hostile.mint_data = token::fixtures::mint_with_extension(authority, 0, discriminant);
            refuses_with(
                &case,
                &request,
                &hostile,
                ClutchError::TokenExtensionNotAllowed,
                label,
            );
        }

        /* And a discriminant no build knows: fail closed, not shrug.  29 is the
         * first one `EXTENSION_KNOWN_MASK` does not cover. */
        let mut future = base;
        future.mint_data = token::fixtures::mint_with_extension(authority, 0, 29);
        refuses_with(
            &case,
            &request,
            &future,
            ClutchError::TokenExtensionNotAllowed,
            "an extension discriminant from the future",
        );
    }

    #[test]
    fn an_outcome_mint_this_program_could_not_have_created_is_refused() {
        let (case, base) = token_case(1, 0, 0);
        let request = materialize(&case, 1, 1, 7);

        // Somebody else holds the mint authority: this is not our liability.
        let mut stolen = base.clone();
        stolen.mint_data = token::fixtures::outcome_mint_bytes([0x9a; 32], 0);
        refuses_with(
            &case,
            &request,
            &stolen,
            ClutchError::MintNotAdmitted,
            "a mint somebody else can mint",
        );

        // Nonzero decimals: the PROPOSED unit decision, enforced.
        let mut scaled = base.clone();
        scaled.mint_data =
            token::fixtures::mint_bytes(6, 0, Some(case.keys[IX_MARKET].to_bytes()), None);
        refuses_with(
            &case,
            &request,
            &scaled,
            ClutchError::MintNotAdmitted,
            "an outcome mint with decimals",
        );

        // A freeze authority on a claim token is discretionary seizure.
        let mut freezable = base.clone();
        freezable.mint_data = token::fixtures::mint_bytes(
            0,
            0,
            Some(case.keys[IX_MARKET].to_bytes()),
            Some([0x3b; 32]),
        );
        refuses_with(
            &case,
            &request,
            &freezable,
            ClutchError::MintNotAdmitted,
            "an outcome mint with a freeze authority",
        );

        // The right bytes under the wrong program.
        let mut wrong_program = base;
        wrong_program.mint_owner = key(0x77);
        refuses_with(
            &case,
            &request,
            &wrong_program,
            ClutchError::WrongTokenProgram,
            "a mint some other program owns",
        );
    }

    #[test]
    fn the_holder_account_is_bound_to_the_mint_and_to_the_authenticated_actor() {
        let (case, base) = token_case(1, 0, 0);
        let request = materialize(&case, 1, 1, 7);

        // A token account for a different mint.
        let mut wrong_mint = base.clone();
        wrong_mint.holder_data =
            token::fixtures::account_bytes([0x21; 32], case.keys[IX_ACTOR].to_bytes(), 0);
        refuses_with(
            &case,
            &request,
            &wrong_mint,
            ClutchError::TokenAccountNotAdmitted,
            "a holder account for another mint",
        );
        /* And the subject matters: the *same* `TokenRefusal::WrongMint` reason
         * on the mint itself is `MintNotAdmitted`, because on a mint it says
         * "wrong asset" and on an account it says "this account holds the wrong
         * asset". */

        /* A stranger's account for the right mint.  The destination is
         * caller-supplied, so this is the check that stops a materialize from
         * minting somebody else's claim into somebody else's wallet. */
        let mut stranger = base.clone();
        stranger.holder_data = token::fixtures::account_bytes(base.mint.to_bytes(), [0x8f; 32], 0);
        refuses_with(
            &case,
            &request,
            &stranger,
            ClutchError::TokenAccountNotAdmitted,
            "a stranger's holder account",
        );

        // A frozen account cannot receive.
        let mut frozen = base;
        let mut data = token::fixtures::account_bytes(
            frozen.mint.to_bytes(),
            case.keys[IX_ACTOR].to_bytes(),
            0,
        );
        data[108] = 2;
        frozen.holder_data = data;
        refuses_with(
            &case,
            &request,
            &frozen,
            ClutchError::TokenAccountNotAdmitted,
            "a frozen holder account",
        );
    }

    #[test]
    fn a_token_leg_never_reaches_a_refused_transition() {
        /* Ordering, stated as a property: every check the ten-account plane
         * already ran still runs, and it runs *first*.  A stale sequence with a
         * hostile token leg reports the replay fault, not a token fault, and
         * writes nothing -- so a caller cannot use the token leg to change
         * which refusal a request gets. */
        let (case, base) = token_case(1, 0, 0);
        let mut hostile = base;
        hostile.mint_data =
            token::fixtures::mint_with_extension(case.keys[IX_MARKET].to_bytes(), 0, 1);
        refuses_with(
            &case,
            &materialize(&case, 9, 1, 7),
            &hostile,
            ClutchError::Replay,
            "a stale sequence outranks a hostile mint",
        );
        refuses_with(
            &case,
            &materialize(&case, 1, 0, 21),
            &hostile,
            ClutchError::TokenExtensionNotAllowed,
            "the kernel has not run yet when the mint is admitted",
        );
    }

    #[test]
    fn merge_agrees_byte_for_byte_with_the_reference_adapter() {
        /* This test replaces the alarm the previous wave left here.  It used to
         * assert that both adapters refuse `Merge` `UnsupportedIntent`, and it
         * was written to fail the day the reference grew an `Intent::Merge`
         * arm.  It did. */
        let mut case = funded();
        case.advance(&merge(1, 4), "merge 4 complete sets");

        let position = PositionAccount::decode(&case.state.position).expect("position decodes");
        let hoard = HoardAccount::decode(&case.state.hoard).expect("hoard decodes");
        let supply = SupplyLedgerAccount::decode(&case.state.supply).expect("ledger decodes");
        /* Every term of a split, with the sign flipped: 20 - 4 internal claims
         * per outcome, 20 - 4 collateral atoms in the hoard, and cash credited
         * rather than debited (100 - 20 after the split, + 4 here). */
        assert_eq!(position.internal[0], 16);
        assert_eq!(position.internal[1], 16);
        assert_eq!(position.cash_atoms, 84);
        assert_eq!(hoard.collateral_atoms, 16);
        assert_eq!(supply.internal_supply[0], 16);
        assert_eq!(supply.internal_supply[1], 16);
        assert_eq!(supply.external_supply[0], 0);
        assert_eq!(supply.external_supply[1], 0);
        /* Reserved cash is untouched: a merge credits free cash and never the
         * encumbered part. */
        assert_eq!(position.reserved_cash_atoms, 7);
    }

    #[test]
    fn split_then_merge_returns_the_seam_to_its_pre_split_bytes() {
        /* PROJECT.md's recombination promise at byte resolution, on the program
         * side of the differential: eight of the nine accounts come back
         * exactly, and the replay account must not, because two sequences were
         * consumed. */
        let before = fixture();
        let mut case = before.clone();
        case.advance(&split_request(0, 11), "split 11");
        assert_ne!(case.state, before.state, "the split moved something");
        case.advance(&merge(1, 11), "merge all 11 back");

        assert_eq!(case.state.realm, before.state.realm);
        assert_eq!(case.state.profile, before.state.profile);
        assert_eq!(case.state.market, before.state.market);
        assert_eq!(case.state.hoard, before.state.hoard);
        assert_eq!(case.state.position, before.state.position);
        assert_eq!(case.state.kernel, before.state.kernel);
        assert_eq!(case.state.external, before.state.external);
        assert_eq!(case.state.supply, before.state.supply);
        assert_ne!(
            case.state.replay, before.state.replay,
            "two transitions were consumed and the sequence must show it"
        );
        assert_eq!(
            ReplayAccount::decode(&case.state.replay)
                .expect("replay decodes")
                .sequence,
            2
        );
    }

    #[test]
    fn merge_refusals_agree_with_the_reference_adapter() {
        let case = funded();

        /* More than the market holds as collateral.  `MarketState::merge` tests
         * collateral before the per-outcome balances, so a single-position
         * market over-merging reports the collateral fault, which is the only
         * way that refusal is reachable at all. */
        case.refuses(
            &merge(1, 21),
            "merge beyond the hoard",
            Class::Kernel(KernelError::InsufficientCollateral),
        );

        /* More than *this position* holds, in a market a second position has
         * funded past the request.  This is the counterfeit direction, and it
         * reports the balance fault. */
        let ids = ids();
        let mut second = second_position(&funded());
        second.advance(
            &layout_request(
                0,
                Intent::Split {
                    market: ids.market,
                    owner: h(32),
                    quantity: 30,
                },
            ),
            "second owner splits 30",
        );
        assert_eq!(
            HoardAccount::decode(&second.state.hoard)
                .expect("hoard decodes")
                .collateral_atoms,
            50
        );
        second.refuses(
            &layout_request(
                1,
                Intent::Merge {
                    market: ids.market,
                    owner: h(32),
                    quantity: 31,
                },
            ),
            "merge against another position's claims",
            Class::Kernel(KernelError::InsufficientBalance),
        );

        // A closing position cannot recombine its way back into cash.
        let mut closed = funded();
        edit_position(&mut closed.state, |position| position.close_state = 1);
        closed.refuses(
            &merge(1, 1),
            "closing position merges",
            Class::MismatchedState,
        );

        // The intent names a market these accounts do not carry.
        case.refuses(
            &layout_request(
                1,
                Intent::Merge {
                    market: h(0x77),
                    owner: ids.owner,
                    quantity: 1,
                },
            ),
            "intent names another market",
            Class::MismatchedState,
        );

        // Wrong signer, and no signer at all.
        let mut stranger = funded();
        stranger.keys[IX_ACTOR] = key(0x9e);
        stranger.refuses(&merge(1, 1), "stranger merges", Class::Actor);

        let mut unsigned = funded();
        unsigned.signer = false;
        unsigned.refuses(&merge(1, 1), "unsigned merge", Class::Signature);

        // Aliased accounts, and a stale replay sequence.
        let mut aliased = funded();
        aliased.keys[IX_REPLAY] = aliased.keys[IX_EXTERNAL];
        aliased.bindings.replay = aliased.bindings.external;
        aliased.refuses(&merge(1, 1), "replay aliases the shadow", Class::Alias);

        case.refuses(&merge(9, 1), "stale merge sequence", Class::Replay);
    }

    #[test]
    fn a_counterfeit_merge_cannot_melt_claims_the_ledger_never_carried() {
        /* The mirror of `a_forged_position_cannot_split_against_an_empty_ledger`
         * and the reason a merge needs C2 at least as much as a split does: a
         * split mints against cash the position must actually hold, but a merge
         * *pays cash out*, so a position claiming internal balance the
         * market-wide ledger does not carry would be converting a counterfeit
         * into collateral.  CLO-DELTA-V1's representation bound refuses it
         * before any write. */
        let mut forged = fixture();
        edit_position(&mut forged.state, |position| {
            position.internal[0] = 5;
            position.internal[1] = 5;
        });
        forged.refuses(&merge(0, 5), "forged complete set", Class::Closure);

        let (result, post) = forged.program(&merge(0, 5));
        assert_eq!(result, Err(ClutchError::AggregateClosureMismatch.into()));
        assert_eq!(post, forged.state, "a refused merge writes nothing");
        let hoard = HoardAccount::decode(&post.hoard).expect("hoard decodes");
        assert_eq!(hoard.collateral_atoms, 0, "no collateral was released");
        let position = PositionAccount::decode(&post.position).expect("position decodes");
        assert_eq!(
            position.cash_atoms, 100,
            "no cash was credited for the counterfeit"
        );
    }
}
