mod common;

use clutch_kernel::PayoutVector;
use clutch_structured_claim::Error as CoreError;
use clutch_structured_claim_adapter::{
    plan_route, reconcile_post_state, reconcile_receipts, Action, CpiReceipt, CpiStepKind, Error,
    ExpectedPostState, RoutePlan, RouteScratch, MAX_CPI_STEPS,
};

use common::{FakePda, Fixture};

#[test]
fn caller_owned_sbf_scratch_stays_bounded() {
    let bytes = core::mem::size_of::<RouteScratch>();
    eprintln!("RouteScratch host bytes: {bytes}");
    assert!(bytes <= 8 * 1024);
}

#[test]
fn canonical_and_full_routes_reach_the_same_exact_vault() {
    let canonical_fixture = Fixture::new();
    let canonical = plan_route(
        &canonical_fixture.context(),
        &canonical_fixture.request,
        &FakePda,
    )
    .unwrap();
    assert_eq!(canonical.step_count, 2);
    assert_eq!(canonical.steps[0].kind, CpiStepKind::TransferIntoVault);
    assert_eq!(canonical.steps[0].cash_atoms, 2);
    assert_eq!(&canonical.steps[0].internal[..2], &[0, 2]);
    assert_eq!(canonical.steps[1].kind, CpiStepKind::TokenMintChecked);
    assert_eq!(canonical.post.vault_position.cash_atoms, 2);
    assert_eq!(&canonical.post.vault_position.internal[..2], &[0, 2]);
    assert_eq!(canonical.post.mint.supply, 2);
    assert_eq!(canonical.post.holder_token.unwrap().amount, 2);
    assert_eq!(canonical.post.holder_position.unwrap().cash_atoms, 8);
    assert_eq!(canonical.post.hoard_atoms, 100);
    assert_eq!(&canonical.post.total_supply[..2], &[100, 100]);
    assert_eq!(canonical.post.wrapper_replay.sequence, 4);
    assert_eq!(canonical.post.source_replay.unwrap().sequence, 6);
    assert_eq!(canonical.post.vault_replay.sequence, 8);

    let mut full_fixture = Fixture::new();
    full_fixture.request.action = Action::WrapFull;
    let full = plan_route(&full_fixture.context(), &full_fixture.request, &FakePda).unwrap();
    assert_eq!(full.step_count, 3);
    assert_eq!(full.steps[0].kind, CpiStepKind::TransferIntoVault);
    assert_eq!(&full.steps[0].internal[..2], &[2, 4]);
    assert_eq!(full.steps[1].kind, CpiStepKind::MergeCompleteSet);
    assert_eq!(full.steps[1].quantity, 2);
    assert_eq!(&full.steps[1].internal[..2], &[2, 2]);
    assert_eq!(full.steps[2].kind, CpiStepKind::TokenMintChecked);
    assert_eq!(full.post.vault_position, canonical.post.vault_position);
    assert_eq!(full.post.mint, canonical.post.mint);
    assert_eq!(full.post.holder_token, canonical.post.holder_token);
    assert_eq!(full.post.hoard_atoms, 98);
    assert_eq!(&full.post.total_supply[..2], &[98, 98]);
    assert_eq!(&full.post.supply.internal_supply[..2], &[98, 98]);
    assert_eq!(&full.post.holder_position.unwrap().internal[..2], &[8, 16]);
    assert_eq!(full.post.vault_replay.sequence, 9);
}

#[test]
fn reservations_generations_replays_deployments_and_aliases_fail_closed() {
    let mut fixture = Fixture::new();
    fixture.source.cash_atoms = 3;
    let error = plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err();
    assert_eq!(error, Error::StructuredClaim(CoreError::InsufficientCash));

    let mut fixture = Fixture::new();
    fixture.vault.cash_atoms = 1;
    fixture.vault.reserved_cash_atoms = 1;
    let error = plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err();
    assert_eq!(error, Error::InvalidPosition);

    let mut fixture = Fixture::new();
    fixture.request.source_generation += 1;
    assert_eq!(
        plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err(),
        Error::ReplayMismatch
    );

    let mut fixture = Fixture::new();
    fixture.request.wrapper_sequence += 1;
    assert_eq!(
        plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err(),
        Error::ReplayMismatch
    );

    let mut fixture = Fixture::new();
    fixture.deployments.binding.base_deployment_slot += 1;
    assert_eq!(
        plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err(),
        Error::DeploymentMismatch
    );

    let mut fixture = Fixture::new();
    fixture.accounts.accounts[16].key = fixture.accounts.accounts[4].key;
    assert_eq!(
        plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err(),
        Error::InvalidAccountSet
    );
}

#[test]
fn token_profile_and_supplyledger_closure_are_not_advisory() {
    let mut fixture = Fixture::new();
    fixture.mint.decimals = 6;
    assert_eq!(
        plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err(),
        Error::InvalidTokenProjection
    );

    let mut fixture = Fixture::new();
    fixture.token.delegate_present = true;
    assert_eq!(
        plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err(),
        Error::InvalidTokenProjection
    );

    let mut fixture = Fixture::new();
    fixture.supply.internal_supply[0] -= 1;
    assert_eq!(
        plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err(),
        Error::SupplyClosureMismatch
    );

    let mut fixture = Fixture::new();
    fixture.request.expected_mint_supply = 1;
    assert_eq!(
        plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err(),
        Error::TokenDeltaMismatch
    );
}

#[test]
fn donation_is_all_surplus_beneficiary_free_and_cap_gated() {
    let mut fixture = Fixture::new();
    fixture.vault.cash_atoms = 5;
    fixture.accounts = fixture.compact_accounts();
    fixture.request.action = Action::CompactDonation;
    fixture.request.quantity = 0;
    fixture.request.expected_holder_amount = 0;
    fixture.request.source_base_sequence = 0;
    fixture.request.source_generation = 0;
    let plan = plan_route(&fixture.compact_context(), &fixture.request, &FakePda).unwrap();
    assert_eq!(plan.step_count, 1);
    assert_eq!(plan.steps[0].kind, CpiStepKind::DonateCollateral);
    assert_eq!(plan.steps[0].cash_atoms, 5);
    assert_eq!(plan.post.hoard_atoms, 105);
    assert_eq!(plan.post.vault_position.cash_atoms, 0);
    assert_eq!(plan.post.holder_position, None);
    assert_eq!(plan.post.holder_token, None);

    let mut capped = Fixture::new();
    capped.vault.cash_atoms = 901;
    capped.accounts = capped.compact_accounts();
    capped.request.action = Action::CompactDonation;
    capped.request.quantity = 0;
    capped.request.expected_holder_amount = 0;
    capped.request.source_base_sequence = 0;
    capped.request.source_generation = 0;
    assert_eq!(
        plan_route(&capped.compact_context(), &capped.request, &FakePda).unwrap_err(),
        Error::CollateralCapExceeded
    );
}

#[test]
fn direct_burn_surplus_compacts_as_cash_and_vector_with_no_recipient() {
    let mut fixture = Fixture::new();
    let wrapped = plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap();
    fixture.adopt(&wrapped.post);
    // Ordinary Token-2022 burn bypasses the wrapper and releases nothing.
    fixture.mint.supply -= 1;
    fixture.token.amount -= 1;
    fixture.request.action = Action::CompactDonation;
    fixture.request.quantity = 0;
    fixture.request.expected_mint_supply = 1;
    fixture.request.expected_holder_amount = 0;
    fixture.request.source_base_sequence = 0;
    fixture.request.source_generation = 0;
    fixture.accounts = fixture.compact_accounts();
    let plan = plan_route(&fixture.compact_context(), &fixture.request, &FakePda).unwrap();
    assert_eq!(plan.step_count, 2);
    assert_eq!(plan.steps[0].kind, CpiStepKind::DonateCollateral);
    assert_eq!(plan.steps[0].cash_atoms, 1);
    assert_eq!(plan.steps[1].kind, CpiStepKind::DonateInternalVector);
    assert_eq!(&plan.steps[1].internal[..2], &[0, 1]);
    assert_eq!(plan.post.hoard_atoms, 101);
    assert_eq!(&plan.post.total_supply[..2], &[100, 99]);
    assert_eq!(&plan.post.supply.internal_supply[..2], &[100, 99]);
    assert_eq!(plan.post.vault_position.cash_atoms, 1);
    assert_eq!(&plan.post.vault_position.internal[..2], &[0, 1]);
    assert_eq!(plan.post.holder_position, None);
    assert_eq!(plan.post.holder_token, None);
}

#[test]
fn exact_vector_redemption_and_retirement_have_no_rounding_or_recreation_hole() {
    let mut fixture = Fixture::new();
    let wrapped = plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap();
    fixture.adopt(&wrapped.post);
    let mut weights = [0; 16];
    weights[0] = 4;
    weights[1] = 4;
    fixture
        .base
        .resolve_with_vector(PayoutVector::new(8, weights))
        .unwrap();
    fixture.market.lifecycle = 1;
    fixture.request.action = Action::RedeemVector;
    fixture.request.quantity = 1;
    assert_eq!(
        plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap_err(),
        Error::StructuredClaim(CoreError::InexactRedemption)
    );

    fixture.request.quantity = 2;
    let redeemed = plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap();
    assert_eq!(redeemed.step_count, 2);
    assert_eq!(redeemed.steps[0].kind, CpiStepKind::TokenBurnChecked);
    assert_eq!(redeemed.steps[1].kind, CpiStepKind::RedeemInternalVector);
    // Two cash-floor atoms leave the vault; the residual vector pays one more.
    assert_eq!(redeemed.steps[1].quantity, 2);
    assert_eq!(redeemed.steps[1].cash_atoms, 3);
    assert_eq!(&redeemed.steps[1].internal[..2], &[0, 2]);
    assert_eq!(redeemed.post.mint.supply, 0);
    assert_eq!(redeemed.post.vault_position.cash_atoms, 0);
    assert_eq!(&redeemed.post.vault_position.internal[..2], &[0, 0]);
    assert_eq!(redeemed.post.hoard_atoms, 99);
    assert_eq!(&redeemed.post.total_supply[..2], &[100, 98]);
    assert_eq!(redeemed.post.holder_position.unwrap().cash_atoms, 11);

    fixture.adopt(&redeemed.post);
    fixture.request.action = Action::Retire;
    fixture.request.quantity = 0;
    fixture.request.expected_holder_amount = 0;
    fixture.request.source_base_sequence = 0;
    fixture.request.source_generation = 0;
    fixture.accounts = fixture.compact_accounts();
    for account in &mut fixture.accounts.accounts[..usize::from(fixture.accounts.count)] {
        if account.role == clutch_structured_claim_adapter::AccountRole::Descriptor {
            account.writable = true;
        }
    }
    let retired = plan_route(&fixture.compact_context(), &fixture.request, &FakePda).unwrap();
    assert_eq!(retired.step_count, 0);
    assert_eq!(retired.post.descriptor_state, 1);
    assert_eq!(retired.post.mint.supply, 0);
    assert_eq!(retired.post.vault_position.cash_atoms, 0);
    assert_eq!(retired.post.vault_position.internal, [0; 16]);
}

#[test]
fn canonical_and_full_unwind_are_exact_inverses_of_their_entry_routes() {
    for action in [Action::WrapCanonical, Action::WrapFull] {
        let mut fixture = Fixture::new();
        let initial_source = fixture.source;
        let initial_vault = fixture.vault;
        let initial_supply = fixture.supply;
        let initial_hoard = fixture.hoard.collateral_atoms;
        let initial_total = fixture.base.total_supply;
        fixture.request.action = action;
        let wrapped = plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap();
        fixture.adopt(&wrapped.post);
        fixture.request.action = if action == Action::WrapCanonical {
            Action::UnwindCanonical
        } else {
            Action::UnwindFull
        };
        let unwound = plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap();
        assert_eq!(unwound.post.holder_position, Some(initial_source));
        assert_eq!(unwound.post.vault_position, initial_vault);
        assert_eq!(unwound.post.supply, initial_supply);
        assert_eq!(unwound.post.hoard_atoms, initial_hoard);
        assert_eq!(unwound.post.total_supply, initial_total);
        assert_eq!(unwound.post.mint.supply, 0);
        assert_eq!(unwound.post.holder_token.unwrap().amount, 0);
    }
}

#[test]
fn receipts_and_authoritative_post_accounts_are_both_required() {
    let fixture = Fixture::new();
    let plan = plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap();
    let mut receipts = [CpiReceipt::EMPTY; MAX_CPI_STEPS];
    for (i, receipt) in receipts
        .iter_mut()
        .enumerate()
        .take(usize::from(plan.step_count))
    {
        *receipt = CpiReceipt {
            executed: plan.steps[i],
            success: true,
        };
    }
    assert_eq!(
        reconcile_receipts(&plan, plan.step_count, &receipts),
        Ok(())
    );
    assert_eq!(reconcile_post_state(&plan, &plan.post), Ok(()));

    receipts[0].executed.cash_atoms += 1;
    assert_eq!(
        reconcile_receipts(&plan, plan.step_count, &receipts),
        Err(Error::CpiReceiptMismatch)
    );
    let mut lying_post = plan.post;
    lying_post.mint.supply += 1;
    assert_eq!(
        reconcile_post_state(&plan, &lying_post),
        Err(Error::PostStateMismatch)
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BankModel {
    state: ExpectedPostState,
}

impl BankModel {
    fn from_fixture(fixture: &Fixture) -> Self {
        Self {
            state: ExpectedPostState {
                descriptor_state: fixture.descriptor.state,
                hoard_atoms: fixture.hoard.collateral_atoms,
                total_supply: fixture.base.total_supply,
                holder_position: Some(fixture.source),
                vault_position: fixture.vault,
                supply: fixture.supply,
                mint: fixture.mint,
                holder_token: Some(fixture.token),
                wrapper_replay: fixture.wrapper_replay,
                source_replay: Some(fixture.source_replay),
                vault_replay: fixture.vault_replay,
            },
        }
    }

    fn execute(&mut self, plan: &RoutePlan, fail_at: Option<usize>) -> Result<(), ()> {
        let mut work = *self;
        work.state.wrapper_replay.sequence += 1;
        for i in 0..usize::from(plan.step_count) {
            if fail_at == Some(i) {
                return Err(());
            }
            work.apply(plan.steps[i])?;
        }
        work.state.descriptor_state = plan.post.descriptor_state;
        if reconcile_post_state(plan, &work.state).is_err() {
            return Err(());
        }
        *self = work;
        Ok(())
    }

    fn apply(&mut self, step: clutch_structured_claim_adapter::CpiStep) -> Result<(), ()> {
        match step.kind {
            CpiStepKind::TransferIntoVault => {
                let holder = self.state.holder_position.as_mut().ok_or(())?;
                holder.cash_atoms = holder.cash_atoms.checked_sub(step.cash_atoms).ok_or(())?;
                self.state.vault_position.cash_atoms += step.cash_atoms;
                for i in 0..16 {
                    holder.internal[i] =
                        holder.internal[i].checked_sub(step.internal[i]).ok_or(())?;
                    self.state.vault_position.internal[i] += step.internal[i];
                }
                self.state.source_replay.as_mut().ok_or(())?.sequence += 1;
                self.state.vault_replay.sequence += 1;
            }
            CpiStepKind::MergeCompleteSet => {
                self.state.hoard_atoms = self
                    .state
                    .hoard_atoms
                    .checked_sub(step.cash_atoms)
                    .ok_or(())?;
                self.state.vault_position.cash_atoms += step.cash_atoms;
                for i in 0..16 {
                    self.state.vault_position.internal[i] = self.state.vault_position.internal[i]
                        .checked_sub(step.internal[i])
                        .ok_or(())?;
                    self.state.supply.internal_supply[i] = self.state.supply.internal_supply[i]
                        .checked_sub(step.internal[i])
                        .ok_or(())?;
                    self.state.total_supply[i] = self.state.total_supply[i]
                        .checked_sub(step.internal[i])
                        .ok_or(())?;
                }
                self.state.vault_replay.sequence += 1;
            }
            CpiStepKind::TokenMintChecked => {
                self.state.mint.supply += step.quantity;
                self.state.holder_token.as_mut().ok_or(())?.amount += step.quantity;
            }
            _ => return Err(()),
        }
        Ok(())
    }
}

#[test]
fn bank_model_rolls_back_after_each_cpi_and_commits_only_exact_post_state() {
    let mut fixture = Fixture::new();
    fixture.request.action = Action::WrapFull;
    let plan = plan_route(&fixture.context(), &fixture.request, &FakePda).unwrap();
    assert_eq!(plan.step_count, MAX_CPI_STEPS as u8);
    for fail_at in 0..MAX_CPI_STEPS {
        let mut bank = BankModel::from_fixture(&fixture);
        let before = bank;
        assert_eq!(bank.execute(&plan, Some(fail_at)), Err(()));
        assert_eq!(bank, before, "failure after CPI {fail_at} leaked writes");
    }
    let mut bank = BankModel::from_fixture(&fixture);
    assert_eq!(bank.execute(&plan, None), Ok(()));
    assert_eq!(bank.state, plan.post);
}
