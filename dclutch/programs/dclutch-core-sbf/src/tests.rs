//! Hostile adapter dispatch and exact transaction-envelope tests.

extern crate std;

use std::{boxed::Box, vec, vec::Vec};

use dclutch_market::{
    Action, CAPABILITY_FUNDING_HEADER_BYTES_V2, CORE_EFFECT_ENVELOPE_BYTES_V1,
    CapabilityFundingHeaderV1, CapabilityFundingHeaderV2, CoreEffectActionV1, CoreEffectEnvelopeV1,
    Identity, REQUEST_BYTES, Request, Role, SeriesCoreRequestV1,
};
use dclutch_registry::release_set::{
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1, CapabilityExecutionSelectionV1,
};
use dclutch_source::{
    ContentId, Error as SourceError, RecoveryAttemptV2, RecoveryPolicyV2, SourceMaterialV3,
    SourceResolutionPhaseV1, SourceResolutionStateV2, WindowKind, WindowSpecV1,
};
use solana_hash::Hash;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    CAPABILITY_PREFIX_BYTES_V1, CAPABILITY_ROLE_PREFIX_BYTES_V2, CoreSbfError, process_instruction,
    resolution::ComposedResolutionActionV1,
};

const PACKET_DATA_BYTES: usize = 1_232;
const MAX_FUNDING_ACCOUNTS: usize = 16;
// GEN-V3ACT-r (2026-08-27, board): the child tail Core forwards verbatim
// (`capability.rs::invoke_child`) becomes Trading's own `family_accounts`
// slice, and `outer.rs`'s `AuthenticatedSuffixV2::parse` refuses any frame
// narrower than `AUTHENTICATION_ACCOUNTS_V1 + generation.extra_accounts()`.
// There are TWO floors, not one: 16 for `FlatDescriptor`, 18 for
// `ProgramSet` (the two extra accounts are the selected descriptor's raw
// record and its staging cursor, `SET_DESCRIPTOR_RAW`/`_STAGING`). General
// is a `ProgramSet` family — its own bundle emits a "one-entry ProgramSetV2
// selecting General's descriptor"
// (`trading-sbf/program-test/bundle-builder/src/general.rs`) — so 18, not
// 16, is the floor that applies to this measurement. The prior value of 3
// measured a frame `AuthenticatedSuffixV2` itself refuses.
//
// This is still a FLOOR, not General's maximum: a real General activation
// carries the admitted-AOT strategy extras in the same tail (eight
// authenticated records plus one Trading caller authority per register-bank
// page plus the accelerator program and its ProgramData — `general.rs`
// boundary item 2), none of which is modelled here. Read the claim below as
// "the narrowest frame General's dispatch will accept still fits one lookup
// v0 packet", not as a bound on the widest one.
const STANDARD_GENERAL_CHILD_TAIL_ACCOUNTS: usize = 18;
const GENERIC_FIXED_ACCOUNTS: usize = 14;
// Exact current General V2 activation request width used by this physical
// profile measurement. General remains the semantic owner of those bytes.
const MEASURED_GENERAL_V2_REQUEST_BYTES: usize = 256;

fn identity(byte: u8) -> Identity {
    Identity::new([byte; 32]).expect("nonzero identity")
}

fn valid_capability_instruction() -> Vec<u8> {
    let market = identity(1);
    let request = Request::administrative(Action::ActivateCapability, 7, market)
        .encode()
        .expect("request");
    let selection =
        CapabilityExecutionSelectionV1::from_bytes(0, [2; 32], [3; 32], [4; 32], [5; 32])
            .expect("selection")
            .to_bytes();
    let header = CapabilityFundingHeaderV2::new(1, 1, 1)
        .expect("header")
        .encode();
    let family_request = [42];
    let role_request_bytes =
        u32::try_from(selection.len() + header.len() + family_request.len()).expect("role width");
    let envelope = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::ActivateCapability,
        Role::Trading,
        identity(6),
        identity(7),
        identity(8),
        market,
        identity(9),
        identity(10),
        identity(11),
        7,
        0,
        0,
        role_request_bytes,
    )
    .expect("envelope")
    .encode()
    .expect("envelope bytes");
    let mut output = Vec::with_capacity(
        request.len() + envelope.len() + selection.len() + header.len() + family_request.len(),
    );
    output.extend_from_slice(&request);
    output.extend_from_slice(&envelope);
    output.extend_from_slice(&selection);
    output.extend_from_slice(&header);
    output.extend_from_slice(&family_request);
    output
}

fn account(key: Pubkey) -> AccountInfo<'static> {
    AccountInfo::new(
        Box::leak(Box::new(key)),
        false,
        false,
        Box::leak(Box::new(0)),
        Box::leak(Vec::new().into_boxed_slice()),
        Box::leak(Box::new(Pubkey::default())),
        false,
    )
}

#[test]
fn truncated_instruction_refuses_before_account_access() {
    assert_eq!(
        process_instruction(&Pubkey::new_unique(), &[], &[0; 71]),
        Err(ProgramError::Custom(CoreSbfError::Instruction as u32))
    );
}

#[test]
fn non_consume_series_action_refuses_before_account_access() {
    let request = SeriesCoreRequestV1::close(identity(1), identity(2), identity(3), 4, 5)
        .expect("canonical historical Close")
        .encode()
        .expect("Series request");
    assert_eq!(
        process_instruction(&Pubkey::new_unique(), &[], &request),
        Err(ProgramError::Custom(CoreSbfError::Instruction as u32))
    );
}

#[test]
fn noncanonical_funding_header_refuses_before_account_access() {
    let mut instruction = valid_capability_instruction();
    let header_start = CAPABILITY_PREFIX_BYTES_V1 + CAPABILITY_EXECUTION_SELECTION_BYTES_V1;
    let reserved = header_start + CAPABILITY_FUNDING_HEADER_BYTES_V2 - 1;
    let byte = instruction.get_mut(reserved).expect("reserved header byte");
    *byte = 1;
    assert_eq!(
        process_instruction(&Pubkey::new_unique(), &[], &instruction),
        Err(ProgramError::Custom(CoreSbfError::Instruction as u32))
    );
}

#[test]
fn legacy_funding_header_refuses_at_v2_dispatch_before_account_access() {
    let mut instruction = valid_capability_instruction();
    let header_start = CAPABILITY_PREFIX_BYTES_V1 + CAPABILITY_EXECUTION_SELECTION_BYTES_V1;
    let header_end = header_start + CAPABILITY_FUNDING_HEADER_BYTES_V2;
    instruction
        .get_mut(header_start..header_end)
        .expect("legacy header span")
        .copy_from_slice(
            &CapabilityFundingHeaderV1::new(1)
                .expect("legacy header")
                .encode(),
        );
    assert_eq!(
        process_instruction(&Pubkey::new_unique(), &[], &instruction),
        Err(ProgramError::Custom(CoreSbfError::Instruction as u32))
    );
}

#[test]
fn unknown_funding_header_refuses_at_v2_dispatch_before_account_access() {
    let mut instruction = valid_capability_instruction();
    let header_start = CAPABILITY_PREFIX_BYTES_V1 + CAPABILITY_EXECUTION_SELECTION_BYTES_V1;
    *instruction
        .get_mut(header_start)
        .expect("header magic byte") ^= 0xff;
    assert_eq!(
        process_instruction(&Pubkey::new_unique(), &[], &instruction),
        Err(ProgramError::Custom(CoreSbfError::Instruction as u32))
    );
}

#[test]
fn aliased_outer_accounts_refuse_before_state_or_child_access() {
    let duplicate = Pubkey::new_unique();
    let accounts = vec![account(duplicate), account(duplicate)];
    assert_eq!(
        process_instruction(
            &Pubkey::new_unique(),
            &accounts,
            &valid_capability_instruction()
        ),
        Err(ProgramError::Custom(CoreSbfError::AccountFrame as u32))
    );
}

#[test]
fn maximum_profile_general_activation_fits_one_lookup_v0_packet() {
    let payer = Pubkey::new_from_array([1; 32]);
    let program_id = Pubkey::new_from_array([2; 32]);
    let account_count =
        GENERIC_FIXED_ACCOUNTS + MAX_FUNDING_ACCOUNTS + STANDARD_GENERAL_CHILD_TAIL_ACCOUNTS;
    assert_eq!(account_count, 48);
    let addresses = (0..account_count)
        .map(|index| Pubkey::new_from_array([u8::try_from(index + 3).expect("key"); 32]))
        .collect::<Vec<_>>();
    let accounts = addresses
        .iter()
        .map(|key| AccountMeta::new_readonly(*key, false))
        .collect::<Vec<_>>();
    let role_bytes = CAPABILITY_ROLE_PREFIX_BYTES_V2 + MEASURED_GENERAL_V2_REQUEST_BYTES;
    assert_eq!(role_bytes, 416);
    let instruction_bytes = REQUEST_BYTES + CORE_EFFECT_ENVELOPE_BYTES_V1 + role_bytes;
    assert_eq!(instruction_bytes, 768);
    let instruction = Instruction {
        program_id,
        accounts,
        data: vec![0; instruction_bytes],
    };
    let blockhash = Hash::new_from_array([255; 32]);
    let uncompressed =
        v0::Message::try_compile(&payer, core::slice::from_ref(&instruction), &[], blockhash)
            .expect("uncompressed v0 message");
    let uncompressed_bytes = 1
        + usize::from(uncompressed.header.num_required_signatures) * 64
        + VersionedMessage::V0(uncompressed).serialize().len();
    let compressed = v0::Message::try_compile(
        &payer,
        &[instruction],
        &[AddressLookupTableAccount {
            key: Pubkey::new_from_array([254; 32]),
            addresses,
        }],
        blockhash,
    )
    .expect("lookup v0 message");
    let compressed_bytes = 1
        + usize::from(compressed.header.num_required_signatures) * 64
        + VersionedMessage::V0(compressed).serialize().len();
    assert_eq!(uncompressed_bytes, 2_524);
    assert_eq!(compressed_bytes, 1_070);
    assert!(compressed_bytes <= PACKET_DATA_BYTES);
}

/// Liveness census R2 / queue Q2 — the weld is gone, and this is why.
///
/// The weld -- a predicate named recovery-walk-has-a-live-route, deleted with
/// this rewrite, so the name is history rather than a pointer -- refused
/// `CreateFund`
/// over a recovery-bearing material because such a market had no terminal at
/// all: the primary exhaustion refuses that material by name, and nothing could
/// advance the attempt it was refusing on behalf of. Its own docstring said it
/// returned `true` again "the moment the ladder gets a live route", and that
/// deleting it was then the whole of the revert.
///
/// This test is the premise, re-executed in the new direction. It does not cite
/// the ladder; it walks it. If somebody deletes the crank, the second half goes
/// red here, beside the founding this weld used to guard.
#[test]
fn a_recovery_bearing_market_now_has_the_terminal_the_weld_was_protecting_it_from() {
    fn content(tag: u8) -> ContentId {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        ContentId::new(bytes).expect("nonzero Source content ID")
    }
    fn market(tag: u8) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        bytes
    }
    fn primary_state() -> SourceResolutionStateV2 {
        SourceResolutionStateV2::fresh(market(1), 9, content(2), market(3), 7, 0, 0)
            .expect("fresh Primary resolution state")
            .state()
    }

    // A terminal window whose deadline is `end + max_age` = 1_000_600.
    let window = WindowSpecV1::new(
        content(4),
        WindowKind::Terminal,
        1_000_000 - 600,
        1_000_000,
        600,
        1,
        content(9),
    )
    .expect("terminal window");
    let policy = RecoveryPolicyV2::new(
        content(0x60),
        [
            Some(
                RecoveryAttemptV2::new(content(0x61), content(0x62), 1_002_000, content(0x63))
                    .expect("attempt"),
            ),
            None,
            None,
            None,
        ],
        1,
    )
    .expect("one-attempt policy");
    let policy_id = content(8);
    let bought_recovery = SourceMaterialV3::explicitly_unbounded(
        content(3),
        content(4),
        content(5),
        content(6),
        Some(policy_id),
        content(7),
    );
    let bought_none = SourceMaterialV3::explicitly_unbounded(
        content(3),
        content(4),
        content(5),
        content(6),
        None,
        content(7),
    );

    // Half one, unchanged: the primary exhaustion still refuses a market that
    // bought alternative sources, and still for the reason it always gave.
    // Skipping paid-for legs would take an outcome away from the holders who
    // paid for them, so the ladder replaces the refusal's ABSENT sibling rather
    // than the refusal.
    let mut walkable = primary_state();
    assert_eq!(
        walkable.exhaust_after_primary_deadline(
            content(2),
            bought_none,
            content(5),
            window,
            9,
            1_000_601,
        ),
        Ok(())
    );
    let mut stranded = primary_state();
    assert_eq!(
        stranded.exhaust_after_primary_deadline(
            content(2),
            bought_recovery,
            content(5),
            window,
            9,
            i64::MAX,
        ),
        Err(SourceError::RecoveryNotExhausted),
        "the primary exhaustion is still not a recovery market's terminal"
    );

    // Half two, and this is what changed: the same market walks its own ladder
    // to `Exhausted`, which is where the failure commit begins. It is no longer
    // stranded, so `CreateFund` no longer has to refuse to mint it.
    let mut ladder = primary_state();
    assert!(
        ladder
            .crank_recovery_ladder(
                content(2),
                bought_recovery,
                content(5),
                window,
                policy_id,
                policy,
                9,
                1_000_601,
            )
            .is_ok(),
        "the primary window closed and the funded alternative is enterable"
    );
    assert_eq!(ladder.phase(), SourceResolutionPhaseV1::Recovery);
    assert!(
        ladder
            .crank_recovery_ladder(
                content(2),
                bought_recovery,
                content(5),
                window,
                policy_id,
                policy,
                9,
                1_002_001,
            )
            .is_ok(),
        "and the last funded window closed"
    );
    assert_eq!(ladder.phase(), SourceResolutionPhaseV1::Exhausted);
}

/// Every named admissible-prestate constant against the exact inline condition/// Every named admissible-prestate constant against the exact inline condition
/// it replaced.
///
/// The named constants are the guards themselves, so this is not a second
/// implementation kept in step with a first: it is the substitution's
/// receipt. Each case writes out the boolean expression that stood at the
/// guard site before the constant existed, and asserts agreement over all
/// fifteen `(Phase, Readiness)` prestates. A rename that widened a set by one
/// prestate would compile, pass every program test whose fixture never
/// reaches that prestate, and fail here.
mod admissible_prestates {
    use dclutch_market::{MarketAdmissionV1, Phase, Readiness};

    use crate::{
        execute_provider_v3::EXECUTE_PROVIDER_ADMISSIBLE_PRESTATES_V1,
        generic_founding_v1::GENERIC_FOUNDING_OPEN_ADMISSIBLE_PRESTATES_V1,
        open_market::OPEN_MARKET_ADMISSIBLE_PRESTATES_V1,
        resolution::{
            ADMIT_TERMINAL_ADMISSIBLE_PRESTATES_V1, CREATE_FUND_ADMISSIBLE_PRESTATES_V1,
            VERIFY_FUND_READY_ADMISSIBLE_PRESTATES_V1,
        },
        retire_v1::{
            RETIRE_ADMISSIBLE_PRESTATES_V1, RETIRE_CHECKPOINT_SUFFIX_ADMISSIBLE_PRESTATES_V1,
        },
        retirement_replay_handoff_v1::CORE_RETIREMENT_REPLAY_HANDOFF_ADMISSIBLE_PRESTATES_V1,
        series_open::SERIES_OPEN_ADMISSIBLE_PRESTATES_V1,
    };

    const ALL_PHASES: [Phase; 5] = [
        Phase::Founding,
        Phase::Open,
        Phase::Terminal,
        Phase::Retiring,
        Phase::Retired,
    ];
    const ALL_READINESS: [Readiness; 3] =
        [Readiness::Prepaid, Readiness::Ready, Readiness::Consumed];

    /// Assert `declared` admits exactly the prestates `inline` accepts.
    fn agrees(name: &str, declared: MarketAdmissionV1, inline: impl Fn(Phase, Readiness) -> bool) {
        let mut admitted = 0;
        for phase in ALL_PHASES {
            for readiness in ALL_READINESS {
                assert_eq!(
                    declared.admits(phase, readiness),
                    inline(phase, readiness),
                    "{name} disagrees with the condition it replaced at {phase:?}/{readiness:?}"
                );
                if declared.admits(phase, readiness) {
                    admitted += 1;
                }
            }
        }
        assert!(admitted > 0, "{name} admits nothing at all");
    }

    #[test]
    fn open_market_admits_what_its_inline_guard_admitted() {
        agrees(
            "OPEN_MARKET_ADMISSIBLE_PRESTATES_V1",
            OPEN_MARKET_ADMISSIBLE_PRESTATES_V1,
            |phase, readiness| !(phase != Phase::Founding || readiness != Readiness::Ready),
        );
    }

    #[test]
    fn execute_provider_admits_what_its_inline_guard_admitted() {
        agrees(
            "EXECUTE_PROVIDER_ADMISSIBLE_PRESTATES_V1",
            EXECUTE_PROVIDER_ADMISSIBLE_PRESTATES_V1,
            |phase, readiness| !(phase != Phase::Open || readiness != Readiness::Consumed),
        );
    }

    #[test]
    fn resolution_fund_actions_admit_what_their_inline_guards_admitted() {
        // `CreateFund`'s inline guard also required an absent terminal receipt.
        // That conjunct is not a prestate and stays at the guard; what is
        // declared here is the `matches!` half, exactly.
        agrees(
            "CREATE_FUND_ADMISSIBLE_PRESTATES_V1",
            CREATE_FUND_ADMISSIBLE_PRESTATES_V1,
            |phase, readiness| {
                matches!(
                    (phase, readiness),
                    (Phase::Founding, Readiness::Prepaid) | (Phase::Open, Readiness::Consumed)
                )
            },
        );
        agrees(
            "VERIFY_FUND_READY_ADMISSIBLE_PRESTATES_V1",
            VERIFY_FUND_READY_ADMISSIBLE_PRESTATES_V1,
            |phase, readiness| {
                matches!(
                    (phase, readiness),
                    (Phase::Founding, Readiness::Prepaid)
                        | (Phase::Founding, Readiness::Ready)
                        | (Phase::Open, Readiness::Consumed)
                )
            },
        );
        agrees(
            "ADMIT_TERMINAL_ADMISSIBLE_PRESTATES_V1",
            ADMIT_TERMINAL_ADMISSIBLE_PRESTATES_V1,
            |phase, readiness| {
                matches!(phase, Phase::Open | Phase::Terminal) && readiness == Readiness::Consumed
            },
        );
    }

    #[test]
    fn founding_open_stages_admit_what_their_inline_guards_admitted() {
        for (name, declared) in [
            (
                "GENERIC_FOUNDING_OPEN_ADMISSIBLE_PRESTATES_V1",
                GENERIC_FOUNDING_OPEN_ADMISSIBLE_PRESTATES_V1,
            ),
            (
                "SERIES_OPEN_ADMISSIBLE_PRESTATES_V1",
                SERIES_OPEN_ADMISSIBLE_PRESTATES_V1,
            ),
        ] {
            agrees(name, declared, |phase, readiness| {
                !(!matches!(phase, Phase::Founding) || readiness != Readiness::Prepaid)
            });
        }
    }

    #[test]
    fn retirement_routes_admit_what_their_inline_guards_admitted() {
        // These three guards named no readiness, so every readiness in
        // `Retiring` was admitted and the declaration says so.
        for (name, declared) in [
            (
                "RETIRE_ADMISSIBLE_PRESTATES_V1",
                RETIRE_ADMISSIBLE_PRESTATES_V1,
            ),
            (
                "RETIRE_CHECKPOINT_SUFFIX_ADMISSIBLE_PRESTATES_V1",
                RETIRE_CHECKPOINT_SUFFIX_ADMISSIBLE_PRESTATES_V1,
            ),
            (
                "CORE_RETIREMENT_REPLAY_HANDOFF_ADMISSIBLE_PRESTATES_V1",
                CORE_RETIREMENT_REPLAY_HANDOFF_ADMISSIBLE_PRESTATES_V1,
            ),
        ] {
            agrees(name, declared, |phase, _readiness| {
                !(phase != Phase::Retiring)
            });
            for phase in ALL_PHASES {
                assert_eq!(
                    declared.admits_phase(phase),
                    phase == Phase::Retiring,
                    "{name} phase projection disagrees at {phase:?}"
                );
            }
        }
    }
}
