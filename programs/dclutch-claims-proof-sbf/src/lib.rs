#![cfg_attr(target_os = "solana", no_std)]

//! Alias-simple exact-account SBF executor for the Lean-owned claim plan.

include!("generated_profile.rs");

const ERROR: u64 = 5_u64 << 32;

#[cfg(target_os = "solana")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}

#[inline(always)]
unsafe fn read_u64(input: *mut u8, offset: usize) -> u64 {
    core::ptr::read(input.add(offset).cast::<u64>())
}

#[inline(always)]
unsafe fn write_u64(input: *mut u8, offset: usize, value: u64) {
    core::ptr::write(input.add(offset).cast::<u64>(), value);
}

#[inline(always)]
unsafe fn equal_32(input: *mut u8, left: usize, right: usize) -> bool {
    read_u64(input, left) == read_u64(input, right)
        && read_u64(input, left + 8) == read_u64(input, right + 8)
        && read_u64(input, left + 16) == read_u64(input, right + 16)
        && read_u64(input, left + 24) == read_u64(input, right + 24)
}

#[inline(always)]
fn add_checked(left: u64, right: u64) -> Option<u64> {
    let result = left.wrapping_add(right);
    if result < left {
        None
    } else {
        Some(result)
    }
}

/// Execute one exact controller-authorized four-effect claim plan.
///
/// # Safety boundary
///
/// The pinned Solana loader must provide a non-null, aligned ABI-v1 buffer with
/// the complete extent implied by the Lean-generated exact-account profile.
#[no_mangle]
pub extern "C" fn entrypoint(input: *mut u8) -> u64 {
    if input.is_null() {
        return ERROR;
    }

    // SAFETY: the loader extent/alignment assumption is named above. All
    // semantic bytes and write authority are checked before the four writes.
    unsafe {
        if read_u64(input, ACCOUNT_COUNT_OFFSET) != 2
            || read_u64(input, AUTHORITY_OFFSET) != AUTHORITY_FRAME_WORD
            || read_u64(input, AUTHORITY_OFFSET + 80) != 0
            || read_u64(input, PROJECTION_OFFSET) != PROJECTION_FRAME_WORD
            || read_u64(input, PROJECTION_OFFSET + 80) != PROJECTION_DATA_BYTES
            || equal_32(input, AUTHORITY_OFFSET + 8, PROJECTION_OFFSET + 8)
            || read_u64(input, INSTRUCTION_LENGTH_OFFSET) != INSTRUCTION_BYTES
            || !equal_32(input, PROJECTION_OFFSET + 40, PROGRAM_ID_OFFSET)
        {
            return ERROR;
        }

        let authority0 = read_u64(input, AUTHORITY_OFFSET + 8);
        let authority1 = read_u64(input, AUTHORITY_OFFSET + 16);
        let authority2 = read_u64(input, AUTHORITY_OFFSET + 24);
        let authority3 = read_u64(input, AUTHORITY_OFFSET + 32);
        if (authority0 | authority1 | authority2 | authority3) == 0
            || authority0 != read_u64(input, PROJECTION_DATA_OFFSET + 8)
            || authority1 != read_u64(input, PROJECTION_DATA_OFFSET + 16)
            || authority2 != read_u64(input, PROJECTION_DATA_OFFSET + 24)
            || authority3 != read_u64(input, PROJECTION_DATA_OFFSET + 32)
            || read_u64(input, PROJECTION_DATA_OFFSET) != STATE_MAGIC_WORD
        {
            return ERROR;
        }

        let outcome = read_u64(input, PROJECTION_DATA_OFFSET + 40);
        if outcome > u64::from(u32::MAX)
            || read_u64(input, INSTRUCTION_OFFSET) != PLAN_HEADER_WORD
            || read_u64(input, INSTRUCTION_OFFSET + 8) != EFFECT_0_TAG
            || read_u64(input, INSTRUCTION_OFFSET + 24) != EFFECT_1_TAG
            || (read_u64(input, INSTRUCTION_OFFSET + 40) & 0xffff_ffff) != EFFECT_2_TAG
            || (read_u64(input, INSTRUCTION_OFFSET + 40) >> 32) != outcome
            || (read_u64(input, INSTRUCTION_OFFSET + 56) & 0xffff_ffff) != EFFECT_3_TAG
            || (read_u64(input, INSTRUCTION_OFFSET + 56) >> 32) != outcome
        {
            return ERROR;
        }

        let seller_nonce = read_u64(input, PROJECTION_DATA_OFFSET + 48);
        let buyer_nonce = read_u64(input, PROJECTION_DATA_OFFSET + 56);
        let seller_claims = read_u64(input, PROJECTION_DATA_OFFSET + 64);
        let buyer_claims = read_u64(input, PROJECTION_DATA_OFFSET + 72);
        let next_seller = match add_checked(seller_nonce, 1) {
            Some(value) => value,
            None => return ERROR,
        };
        let next_buyer = match add_checked(buyer_nonce, 1) {
            Some(value) => value,
            None => return ERROR,
        };
        let plan_seller_nonce = read_u64(input, INSTRUCTION_OFFSET + 16);
        let plan_buyer_nonce = read_u64(input, INSTRUCTION_OFFSET + 32);
        let claim_debit = read_u64(input, INSTRUCTION_OFFSET + 48);
        let claim_credit = read_u64(input, INSTRUCTION_OFFSET + 64);

        if plan_seller_nonce != next_seller
            || plan_buyer_nonce != next_buyer
            || claim_debit != claim_credit
            || seller_claims < claim_debit
        {
            return ERROR;
        }
        let next_buyer_claims = match add_checked(buyer_claims, claim_credit) {
            Some(value) => value,
            None => return ERROR,
        };

        write_u64(input, PROJECTION_DATA_OFFSET + 48, next_seller);
        write_u64(input, PROJECTION_DATA_OFFSET + 56, next_buyer);
        write_u64(
            input,
            PROJECTION_DATA_OFFSET + 64,
            seller_claims - claim_debit,
        );
        write_u64(input, PROJECTION_DATA_OFFSET + 72, next_buyer_claims);
        0
    }
}
