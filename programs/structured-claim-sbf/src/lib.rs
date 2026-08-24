#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

//! Separately deployed executable wrapper for StructuredClaim descriptor v2.
//!
//! The unified successor development profile admits the exact current
//! Structured action set through a three-release capability join.
//! Full-vector wrap executes base custody before Token-2022 mint; full-vector
//! unwind and terminal redemption burn before base custody. SVM rollback makes
//! each sequence atomic, and this program re-reads exact integer deltas before
//! success. Historical canonical actions 2/4 are decode-only refusals.

#[cfg(not(feature = "profile-successor-chain-attached-dev"))]
compile_error!("select the explicit profile-successor-chain-attached-dev wrapper profile");

mod error;
mod executor;
mod loader;
mod system;

use solana_account_info::AccountInfo;
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;

/// Exact deployable capability-profile label.
pub const PROFILE_LABEL: &str =
    clutch_structured_claim_adapter::STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_LABEL_V1;
/// SHA-256 of [`PROFILE_LABEL`], frozen into the wrapper artifact identity.
pub const PROFILE_ID: [u8; 32] =
    clutch_structured_claim_adapter::STRUCTURED_CURRENT_RELEASE_CONTRACT_V1
        .wrapper_capability_manifest_id;
/// Actions with one exact current source/account contract.
pub const IMPLEMENTED_ACTION_MASK: u16 =
    clutch_structured_claim_adapter::STRUCTURED_CURRENT_RELEASE_CONTRACT_V1
        .implemented_action_mask;
/// Actions admitted by the checked wrapper/base/Token-2022 releases.
pub const ENABLED_ACTION_MASK: u16 =
    clutch_structured_claim_adapter::STRUCTURED_CURRENT_RELEASE_CONTRACT_V1.admitted_action_mask;
/// Exact source/account/token-effect contract compiled by this wrapper.
pub const ACCOUNT_CONTRACT_ID: [u8; 32] =
    clutch_structured_claim_adapter::STRUCTURED_CURRENT_RELEASE_CONTRACT_V1.account_contract_id;

const _: () = assert!(ENABLED_ACTION_MASK == IMPLEMENTED_ACTION_MASK);
const _: () = assert!(ENABLED_ACTION_MASK & !IMPLEMENTED_ACTION_MASK == 0);

/// Program entrypoint implementation, also callable by host harnesses.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> core::result::Result<(), ProgramError> {
    executor::process(program_id, accounts, instruction_data).map_err(Into::into)
}

#[cfg(target_os = "solana")]
mod bpf {
    //! SBF adapter/runtime trust boundary.
    //!
    //! The allocator below is the only first-party unsafe in this artifact:
    //! it manipulates the runtime-provided heap cursor required by the Solana
    //! entrypoint ABI. It is outside Eggcrate and every pure Structured
    //! contract, is not proof or kernel evidence, and must be reviewed and
    //! measured with the exact deployed ELF.

    use solana_account_info::AccountInfo;
    use solana_program_entrypoint::{entrypoint, ProgramResult, HEAP_START_ADDRESS};
    use solana_pubkey::Pubkey;

    entrypoint!(process_instruction);

    const HEAP_CEILING: usize = 256 * 1024;

    struct GrowableBump;

    // SAFETY BOUNDARY: Solana supplies HEAP_START_ADDRESS; this adapter owns
    // the cursor word and admits only checked, aligned allocations below the
    // fixed transaction heap ceiling. The runtime, ABI, and allocator remain
    // explicitly unverified.
    unsafe impl std::alloc::GlobalAlloc for GrowableBump {
        #[inline]
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            let start = HEAP_START_ADDRESS;
            let cursor = core::ptr::with_exposed_provenance_mut::<usize>(start);
            let mut position = *cursor;
            if position == 0 {
                position = start + core::mem::size_of::<usize>();
            }
            let align = layout.align().max(core::mem::size_of::<usize>());
            let aligned = match position.checked_add(align - 1) {
                Some(value) => value & !(align - 1),
                None => return core::ptr::null_mut(),
            };
            let end = match aligned.checked_add(layout.size()) {
                Some(value) => value,
                None => return core::ptr::null_mut(),
            };
            if end > start + HEAP_CEILING {
                return core::ptr::null_mut();
            }
            *cursor = end;
            core::ptr::with_exposed_provenance_mut::<u8>(aligned)
        }

        #[inline]
        unsafe fn dealloc(&self, _: *mut u8, _: std::alloc::Layout) {}
    }

    #[global_allocator]
    static ALLOCATOR: GrowableBump = GrowableBump;

    fn process_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo<'_>],
        instruction_data: &[u8],
    ) -> ProgramResult {
        crate::process_instruction(program_id, accounts, instruction_data)
    }
}

#[cfg(test)]
mod tests {
    use clutch_structured_claim_adapter::{admit_runtime_envelope_v1, Error};

    #[test]
    fn capability_profile_admits_exact_current_actions() {
        assert_eq!(
            crate::executor::STRUCTURED_WRAPPER_HANDLER_ACTION_MASK_V1,
            crate::ENABLED_ACTION_MASK,
        );
        for action in 1_u8..=8 {
            let input = [75, 1, action];
            let admitted = admit_runtime_envelope_v1(&input).map(|value| value.action.tag());
            if matches!(action, 1 | 3 | 5 | 6 | 7 | 8) {
                assert_eq!(admitted, Ok(action));
            } else {
                assert_eq!(admitted, Err(Error::CapabilityDisabled));
            }
        }
    }
}
