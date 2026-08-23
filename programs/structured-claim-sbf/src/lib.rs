#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

//! Separately deployed executable wrapper for StructuredClaim descriptor v2.
//!
//! The exact non-production capability profile admits only actions 1, 2, and
//! 4. Canonical wrap executes base custody before Token-2022 mint; canonical
//! unwind burns before base custody. SVM rollback makes either sequence atomic,
//! and this program independently re-reads exact integer deltas before success.

#[cfg(not(feature = "non-production-live-canonical"))]
compile_error!("select the explicit non-production-live-canonical wrapper profile");

mod error;
mod executor;
mod loader;
mod system;

use solana_account_info::AccountInfo;
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;

/// Exact deployable capability-profile label.
pub const PROFILE_LABEL: &str =
    "dragons-clutch/structured-claim-wrapper/non-production-live-canonical/v1";
/// SHA-256 of [`PROFILE_LABEL`], frozen into the wrapper artifact identity.
pub const PROFILE_ID: [u8; 32] = [
    0xfe, 0x4f, 0x88, 0xde, 0xb6, 0x12, 0xa8, 0x7e, 0xb9, 0x8f, 0x4c, 0xf8, 0xfa, 0xd9, 0x39, 0x8f,
    0xf9, 0xc3, 0x85, 0x8c, 0x73, 0x77, 0xd9, 0xf1, 0x1e, 0xe2, 0x4f, 0x0c, 0xd3, 0xb1, 0x16, 0xa9,
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
    use solana_account_info::AccountInfo;
    use solana_program_entrypoint::{entrypoint, ProgramResult, HEAP_START_ADDRESS};
    use solana_pubkey::Pubkey;

    entrypoint!(process_instruction);

    const HEAP_CEILING: usize = 256 * 1024;

    struct GrowableBump;

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
    fn capability_profile_admits_only_create_and_canonical_routes() {
        for action in 1_u8..=8 {
            let input = [75, 1, action];
            let admitted = admit_runtime_envelope_v1(&input).map(|value| value.action.tag());
            if matches!(action, 1 | 2 | 4) {
                assert_eq!(admitted, Ok(action));
            } else {
                assert_eq!(admitted, Err(Error::CapabilityDisabled));
            }
        }
    }
}
