#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! The one readonly, stateless accelerator: General clearing, the Dealer
//! families and the Series shadow behind one program id.
//!
//! Trading composes every invocation. It authenticates the release, the
//! selected artifacts, the exact request and the account frame, CPIs this
//! program with the admitted frame, and reads back one typed
//! acknowledgement. This program writes nothing it does not own, invokes no
//! child, and holds no protocol state; Trading stays the sole effect
//! projector and commit-last writer.
//!
//! Which arm runs is read from the same fact every arm already authenticates:
//! the Shadow transport announces itself in the instruction data
//! (`series`), and the two admitted transports carry the family request in
//! the top-level Trading instruction, which the Instructions sysvar exposes
//! at one fixed coordinate of the admitted frame (`dealer`, else `general`).
//! The read here is a selection, not an authentication -- it allocates
//! nothing and refuses nothing -- and the arm then re-reads the same bytes
//! under its own conjuncts and refuses, by its own name, anything the
//! selection got wrong.

extern crate alloc;
extern crate std;

pub mod dealer;
pub mod general;
pub mod series;

use dclutch_market::capability_program::hot_v3::HotExecutionEnvelopeV3;
use dclutch_market::execution_strategy::{
    admitted_v3::ADMITTED_INSTRUCTIONS_ACCOUNT_V3, shadow_v3::SHADOW_REQUEST_MAGIC_V3,
};
use dclutch_trading_sbf::dealer::{
    equity_request::DEALER_EQUITY_REQUEST_MAGIC_V3, lp_request::DEALER_MULTI_LP_REQUEST_MAGIC_V3,
};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

/// The program heap every arm shares.
///
/// `solana_program::entrypoint!` elides its stock allocator exactly when the
/// calling crate declares a feature named `custom-heap`, and then the crate
/// owes an allocator of its own. [`dclutch_sbf_runtime::program_heap_v1`]
/// is Trading's: it bumps UPWARD, so the ceiling is a comparison rather than
/// an origin and can be raised mid-invocation, and it is a shared crate
/// rather than a second copy so this one keeps `#![forbid(unsafe_code)]`.
/// The ceiling starts at the protocol default; the General arm raises it to
/// the frame the top-level transaction declared once it has proved the
/// request, and the Dealer arm raises it to the grant it reads back out of
/// the Instructions sysvar.
///
/// What makes this sound across the CPI boundary is the pair of runtime facts
/// Trading's adapter states as its trust-surface assumptions 5 and 6
/// (`dclutch-trading-sbf/src/entrypoint_adapter.rs`): the heap region is
/// zero-filled at the start of every invocation *including each CPI depth*,
/// so this allocator reads its own fresh header rather than the caller's bump
/// position, and a validated `RequestHeapFrame(n)` maps a heap of exactly `n`
/// bytes for every invocation in the transaction.
#[cfg(all(
    target_os = "solana",
    feature = "custom-heap",
    not(feature = "no-entrypoint")
))]
#[global_allocator]
pub(crate) static PROGRAM_HEAP_V1: dclutch_sbf_runtime::BumpHeapV1 =
    dclutch_sbf_runtime::program_heap_v1();

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Route one invocation to the arm its request names.
///
/// Written as the guard chain the route census reads -- a magic guard for the
/// Shadow transport, a predicate for the Dealer families, General as the
/// fallthrough -- so each arm is a route the census can name and a campaign
/// can bind.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.get(..SHADOW_REQUEST_MAGIC_V3.len())
        == Some(SHADOW_REQUEST_MAGIC_V3.as_slice())
    {
        return series::process(program_id, accounts, instruction_data);
    }
    if dealer_family_selected(accounts) {
        return dealer::process(program_id, accounts, instruction_data);
    }
    general::process(program_id, accounts, instruction_data)
}

/// Whether the top-level Trading instruction names a Dealer family.
///
/// The two admitted transports are not family-specific, so the family is read
/// where both admitted arms authenticate it: the top-level Trading instruction,
/// exposed by the Instructions sysvar at [`ADMITTED_INSTRUCTIONS_ACCOUNT_V3`]
/// -- the same coordinate in both admitted frames -- whose Hot envelope
/// carries the family request, whose leading eight bytes are the family magic.
/// Anything the read cannot classify is General's, whose authenticator refuses
/// a bad sysvar, a foreign caller and a non-General family each by name,
/// exactly as the standalone General accelerator did.
#[must_use]
pub fn dealer_family_selected(accounts: &[AccountInfo<'_>]) -> bool {
    let Some(instructions) = accounts.get(ADMITTED_INSTRUCTIONS_ACCOUNT_V3) else {
        return false;
    };
    if instructions.key != &solana_sdk_ids::sysvar::instructions::ID {
        return false;
    }
    let Ok(data) = instructions.try_borrow_data() else {
        return false;
    };
    matches!(
        top_level_family_magic(&data),
        Some(magic)
            if magic == DEALER_MULTI_LP_REQUEST_MAGIC_V3
                || magic == DEALER_EQUITY_REQUEST_MAGIC_V3
    )
}

/// The family magic of the instruction currently executing at the top level,
/// read out of the raw Instructions sysvar bytes.
///
/// The layout is the one `solana_instructions_sysvar` documents and
/// constructs: a `u16` instruction count, a `u16` offset per instruction, and
/// per instruction a `u16` account count, 33 bytes per account meta, a 32-byte
/// program id, a `u16` data length and the data; the current index is the
/// trailing `u16`. `load_instruction_at_checked` reads the same bytes but
/// allocates a copy of the instruction, and this program's heap peak is set
/// by the bank it evaluates, not by a copy of the request it was called under.
fn top_level_family_magic(data: &[u8]) -> Option<[u8; 8]> {
    const META_BYTES: usize = 33;
    let current_at = data.len().checked_sub(2)?;
    let current = usize::from(read_u16(data, current_at)?);
    let count = usize::from(read_u16(data, 0)?);
    if current >= count {
        return None;
    }
    let offset_at = current.checked_mul(2)?.checked_add(2)?;
    let start = usize::from(read_u16(data, offset_at)?);
    let accounts = usize::from(read_u16(data, start)?);
    let data_len_at = accounts
        .checked_mul(META_BYTES)?
        .checked_add(start)?
        .checked_add(2)?
        .checked_add(32)?;
    let data_len = usize::from(read_u16(data, data_len_at)?);
    let data_at = data_len_at.checked_add(2)?;
    let instruction_data = data.get(data_at..data_at.checked_add(data_len)?)?;
    let (_, family) = HotExecutionEnvelopeV3::split_instruction(instruction_data).ok()?;
    family.get(..8)?.try_into().ok()
}

fn read_u16(data: &[u8], at: usize) -> Option<u16> {
    let bytes: [u8; 2] = data.get(at..at.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use dclutch_market::capability_program::hot_v3::HOT_EXECUTION_ENVELOPE_BYTES_V3;

    use super::*;

    /// One serialized Instructions sysvar, laid out the way the runtime lays
    /// it out, over `instructions` of `(program_id, data)` with the current
    /// index at the tail.
    fn sysvar(instructions: &[([u8; 32], Vec<u8>)], current: u16) -> Vec<u8> {
        let count = u16::try_from(instructions.len()).expect("count");
        let mut bytes = count.to_le_bytes().to_vec();
        let table_at = bytes.len();
        bytes.resize(table_at + 2 * instructions.len(), 0);
        for (index, (program_id, data)) in instructions.iter().enumerate() {
            let start = u16::try_from(bytes.len()).expect("offset");
            bytes[table_at + 2 * index..table_at + 2 * index + 2]
                .copy_from_slice(&start.to_le_bytes());
            bytes.extend_from_slice(&0_u16.to_le_bytes());
            bytes.extend_from_slice(program_id);
            bytes.extend_from_slice(&u16::try_from(data.len()).expect("len").to_le_bytes());
            bytes.extend_from_slice(data);
        }
        bytes.extend_from_slice(&current.to_le_bytes());
        bytes
    }

    fn hot_instruction(family: &[u8]) -> Vec<u8> {
        let envelope = HotExecutionEnvelopeV3::new(
            u32::try_from(family.len()).expect("family"),
            [1; 32],
            [2; 32],
            7,
            [3; 32],
        )
        .expect("envelope");
        let mut bytes = envelope.to_bytes().to_vec();
        bytes.extend_from_slice(family);
        assert_eq!(bytes.len(), HOT_EXECUTION_ENVELOPE_BYTES_V3 + family.len());
        bytes
    }

    #[test]
    fn the_family_magic_is_read_from_the_current_top_level_instruction() {
        let mut family = DEALER_EQUITY_REQUEST_MAGIC_V3.to_vec();
        family.extend_from_slice(&[0xaa; 40]);
        let image = sysvar(
            &[
                ([9; 32], alloc::vec![2, 0, 0, 1, 0]),
                ([4; 32], hot_instruction(&family)),
            ],
            1,
        );
        assert_eq!(
            top_level_family_magic(&image),
            Some(DEALER_EQUITY_REQUEST_MAGIC_V3)
        );
        // The current index selects, not the last instruction.
        let image = sysvar(
            &[
                ([4; 32], hot_instruction(&family)),
                ([9; 32], alloc::vec![2, 0, 0, 1, 0]),
            ],
            1,
        );
        assert_eq!(top_level_family_magic(&image), None);
    }

    #[test]
    fn a_malformed_sysvar_selects_nothing_and_panics_nowhere() {
        assert_eq!(top_level_family_magic(&[]), None);
        assert_eq!(top_level_family_magic(&[1, 0, 0, 0]), None);
        let image = sysvar(&[([4; 32], alloc::vec![0; 3])], 0);
        assert_eq!(top_level_family_magic(&image), None);
        let image = sysvar(&[([4; 32], hot_instruction(&[0; 4]))], 3);
        assert_eq!(top_level_family_magic(&image), None);
    }

    #[test]
    fn an_absent_sysvar_coordinate_selects_no_dealer_family() {
        assert!(!dealer_family_selected(&[]));
    }
}
