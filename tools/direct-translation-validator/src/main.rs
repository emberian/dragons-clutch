//! Independent executable translation validator for the Lean-owned Direct ABI
//! and transition program.

use std::{collections::BTreeMap, env, error::Error, fmt, fs, path::Path};

use dclutch_direct_aot_contract::{
    DIRECT_PROGRAM_V2_IDENTITIES, DIRECT_PROGRAM_V2_SCALARS, RegisterInput as AotInput,
    RegisterOutput as AotOutput, execute_atomic as execute_aot,
};
use dclutch_direct_codec::{
    COMPACT_INTENT_BYTES, CONTROLLER_INSTRUCTION_BYTES, CompactIntentV1, ControllerInstructionV1,
    REGISTERED_CLAIM_TERMINAL_BYTES, REGISTERED_CREATE_INSTRUCTION_BYTES,
    REGISTERED_TERMINAL_INSTRUCTION_BYTES, RegisteredCreateInstructionV1, RegisteredTerminalAction,
    RegisteredTerminalInstructionV1, registered_claim_terminal_instruction,
};
use dclutch_transition_vm::{Registers, execute};

#[cfg(kani)]
mod kani_proofs;

mod registration;
mod terminal;

mod generated_program {
    #![allow(dead_code, unused_imports, unused_macros)]

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../programs/dclutch-controller-proof-sbf/src/generated_direct_program.rs"
    ));

    pub(super) fn bytes() -> &'static [u8] {
        &DIRECT_PROGRAM
    }
}

#[derive(Debug)]
struct Failure(String);

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for Failure {}

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Default)]
struct Statistics {
    intents: usize,
    controllers: usize,
    registered_creations: usize,
    registered_creation_mutations: usize,
    registered_creation_hostile_widths: usize,
    terminal_controllers: usize,
    terminal_claims: usize,
    abi_mutations: usize,
    abi_hostile_widths: usize,
    vm_cases: usize,
    vm_accepts: usize,
    vm_refusals: usize,
    aot_cases: usize,
    aot_accepts: usize,
    aot_refusals: usize,
    terminal_transitions: usize,
    terminal_accepts: usize,
    terminal_refusals: usize,
    creation_transitions: usize,
    creation_accepts: usize,
    creation_refusals: usize,
    rust_roundtrips: usize,
}

fn failure(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(Failure(message.into()))
}

fn field<'a>(fields: &'a [&str], index: usize, context: &str) -> Result<&'a str> {
    fields
        .get(index)
        .copied()
        .ok_or_else(|| failure(format!("{context}: missing field {index}")))
}

fn parse_u64(value: &str, context: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .map_err(|error| failure(format!("{context}: invalid u64 {value:?}: {error}")))
}

fn parse_u16(value: &str, context: &str) -> Result<u16> {
    value
        .parse::<u16>()
        .map_err(|error| failure(format!("{context}: invalid u16 {value:?}: {error}")))
}

fn parse_u8(value: &str, context: &str) -> Result<u8> {
    value
        .parse::<u8>()
        .map_err(|error| failure(format!("{context}: invalid u8 {value:?}: {error}")))
}

fn parse_bool01(value: &str, context: &str) -> Result<bool> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(failure(format!("{context}: invalid bit {value:?}"))),
    }
}

fn hex_nibble(byte: u8, context: &str) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(failure(format!(
            "{context}: noncanonical lowercase hex byte {byte:?}"
        ))),
    }
}

fn decode_hex(value: &str, context: &str) -> Result<Vec<u8>> {
    let input = value.as_bytes();
    if !input.len().is_multiple_of(2) {
        return Err(failure(format!("{context}: odd hex length")));
    }
    input
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0], context)?;
            let low = hex_nibble(pair[1], context)?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn decode_array<const N: usize>(value: &str, context: &str) -> Result<[u8; N]> {
    decode_hex(value, context)?
        .try_into()
        .map_err(|bytes: Vec<u8>| {
            failure(format!(
                "{context}: expected {N} bytes, observed {}",
                bytes.len()
            ))
        })
}

fn parse_csv(value: &str, context: &str) -> Result<Vec<u64>> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .enumerate()
        .map(|(index, item)| parse_u64(item, &format!("{context}[{index}]")))
        .collect()
}

fn changed_byte(byte: u8) -> u8 {
    byte.wrapping_add(1)
}

fn require_equal<T: PartialEq + fmt::Debug>(actual: T, expected: T, context: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(failure(format!(
            "{context}: mismatch\n  actual: {actual:?}\nexpected: {expected:?}"
        )))
    }
}

fn validate_intent(
    fields: &[&str],
    intents: &mut BTreeMap<String, CompactIntentV1>,
    intent_bytes: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let context = format!("intent {}", field(fields, 1, "intent")?);
    if fields.len() != 15 {
        return Err(failure(format!(
            "{context}: expected 15 fields, observed {}",
            fields.len()
        )));
    }
    let name = fields[1].to_owned();
    let intent = CompactIntentV1 {
        side: parse_u8(fields[2], &context)?,
        outcome: parse_u8(fields[3], &context)?,
        lifecycle: parse_u8(fields[4], &context)?,
        market: decode_array(fields[5], &context)?,
        generation: parse_u64(fields[6], &context)?,
        nonce: parse_u64(fields[7], &context)?,
        valid_from: parse_u64(fields[8], &context)?,
        valid_through: parse_u64(fields[9], &context)?,
        maximum_fill: parse_u64(fields[10], &context)?,
        limit_price: parse_u64(fields[11], &context)?,
        fee_basis_points: parse_u16(fields[12], &context)?,
        collateral_account: decode_array(fields[13], &context)?,
    };
    let expected = decode_hex(fields[14], &context)?;
    require_equal(expected.len(), COMPACT_INTENT_BYTES, &context)?;
    let encoded = intent
        .encode()
        .map_err(|error| failure(format!("{context}: safe Rust encoder refused: {error:?}")))?;
    require_equal(encoded.as_slice(), expected.as_slice(), &context)?;
    let decoded = CompactIntentV1::decode(&expected)
        .map_err(|error| failure(format!("{context}: safe Rust decoder refused: {error:?}")))?;
    require_equal(decoded, intent, &context)?;
    if intents.insert(name.clone(), intent).is_some()
        || intent_bytes.insert(name, expected).is_some()
    {
        return Err(failure(format!("{context}: duplicate name")));
    }
    Ok(())
}

fn validate_intent_mutation(
    fields: &[&str],
    intent_bytes: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    if fields.len() != 4 {
        return Err(failure("intent mutation: wrong field count"));
    }
    let context = format!("intent mutation {} byte {}", fields[1], fields[2]);
    let mut bytes = intent_bytes
        .get(fields[1])
        .cloned()
        .ok_or_else(|| failure(format!("{context}: unknown intent")))?;
    let offset = fields[2]
        .parse::<usize>()
        .map_err(|error| failure(format!("{context}: invalid offset: {error}")))?;
    let byte = bytes
        .get_mut(offset)
        .ok_or_else(|| failure(format!("{context}: offset out of bounds")))?;
    *byte = changed_byte(*byte);
    let actual = CompactIntentV1::decode(&bytes).is_ok();
    let expected = fields[3] == "accept";
    if fields[3] != "accept" && fields[3] != "reject" {
        return Err(failure(format!("{context}: invalid Lean disposition")));
    }
    require_equal(actual, expected, &context)
}

fn validate_controller(
    fields: &[&str],
    intents: &BTreeMap<String, CompactIntentV1>,
    controller_bytes: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let context = format!("controller {}", field(fields, 1, "controller")?);
    if fields.len() != 12 {
        return Err(failure(format!(
            "{context}: expected 12 fields, observed {}",
            fields.len()
        )));
    }
    let name = fields[1].to_owned();
    let seller = *intents
        .get(fields[9])
        .ok_or_else(|| failure(format!("{context}: unknown seller intent")))?;
    let buyer = *intents
        .get(fields[10])
        .ok_or_else(|| failure(format!("{context}: unknown buyer intent")))?;
    let instruction = ControllerInstructionV1 {
        controller_bump: parse_u8(fields[2], &context)?,
        seller_replay_bump: parse_u8(fields[3], &context)?,
        buyer_replay_bump: parse_u8(fields[4], &context)?,
        seller_position_bump: parse_u8(fields[5], &context)?,
        buyer_position_bump: parse_u8(fields[6], &context)?,
        fill: parse_u64(fields[7], &context)?,
        execution_price: parse_u64(fields[8], &context)?,
        seller,
        buyer,
    };
    let expected = decode_hex(fields[11], &context)?;
    require_equal(expected.len(), CONTROLLER_INSTRUCTION_BYTES, &context)?;
    let encoded = instruction
        .encode()
        .map_err(|error| failure(format!("{context}: safe Rust encoder refused: {error:?}")))?;
    require_equal(encoded.as_slice(), expected.as_slice(), &context)?;
    let decoded = ControllerInstructionV1::decode(&expected)
        .map_err(|error| failure(format!("{context}: safe Rust decoder refused: {error:?}")))?;
    require_equal(decoded, instruction, &context)?;
    if controller_bytes.insert(name, expected).is_some() {
        return Err(failure(format!("{context}: duplicate name")));
    }
    Ok(())
}

fn validate_controller_mutation(
    fields: &[&str],
    controller_bytes: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    if fields.len() != 4 {
        return Err(failure("controller mutation: wrong field count"));
    }
    let context = format!("controller mutation {} byte {}", fields[1], fields[2]);
    let mut bytes = controller_bytes
        .get(fields[1])
        .cloned()
        .ok_or_else(|| failure(format!("{context}: unknown controller")))?;
    let offset = fields[2]
        .parse::<usize>()
        .map_err(|error| failure(format!("{context}: invalid offset: {error}")))?;
    let byte = bytes
        .get_mut(offset)
        .ok_or_else(|| failure(format!("{context}: offset out of bounds")))?;
    *byte = changed_byte(*byte);
    let actual = ControllerInstructionV1::decode(&bytes).is_ok();
    let expected = fields[3] == "accept";
    if fields[3] != "accept" && fields[3] != "reject" {
        return Err(failure(format!("{context}: invalid Lean disposition")));
    }
    require_equal(actual, expected, &context)
}

#[derive(Clone)]
struct RegisteredCreateCase {
    bytes: Vec<u8>,
}

fn validate_registered_create(
    fields: &[&str],
    cases: &mut BTreeMap<String, RegisteredCreateCase>,
) -> Result<()> {
    if fields.len() != 10 {
        return Err(failure("registered create: wrong field count"));
    }
    let context = format!("registered create {}", fields[1]);
    let market = decode_array(fields[5], &context)?;
    let generation = parse_u64(fields[6], &context)?;
    let nonce = parse_u64(fields[7], &context)?;
    let intent_bytes = decode_hex(fields[8], &context)?;
    require_equal(intent_bytes.len(), COMPACT_INTENT_BYTES, &context)?;
    let intent = CompactIntentV1::decode(&intent_bytes)
        .map_err(|error| failure(format!("{context}: intent decoder refused: {error:?}")))?;
    require_equal(
        intent.market,
        market,
        &format!("{context}: Market PDA projection"),
    )?;
    require_equal(
        intent.generation,
        generation,
        &format!("{context}: generation PDA projection"),
    )?;
    require_equal(
        intent.nonce,
        nonce,
        &format!("{context}: registration nonce projection"),
    )?;
    let instruction = RegisteredCreateInstructionV1 {
        controller_bump: parse_u8(fields[2], &context)?,
        replay_bump: parse_u8(fields[3], &context)?,
        registration_bump: parse_u8(fields[4], &context)?,
        intent,
    };
    let expected = decode_hex(fields[9], &context)?;
    require_equal(
        expected.len(),
        REGISTERED_CREATE_INSTRUCTION_BYTES,
        &context,
    )?;
    let encoded = instruction
        .encode()
        .map_err(|error| failure(format!("{context}: Rust encoder refused: {error:?}")))?;
    require_equal(encoded.as_slice(), expected.as_slice(), &context)?;
    require_equal(
        RegisteredCreateInstructionV1::decode(&expected),
        Ok(instruction),
        &context,
    )?;
    if cases
        .insert(
            fields[1].to_owned(),
            RegisteredCreateCase { bytes: expected },
        )
        .is_some()
    {
        return Err(failure(format!("{context}: duplicate name")));
    }
    Ok(())
}

fn validate_registered_create_mutation(
    fields: &[&str],
    cases: &BTreeMap<String, RegisteredCreateCase>,
) -> Result<()> {
    if fields.len() != 4 {
        return Err(failure("registered create mutation: wrong field count"));
    }
    let context = format!(
        "registered create mutation {} byte {}",
        fields[1], fields[2]
    );
    let case = cases
        .get(fields[1])
        .ok_or_else(|| failure(format!("{context}: unknown case")))?;
    let mut bytes = case.bytes.clone();
    let offset = fields[2]
        .parse::<usize>()
        .map_err(|error| failure(format!("{context}: invalid offset: {error}")))?;
    let byte = bytes
        .get_mut(offset)
        .ok_or_else(|| failure(format!("{context}: offset out of bounds")))?;
    *byte = changed_byte(*byte);
    let actual = RegisteredCreateInstructionV1::decode(&bytes).is_ok();
    require_equal(actual, disposition(fields[3], &context)?, &context)
}

fn validate_registered_create_hostile(
    fields: &[&str],
    cases: &BTreeMap<String, RegisteredCreateCase>,
) -> Result<()> {
    if fields.len() != 5 {
        return Err(failure("registered create hostile: wrong field count"));
    }
    let context = format!("registered create hostile {} {}", fields[1], fields[2]);
    if !cases.contains_key(fields[1]) {
        return Err(failure(format!("{context}: unknown case")));
    }
    let bytes = decode_hex(fields[3], &context)?;
    let actual = RegisteredCreateInstructionV1::decode(&bytes).is_ok();
    require_equal(actual, disposition(fields[4], &context)?, &context)
}

fn validate_registered_create_transition(
    fields: &[&str],
    statistics: &mut Statistics,
) -> Result<()> {
    if fields.len() != 18 && fields.len() != 26 {
        return Err(failure("registered create transition: wrong field count"));
    }
    let context = format!("registered create transition {}", fields[1]);
    let vacant = parse_bool01(fields[2], &context)?;
    let next_nonce = parse_u64(fields[16], &context)?;
    let before = registration::store(next_nonce, vacant);
    let request = registration::Request {
        market_phase: parse_u8(fields[3], &context)?,
        slot: parse_u64(fields[4], &context)?,
        market: parse_u64(fields[5], &context)?,
        generation: parse_u64(fields[6], &context)?,
        maker: parse_u64(fields[7], &context)?,
        nonce: parse_u64(fields[8], &context)?,
        valid_from: parse_u64(fields[9], &context)?,
        valid_through: parse_u64(fields[10], &context)?,
        maximum: parse_u64(fields[11], &context)?,
        outcome: parse_u64(fields[12], &context)?,
        outcome_count: parse_u64(fields[13], &context)?,
        intent_fee: parse_u64(fields[14], &context)?,
        policy_fee: parse_u64(fields[15], &context)?,
    };
    let expected_accept = disposition(fields[17], &context)?;
    let mut after = before;
    let actual_accept = registration::apply(&mut after, request);
    require_equal(actual_accept, expected_accept, &context)?;
    if expected_accept {
        if fields.len() != 26 {
            return Err(failure(format!("{context}: acceptance omitted post-state")));
        }
        let expected = registration::Store {
            next_nonce: parse_u64(fields[25], &context)?,
            registration: registration::RegistrationSlot::Occupied(registration::RegisteredState {
                phase: parse_u8(fields[18], &context)?,
                remaining: parse_u64(fields[19], &context)?,
                sequence: parse_u64(fields[20], &context)?,
                market: parse_u64(fields[21], &context)?,
                generation: parse_u64(fields[22], &context)?,
                maker: parse_u64(fields[23], &context)?,
                nonce: parse_u64(fields[24], &context)?,
            }),
        };
        require_equal(after, expected, &context)?;
        statistics.creation_accepts += 1;
    } else {
        if fields.len() != 18 {
            return Err(failure(format!("{context}: refusal carried post-state")));
        }
        require_equal(after, before, &format!("{context}: rollback"))?;
        statistics.creation_refusals += 1;
    }
    statistics.creation_transitions += 1;
    Ok(())
}

#[derive(Clone)]
struct TerminalControllerCase {
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct TerminalClaimCase {
    bytes: Vec<u8>,
}

fn terminal_actions(
    tag: &str,
    context: &str,
) -> Result<(terminal::Action, RegisteredTerminalAction)> {
    let tag = parse_u8(tag, context)?;
    let local = terminal::Action::from_tag(tag)
        .ok_or_else(|| failure(format!("{context}: unknown action tag {tag}")))?;
    let rust = match local {
        terminal::Action::Cancel => RegisteredTerminalAction::Cancel,
        terminal::Action::Expire => RegisteredTerminalAction::Expire,
    };
    Ok((local, rust))
}

fn disposition(value: &str, context: &str) -> Result<bool> {
    match value {
        "accept" => Ok(true),
        "reject" => Ok(false),
        _ => Err(failure(format!("{context}: invalid Lean disposition"))),
    }
}

fn validate_terminal_controller(
    fields: &[&str],
    cases: &mut BTreeMap<String, TerminalControllerCase>,
) -> Result<()> {
    if fields.len() != 7 {
        return Err(failure("terminal controller: wrong field count"));
    }
    let context = format!("terminal controller {}", fields[1]);
    let (_, action) = terminal_actions(fields[2], &context)?;
    let instruction = RegisteredTerminalInstructionV1 {
        action,
        controller_bump: parse_u8(fields[3], &context)?,
        registration_bump: parse_u8(fields[4], &context)?,
        expected_sequence: parse_u64(fields[5], &context)?,
    };
    let expected = decode_hex(fields[6], &context)?;
    require_equal(
        expected.len(),
        REGISTERED_TERMINAL_INSTRUCTION_BYTES,
        &context,
    )?;
    let encoded = instruction
        .encode()
        .map_err(|error| failure(format!("{context}: Rust encoder refused: {error:?}")))?;
    require_equal(encoded.as_slice(), expected.as_slice(), &context)?;
    require_equal(
        RegisteredTerminalInstructionV1::decode(&expected),
        Ok(instruction),
        &context,
    )?;
    if cases
        .insert(
            fields[1].to_owned(),
            TerminalControllerCase { bytes: expected },
        )
        .is_some()
    {
        return Err(failure(format!("{context}: duplicate name")));
    }
    Ok(())
}

fn validate_terminal_controller_mutation(
    fields: &[&str],
    cases: &BTreeMap<String, TerminalControllerCase>,
) -> Result<()> {
    if fields.len() != 4 {
        return Err(failure("terminal controller mutation: wrong field count"));
    }
    let context = format!(
        "terminal controller mutation {} byte {}",
        fields[1], fields[2]
    );
    let case = cases
        .get(fields[1])
        .ok_or_else(|| failure(format!("{context}: unknown case")))?;
    let mut bytes = case.bytes.clone();
    let offset = fields[2]
        .parse::<usize>()
        .map_err(|error| failure(format!("{context}: invalid offset: {error}")))?;
    let byte = bytes
        .get_mut(offset)
        .ok_or_else(|| failure(format!("{context}: offset out of bounds")))?;
    *byte = changed_byte(*byte);
    let actual = RegisteredTerminalInstructionV1::decode(&bytes).is_ok();
    require_equal(actual, disposition(fields[3], &context)?, &context)
}

fn validate_terminal_controller_hostile(
    fields: &[&str],
    cases: &BTreeMap<String, TerminalControllerCase>,
) -> Result<()> {
    if fields.len() != 5 {
        return Err(failure("terminal controller hostile: wrong field count"));
    }
    let context = format!("terminal controller hostile {} {}", fields[1], fields[2]);
    if !cases.contains_key(fields[1]) {
        return Err(failure(format!("{context}: unknown case")));
    }
    let bytes = decode_hex(fields[3], &context)?;
    let actual = RegisteredTerminalInstructionV1::decode(&bytes).is_ok();
    require_equal(actual, disposition(fields[4], &context)?, &context)
}

fn validate_terminal_claim(
    fields: &[&str],
    cases: &mut BTreeMap<String, TerminalClaimCase>,
) -> Result<()> {
    if fields.len() != 5 {
        return Err(failure("terminal claim: wrong field count"));
    }
    let context = format!("terminal claim {}", fields[1]);
    let (local_action, rust_action) = terminal_actions(fields[2], &context)?;
    let sequence = parse_u64(fields[3], &context)?;
    let expected = decode_hex(fields[4], &context)?;
    require_equal(expected.len(), REGISTERED_CLAIM_TERMINAL_BYTES, &context)?;
    let encoded = registered_claim_terminal_instruction(rust_action, sequence)
        .map_err(|error| failure(format!("{context}: Rust encoder refused: {error:?}")))?;
    require_equal(encoded.as_slice(), expected.as_slice(), &context)?;
    require_equal(
        terminal::decode_claim(&expected),
        Some((local_action, sequence)),
        &context,
    )?;
    if cases
        .insert(fields[1].to_owned(), TerminalClaimCase { bytes: expected })
        .is_some()
    {
        return Err(failure(format!("{context}: duplicate name")));
    }
    Ok(())
}

fn validate_terminal_claim_mutation(
    fields: &[&str],
    cases: &BTreeMap<String, TerminalClaimCase>,
) -> Result<()> {
    if fields.len() != 4 {
        return Err(failure("terminal claim mutation: wrong field count"));
    }
    let context = format!("terminal claim mutation {} byte {}", fields[1], fields[2]);
    let case = cases
        .get(fields[1])
        .ok_or_else(|| failure(format!("{context}: unknown case")))?;
    let mut bytes = case.bytes.clone();
    let offset = fields[2]
        .parse::<usize>()
        .map_err(|error| failure(format!("{context}: invalid offset: {error}")))?;
    let byte = bytes
        .get_mut(offset)
        .ok_or_else(|| failure(format!("{context}: offset out of bounds")))?;
    *byte = changed_byte(*byte);
    let actual = terminal::decode_claim(&bytes).is_some();
    require_equal(actual, disposition(fields[3], &context)?, &context)
}

fn validate_terminal_claim_hostile(
    fields: &[&str],
    cases: &BTreeMap<String, TerminalClaimCase>,
) -> Result<()> {
    if fields.len() != 5 {
        return Err(failure("terminal claim hostile: wrong field count"));
    }
    let context = format!("terminal claim hostile {} {}", fields[1], fields[2]);
    if !cases.contains_key(fields[1]) {
        return Err(failure(format!("{context}: unknown case")));
    }
    let bytes = decode_hex(fields[3], &context)?;
    let actual = terminal::decode_claim(&bytes).is_some();
    require_equal(actual, disposition(fields[4], &context)?, &context)
}

fn validate_terminal_transition(fields: &[&str], statistics: &mut Statistics) -> Result<()> {
    if fields.len() != 13 && fields.len() != 19 {
        return Err(failure("terminal transition: wrong field count"));
    }
    let context = format!("terminal transition {}", fields[1]);
    let (action, _) = terminal_actions(fields[2], &context)?;
    let before = terminal::State {
        phase: parse_u8(fields[3], &context)?,
        remaining: parse_u64(fields[4], &context)?,
        maximum: parse_u64(fields[5], &context)?,
        sequence: parse_u64(fields[6], &context)?,
        valid_through: parse_u64(fields[7], &context)?,
        maker: parse_u64(fields[10], &context)?,
    };
    let request = terminal::Request {
        action,
        slot: parse_u64(fields[8], &context)?,
        expected_sequence: parse_u64(fields[9], &context)?,
        actor_maker: parse_u64(fields[11], &context)?,
    };
    let expected_accept = disposition(fields[12], &context)?;
    let mut after = before;
    let actual_accept = terminal::apply(&mut after, request);
    require_equal(actual_accept, expected_accept, &context)?;
    if expected_accept {
        if fields.len() != 19 {
            return Err(failure(format!("{context}: acceptance omitted post-state")));
        }
        let expected = terminal::State {
            phase: parse_u8(fields[13], &context)?,
            remaining: parse_u64(fields[14], &context)?,
            maximum: parse_u64(fields[15], &context)?,
            sequence: parse_u64(fields[16], &context)?,
            valid_through: parse_u64(fields[17], &context)?,
            maker: parse_u64(fields[18], &context)?,
        };
        require_equal(after, expected, &context)?;
        statistics.terminal_accepts += 1;
    } else {
        if fields.len() != 13 {
            return Err(failure(format!("{context}: refusal carried post-state")));
        }
        require_equal(after, before, &format!("{context}: rollback"))?;
        statistics.terminal_refusals += 1;
    }
    statistics.terminal_transitions += 1;
    Ok(())
}

fn identity(value: u64) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&value.to_le_bytes());
    bytes
}

fn registers(scalars: &[u64], identities: &[u64], context: &str) -> Result<Registers> {
    let mut registers = Registers::zeroed();
    for (index, value) in scalars.iter().copied().enumerate() {
        registers
            .set_scalar(index, value)
            .map_err(|error| failure(format!("{context}: scalar {index}: {error:?}")))?;
    }
    for (index, value) in identities.iter().copied().enumerate() {
        registers
            .set_identity(index, identity(value))
            .map_err(|error| failure(format!("{context}: identity {index}: {error:?}")))?;
    }
    Ok(registers)
}

fn validate_registers(
    actual: &Registers,
    scalars: &[u64],
    identities: &[u64],
    context: &str,
) -> Result<()> {
    for (index, expected) in scalars.iter().copied().enumerate() {
        let observed = actual
            .scalar(index)
            .map_err(|error| failure(format!("{context}: scalar {index}: {error:?}")))?;
        require_equal(observed, expected, &format!("{context}: scalar {index}"))?;
    }
    for (index, expected) in identities.iter().copied().enumerate() {
        let observed = actual
            .identity(index)
            .map_err(|error| failure(format!("{context}: identity {index}: {error:?}")))?;
        require_equal(
            observed,
            identity(expected),
            &format!("{context}: identity {index}"),
        )?;
    }
    Ok(())
}

fn validate_vm_case(
    program: &[u8],
    name: &str,
    input_scalars_csv: &str,
    input_identities_csv: &str,
    disposition: &str,
    output: Option<(&str, &str)>,
    statistics: &mut Statistics,
) -> Result<()> {
    let context = format!("vm {name}");
    let input_scalars = parse_csv(input_scalars_csv, &format!("{context} input scalars"))?;
    let input_identities = parse_csv(input_identities_csv, &format!("{context} input identities"))?;
    let before = registers(&input_scalars, &input_identities, &context)?;
    let mut after = before;
    let result = execute(program, &mut after);
    match disposition {
        "reject" => {
            if output.is_some() {
                return Err(failure(format!("{context}: refusal carried post-state")));
            }
            if result.is_ok() {
                return Err(failure(format!("{context}: Rust accepted Lean refusal")));
            }
            require_equal(after, before, &format!("{context}: transactional refusal"))?;
            statistics.vm_refusals += 1;
        }
        "accept" => {
            let (output_scalars_csv, output_identities_csv) = output
                .ok_or_else(|| failure(format!("{context}: acceptance omitted post-state")))?;
            result.map_err(|error| {
                failure(format!(
                    "{context}: Rust refused Lean acceptance: {error:?}"
                ))
            })?;
            let output_scalars =
                parse_csv(output_scalars_csv, &format!("{context} output scalars"))?;
            let output_identities = parse_csv(
                output_identities_csv,
                &format!("{context} output identities"),
            )?;
            validate_registers(&after, &output_scalars, &output_identities, &context)?;
            statistics.vm_accepts += 1;
        }
        value => return Err(failure(format!("{context}: invalid disposition {value:?}"))),
    }
    statistics.vm_cases += 1;
    Ok(())
}

fn validate_direct_aot_case(
    name: &str,
    input_scalars_csv: &str,
    input_identities_csv: &str,
    disposition: &str,
    output: Option<(&str, &str)>,
    statistics: &mut Statistics,
) -> Result<()> {
    const SCALARS: usize = DIRECT_PROGRAM_V2_SCALARS as usize;
    const IDENTITIES: usize = DIRECT_PROGRAM_V2_IDENTITIES as usize;

    let context = format!("Direct AOT {name}");
    let input_scalars = parse_csv(input_scalars_csv, &format!("{context} input scalars"))?;
    let input_identities = parse_csv(input_identities_csv, &format!("{context} input identities"))?;
    let scalars: [u64; SCALARS] = input_scalars.try_into().map_err(|values: Vec<u64>| {
        failure(format!(
            "{context}: expected {SCALARS} scalar inputs, observed {}",
            values.len()
        ))
    })?;
    let identity_values: [u64; IDENTITIES] =
        input_identities.try_into().map_err(|values: Vec<u64>| {
            failure(format!(
                "{context}: expected {IDENTITIES} identity inputs, observed {}",
                values.len()
            ))
        })?;
    let identities = identity_values.map(identity);

    let mut scratch_scalars = [0xa5a5_a5a5_a5a5_a5a5_u64; SCALARS];
    let mut scratch_identities = [[0xa5_u8; 32]; IDENTITIES];
    let output_scalars_before = [0x5a5a_5a5a_5a5a_5a5a_u64; SCALARS];
    let output_identities_before = [[0x5a_u8; 32]; IDENTITIES];
    let mut actual_scalars = output_scalars_before;
    let mut actual_identities = output_identities_before;
    let result = execute_aot(
        AotInput {
            scalars: &scalars,
            identities: &identities,
        },
        AotOutput {
            scalars: &mut scratch_scalars,
            identities: &mut scratch_identities,
        },
        AotOutput {
            scalars: &mut actual_scalars,
            identities: &mut actual_identities,
        },
    );

    match disposition {
        "reject" => {
            if output.is_some() {
                return Err(failure(format!("{context}: refusal carried post-state")));
            }
            if result.is_ok() {
                return Err(failure(format!("{context}: AOT accepted Lean refusal")));
            }
            require_equal(
                actual_scalars,
                output_scalars_before,
                &format!("{context}: scalar rollback"),
            )?;
            require_equal(
                actual_identities,
                output_identities_before,
                &format!("{context}: identity rollback"),
            )?;
            statistics.aot_refusals += 1;
        }
        "accept" => {
            let (output_scalars_csv, output_identities_csv) = output
                .ok_or_else(|| failure(format!("{context}: acceptance omitted post-state")))?;
            result.map_err(|error| {
                failure(format!("{context}: AOT refused Lean acceptance: {error:?}"))
            })?;
            let expected_scalars: [u64; SCALARS] =
                parse_csv(output_scalars_csv, &format!("{context} output scalars"))?
                    .try_into()
                    .map_err(|values: Vec<u64>| {
                        failure(format!(
                            "{context}: expected {SCALARS} scalar outputs, observed {}",
                            values.len()
                        ))
                    })?;
            let expected_identity_values: [u64; IDENTITIES] = parse_csv(
                output_identities_csv,
                &format!("{context} output identities"),
            )?
            .try_into()
            .map_err(|values: Vec<u64>| {
                failure(format!(
                    "{context}: expected {IDENTITIES} identity outputs, observed {}",
                    values.len()
                ))
            })?;
            require_equal(actual_scalars, expected_scalars, &context)?;
            require_equal(
                actual_identities,
                expected_identity_values.map(identity),
                &context,
            )?;
            statistics.aot_accepts += 1;
        }
        value => return Err(failure(format!("{context}: invalid disposition {value:?}"))),
    }
    statistics.aot_cases += 1;
    Ok(())
}

fn validate_vm(fields: &[&str], statistics: &mut Statistics) -> Result<()> {
    if fields.len() != 5 && fields.len() != 7 {
        return Err(failure("vm case: wrong field count"));
    }
    validate_vm_case(
        generated_program::bytes(),
        fields[1],
        fields[2],
        fields[3],
        fields[4],
        (fields.len() == 7).then(|| (fields[5], fields[6])),
        statistics,
    )?;
    validate_direct_aot_case(
        fields[1],
        fields[2],
        fields[3],
        fields[4],
        (fields.len() == 7).then(|| (fields[5], fields[6])),
        statistics,
    )
}

fn validate_vm_program(fields: &[&str], statistics: &mut Statistics) -> Result<()> {
    if fields.len() != 6 && fields.len() != 8 {
        return Err(failure("vm program case: wrong field count"));
    }
    let program = decode_hex(fields[2], &format!("vm program {}", fields[1]))?;
    validate_vm_case(
        &program,
        fields[1],
        fields[3],
        fields[4],
        fields[5],
        (fields.len() == 8).then(|| (fields[6], fields[7])),
        statistics,
    )
}

fn next(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

fn random_bytes(seed: &mut u64) -> [u8; 32] {
    let mut output = [0_u8; 32];
    for chunk in output.chunks_exact_mut(8) {
        chunk.copy_from_slice(&next(seed).to_le_bytes());
    }
    output
}

fn random_intent(seed: &mut u64) -> CompactIntentV1 {
    CompactIntentV1 {
        side: next(seed).to_le_bytes()[0],
        outcome: next(seed).to_le_bytes()[0],
        lifecycle: next(seed).to_le_bytes()[0],
        market: random_bytes(seed),
        generation: next(seed),
        nonce: next(seed),
        valid_from: next(seed),
        valid_through: next(seed),
        maximum_fill: next(seed),
        limit_price: next(seed),
        fee_basis_points: u16::from_le_bytes(
            next(seed).to_le_bytes()[..2].try_into().unwrap_or([0; 2]),
        ),
        collateral_account: random_bytes(seed),
    }
}

fn run_rust_roundtrips(count: usize) -> Result<()> {
    let mut seed = 0x4d59_5df4_d0f3_3173_u64;
    for index in 0..count {
        let seller = random_intent(&mut seed);
        let buyer = random_intent(&mut seed);
        let instruction = ControllerInstructionV1 {
            controller_bump: next(&mut seed).to_le_bytes()[0],
            seller_replay_bump: next(&mut seed).to_le_bytes()[0],
            buyer_replay_bump: next(&mut seed).to_le_bytes()[0],
            seller_position_bump: next(&mut seed).to_le_bytes()[0],
            buyer_position_bump: next(&mut seed).to_le_bytes()[0],
            fill: next(&mut seed),
            execution_price: next(&mut seed),
            seller,
            buyer,
        };
        let intent_bytes = seller
            .encode()
            .map_err(|error| failure(format!("roundtrip {index}: intent encode: {error:?}")))?;
        require_equal(
            CompactIntentV1::decode(&intent_bytes),
            Ok(seller),
            &format!("roundtrip {index}: intent"),
        )?;
        let controller_bytes = instruction
            .encode()
            .map_err(|error| failure(format!("roundtrip {index}: controller encode: {error:?}")))?;
        require_equal(
            ControllerInstructionV1::decode(&controller_bytes),
            Ok(instruction),
            &format!("roundtrip {index}: controller"),
        )?;
    }
    Ok(())
}

fn validate(path: &Path) -> Result<Statistics> {
    let corpus = fs::read_to_string(path)?;
    let mut lines = corpus.lines();
    require_equal(
        lines.next(),
        Some("dclutch-direct-translation-corpus-v1"),
        "corpus header",
    )?;

    let mut statistics = Statistics::default();
    let mut intents = BTreeMap::new();
    let mut intent_bytes = BTreeMap::new();
    let mut controller_bytes = BTreeMap::new();
    let mut registered_creations = BTreeMap::new();
    let mut terminal_controllers = BTreeMap::new();
    let mut terminal_claims = BTreeMap::new();
    let mut program_seen = false;

    for (line_index, line) in lines.enumerate() {
        let fields: Vec<_> = line.split('|').collect();
        match fields.first().copied() {
            Some("program") => {
                if fields.len() != 2 || program_seen {
                    return Err(failure(format!(
                        "line {}: invalid program record",
                        line_index + 2
                    )));
                }
                let expected = decode_hex(fields[1], "program")?;
                require_equal(
                    generated_program::bytes(),
                    expected.as_slice(),
                    "program bytes",
                )?;
                program_seen = true;
            }
            Some("intent") => {
                validate_intent(&fields, &mut intents, &mut intent_bytes)?;
                statistics.intents += 1;
            }
            Some("intent-mutation") => {
                validate_intent_mutation(&fields, &intent_bytes)?;
                statistics.abi_mutations += 1;
            }
            Some("controller") => {
                validate_controller(&fields, &intents, &mut controller_bytes)?;
                statistics.controllers += 1;
            }
            Some("controller-mutation") => {
                validate_controller_mutation(&fields, &controller_bytes)?;
                statistics.abi_mutations += 1;
            }
            Some("registered-create") => {
                validate_registered_create(&fields, &mut registered_creations)?;
                statistics.registered_creations += 1;
            }
            Some("registered-create-mutation") => {
                validate_registered_create_mutation(&fields, &registered_creations)?;
                statistics.abi_mutations += 1;
                statistics.registered_creation_mutations += 1;
            }
            Some("registered-create-hostile") => {
                validate_registered_create_hostile(&fields, &registered_creations)?;
                statistics.abi_hostile_widths += 1;
                statistics.registered_creation_hostile_widths += 1;
            }
            Some("registered-create-transition") => {
                validate_registered_create_transition(&fields, &mut statistics)?;
            }
            Some("terminal-controller") => {
                validate_terminal_controller(&fields, &mut terminal_controllers)?;
                statistics.terminal_controllers += 1;
            }
            Some("terminal-controller-mutation") => {
                validate_terminal_controller_mutation(&fields, &terminal_controllers)?;
                statistics.abi_mutations += 1;
            }
            Some("terminal-controller-hostile") => {
                validate_terminal_controller_hostile(&fields, &terminal_controllers)?;
                statistics.abi_hostile_widths += 1;
            }
            Some("terminal-claim") => {
                validate_terminal_claim(&fields, &mut terminal_claims)?;
                statistics.terminal_claims += 1;
            }
            Some("terminal-claim-mutation") => {
                validate_terminal_claim_mutation(&fields, &terminal_claims)?;
                statistics.abi_mutations += 1;
            }
            Some("terminal-claim-hostile") => {
                validate_terminal_claim_hostile(&fields, &terminal_claims)?;
                statistics.abi_hostile_widths += 1;
            }
            Some("terminal-transition") => {
                validate_terminal_transition(&fields, &mut statistics)?;
            }
            Some("vm") => validate_vm(&fields, &mut statistics)?,
            Some("vm-program") => validate_vm_program(&fields, &mut statistics)?,
            Some(kind) => {
                return Err(failure(format!(
                    "line {}: unknown record {kind:?}",
                    line_index + 2
                )));
            }
            None => return Err(failure(format!("line {}: empty record", line_index + 2))),
        }
    }
    if !program_seen {
        return Err(failure("corpus omitted program bytes"));
    }
    require_equal(
        statistics.registered_creations,
        14,
        "registered creation ABI record count",
    )?;
    require_equal(
        statistics.registered_creation_mutations,
        2_128,
        "registered creation mutation count",
    )?;
    require_equal(
        statistics.registered_creation_hostile_widths,
        2_142,
        "registered creation hostile-width count",
    )?;
    require_equal(
        (
            statistics.creation_transitions,
            statistics.creation_accepts,
            statistics.creation_refusals,
        ),
        (17, 6, 11),
        "registered creation transition accounting",
    )?;
    const RUST_ROUNDTRIPS: usize = 4096;
    run_rust_roundtrips(RUST_ROUNDTRIPS)?;
    statistics.rust_roundtrips = RUST_ROUNDTRIPS;
    Ok(statistics)
}

fn main() -> Result<()> {
    let mut arguments = env::args_os();
    let executable = arguments.next().unwrap_or_default();
    let path = arguments.next().ok_or_else(|| {
        failure(format!(
            "usage: {} CORPUS",
            Path::new(&executable).display()
        ))
    })?;
    if arguments.next().is_some() {
        return Err(failure("expected exactly one corpus path"));
    }
    let statistics = validate(Path::new(&path))?;
    println!(
        "translation validation passed: {} Lean ABI values, {} single-byte ABI mutations, {} hostile ABI widths, {} Lean VM states ({} accepted, {} refused with rollback), {} Direct AOT states ({} accepted, {} refused with rollback), registered creation corpus {} ABIs/{} mutations/{} hostile widths and {} transitions ({} accepted, {} refused with exact rollback), {} registered terminal transitions ({} accepted, {} refused with exact rollback), {} deterministic Rust roundtrips",
        statistics.intents
            + statistics.controllers
            + statistics.registered_creations
            + statistics.terminal_controllers
            + statistics.terminal_claims,
        statistics.abi_mutations,
        statistics.abi_hostile_widths,
        statistics.vm_cases,
        statistics.vm_accepts,
        statistics.vm_refusals,
        statistics.aot_cases,
        statistics.aot_accepts,
        statistics.aot_refusals,
        statistics.registered_creations,
        statistics.registered_creation_mutations,
        statistics.registered_creation_hostile_widths,
        statistics.creation_transitions,
        statistics.creation_accepts,
        statistics.creation_refusals,
        statistics.terminal_transitions,
        statistics.terminal_accepts,
        statistics.terminal_refusals,
        statistics.rust_roundtrips,
    );
    Ok(())
}
