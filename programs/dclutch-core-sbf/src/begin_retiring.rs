//! Permissionless terminal-to-retiring transition under current Core release.

use dclutch_market_core_codec::{CoreState, Request, Role, begin_retiring};
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    CoreSbfError,
    fixed_role::{authenticate_market, persist_state, read_market_bytes},
    release::authenticate_role,
};

/// Exact account count for one permissionless BeginRetiring action.
pub const BEGIN_RETIRING_ACCOUNT_COUNT_V1: usize = 5;

const MARKET: usize = 0;
const ACTIVATION_CACHE: usize = 1;
const REGISTRY_PROGRAM: usize = 2;
const CORE_PROGRAM: usize = 3;
const CORE_PROGRAMDATA: usize = 4;

/// Reauthenticate the Market-selected current Core deployment and move the
/// terminal Market into Retiring. No caller, child, or static client supplies
/// transition authority.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
) -> Result<(), ProgramError> {
    validate_frame(program_id, accounts)?;
    let market = account(accounts, MARKET)?;
    let state_bytes = read_market_bytes(program_id, market)?;
    let mut state = CoreState::decode(&state_bytes).map_err(|_| CoreSbfError::Market)?;
    authenticate_market(program_id, market, state, request)?;
    let admission = authenticate_role(
        account(accounts, ACTIVATION_CACHE)?,
        account(accounts, REGISTRY_PROGRAM)?,
        account(accounts, CORE_PROGRAM)?,
        account(accounts, CORE_PROGRAMDATA)?,
        state.identity.registry_program,
        state.identity.selected_release_set.to_bytes(),
        Role::Core,
    )?;
    begin_retiring(request, &mut state, admission).map_err(|_| CoreSbfError::Transition)?;
    persist_state(market, state)
}

fn validate_frame(program_id: &Pubkey, accounts: &[AccountInfo<'_>]) -> Result<(), CoreSbfError> {
    if accounts.len() != BEGIN_RETIRING_ACCOUNT_COUNT_V1 {
        return Err(CoreSbfError::AccountFrame);
    }
    let market = account(accounts, MARKET)?;
    let cache = account(accounts, ACTIVATION_CACHE)?;
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    let core = account(accounts, CORE_PROGRAM)?;
    let core_programdata = account(accounts, CORE_PROGRAMDATA)?;
    if accounts.iter().any(|value| value.is_signer)
        || !market.is_writable
        || market.executable
        || cache.is_writable
        || cache.executable
        || registry.is_writable
        || !registry.executable
        || core.key != program_id
        || core.is_writable
        || !core.executable
        || core_programdata.is_writable
        || core_programdata.executable
    {
        return Err(CoreSbfError::AccountFrame);
    }
    for (left, value) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left.saturating_add(1))
            .any(|other| other.key == value.key)
        {
            return Err(CoreSbfError::AccountFrame);
        }
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}

#[cfg(test)]
mod tests {
    use dclutch_market_core_codec::{
        Admission, Binding, Identity, MarketIdentity, Phase, Readiness, ReleaseReceipt, ReleaseSet,
        Role,
    };

    use super::*;

    fn id(value: u8) -> Identity {
        Identity::new([value; 32]).expect("nonzero identity")
    }

    fn release_set() -> ReleaseSet {
        ReleaseSet {
            release_set_id: id(9),
            bindings: [
                Binding {
                    program: id(1),
                    artifact_release: id(11),
                    semantic_release: id(21),
                },
                Binding {
                    program: id(2),
                    artifact_release: id(12),
                    semantic_release: id(22),
                },
                Binding {
                    program: id(3),
                    artifact_release: id(13),
                    semantic_release: id(23),
                },
                Binding {
                    program: id(4),
                    artifact_release: id(14),
                    semantic_release: id(24),
                },
                Binding {
                    program: id(5),
                    artifact_release: id(15),
                    semantic_release: id(25),
                },
            ],
        }
    }

    fn state(phase: Phase) -> CoreState {
        CoreState {
            phase,
            readiness: Readiness::Consumed,
            terminal_winner: 1,
            identity: MarketIdentity {
                market_id: id(31),
                realm_id: id(32),
                product_record: id(33),
                product_id: id(34),
                resolution_policy: id(35),
                capability_manifest: id(36),
                selected_release_set: id(9),
                registry_program: id(37),
                generation: 1,
            },
            outstanding_capabilities: 0,
            rent_beneficiary: id(38),
            terminal_receipt: Some(id(39)),
        }
    }

    fn admission(selected: ReleaseSet) -> Admission {
        let [core, _, _, _, _] = selected.bindings;
        Admission {
            market_registry_program: id(37),
            market_release_set_id: id(9),
            selected,
            receipt: ReleaseReceipt {
                registry_program: id(37),
                release_set_id: id(9),
                role: Role::Core,
                observed: core,
                activation_cache_authenticated: true,
                current_deployment_reauthenticated: true,
            },
        }
    }

    fn request() -> Request {
        Request::administrative(dclutch_market_core_codec::Action::BeginRetiring, 1, id(31))
    }

    #[test]
    fn terminal_moves_to_retiring_only_under_exact_core_release() {
        let mut exact = state(Phase::Terminal);
        assert_eq!(
            begin_retiring(request(), &mut exact, admission(release_set())),
            Ok(())
        );
        assert_eq!(exact.phase, Phase::Retiring);

        let mut wrong_phase = state(Phase::Open);
        assert!(begin_retiring(request(), &mut wrong_phase, admission(release_set())).is_err());

        let mut stale_release = admission(release_set());
        stale_release.receipt.current_deployment_reauthenticated = false;
        let mut unchanged = state(Phase::Terminal);
        let before = unchanged;
        assert!(begin_retiring(request(), &mut unchanged, stale_release).is_err());
        assert_eq!(unchanged, before);
    }
}
