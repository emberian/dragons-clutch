//! What a capability activation can and cannot compose for a Fractional root.
//!
//! Trading's activation seam runs a selected descriptor's effect program and
//! writes its projected request buffer as the new root's family tail. The
//! effect reads named register banks that the seam publishes
//! (`dclutch_capability_program_contract::activation_registers_v2`), plus
//! whatever a descriptor's own account profile projects out of the accounts it
//! declares. Nothing else is in scope at that moment.
//!
//! These tests exist because a wrong tail bricks a root permanently, and
//! because Trading's own guard against that does not catch the way Fractional
//! would get it wrong. That guard refuses a tail that is entirely zero
//! (`programs/dclutch-trading-sbf/src/outer.rs`, the `root_state_bytes != 0 &&
//! output_request.iter().all(|byte| *byte == 0)` refusal). A Fractional tail
//! composed only from values the seam actually publishes is NOT entirely zero
//! -- it has a magic, a version, a market and a rent principal -- so it sails
//! past that guard and is still undecodable forever after.
//!
//! The General family is the contrast that makes this precise rather than
//! pessimistic. `GeneralRootV2::active(market, config_id, generation)` takes
//! exactly three variable inputs and every one of them is seam-published:
//! `ACTIVATION_MARKET_IDENTITY_V2`, `ACTIVATION_CONFIG_IDENTITY_V2` and
//! `ACTIVATION_GENERATION_SCALAR_V2`. Every other field of a new General root
//! is a constant. That is why General's activation is authorable family-side.
//! Fractional's root needs three values the seam publishes none of, so the same
//! shape of work is not available to it without a seam change.

use dclutch_fractional_claim_contract::{
    FRACTIONAL_ROOT_BYTES_V1, FRACTIONAL_ROOT_MAGIC_V1, FRACTIONAL_ROOT_MARKET_OFFSET_V1,
    FRACTIONAL_ROOT_RENT_BENEFICIARY_OFFSET_V1, FRACTIONAL_ROOT_RENT_PRINCIPAL_OFFSET_V1,
    FRACTIONAL_ROOT_REVISION_OFFSET_V1, FRACTIONAL_ROOT_TERMS_OFFSET_V1, FractionalRootInputV1,
    FractionalRootV1,
};

/// A Market identity, which the activation seam does publish.
const MARKET: [u8; 32] = [7; 32];
/// The root's rent-exempt minimum, projectable from the funding ledger's quote
/// exactly as the Direct activation descriptor projects its own.
const RENT_PRINCIPAL: u64 = 2_672_640;
/// The one revision a newly created root can be at.
const INITIAL_REVISION: u64 = 1;

/// Compose a tail the way an effect program actually would.
///
/// Effect kernel V2 offers exactly two request-buffer writes,
/// `write_request_u64` (8 bytes) and `write_request_identity` (32 bytes), and
/// the encoder refuses writes that overlap. So a Fractional tail is seven
/// non-overlapping writes tiling 0..128, and this helper is those seven writes
/// with each value named by where it would have to come from.
fn projected_tail(
    header_word: u64,
    terms: [u8; 32],
    market: [u8; 32],
    rent_beneficiary: [u8; 32],
    revision: u64,
    rent_principal: u64,
) -> [u8; FRACTIONAL_ROOT_BYTES_V1] {
    let mut tail = [0_u8; FRACTIONAL_ROOT_BYTES_V1];
    tail[..8].copy_from_slice(&FRACTIONAL_ROOT_MAGIC_V1);
    tail[8..16].copy_from_slice(&header_word.to_le_bytes());
    tail[FRACTIONAL_ROOT_TERMS_OFFSET_V1..FRACTIONAL_ROOT_TERMS_OFFSET_V1 + 32]
        .copy_from_slice(&terms);
    tail[FRACTIONAL_ROOT_MARKET_OFFSET_V1..FRACTIONAL_ROOT_MARKET_OFFSET_V1 + 32]
        .copy_from_slice(&market);
    tail[FRACTIONAL_ROOT_RENT_BENEFICIARY_OFFSET_V1
        ..FRACTIONAL_ROOT_RENT_BENEFICIARY_OFFSET_V1 + 32]
        .copy_from_slice(&rent_beneficiary);
    tail[FRACTIONAL_ROOT_REVISION_OFFSET_V1..FRACTIONAL_ROOT_REVISION_OFFSET_V1 + 8]
        .copy_from_slice(&revision.to_le_bytes());
    tail[FRACTIONAL_ROOT_RENT_PRINCIPAL_OFFSET_V1..FRACTIONAL_ROOT_RENT_PRINCIPAL_OFFSET_V1 + 8]
        .copy_from_slice(&rent_principal.to_le_bytes());
    tail
}

/// The header word at `[8, 16)`: version, then bump, then five canonical zeros.
///
/// One aligned `u64`, which is the only reason a single `write_request_u64` can
/// author it. The version is a constant; the bump is not.
fn header_word(bump: u8) -> u64 {
    u64::from(1_u16) | (u64::from(bump) << 16)
}

/// Seven writes from seam-published values alone produce a permanent brick.
///
/// This is the whole finding, stated as an assertion instead of a memo. The
/// tail is well-formed at its magic, carries the real Market and the real rent
/// principal, and is emphatically not all-zero -- so Trading's brick guard
/// admits it -- and `FractionalRootV1::decode` still refuses it, because the
/// three fields no register carries are zero and the root refuses zero there.
///
/// A root written this way can never be decoded by any Fractional action. That
/// is not a bug to be fixed downstream; it is why the descriptor is not being
/// written this way.
#[test]
fn a_tail_composed_only_from_seam_published_values_is_a_brick_the_guard_admits() {
    let tail = projected_tail(
        // No register carries the root's PDA bump: Trading derives it in
        // `commit_activation`, AFTER the effect has already run and its output
        // has been fixed. Zero is the only value an effect could write.
        header_word(0),
        // No register carries the Terms identity. See the test below.
        [0; 32],
        MARKET,
        // No register carries a rent beneficiary.
        [0; 32],
        INITIAL_REVISION,
        RENT_PRINCIPAL,
    );

    // Trading's all-zero brick guard would ADMIT this tail.
    assert!(
        tail.iter().any(|byte| *byte != 0),
        "the seam guard only catches an all-zero tail, so this must not be one"
    );
    assert_eq!(tail[..8], FRACTIONAL_ROOT_MAGIC_V1);

    // And the root is still undecodable, forever.
    assert_eq!(FractionalRootV1::decode(&tail), None);
}

/// The three missing authors are exactly what separates brick from root.
///
/// The control for the test above. Same seven writes, same offsets, same
/// helper; the only change is supplying the three values the seam does not
/// publish. If this failed, the test above would be evidence about a broken
/// layout rather than about missing authors.
#[test]
fn supplying_the_three_unpublished_values_is_the_whole_difference() {
    let complete = projected_tail(
        header_word(254),
        [1; 32],
        MARKET,
        [3; 32],
        INITIAL_REVISION,
        RENT_PRINCIPAL,
    );
    let decoded = FractionalRootV1::decode(&complete).expect("a complete tail decodes");
    assert_eq!(
        decoded,
        FractionalRootV1::new(FractionalRootInputV1 {
            bump: 254,
            terms: [1; 32],
            market: MARKET,
            rent_beneficiary: [3; 32],
            revision: INITIAL_REVISION,
            historical_rent_principal: RENT_PRINCIPAL,
        })
        .expect("root")
    );

    // And each of the root's three nonzero requirements alone is sufficient to
    // brick it, so none is an incidental extra a descriptor could quietly skip.
    // Note that only two of these three are unavailable at activation: the rent
    // principal IS projectable, from the funding ledger's rent quote, exactly as
    // the Direct descriptor projects its own. It is here to show the refusal is
    // a property of the root's constructor rather than of the two missing ones.
    for (label, tail) in [
        (
            "terms",
            projected_tail(
                header_word(254),
                [0; 32],
                MARKET,
                [3; 32],
                INITIAL_REVISION,
                RENT_PRINCIPAL,
            ),
        ),
        (
            "rent_beneficiary",
            projected_tail(
                header_word(254),
                [1; 32],
                MARKET,
                [0; 32],
                INITIAL_REVISION,
                RENT_PRINCIPAL,
            ),
        ),
        (
            "historical_rent_principal",
            projected_tail(
                header_word(254),
                [1; 32],
                MARKET,
                [3; 32],
                INITIAL_REVISION,
                0,
            ),
        ),
    ] {
        assert_eq!(
            FractionalRootV1::decode(&tail),
            None,
            "a zero {label} must refuse"
        );
    }
}

/// A zero bump decodes, which is the trap worth naming out loud.
///
/// Unlike the other two missing values, the bump has no nonzero requirement, so
/// an activation that wrote a zero bump would produce a root that DECODES and
/// only misbehaves later, wherever the canonical bump is expected to be the
/// real one. That is strictly worse than a refusal, and it is the reason the
/// bump cannot simply be defaulted and left for someone else to notice.
#[test]
fn a_zero_bump_decodes_and_is_therefore_the_dangerous_one() {
    let zero_bump = projected_tail(
        header_word(0),
        [1; 32],
        MARKET,
        [3; 32],
        INITIAL_REVISION,
        RENT_PRINCIPAL,
    );
    let decoded = FractionalRootV1::decode(&zero_bump).expect("a zero bump still decodes");
    assert_eq!(decoded.input().bump, 0);
    // The same tail with the real bump is a different root, and nothing in the
    // decode path can tell you which one you were supposed to have.
    let real_bump = projected_tail(
        header_word(254),
        [1; 32],
        MARKET,
        [3; 32],
        INITIAL_REVISION,
        RENT_PRINCIPAL,
    );
    assert_ne!(FractionalRootV1::decode(&real_bump), Some(decoded));
}
