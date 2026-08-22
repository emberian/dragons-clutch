//! A no-op SBF program, standing in for the Pyth pull receiver on a bank.
//!
//! This is not a model of the receiver and does not implement `post_update`.
//! It writes nothing, reads nothing, and returns success for every input.
//!
//! That is the whole requirement. `crate::source_v2::auth` in the program
//! under test authenticates the *receiver deployment* — the pinned program
//! key, its Upgradeable Loader ownership, its ProgramData link and deployment
//! slot, and its governance `Config` digest — and separately authenticates
//! *adjacency*, that the immediately preceding instruction in this transaction
//! invoked that program id naming this exact ephemeral update account. It then
//! reads the price from the update account's own 134 bytes. At no point does
//! it depend on what the receiver did. A laboratory receiver therefore only
//! has to be a program that loads and succeeds, and being a no-op is what
//! makes it honest: nothing here can accidentally supply evidence.
//!
//! The fabricated update account is installed by the test harness, exactly as
//! the harness installs every other fabricated account in these planes.
#![no_std]

/// Panic handler for a `no_std` cdylib. Unreachable: nothing below panics.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // `abort` is the syscall a Solana program's panic path ends in; spinning
    // is unreachable and costs no compute because nothing here can panic.
    loop {}
}

/// The SBF entrypoint. Returns `0`, the runtime's success code.
///
/// # Safety
///
/// The runtime passes a pointer to the serialized instruction context. This
/// function never dereferences it.
#[no_mangle]
pub unsafe extern "C" fn entrypoint(_input: *mut u8) -> u64 {
    0
}
