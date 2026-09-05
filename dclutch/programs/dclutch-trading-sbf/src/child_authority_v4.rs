//! One author for the child-CPI caller authority, derived once per execution.
//!
//! Every child route signs its CPI as a `CallerAuthoritySeedsV1` PDA of the
//! Trading program. Those seeds end in a PER-EXECUTION request digest, so no
//! record can carry the bump the way `hot_v3::borrow_finalized_record_at` reads
//! the Market-selected records' bumps: the coordinate exists only for the
//! length of one instruction.
//!
//! What it CAN do is derive the bump once and carry it, and until this module
//! existed it did not. `preflight_child_routes_v3` and `execute_child_routes_v3`
//! walk the same Effect at the same registers over the same downgraded account
//! vector, and each composition's `prepare` ran a full
//! [`Pubkey::find_program_address`] in BOTH walks, from byte-identical seeds,
//! for the same address. Measured against the shipped ELF at fixture seed 9,
//! Custody's preflight alone cost 13,433 CU of which the search is
//! 1,500 per attempt, and the cross-seed spread of that one phase was 3,000 CU
//! (two attempts) with another 4,501 in the execution walk.
//!
//! So: the PREFLIGHT walk derives, canonically, and the EXECUTION walk
//! reproduces the address from the bump the preflight found. The reproduction
//! is [`Pubkey::create_program_address`], and it is not a weakened check --
//! it is the same conjunction with the search removed:
//!
//! * the address it produces is still required to equal the account the frame
//!   supplied at coordinate 0 (each composition checks that itself, unchanged);
//! * the seeds are not caller input on either walk. They are rebuilt from the
//!   same request bytes at the same coordinates in the same instruction, and
//!   nothing between the two walks can move them: the request bank is a heap
//!   buffer this instruction owns, and no child CPI has run when the preflight
//!   derives;
//! * a wrong bump reproduces a DIFFERENT address and refuses at the equality
//!   above, so the carried bump is a memo of this executable's own computation,
//!   never an authority -- the identical argument
//!   `LifecycleSeedsV4::pending_bump` already makes for the lifecycle replan;
//! * and canonicity is enforced twice over regardless: the signature the
//!   runtime derives from `seeds + bump` is what the CALLEE sees, and every
//!   callee re-derives the caller authority canonically from its own copy of
//!   the seeds before it will honour the signer bit.
//!
//! One fact, one author: no composition writes this derivation by hand any
//! more, so a fourth family cannot reintroduce the second search by copying a
//! third.

use dclutch_registry::release_set::CallerAuthoritySeedsV1;
use solana_program::{program_error::ProgramError, pubkey::Pubkey};

use crate::TradingSbfError;

/// The canonical bump one walk found for a child caller authority.
///
/// `None` on the preflight walk, which searches; `Some` on the execution walk,
/// which reproduces.
pub type PreflightedCallerBumpV4 = Option<u8>;

/// Derive a child caller authority, or reproduce the preflight's derivation.
///
/// Returns the address and the canonical bump. Pass `None` from the walk that
/// derives and `Some(bump)` from the walk that follows it.
pub fn child_caller_authority_v4(
    seeds: &CallerAuthoritySeedsV1,
    program_id: &Pubkey,
    preflighted: PreflightedCallerBumpV4,
) -> Result<(Pubkey, u8), ProgramError> {
    match preflighted {
        None => Ok(Pubkey::find_program_address(&seeds.as_slices(), program_id)),
        Some(bump) => {
            let [domain, release, market, role, context, digest] = seeds.as_slices();
            let bump_seed = [bump];
            let address = Pubkey::create_program_address(
                &[domain, release, market, role, context, digest, &bump_seed],
                program_id,
            )
            .map_err(|_| TradingSbfError::Release)?;
            Ok((address, bump))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use dclutch_core_contract::ContentId;
    use dclutch_registry::release_set::ExecutionRoleV1;

    /// A hint that is not this authority's canonical bump names a DIFFERENT
    /// address, which is what every composition's coordinate-0 equality
    /// refuses.
    ///
    /// The module already argued this for the bump the PREFLIGHT walk carries
    /// to the execution walk. The argument does not weaken when the byte comes
    /// from the caller instead: it is the same reproduction, checked the same
    /// way, and the caller mined it off chain from seeds it had to compute to
    /// build the request at all. See `HotBumpHintsV1`.
    #[test]
    fn a_wrong_caller_authority_bump_hint_names_another_address() {
        let program_id = Pubkey::new_from_array([0x21; 32]);
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new([0x31; 32]).expect("release set"),
            [0x41; 32],
            ExecutionRoleV1::Trading,
            [0x51; 32],
            [0x61; 32],
        )
        .expect("caller authority seeds");

        let (canonical_address, canonical) =
            child_caller_authority_v4(&seeds, &program_id, None).expect("searched");
        assert_eq!(
            child_caller_authority_v4(&seeds, &program_id, Some(canonical)).expect("reproduced"),
            (canonical_address, canonical),
        );

        let mut refused = 0_u32;
        for hint in 0..=u8::MAX {
            if hint == canonical {
                continue;
            }
            match child_caller_authority_v4(&seeds, &program_id, Some(hint)) {
                Ok((address, bump)) => {
                    assert_ne!(address, canonical_address, "hint {hint}");
                    assert_eq!(bump, hint);
                }
                Err(_) => {}
            }
            refused = refused.saturating_add(1);
        }
        assert_eq!(refused, 255);
    }
}
