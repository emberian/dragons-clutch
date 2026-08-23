#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

//! Separately deployed executable wrapper for StructuredClaim descriptor v2.
//!
//! The exact non-production capability profile admits actions 1 through 5.
//! Canonical and full wrap execute base custody before Token-2022 mint;
//! canonical and full unwind burn before base custody. SVM rollback makes each
//! sequence atomic, and this program re-reads exact integer deltas before success.

#[cfg(not(feature = "non-production-live-current"))]
compile_error!("select the explicit non-production-live-current wrapper profile");

mod error;
mod executor;
mod loader;
mod system;

use solana_account_info::AccountInfo;
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;

/// Exact deployable capability-profile label.
pub const PROFILE_LABEL: &str =
    "dragons-clutch/structured-claim-wrapper/non-production-live-current/v1";
/// SHA-256 of [`PROFILE_LABEL`], frozen into the wrapper artifact identity.
pub const PROFILE_ID: [u8; 32] = [
    0x0e, 0xb4, 0x84, 0xab, 0xe0, 0x2b, 0x9a, 0xa6, 0x66, 0x1f, 0xb0, 0xd6, 0xcb, 0x36, 0x00, 0xfb,
    0xb8, 0xd7, 0x54, 0x87, 0xec, 0xa7, 0x31, 0x09, 0x8f, 0x3e, 0x03, 0x12, 0x51, 0x7b, 0xd8, 0x3e,
];

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
    fn capability_profile_admits_current_construction_and_wrap_routes() {
        for action in 1_u8..=8 {
            let input = [75, 1, action];
            let admitted = admit_runtime_envelope_v1(&input).map(|value| value.action.tag());
            if matches!(action, 1 | 2 | 3 | 4 | 5) {
                assert_eq!(admitted, Ok(action));
            } else {
                assert_eq!(admitted, Err(Error::CapabilityDisabled));
            }
        }
    }
}
