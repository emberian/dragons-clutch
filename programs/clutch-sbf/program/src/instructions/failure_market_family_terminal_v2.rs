// SPDX-License-Identifier: AGPL-3.0-or-later
//! Atomic Failure-family aggregate, permanent replay, and history seal.
//!
//! Recovery close alone is not Product authority. This disabled composer joins
//! the hostile-reopened `RecoveryClosed` runtime and canonical Idle interval
//! pair, mints the exact pre-replay aggregate, writes fresh permanent
//! `0xa3/v2`, advances `0xa0/v3` to `FamilyTerminal`, and only then seals the
//! append-only `0xac/v2` history. Its private postwrite is the sole Failure
//! object a future Product shared-core wrapper may consume.

use crate::accounts::{require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::{
    authenticate_failure_market_root_v2, AuthenticatedFailureMarketRootV2,
};
use crate::instructions::failure_market_interval_v2::{
    authenticate_failure_market_interval_accounts_v2, write_failure_market_interval_family_seal_v2,
    AuthenticatedFailureMarketIntervalAccountsV2,
};
use crate::instructions::failure_market_recovery_terminal_v2::AuthenticatedFailureMarketRecoveryClosePostwriteV2;
use crate::instructions::failure_market_replay_v2::{
    authenticate_failure_market_replay_v2, write_failure_market_replay_terminal_v2,
    AuthenticatedFailureMarketReplayV2,
};
use crate::instructions::failure_market_runtime::{
    authenticate_failure_market_runtime_root_v1, write_failure_market_runtime_terminal_plan_v2,
    AuthenticatedFailureMarketRuntimeRootV1,
};
use clutch_failure_policy_runtime::market_interval_history_v2::{
    plan_seal_failure_market_interval_history_v2, AuthenticatedFailureMarketIntervalFamilySealV2,
    FailureMarketIntervalFamilySealFactsV2, FailureMarketIntervalFamilySealReceiptV2,
};
use clutch_failure_policy_runtime::market_replay_v2::{
    plan_terminalize_failure_market_replay_v2, AuthenticatedFailureMarketReplayTerminalV2,
    FailureMarketReplayTerminalFactsV2, FailureMarketReplayTerminalReceiptV2,
};
use clutch_failure_policy_runtime::market_runtime_v1::{
    admit_failure_market_family_aggregate_v2, plan_finalize_failure_market_family_v2,
    AuthenticatedFailureMarketFamilyAggregateV2, AuthenticatedFailureMarketFamilyTerminalV2,
    FailureMarketFamilyAggregateFactsV2, FailureMarketFamilyAggregateReceiptV2,
    FailureMarketFamilyTerminalDispositionV2, FailureMarketFamilyTerminalFactsV2,
    FailureMarketFamilyTerminalReceiptV2,
};
use clutch_product_series::ContentId;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const FAMILY_TERMINAL_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-family-terminal-postwrite/v2";
const FAMILY_TERMINAL_OWNER_RELEASE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-terminal-owner-release/v2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketFamilyAggregateAuthorityV2 {
    expected: FailureMarketFamilyAggregateFactsV2,
}

impl AuthenticatedFailureMarketFamilyAggregateV2 for FailureMarketFamilyAggregateAuthorityV2 {
    fn authenticate_failure_market_family_aggregate(
        &self,
        expected: FailureMarketFamilyAggregateFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketReplayTerminalAuthorityV2 {
    replay_before: clutch_failure_policy_runtime::market_replay_v2::FailureMarketReplayStateIdV2,
    replay_account: clutch_failure_policy_runtime::market_policy_v1::FailureMarketAccountIdV1,
    funding_receipt_id:
        clutch_failure_policy_runtime::market_replay_v2::FailureMarketReplayFundingReceiptIdV2,
    family_aggregate_receipt_id:
        clutch_failure_policy_runtime::market_runtime_v1::FailureMarketFamilyAggregateReceiptIdV2,
    runtime_terminal_state_commitment:
        clutch_failure_policy_runtime::market_runtime_v1::FailureMarketRuntimeStateCommitmentV1,
}

impl AuthenticatedFailureMarketReplayTerminalV2 for FailureMarketReplayTerminalAuthorityV2 {
    fn authenticate_failure_market_replay_terminal(
        &self,
        expected: FailureMarketReplayTerminalFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected.replay_before != self.replay_before
            || expected.replay_account != self.replay_account
            || expected.funding_receipt_id != self.funding_receipt_id
            || expected.family_aggregate_receipt_id != self.family_aggregate_receipt_id
            || expected.runtime_terminal_state_commitment != self.runtime_terminal_state_commitment
            || expected.replay_after.bytes() == [0; 32]
            || expected.replay_after == expected.replay_before
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketFamilyTerminalAuthorityV2 {
    expected: FailureMarketFamilyTerminalFactsV2,
}

impl AuthenticatedFailureMarketFamilyTerminalV2 for FailureMarketFamilyTerminalAuthorityV2 {
    fn authenticate_failure_market_family_terminal(
        &self,
        expected: FailureMarketFamilyTerminalFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketHistorySealAuthorityV2 {
    expected: FailureMarketIntervalFamilySealFactsV2,
}

impl AuthenticatedFailureMarketIntervalFamilySealV2 for FailureMarketHistorySealAuthorityV2 {
    fn authenticate_failure_market_interval_family_seal(
        &self,
        expected: FailureMarketIntervalFamilySealFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Durable final Failure-family postwrite, still private to Product's wrapper.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedFailureMarketFamilyTerminalPostwriteV2 {
    id: ContentId,
    owner_release_id: ContentId,
    admission: AuthenticatedFailureMarketRootV2,
    aggregate: FailureMarketFamilyAggregateReceiptV2,
    replay_terminal: FailureMarketReplayTerminalReceiptV2,
    family_terminal: FailureMarketFamilyTerminalReceiptV2,
    family_seal: FailureMarketIntervalFamilySealReceiptV2,
    replay: AuthenticatedFailureMarketReplayV2,
    runtime: AuthenticatedFailureMarketRuntimeRootV1,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
}

impl AuthenticatedFailureMarketFamilyTerminalPostwriteV2 {
    /// Complete physical/semantic terminal identity.
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    /// Fresh release of the complete authenticated Failure terminal owner.
    pub(crate) const fn owner_release_id(self) -> ContentId {
        self.owner_release_id
    }

    /// Exact immutable Failure admission root retained through Product consume.
    pub(crate) const fn admission(self) -> AuthenticatedFailureMarketRootV2 {
        self.admission
    }

    /// Full-width shared Market.
    pub(crate) const fn market_instance_id(
        self,
    ) -> clutch_product_series::MarketInstanceV2Id {
        self.family_terminal.facts().market_instance_id
    }

    /// Shared Failure/liveness generation.
    pub(crate) const fn generation(self) -> u64 {
        self.family_terminal.facts().generation
    }

    /// Permanent replay is the Product shared-core owner account.
    pub(crate) const fn owner_account_id(self) -> ContentId {
        ContentId::from_bytes(self.replay.account().to_bytes())
    }

    /// Exact immutable admission-root account.
    pub(crate) const fn admission_root_account(self) -> Pubkey {
        self.admission.account()
    }

    /// Exact immutable admission semantic state.
    pub(crate) const fn admission_state_id(
        self,
    ) -> clutch_failure_policy_runtime::market_policy_v1::FailureMarketAdmissionStateIdV1 {
        self.family_terminal.facts().admission_state_id
    }

    /// Exact mutable runtime-root account.
    pub(crate) const fn runtime_root_account(self) -> Pubkey {
        self.runtime.account()
    }

    /// Exact final runtime semantic commitment.
    pub(crate) const fn runtime_state_commitment(
        self,
    ) -> clutch_failure_policy_runtime::market_runtime_v1::FailureMarketRuntimeStateCommitmentV1 {
        self.runtime.state_commitment()
    }

    /// Exact permanent replay account.
    pub(crate) const fn replay_account(self) -> Pubkey {
        self.replay.account()
    }

    /// Complete terminal replay semantic state.
    pub(crate) const fn replay_state_id(
        self,
    ) -> clutch_failure_policy_runtime::market_replay_v2::FailureMarketReplayStateIdV2 {
        self.replay.state_id()
    }

    /// Owner/PDA/frame/body/balance authentication of terminal replay.
    pub(crate) const fn replay_authentication_id(self) -> ContentId {
        self.replay.authentication_id()
    }

    /// Exact sealed append-only history account.
    pub(crate) const fn history_account(self) -> Pubkey {
        self.interval.history_account()
    }

    /// Complete sealed history semantic state.
    pub(crate) const fn history_state_id(
        self,
    ) -> clutch_failure_policy_runtime::market_interval_history_v2::FailureMarketIntervalHistoryStateIdV2 {
        self.interval.history_state_id()
    }

    /// Owner/PDA/frame/body/balance authentication of sealed history.
    pub(crate) const fn history_authentication_id(self) -> ContentId {
        self.interval.history_authentication_id()
    }

    /// Sole append-only Market history root.
    pub(crate) const fn history_root(
        self,
    ) -> clutch_failure_policy_runtime::market_interval_history_v2::FailureMarketIntervalHistoryRootV2 {
        self.interval.history().history_root()
    }

    /// Intermediate pre-replay aggregate.
    pub(crate) const fn aggregate(self) -> FailureMarketFamilyAggregateReceiptV2 {
        self.aggregate
    }

    /// Exact permanent replay terminal receipt.
    pub(crate) const fn replay_terminal(self) -> FailureMarketReplayTerminalReceiptV2 {
        self.replay_terminal
    }

    /// Sole typed Failure-family terminal receipt for Product.
    pub(crate) const fn family_terminal(self) -> FailureMarketFamilyTerminalReceiptV2 {
        self.family_terminal
    }

    /// Exact append-only history seal.
    pub(crate) const fn family_seal(self) -> FailureMarketIntervalFamilySealReceiptV2 {
        self.family_seal
    }

    /// Hostile-reopened permanent replay postimage.
    pub(crate) const fn replay(self) -> AuthenticatedFailureMarketReplayV2 {
        self.replay
    }

    /// Hostile-reopened `FamilyTerminal` runtime postimage.
    pub(crate) const fn runtime(self) -> AuthenticatedFailureMarketRuntimeRootV1 {
        self.runtime
    }

    /// Canonical Idle cell and sealed full history postimage.
    pub(crate) const fn interval(self) -> AuthenticatedFailureMarketIntervalAccountsV2 {
        self.interval
    }
}

/// Produce one durable Failure terminal chain without a caller terminal DTO.
#[allow(clippy::too_many_arguments)]
pub(crate) fn terminalize_failure_market_family_v2<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    runtime_root_account: &AccountInfo<'a>,
    interval_cell_account: &AccountInfo<'a>,
    interval_history_account: &AccountInfo<'a>,
    replay_account: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    recovery_close: AuthenticatedFailureMarketRecoveryClosePostwriteV2,
    replay_before: AuthenticatedFailureMarketReplayV2,
) -> Outcome<AuthenticatedFailureMarketFamilyTerminalPostwriteV2> {
    require_distinct(&[
        admission_root_account.clone(),
        runtime_root_account.clone(),
        interval_cell_account.clone(),
        interval_history_account.clone(),
        replay_account.clone(),
    ])?;
    let live_admission =
        authenticate_failure_market_root_v2(program_id, admission_root_account, false)?;
    require(live_admission == admission, ClutchError::MismatchedState)?;
    let live_runtime = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root_account,
        live_admission,
        true,
    )?;
    require(
        live_runtime == recovery_close.runtime(),
        ClutchError::MismatchedState,
    )?;
    let retained_interval = recovery_close.interval();
    let live_interval = authenticate_failure_market_interval_accounts_v2(
        program_id,
        interval_cell_account,
        interval_history_account,
        live_admission,
        retained_interval.funding(),
        retained_interval.quote(),
        false,
        true,
    )?;
    require(
        live_interval == retained_interval,
        ClutchError::MismatchedState,
    )?;
    let live_replay = authenticate_failure_market_replay_v2(
        program_id,
        replay_account,
        live_admission,
        replay_before.funding(),
        true,
    )?;
    require(live_replay == replay_before, ClutchError::MismatchedState)?;

    let runtime = live_runtime.state();
    let history = live_interval.history();
    let policy = live_admission.state().binding().facts();
    let recovery_facts = recovery_close.close().facts();
    let expected_aggregate = FailureMarketFamilyAggregateFactsV2 {
        disposition: FailureMarketFamilyTerminalDispositionV2::Resolved,
        runtime_before: live_runtime.state_commitment(),
        admission_state_id: live_admission
            .state()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        failure_policy_binding_id: live_admission.state().binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        admission_root_account_id: live_admission
            .state()
            .root_funding()
            .facts()
            .root_account_id,
        runtime_root_account_id: runtime.runtime_account_id(),
        interval_work_account_id: history.work_account(),
        interval_history_account_id: history.history_account(),
        interval_cell_state_id: live_interval.cell_state_id(),
        interval_history_state_id: live_interval.history_state_id(),
        interval_history_root: history.history_root(),
        completed_session_count: history.completed_session_count(),
        completed_work_calls: history.completed_work_calls(),
        exact_reward_lamports: history.exact_reward_lamports(),
        recovery_close_receipt_id: recovery_close.close().id(),
        resolution_activation_receipt_id: recovery_facts.resolution_activation_receipt_id,
    };
    let aggregate = admit_failure_market_family_aggregate_v2(
        &FailureMarketFamilyAggregateAuthorityV2 {
            expected: expected_aggregate,
        },
        runtime,
        live_admission.state(),
        live_interval.funding(),
        live_interval.quote(),
        live_interval.cell(),
        history,
        recovery_close.close(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_authority = FailureMarketReplayTerminalAuthorityV2 {
        replay_before: live_replay.state_id(),
        replay_account: live_replay.funding().facts().replay_account,
        funding_receipt_id: live_replay.funding().id(),
        family_aggregate_receipt_id: aggregate.id(),
        runtime_terminal_state_commitment: live_runtime.state_commitment(),
    };
    let (replay_plan, replay_terminal) = plan_terminalize_failure_market_replay_v2(
        &replay_authority,
        live_replay.replay(),
        live_admission.state(),
        live_replay.funding(),
        aggregate,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_family_terminal = FailureMarketFamilyTerminalFactsV2 {
        disposition: FailureMarketFamilyTerminalDispositionV2::Resolved,
        family_aggregate_receipt_id: aggregate.id(),
        failure_replay_account_id: live_replay.funding().facts().replay_account,
        failure_replay_terminal_receipt_id: replay_terminal.id(),
        runtime_before: live_runtime.state_commitment(),
        admission_state_id: expected_aggregate.admission_state_id,
        failure_policy_binding_id: expected_aggregate.failure_policy_binding_id,
        market_instance_id: expected_aggregate.market_instance_id,
        generation: expected_aggregate.generation,
        interval_history_state_id: live_interval.history_state_id(),
        interval_history_root: history.history_root(),
        completed_session_count: history.completed_session_count(),
    };
    let (runtime_plan, family_terminal) = plan_finalize_failure_market_family_v2(
        &FailureMarketFamilyTerminalAuthorityV2 {
            expected: expected_family_terminal,
        },
        runtime,
        live_admission.state(),
        live_interval.funding(),
        live_interval.quote(),
        live_interval.cell(),
        history,
        aggregate,
        replay_terminal,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_seal = FailureMarketIntervalFamilySealFactsV2 {
        history_before: live_interval.history_state_id(),
        history_root: history.history_root(),
        completed_session_count: history.completed_session_count(),
        completed_work_calls: history.completed_work_calls(),
        exact_reward_lamports: history.exact_reward_lamports(),
        family_terminal_receipt_id: family_terminal.id(),
    };
    let (history_plan, family_seal) = plan_seal_failure_market_interval_history_v2(
        &FailureMarketHistorySealAuthorityV2 {
            expected: expected_seal,
        },
        history,
        live_admission.state(),
        live_interval.quote(),
        family_terminal.id(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    // Every semantic postimage is fixed before the first write. Any later
    // refusal rolls replay, runtime, and history back together.
    let replay_after = write_failure_market_replay_terminal_v2(
        program_id,
        replay_account,
        live_admission,
        live_replay,
        replay_plan,
        replay_terminal,
    )?;
    let runtime_after = write_failure_market_runtime_terminal_plan_v2(
        program_id,
        admission_root_account,
        runtime_root_account,
        live_admission,
        live_runtime,
        runtime_plan,
    )?;
    let interval_after = write_failure_market_interval_family_seal_v2(
        program_id,
        interval_cell_account,
        interval_history_account,
        live_interval,
        history_plan,
        family_seal,
    )?;
    require(
        replay_after.replay().family_aggregate_receipt_id() == aggregate.id()
            && runtime_after.state().family_terminal_receipt_id().bytes()
                == family_terminal.id().bytes()
            && interval_after.history().family_terminal_receipt_id() == family_terminal.id()
            && interval_after.cell_state_id() == live_interval.cell_state_id(),
        ClutchError::MismatchedState,
    )?;
    let owner_release_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAMILY_TERMINAL_OWNER_RELEASE_DOMAIN_V2,
            &policy.market_instance_id.bytes(),
            &policy.generation.to_le_bytes(),
            &live_admission.account().to_bytes(),
            &expected_aggregate.admission_state_id.bytes(),
            &expected_aggregate.failure_policy_binding_id.bytes(),
            &runtime_after.account().to_bytes(),
            &runtime_after.state_commitment().bytes(),
            &replay_after.account().to_bytes(),
            &replay_after.state_id().bytes(),
            &replay_after.authentication_id().bytes(),
            &replay_terminal.id().bytes(),
            &interval_after.history_account().to_bytes(),
            &interval_after.history_state_id().bytes(),
            &interval_after.history_authentication_id().bytes(),
            &interval_after.history().history_root().bytes(),
            &family_seal.id().bytes(),
            &family_terminal.id().bytes(),
        ])
        .to_bytes(),
    );
    let owner_account_id = ContentId::from_bytes(replay_after.account().to_bytes());
    let owner_terminal_receipt_id = ContentId::from_bytes(family_terminal.id().bytes());
    require(
        !owner_release_id.is_zero()
            && owner_release_id != owner_account_id
            && owner_release_id != owner_terminal_receipt_id
            && owner_account_id != owner_terminal_receipt_id,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAMILY_TERMINAL_POSTWRITE_DOMAIN_V2,
            &owner_release_id.bytes(),
            &aggregate.id().bytes(),
            &replay_terminal.id().bytes(),
            &family_terminal.id().bytes(),
            &family_seal.id().bytes(),
            &replay_after.authentication_id().bytes(),
            &runtime_after.state_commitment().bytes(),
            &interval_after.history_authentication_id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedFailureMarketFamilyTerminalPostwriteV2 {
        id,
        owner_release_id,
        admission: live_admission,
        aggregate,
        replay_terminal,
        family_terminal,
        family_seal,
        replay: replay_after,
        runtime: runtime_after,
        interval: interval_after,
    })
}

#[cfg(test)]
mod adversarial_family_terminal_tests {
    #[test]
    fn terminal_chain_plans_every_poststate_before_replay_write() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let outer = source
            .split("pub(crate) fn terminalize_failure_market_family_v2")
            .nth(1)
            .expect("single family terminal composer");
        let aggregate = outer
            .find("admit_failure_market_family_aggregate_v2")
            .expect("aggregate");
        let replay_plan = outer
            .find("plan_terminalize_failure_market_replay_v2")
            .expect("replay plan");
        let runtime_plan = outer
            .find("plan_finalize_failure_market_family_v2")
            .expect("runtime plan");
        let seal_plan = outer
            .find("plan_seal_failure_market_interval_history_v2")
            .expect("history seal plan");
        let first_write = outer
            .find("write_failure_market_replay_terminal_v2")
            .expect("first write");
        assert!(aggregate < replay_plan && replay_plan < runtime_plan);
        assert!(runtime_plan < seal_plan && seal_plan < first_write);
    }

    #[test]
    fn terminal_chain_rejects_account_aliases_and_legacy_replay() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert!(!production.contains("failure_replay_tombstone_pda"));
        assert!(!production.contains("FailureReplayV1"));
        let outer = production
            .split("pub(crate) fn terminalize_failure_market_family_v2")
            .nth(1)
            .expect("single family terminal composer");
        let aliases = outer
            .split("require_distinct(&[")
            .nth(1)
            .and_then(|value| value.split("])?;").next())
            .expect("explicit aliases");
        for role in [
            "admission_root_account",
            "runtime_root_account",
            "interval_cell_account",
            "interval_history_account",
            "replay_account",
        ] {
            assert!(aliases.contains(role), "missing role {role}");
        }
    }

    #[test]
    fn terminal_chain_reauthenticates_every_cached_mutable_prestate() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let outer = source
            .split("pub(crate) fn terminalize_failure_market_family_v2")
            .nth(1)
            .expect("single family terminal composer");
        let first_write = outer
            .find("write_failure_market_replay_terminal_v2")
            .expect("first write");
        let prewrite = &outer[..first_write];
        for guard in [
            "authenticate_failure_market_root_v2",
            "live_admission == admission",
            "authenticate_failure_market_runtime_root_v1",
            "live_runtime == recovery_close.runtime()",
            "authenticate_failure_market_interval_accounts_v2",
            "live_interval == retained_interval",
            "authenticate_failure_market_replay_v2",
            "live_replay == replay_before",
        ] {
            assert!(prewrite.contains(guard), "missing stale-prestate guard {guard}");
        }
    }

    #[test]
    fn owner_release_commits_the_complete_terminal_owner_tuple() {
        let source = include_str!("failure_market_family_terminal_v2.rs");
        let release = source
            .split("let owner_release_id =")
            .nth(1)
            .and_then(|value| value.split("let owner_account_id").next())
            .expect("owner release derivation");
        for committed in [
            "policy.market_instance_id",
            "policy.generation",
            "live_admission.account()",
            "expected_aggregate.admission_state_id",
            "expected_aggregate.failure_policy_binding_id",
            "runtime_after.account()",
            "runtime_after.state_commitment()",
            "replay_after.account()",
            "replay_after.state_id()",
            "replay_after.authentication_id()",
            "replay_terminal.id()",
            "interval_after.history_account()",
            "interval_after.history_state_id()",
            "interval_after.history_authentication_id()",
            "interval_after.history().history_root()",
            "family_seal.id()",
            "family_terminal.id()",
        ] {
            assert!(release.contains(committed), "missing owner fact {committed}");
        }
        assert!(!release.contains("FailureReplayV1"));
        assert!(!release.contains("ExternalV2"));
    }
}
