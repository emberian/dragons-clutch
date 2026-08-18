//! `Intent::Merge`, `Intent::Materialize`, `Intent::Dematerialize`.
//!
//! `Materialize` and `Dematerialize` are **implemented** here.  `Merge` is
//! **refused**, and the reason is the point of this module's documentation.
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
//! ## Why `Merge` refuses
//!
//! `clutch_kernel::MarketState::merge` exists and is exercised in the kernel's
//! own tests.  The offline reference adapter — this program's semantic oracle
//! for the seam — nevertheless has **no `Intent::Merge` arm** in `apply_inner`:
//! the intent falls through to `_ => Err(Error::UnsupportedIntent)`, and the
//! adapter's own test `unsupported_layout_intents_and_unsigned_owner_refuse`
//! pins that refusal.  (`docs/implementation/SBF_BRINGUP.md` line 135 says "the
//! offline adapter implements all three"; for `Merge` that sentence is wrong,
//! and correcting it is a documentation follow-up this lane does not own.)
//!
//! Implementing `Merge` on-chain would therefore do two things this project
//! does not do.  It would make the program **accept a transition its own oracle
//! refuses**, which is fail-open with respect to the only semantics anybody has
//! written down.  And it would require inventing the cash direction of a merge:
//! `Split` debits `PositionAccount::cash_atoms` against the collateral it mints,
//! and nothing anywhere credits it back, so a merge's cash effect is an
//! economic decision that belongs to the reference adapter, not to an account
//! plane.  So the refusal is mirrored instead, at the same point in the check
//! order the reference reaches it — after the kernel invariants, before any
//! write — and it is reported as
//! [`crate::error::ClutchError::UnsupportedInstruction`], the adapter-level
//! analogue of the reference's `Error::UnsupportedIntent`.
//!
//! **Follow-up for the coordinator:** `Merge` becomes implementable here the
//! moment `clutch_solana_reference::apply_inner` grows a `Merge` arm that names
//! the cash direction.  The differential test
//! `merge_is_refused_by_both_adapters` in this module fails the day that
//! happens, which is the intended alarm.
//!
//! ## An asymmetry observed in the oracle, mirrored rather than corrected
//!
//! `Split` refuses when `MarketAccount::lifecycle != 0` **or**
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
/// about them differs either.
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
        edit_position, fixture, h, ids, key, layout_request, split_request, Case, Class,
    };
    use crate::instructions::split::{IX_ACTOR, IX_EXTERNAL, IX_POSITION, IX_REPLAY};
    use clutch_kernel::Error as KernelError;
    use clutch_solana_layout::{Hash32, Intent, PositionAccount, SupplyLedgerAccount};
    use clutch_solana_reference::ExternalAccount;

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

    #[test]
    fn merge_is_refused_by_both_adapters() {
        /* This test is the alarm described in the module docs.  The offline
         * reference adapter has no `Intent::Merge` arm and refuses it
         * `UnsupportedIntent`; this program mirrors that rather than accepting
         * a transition its own oracle refuses.  The day `apply_inner` grows a
         * `Merge` arm, the reference stops refusing, this test fails, and the
         * seam lane implements the transition against a semantics that finally
         * exists. */
        let case = funded();
        case.refuses(&merge(1, 5), "merge a complete set", Class::Unsupported);
        case.refuses(&merge(1, 1), "merge one set", Class::Unsupported);

        let (result, post) = case.program(&merge(1, 5));
        assert_eq!(result, Err(ClutchError::UnsupportedInstruction.into()));
        assert_eq!(post, case.state, "a refused merge writes nothing");
    }
}
