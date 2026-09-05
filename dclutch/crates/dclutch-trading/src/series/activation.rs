//! Stateless capability-root activation for recurring Series V3.
//!
//! Activation is the one Core-signed action that CREATES the Series composite
//! root every Series action executes against, and it is the exact inverse of
//! [`crate::series::terminal::plan_series_root_closure_v3`]. This module is the sole
//! semantic owner of both halves of that inverse:
//!
//! 1. **The creation oracle.** The initial root tail an activation must write is
//!    not a literal anywhere. It is [`crate::series::replay::SeriesStateV3::new`] under
//!    the Template's own close principal and occurrence count, encoded by the
//!    same encoder every later action re-encodes through. A release whose
//!    activation artifact composes anything else creates a root that
//!    `SeriesStateV3::decode` refuses forever, on an account nothing can rewrite.
//!
//! 2. **The funding statement.** A Template's `close_rent` is separately prepaid
//!    principal, not a fee and not Rent. So the founding must park exactly the
//!    composite root's Rent reserve PLUS that authenticated principal, the root
//!    persists the principal unchanged, and terminal Close returns it to the
//!    Template's own refund owner. `plan_series_root_closure_v3` already reads
//!    `close_rent_remaining` back out and classifies it separately from
//!    `root_rent` and from unsolicited donation; this is the statement that puts
//!    it there, and [`series_activation_conserves_close_principal_v3`] is the
//!    proof the two agree for every Template, reserve, and donation.
//!
//! Nothing here accesses an account, derives a PDA, or moves a lamport. The
//! physical transfer is performed by the family-neutral activation effect, which
//! moves the whole parked quote into the vacant root; this module states what
//! that quote must be and refuses every other amount.

use crate::series::{
    TemplateV3,
    replay::{SERIES_STATE_BYTES_V3, SeriesStateV3},
};

/// Stable refusal from Series root activation planning.
///
/// Each variant names exactly one seam. `Funding` is reserved for a parked
/// founding quote that is not the exact sum, so an underfunded and an
/// overdeclared activation refuse at the same, separately assertable, conjunct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesActivationErrorV3 {
    /// The Template's own occurrence count refused its canonical initial state.
    Template,
    /// The exact composite-root Rent reserve was zero.
    Balance,
    /// Checked fixed-width arithmetic overflowed.
    Arithmetic,
    /// The parked founding quote was not exactly root Rent plus close principal.
    Funding,
}

/// Exact typed credit one activation must move into the vacant Series root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRootActivationPlanV3 {
    root_rent: u64,
    close_rent: u64,
    root_tail: [u8; SERIES_STATE_BYTES_V3],
}

impl SeriesRootActivationPlanV3 {
    /// Exact composite-root Rent reserve.
    pub const fn root_rent(self) -> u64 {
        self.root_rent
    }

    /// Separately prepaid Template close principal, persisted unspent.
    pub const fn close_rent(self) -> u64 {
        self.close_rent
    }

    /// Exact lamports the founding must park for this activation to be honest.
    ///
    /// The activation effect moves the whole parked quote and leaves the ledger
    /// row at zero, so this is both the debit and the root's opening balance.
    pub fn parked_quote(self) -> Result<u64, SeriesActivationErrorV3> {
        self.root_rent
            .checked_add(self.close_rent)
            .ok_or(SeriesActivationErrorV3::Arithmetic)
    }

    /// Canonical initial root tail this activation must compose, byte for byte.
    pub const fn root_tail(&self) -> &[u8; SERIES_STATE_BYTES_V3] {
        &self.root_tail
    }
}

/// Return the canonical initial Series root tail for one Template.
///
/// This is the family's creation oracle. An activation artifact declares its
/// constant tail by CALLING this and never by transcribing bytes, so a change to
/// [`SeriesStateV3`] moves the artifact with it or refuses here.
pub fn series_activation_root_tail_v3(
    template: TemplateV3,
) -> Result<[u8; SERIES_STATE_BYTES_V3], SeriesActivationErrorV3> {
    SeriesStateV3::new(template.close_rent())
        .encode(template.occurrence_count())
        .map_err(|_| SeriesActivationErrorV3::Template)
}

/// Plan the exact credit and initial tail for one Series root activation.
///
/// `exact_root_rent` is the observed Rent-exempt minimum for
/// `CapabilityRootHeaderV1 || SeriesStateV3`, which only the adapter can read
/// from the Rent sysvar. A zero reserve is refused rather than defaulted: a root
/// that is not Rent-exempt is a root the runtime may reap with the prepaid close
/// principal still inside it.
pub fn plan_series_root_activation_v3(
    template: TemplateV3,
    exact_root_rent: u64,
) -> Result<SeriesRootActivationPlanV3, SeriesActivationErrorV3> {
    if exact_root_rent == 0 {
        return Err(SeriesActivationErrorV3::Balance);
    }
    let root_tail = series_activation_root_tail_v3(template)?;
    let plan = SeriesRootActivationPlanV3 {
        root_rent: exact_root_rent,
        close_rent: template.close_rent(),
        root_tail,
    };
    // Refuse a Template whose principal cannot be parked beside its own reserve
    // here, rather than at the transfer, where the root would already exist.
    plan.parked_quote()?;
    Ok(plan)
}

/// Require an observed parked founding quote to be exactly the planned sum.
///
/// This is the conjunct that owns underfunding and overdeclaration. It is
/// deliberately an equality and not a lower bound: a quote above the sum is an
/// overdeclared principal the root would persist as Rent it does not owe, and a
/// quote below it is an activation that either bricks the root or silently
/// spends principal the Template promised to a beneficiary.
pub fn require_parked_activation_quote_v3(
    plan: SeriesRootActivationPlanV3,
    observed_parked_quote: u64,
) -> Result<(), SeriesActivationErrorV3> {
    if observed_parked_quote == plan.parked_quote()? {
        Ok(())
    } else {
        Err(SeriesActivationErrorV3::Funding)
    }
}

/// Prove that one activation's credit and one terminal closure's refund are the
/// same lamports, split the same way, for one Template and reserve.
///
/// Returns the exact terminal credit for an observed root balance of the
/// activation quote plus `donation`. It refuses unless the closure classifies
/// exactly the principal this activation persisted and exactly the donation the
/// caller named, which is the whole of the option-B economic statement:
/// principal in, principal out, donation neither created nor consumed.
pub fn series_activation_conserves_close_principal_v3(
    template: TemplateV3,
    exact_root_rent: u64,
    donation: u64,
) -> Result<u64, SeriesActivationErrorV3> {
    let plan = plan_series_root_activation_v3(template, exact_root_rent)?;
    plan.parked_quote()?
        .checked_add(donation)
        .ok_or(SeriesActivationErrorV3::Arithmetic)
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::ContentId;
    use dclutch_market::rent::{
        RefundAuthority,
        lifecycle_v2::{LifecycleAccountIdV2, LifecycleRentCreditV2},
    };

    use super::*;
    use crate::series::{
        AccountKeyV3, generated,
        replay::SeriesPhaseV3,
        terminal::{SeriesLifecycleRentSinkV3, SeriesTerminalErrorV3, plan_series_root_closure_v3},
    };

    const RESERVE: u64 = 1_002_240;

    fn key(byte: u8) -> AccountKeyV3 {
        AccountKeyV3::new([byte; 32]).expect("nonzero")
    }

    fn wallet() -> AccountKeyV3 {
        key(61)
    }

    /// The Lean-owned example Template with exactly its close principal and
    /// refund owner replaced. Every other coordinate stays canonical, so a
    /// refusal here is never a hand-built record's fault.
    fn template(close_rent: u64) -> TemplateV3 {
        let mut bytes = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        put(
            &mut bytes,
            generated::SERIES_TEMPLATE_CLOSE_RENT_OFFSET_V3,
            &close_rent.to_le_bytes(),
        );
        put(
            &mut bytes,
            generated::SERIES_TEMPLATE_REFUND_OWNER_OFFSET_V3,
            &wallet().to_bytes(),
        );
        TemplateV3::decode(&bytes).expect("Template")
    }

    fn put(output: &mut [u8], offset: usize, source: &[u8]) {
        let end = offset.checked_add(source.len()).expect("offset");
        output
            .get_mut(offset..end)
            .expect("region")
            .copy_from_slice(source);
    }

    fn sink() -> SeriesLifecycleRentSinkV3 {
        let credit = LifecycleRentCreditV2::new(
            RefundAuthority::new(wallet().to_bytes()).expect("wallet"),
            LifecycleAccountIdV2::new([31; 32]).expect("Market"),
            LifecycleAccountIdV2::new([32; 32]).expect("release"),
            7,
            9,
        )
        .expect("credit");
        SeriesLifecycleRentSinkV3::admit(
            key(30),
            &credit.to_bytes(),
            key(31),
            ContentId::new([32; 32]).expect("release set"),
            7,
            wallet(),
        )
        .expect("sink")
    }

    /// Drive the real replay evaluator from the activation tail all the way to a
    /// closable terminal root, one occurrence at a time.
    fn terminal_state(template: TemplateV3) -> SeriesStateV3 {
        let tail = series_activation_root_tail_v3(template).expect("tail");
        let mut state =
            SeriesStateV3::decode(&tail, template.occurrence_count()).expect("decode tail");
        for _ in 0..template.occurrence_count() {
            state = state
                .prepare_ticket(state.revision())
                .expect("prepare")
                .settle_current(
                    state.revision().wrapping_add(1),
                    template.occurrence_count(),
                )
                .expect("settle");
            state = state.retire_ticket(state.revision()).expect("retire");
        }
        assert_eq!(state.phase(), SeriesPhaseV3::Terminal);
        state
    }

    #[test]
    fn the_creation_oracle_is_the_state_encoder_and_never_a_literal() {
        for close_rent in [0_u64, 1, 7, u64::from(u32::MAX)] {
            let template = template(close_rent);
            let tail = series_activation_root_tail_v3(template).expect("tail");
            let decoded =
                SeriesStateV3::decode(&tail, template.occurrence_count()).expect("decode tail");
            assert_eq!(decoded, SeriesStateV3::new(close_rent));
            assert_eq!(decoded.close_rent_remaining(), close_rent);
            assert_eq!(decoded.phase(), SeriesPhaseV3::Active);
            assert_eq!(decoded.revision(), 0);
            assert_eq!(decoded.outstanding_ticket_accounts(), 0);
        }
    }

    #[test]
    fn a_zero_root_reserve_refuses_at_balance_and_never_at_funding() {
        assert_eq!(
            plan_series_root_activation_v3(template(7), 0),
            Err(SeriesActivationErrorV3::Balance)
        );
    }

    #[test]
    fn an_overflowing_template_principal_refuses_at_arithmetic() {
        assert_eq!(
            plan_series_root_activation_v3(template(u64::MAX), 1),
            Err(SeriesActivationErrorV3::Arithmetic)
        );
    }

    #[test]
    fn underfunding_and_overdeclaration_both_refuse_at_funding() {
        for close_rent in [0_u64, 5_000] {
            let plan = plan_series_root_activation_v3(template(close_rent), RESERVE).expect("plan");
            let exact = plan.parked_quote().expect("quote");
            assert_eq!(exact, RESERVE.checked_add(close_rent).expect("sum"));
            assert_eq!(plan.close_rent(), close_rent);
            assert_eq!(require_parked_activation_quote_v3(plan, exact), Ok(()));
            assert_eq!(
                require_parked_activation_quote_v3(plan, exact.wrapping_sub(1)),
                Err(SeriesActivationErrorV3::Funding)
            );
            assert_eq!(
                require_parked_activation_quote_v3(plan, exact.wrapping_add(1)),
                Err(SeriesActivationErrorV3::Funding)
            );
            assert_eq!(
                require_parked_activation_quote_v3(plan, RESERVE),
                if close_rent == 0 {
                    Ok(())
                } else {
                    Err(SeriesActivationErrorV3::Funding)
                }
            );
        }
    }

    #[test]
    fn activation_credit_and_terminal_closure_are_the_same_lamports() {
        for close_rent in [0_u64, 1, 5_000, 900_000] {
            for donation in [0_u64, 1, 12_345] {
                let template = template(close_rent);
                let plan = plan_series_root_activation_v3(template, RESERVE).expect("plan");
                let observed =
                    series_activation_conserves_close_principal_v3(template, RESERVE, donation)
                        .expect("observed");
                assert_eq!(
                    observed,
                    plan.parked_quote()
                        .expect("quote")
                        .checked_add(donation)
                        .expect("observed sum")
                );
                let state = terminal_state(template);
                let closure = plan_series_root_closure_v3(
                    template,
                    state,
                    state.revision(),
                    observed,
                    RESERVE,
                    sink(),
                )
                .expect("closure");
                // The three components the activation named, returned unmixed.
                assert_eq!(closure.close_rent(), close_rent);
                assert_eq!(closure.root_rent(), RESERVE);
                assert_eq!(closure.donation(), donation);
                assert_eq!(closure.total_credit(), Ok(observed));
            }
        }
    }

    #[test]
    fn a_root_underfunded_by_one_lamport_cannot_close() {
        let template = template(5_000);
        let plan = plan_series_root_activation_v3(template, RESERVE).expect("plan");
        let state = terminal_state(template);
        let short = plan.parked_quote().expect("quote").wrapping_sub(1);
        assert_eq!(
            plan_series_root_closure_v3(template, state, state.revision(), short, RESERVE, sink()),
            Err(SeriesTerminalErrorV3::Balance)
        );
    }

    #[test]
    fn the_refund_owner_is_the_only_beneficiary_the_closure_will_accept() {
        let template = template(5_000);
        let observed =
            series_activation_conserves_close_principal_v3(template, RESERVE, 0).expect("observed");
        let state = terminal_state(template);
        let redirected = {
            let other = key(62);
            let credit = LifecycleRentCreditV2::new(
                RefundAuthority::new(other.to_bytes()).expect("wallet"),
                LifecycleAccountIdV2::new([31; 32]).expect("Market"),
                LifecycleAccountIdV2::new([32; 32]).expect("release"),
                7,
                9,
            )
            .expect("credit");
            SeriesLifecycleRentSinkV3::admit(
                key(30),
                &credit.to_bytes(),
                key(31),
                ContentId::new([32; 32]).expect("release set"),
                7,
                other,
            )
            .expect("sink")
        };
        assert_eq!(
            plan_series_root_closure_v3(
                template,
                state,
                state.revision(),
                observed,
                RESERVE,
                redirected,
            ),
            Err(SeriesTerminalErrorV3::RentBinding)
        );
    }
}
