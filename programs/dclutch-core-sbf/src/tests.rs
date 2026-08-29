//! Hostile adapter dispatch and exact transaction-envelope tests.

extern crate std;

use std::{boxed::Box, vec, vec::Vec};

use dclutch_market_core_codec::{
    Action, CAPABILITY_FUNDING_HEADER_BYTES_V2, CORE_EFFECT_ENVELOPE_BYTES_V1,
    CapabilityFundingHeaderV1, CapabilityFundingHeaderV2, CoreEffectActionV1, CoreEffectEnvelopeV1,
    Identity, REQUEST_BYTES, Request, Role, SeriesCoreRequestV1,
};
use dclutch_release_set_contract::{
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1, CapabilityExecutionSelectionV1,
};
use dclutch_resolution_codec::ResolutionCoreActionV1;
use dclutch_source_contract::{
    ContentId, Error as SourceError, SourceMaterialV3, SourceResolutionStateV2, WindowKind,
    WindowSpecV1,
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
    resolution::recovery_walk_has_a_live_route,
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

/// Liveness census R2 / queue Q2 — the weld and the fact that justifies it.
///
/// The weld (`resolution::recovery_walk_has_a_live_route`) is only defensible
/// while the recovery walk is genuinely unwalkable, so this test re-executes
/// that premise here, beside the weld, rather than citing it. If somebody makes
/// the ordered ladder live, the first two assertions change shape and this test
/// goes red pointing at the conjunct to delete — which is exactly the signal a
/// weld should carry.
#[test]
fn create_fund_refuses_a_material_whose_recovery_walk_no_route_can_walk() {
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
    let bought_recovery = SourceMaterialV3::explicitly_unbounded(
        content(3),
        content(4),
        content(5),
        content(6),
        Some(content(8)),
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

    // The premise, both halves. A no-recovery market walks to Exhausted one
    // second past the deadline; a recovery market is refused there forever, and
    // the ladder that was meant to serve it has no live route to serve it with
    // (`funded::process_funded_transition`'s only call site is under
    // `#[cfg(any())]`). So the second market has no terminal at all.
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
            1_000_601,
        ),
        Err(SourceError::RecoveryNotExhausted),
        "the premise of the Q2 weld: a recovery market cannot be walked to failure"
    );
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
        "and no later second changes that — the strand is permanent, not a wait"
    );

    // Therefore CreateFund does not mint that state.
    assert!(!recovery_walk_has_a_live_route(
        ResolutionCoreActionV1::CreateFund
    ));
    assert_eq!(CoreSbfError::RecoveryWalkUnavailable as u32, 0x3011);
}

/// A weld may not strand what it finds: it refuses creation, never an exit.
#[test]
fn the_recovery_weld_takes_no_route_from_a_fund_that_already_exists() {
    for action in [
        ResolutionCoreActionV1::VerifyFundReady,
        ResolutionCoreActionV1::CloseFund,
        ResolutionCoreActionV1::AdmitTerminal,
    ] {
        assert!(
            recovery_walk_has_a_live_route(action),
            "welding {action:?} would remove a route from an existing state"
        );
    }
    assert_ne!(
        CoreSbfError::RecoveryWalkUnavailable as u32,
        CoreSbfError::ReleaseSuperseded as u32
    );
    assert!(
        (CoreSbfError::RecoveryWalkUnavailable as u32) < 0x4000
            && (CoreSbfError::RecoveryWalkUnavailable as u32) >= 0x3000,
        "the weld's refusal must stay inside Core's registered band"
    );
}
