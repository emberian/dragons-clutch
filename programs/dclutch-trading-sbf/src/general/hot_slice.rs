//! The General arm of the common Trading hot outer.
//!
//! [`hot_controller::process_general_action_v2`] consumes two already-decoded
//! semantic values -- the immutable [`GeneralConfigV2`] and the mutable
//! [`GeneralRootV2`] tail. Something has to turn account bytes into those two
//! values and bind both to the authenticated selection, and that something is
//! this module: the family owns the schemas, so the family owns the decode.
//!
//! The common outer arrives here holding exactly what it already has after
//! [`TradingFamilyContextV1::authenticate`]: the authenticated context, the
//! composite root account behind it, the selected config bytes, and the exact
//! General suffix. One call replaces the four separate obligations a hot
//! dispatcher would otherwise have to remember.
//!
//! # Why the tail width is General's constant and not the descriptor's
//!
//! [`split_root_account_mut_v1`](dclutch_capability_program_contract::split_root_account_mut_v1)
//! learns the tail width by asking the descriptor for `root_state_bytes`. The
//! common layer has already required that the descriptor's declared root
//! account width equal the observed one, which is what
//! [`TradingFamilyContextV1::root_account_bytes`] carries. General is the
//! semantic owner of `GENERAL_ROOT_SCHEMA_ID_V2` and therefore of its width,
//! so requiring `CAPABILITY_ROOT_HEADER_BYTES_V1 + GENERAL_ROOT_BYTES_V2` here
//! is strictly stronger than repeating the descriptor's own claim: it also
//! refuses a descriptor that named some other schema's tail width for a
//! General capability.
//!
//! # Why the config bytes are bound here
//!
//! `authenticate_common` compares the *root tail's* recorded config identity
//! against the authenticated selection. That says the capability was activated
//! against the selected config; it says nothing about the config value the
//! caller then handed the transition. Binding `hash(config_bytes)` to
//! `selection().config()` is the conjunct that makes the decoded
//! [`GeneralConfigV2`] the selected one rather than a supplied one.

use dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_general_config_contract::{GENERAL_ROOT_BYTES_V2, GeneralConfigV2, GeneralRootV2};
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey,
};

use crate::{
    TradingSbfError, dispatch::TradingFamilyContextV1,
    general::hot_controller::process_general_action_v2,
};

/// Exact composite General root-account width under the common root domain.
pub const GENERAL_COMPOSITE_ROOT_BYTES_V2: usize =
    CAPABILITY_ROOT_HEADER_BYTES_V1 + GENERAL_ROOT_BYTES_V2;

/// Execute one authenticated General hot action from account bytes.
///
/// `root_account` is the composite `CapabilityRootHeaderV1(232) ||
/// GeneralRootV2` account the supplied `context` was authenticated from.
/// `config_bytes` are the selected immutable config bytes. `accounts` is the
/// exact General suffix documented in this family's module header, and
/// `instruction_data` its exact `ControllerRequestV1`.
#[inline(never)]
pub fn process_general_hot_slice_v2(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    root_account: &AccountInfo<'_>,
    config_bytes: &[u8],
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let root_state = split_authenticated_root_state_v2(program_id, context, root_account)?;
    let config = decode_selected_config_v2(context, config_bytes)?;
    process_general_action_v2(
        program_id,
        context,
        accounts,
        instruction_data,
        config,
        root_state,
    )
}

/// Authenticate the composite root account and decode its mutable tail.
fn split_authenticated_root_state_v2(
    program_id: &Pubkey,
    context: TradingFamilyContextV1,
    root_account: &AccountInfo<'_>,
) -> Result<GeneralRootV2, ProgramError> {
    if root_account.key.to_bytes() != context.child_root_key()
        || root_account.owner != program_id
        || root_account.is_signer
        || !root_account.is_writable
        || root_account.executable
        || context.root_account_bytes() != GENERAL_COMPOSITE_ROOT_BYTES_V2
    {
        return Err(TradingSbfError::Root.into());
    }
    let bytes = root_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Root)?;
    if bytes.len() != GENERAL_COMPOSITE_ROOT_BYTES_V2 {
        return Err(TradingSbfError::Root.into());
    }
    let tail = bytes
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(TradingSbfError::Root)?;
    GeneralRootV2::decode(tail).map_err(|_| TradingSbfError::Root.into())
}

/// Decode the config the authenticated selection names, never a supplied one.
fn decode_selected_config_v2(
    context: TradingFamilyContextV1,
    config_bytes: &[u8],
) -> Result<GeneralConfigV2, ProgramError> {
    if hash(config_bytes).to_bytes() != context.selection().config().to_bytes() {
        return Err(TradingSbfError::Content.into());
    }
    GeneralConfigV2::decode(config_bytes).map_err(|_| TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{vec, vec::Vec};

    use dclutch_general_config_contract::GeneralLifecycleV2;
    use solana_program::pubkey::Pubkey;

    use super::*;
    use crate::general::hot_controller::{
        CERTIFICATE, SELECTION, VERIFICATION,
        tests::{
            CompositeRootV2, account, at, borrowed, composite_root_v2, config, consider_frame,
            consider_request,
        },
    };

    struct SliceFixture {
        program_id: Pubkey,
        root: CompositeRootV2,
        frame: Vec<AccountInfo<'static>>,
        request: Vec<u8>,
        config_bytes: Vec<u8>,
    }

    fn fixture() -> SliceFixture {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let config = config();
        SliceFixture {
            program_id,
            root: composite_root_v2(program_id, market, config),
            frame: consider_frame(program_id, market),
            request: consider_request()
                .to_bytes()
                .expect("request bytes")
                .to_vec(),
            config_bytes: config.to_bytes().to_vec(),
        }
    }

    impl SliceFixture {
        fn root_account(&self, bytes: Vec<u8>) -> AccountInfo<'static> {
            account(self.root.root_key, true, bytes, self.program_id, false)
        }

        fn execute(&self, root_account: &AccountInfo<'_>) -> Result<(), ProgramError> {
            self.execute_with_config(root_account, &self.config_bytes)
        }

        fn execute_with_config(
            &self,
            root_account: &AccountInfo<'_>,
            config_bytes: &[u8],
        ) -> Result<(), ProgramError> {
            process_general_hot_slice_v2(
                &self.program_id,
                self.root.context,
                root_account,
                config_bytes,
                &self.frame,
                &self.request,
            )
        }

        fn general_state(&self) -> Vec<Vec<u8>> {
            [SELECTION, VERIFICATION, CERTIFICATE]
                .iter()
                .map(|coordinate| borrowed(at(&self.frame, *coordinate)))
                .collect()
        }
    }

    #[test]
    fn the_slice_decodes_both_semantics_from_accounts_and_executes() {
        let fixture = fixture();
        let root_account = fixture.root_account(fixture.root.account_bytes.clone());
        fixture.execute(&root_account).expect("General hot slice");
        // The transition ran: the family committed its selection, and the
        // composite root is untouched, because this action mutates no root.
        assert_ne!(
            borrowed(at(&fixture.frame, SELECTION)),
            vec![0; borrowed(at(&fixture.frame, SELECTION)).len()]
        );
        assert_eq!(borrowed(&root_account), fixture.root.account_bytes);
    }

    /// A capability that has left Active is a zombie, not an authority.
    ///
    /// The immutable header is byte-identical in all three cases -- it proves
    /// identity and says nothing about whether the capability still accepts
    /// work -- so the sole difference is the one lifecycle byte in the tail
    /// this slice decodes. Both post-Active states refuse, and every General
    /// account the accepted call would have written is byte-identical after.
    #[test]
    fn a_retiring_or_retired_capability_refuses_with_general_state_intact() {
        for lifecycle in [GeneralLifecycleV2::Retiring, GeneralLifecycleV2::Retired] {
            let fixture = fixture();
            let before = fixture.general_state();
            let mut state = fixture.root.root_state;
            state.begin_retiring(1).expect("begin retiring");
            if lifecycle == GeneralLifecycleV2::Retired {
                state.retire(2).expect("retire");
            }
            assert_eq!(state.lifecycle(), lifecycle);
            let zombie = fixture.root_account(fixture.root.with_state(state));
            assert_eq!(
                fixture.execute(&zombie),
                Err(TradingSbfError::Content.into()),
                "{lifecycle:?} must refuse"
            );
            assert_eq!(fixture.general_state(), before);
            assert_eq!(borrowed(&zombie), fixture.root.with_state(state));
        }
    }

    /// The config the transition reads is the selected one, not a supplied one.
    ///
    /// `authenticate_common` binds the root tail's recorded config identity to
    /// the authenticated selection. Nothing binds the config *value* the
    /// caller hands the transition, so this slice owes that conjunct: a
    /// well-formed General config that is not the selected one refuses before
    /// any state moves.
    #[test]
    fn an_unselected_config_refuses_before_any_general_state_moves() {
        let fixture = fixture();
        let before = fixture.general_state();
        let root_account = fixture.root_account(fixture.root.account_bytes.clone());
        let mut substituted = fixture.config_bytes.clone();
        let last = substituted.len().saturating_sub(1);
        *substituted.get_mut(last).expect("config byte") ^= 1;
        assert_eq!(
            fixture.execute_with_config(&root_account, &substituted),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(fixture.general_state(), before);
    }

    /// The composite root must be this capability's own account, untorn.
    #[test]
    fn a_substituted_torn_or_unprivileged_root_account_refuses() {
        let fixture = fixture();
        let before = fixture.general_state();

        // Another address carrying byte-identical content.
        let elsewhere = account(
            Pubkey::new_unique(),
            true,
            fixture.root.account_bytes.clone(),
            fixture.program_id,
            false,
        );
        assert_eq!(
            fixture.execute(&elsewhere),
            Err(TradingSbfError::Root.into())
        );

        // The right address owned by something else.
        let foreign = account(
            fixture.root.root_key,
            true,
            fixture.root.account_bytes.clone(),
            Pubkey::new_unique(),
            false,
        );
        assert_eq!(fixture.execute(&foreign), Err(TradingSbfError::Root.into()));

        // The right address presented readonly: the tail is family-mutable.
        let readonly = account(
            fixture.root.root_key,
            false,
            fixture.root.account_bytes.clone(),
            fixture.program_id,
            false,
        );
        assert_eq!(
            fixture.execute(&readonly),
            Err(TradingSbfError::Root.into())
        );

        // A truncated composite account: the header alone still decodes, so
        // only the exact General tail width refuses this.
        let mut truncated = fixture.root.account_bytes.clone();
        truncated.truncate(CAPABILITY_ROOT_HEADER_BYTES_V1);
        let torn = fixture.root_account(truncated);
        assert_eq!(fixture.execute(&torn), Err(TradingSbfError::Root.into()));

        // A composite account whose General tail is not a General root.
        let mut foreign_tail = fixture.root.account_bytes.clone();
        foreign_tail
            .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .expect("General tail")
            .fill(0);
        let unschematized = fixture.root_account(foreign_tail);
        assert_eq!(
            fixture.execute(&unschematized),
            Err(TradingSbfError::Root.into())
        );

        assert_eq!(fixture.general_state(), before);
    }

    #[test]
    fn the_composite_width_is_the_family_schema_width() {
        assert_eq!(
            GENERAL_COMPOSITE_ROOT_BYTES_V2,
            CAPABILITY_ROOT_HEADER_BYTES_V1 + GENERAL_ROOT_BYTES_V2
        );
        let fixture = fixture();
        assert_eq!(
            fixture.root.context.root_account_bytes(),
            GENERAL_COMPOSITE_ROOT_BYTES_V2
        );
        assert_eq!(
            fixture.root.account_bytes.len(),
            GENERAL_COMPOSITE_ROOT_BYTES_V2
        );
    }
}
