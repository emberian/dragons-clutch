//! Legacy-transaction construction, serialization, and signing.
//!
//! The keeper builds its transactions live rather than replaying a
//! pregenerated plan, so it owns the whole wire path: the reference request
//! envelope, the four message account groups, the compute-budget riders, the
//! compact-u16 framing, and the Ed25519 signatures.
//!
//! Two invariants are enforced here rather than trusted:
//!
//! * [`Message::new`] refuses a duplicate key, because computing an
//!   instruction's account indices by hand is exactly how a list silently
//!   points at the wrong account; [`Message::index`] is the only way an index
//!   is produced.
//! * [`request`] round-trips its own bytes through the real
//!   `clutch_solana_reference::Request::decode`, so a wrong envelope constant
//!   is a panic here rather than a mysterious refusal from the program.  The
//!   envelope constants are private to the reference crate; this copy is kept
//!   honest by that round trip, exactly as `clutch-sbf-svm-tests` keeps its
//!   copy honest.

use clutch_solana_layout::{Intent, MAX_INTENT_BYTES};
use solana_keypair::{Keypair, Signer};

/// The legacy packet budget of the Solana cluster wire, in bytes.
///
/// This is the number the bank-transported evidence in
/// `research/liveness-policy-profile` explicitly does **not** model
/// (`cluster_packet_budget: UNMODELED_BANK_TRANSPORT_ONLY`).  Every
/// transaction this keeper emits is measured against it before submission.
pub const PACKET_BUDGET_BYTES: usize = 1232;

/// `clutch_solana_reference`'s private request tag.
const REQUEST_TAG: u8 = 0xd1;
/// `clutch_solana_reference`'s private envelope version.
const REFERENCE_VERSION: u8 = 1;
/// `clutch_solana_reference::Action::Layout`'s private discriminant.
const ACTION_LAYOUT: u8 = 0;

/// The compute-budget program address, base58.
pub const COMPUTE_BUDGET: &str = "ComputeBudget111111111111111111111111111111";
/// The system program address, base58.
pub const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
/// The Clock sysvar address, base58.
pub const CLOCK_SYSVAR: &str = "SysvarC1ock11111111111111111111111111111111";
/// The Rent sysvar address, base58.
pub const RENT_SYSVAR: &str = "SysvarRent111111111111111111111111111111111";
/// The canonical incinerator: `GENERAL_NEUTRAL_SINK_V1`, the frozen sink of
/// every `TerminalClosure` route.
pub const INCINERATOR: &str = "1nc1nerator11111111111111111111111111111111";

/// `ComputeBudgetInstruction::SetComputeUnitLimit`.
const SET_COMPUTE_UNIT_LIMIT: u8 = 2;
/// `ComputeBudgetInstruction::RequestHeapFrame`.
const REQUEST_HEAP_FRAME: u8 = 1;
/// The heap frame the clearing walk needs: the program boxes the ~48.7 KiB
/// `ClearWorkV1` onto it.
pub const HEAP_FRAME_BYTES: u32 = 262_144;

const B58: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Encode 32 bytes as a base58 address.
#[must_use]
pub fn base58(bytes: &[u8; 32]) -> String {
    let mut digits: Vec<u8> = Vec::with_capacity(45);
    for byte in bytes {
        let mut carry = usize::from(*byte);
        for digit in &mut digits {
            let wide = (usize::from(*digit) << 8) + carry;
            *digit = u8::try_from(wide % 58).expect("a base58 digit is below 58");
            carry = wide / 58;
        }
        while carry > 0 {
            digits.push(u8::try_from(carry % 58).expect("a base58 digit is below 58"));
            carry /= 58;
        }
    }
    let leading = bytes.iter().take_while(|byte| **byte == 0).count();
    let mut out = String::with_capacity(leading + digits.len());
    for _ in 0..leading {
        out.push('1');
    }
    for digit in digits.iter().rev() {
        out.push(char::from(B58[usize::from(*digit)]));
    }
    out
}

/// Decode a 32-byte base58 address.
///
/// # Errors
/// Returns an error for a non-alphabet character or an over-wide value.
pub fn base58_decode_32(text: &str) -> Result<[u8; 32], String> {
    let mut out = [0_u8; 32];
    for character in text.bytes() {
        let digit = B58
            .iter()
            .position(|candidate| *candidate == character)
            .ok_or_else(|| format!("invalid base58 character in {text}"))?;
        let mut carry = digit;
        for byte in out.iter_mut().rev() {
            let wide = usize::from(*byte) * 58 + carry;
            *byte = u8::try_from(wide & 0xff).expect("masked to a byte");
            carry = wide >> 8;
        }
        if carry != 0 {
            return Err(format!("base58 value is wider than 32 bytes: {text}"));
        }
    }
    Ok(out)
}

/// Build the reference request envelope around one frozen layout intent.
///
/// # Panics
/// Panics when the intent does not encode, or when the emitted envelope does
/// not decode back through the real `clutch_solana_reference::Request`.
#[must_use]
pub fn request(sequence: u64, intent: &Intent) -> Vec<u8> {
    let mut intent_bytes = [0_u8; MAX_INTENT_BYTES];
    let len = intent.encode(&mut intent_bytes).expect("intent encodes");
    let mut out = Vec::with_capacity(13 + len);
    out.push(REQUEST_TAG);
    out.push(REFERENCE_VERSION);
    out.extend_from_slice(&sequence.to_le_bytes());
    out.push(ACTION_LAYOUT);
    out.extend_from_slice(&u16::try_from(len).expect("an intent is under 64 KiB").to_le_bytes());
    out.extend_from_slice(&intent_bytes[..len]);
    let decoded = clutch_solana_reference::Request::decode(&out)
        .expect("the keeper must emit an envelope the reference decodes");
    assert_eq!(decoded.sequence, sequence, "envelope sequence round-trips");
    out
}

fn compact_u16(value: usize, out: &mut Vec<u8>) {
    let mut remaining = value;
    loop {
        let mut byte = u8::try_from(remaining & 0x7f).expect("masked to seven bits");
        remaining >>= 7;
        if remaining != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if remaining == 0 {
            break;
        }
    }
}

/// One legacy message, in the account order the runtime requires.
#[derive(Clone, Debug)]
pub struct Message {
    keys: Vec<[u8; 32]>,
    required_signatures: u8,
    readonly_signed: u8,
    readonly_unsigned: u8,
}

impl Message {
    /// Assemble the four ordered account groups.
    ///
    /// # Panics
    /// Panics when one key appears in more than one group, which would make
    /// every later index ambiguous.
    #[must_use]
    pub fn new(
        writable_signers: &[[u8; 32]],
        readonly_signers: &[[u8; 32]],
        writable: &[[u8; 32]],
        readonly: &[[u8; 32]],
    ) -> Self {
        let mut keys = Vec::new();
        keys.extend_from_slice(writable_signers);
        keys.extend_from_slice(readonly_signers);
        keys.extend_from_slice(writable);
        keys.extend_from_slice(readonly);
        for (index, key) in keys.iter().enumerate() {
            assert!(
                !keys[index + 1..].contains(key),
                "duplicate key {} in message account list",
                base58(key)
            );
        }
        Self {
            required_signatures: u8::try_from(writable_signers.len() + readonly_signers.len())
                .expect("a legacy message carries under 256 signers"),
            readonly_signed: u8::try_from(readonly_signers.len())
                .expect("a legacy message carries under 256 signers"),
            readonly_unsigned: u8::try_from(readonly.len())
                .expect("a legacy message carries under 256 read-only accounts"),
            keys,
        }
    }

    /// The message index of one key.
    ///
    /// # Panics
    /// Panics when the key is not in the message; an instruction that names an
    /// account the message does not carry is a construction bug, not a runtime
    /// refusal to discover on chain.
    #[must_use]
    pub fn index(&self, key: &[u8; 32]) -> u8 {
        u8::try_from(
            self.keys
                .iter()
                .position(|candidate| candidate == key)
                .unwrap_or_else(|| panic!("account {} is not in the message", base58(key))),
        )
        .expect("a legacy message carries under 256 accounts")
    }

    /// The message indices of an ordered role list.
    #[must_use]
    pub fn indices(&self, keys: &[[u8; 32]]) -> Vec<u8> {
        keys.iter().map(|key| self.index(key)).collect()
    }
}

/// One instruction with already-resolved indices.
#[derive(Clone, Debug)]
pub struct Instruction {
    /// Message index of the invoked program.
    pub program_index: u8,
    /// Message indices of the instruction's accounts, in role order.
    pub accounts: Vec<u8>,
    /// Instruction data.
    pub data: Vec<u8>,
}

/// One `SetComputeUnitLimit` rider.
#[must_use]
pub fn compute_limit_instruction(message: &Message, budget: &[u8; 32], units: u32) -> Instruction {
    let mut data = Vec::with_capacity(5);
    data.push(SET_COMPUTE_UNIT_LIMIT);
    data.extend_from_slice(&units.to_le_bytes());
    Instruction {
        program_index: message.index(budget),
        accounts: Vec::new(),
        data,
    }
}

/// One `RequestHeapFrame` rider.
#[must_use]
pub fn heap_frame_instruction(message: &Message, budget: &[u8; 32], bytes: u32) -> Instruction {
    let mut data = Vec::with_capacity(5);
    data.push(REQUEST_HEAP_FRAME);
    data.extend_from_slice(&bytes.to_le_bytes());
    Instruction {
        program_index: message.index(budget),
        accounts: Vec::new(),
        data,
    }
}

/// Serialize one legacy transaction with zero-filled signatures.
///
/// The returned tuple is `(bytes, signatures_start, message_start)`; the
/// blockhash is written at `blockhash_start` inside the message and is filled
/// by [`sign`].
#[must_use]
pub fn serialize(
    message: &Message,
    instructions: &[Instruction],
    blockhash: &[u8; 32],
) -> Vec<u8> {
    let mut out = Vec::new();
    compact_u16(usize::from(message.required_signatures), &mut out);
    for _ in 0..message.required_signatures {
        out.extend_from_slice(&[0_u8; 64]);
    }
    out.push(message.required_signatures);
    out.push(message.readonly_signed);
    out.push(message.readonly_unsigned);
    compact_u16(message.keys.len(), &mut out);
    for key in &message.keys {
        out.extend_from_slice(key);
    }
    out.extend_from_slice(blockhash);
    compact_u16(instructions.len(), &mut out);
    for instruction in instructions {
        out.push(instruction.program_index);
        compact_u16(instruction.accounts.len(), &mut out);
        out.extend_from_slice(&instruction.accounts);
        compact_u16(instruction.data.len(), &mut out);
        out.extend_from_slice(&instruction.data);
    }
    out
}

/// Overwrite the recent blockhash of an already-serialized transaction.
///
/// The plan fixtures this keeper can replay carry the zero blockhash, and the
/// bytes that get signed are the ones with the real hash in them.
///
/// # Errors
/// Returns an error when the transaction framing does not parse.
pub fn set_blockhash(transaction: &mut [u8], blockhash: &[u8; 32]) -> Result<(), String> {
    let start = blockhash_offset(transaction)?;
    transaction[start..start + 32].copy_from_slice(blockhash);
    Ok(())
}

fn blockhash_offset(transaction: &[u8]) -> Result<usize, String> {
    let mut cursor = 0;
    let signature_count = short_vec(transaction, &mut cursor)?;
    cursor += signature_count * 64 + 3;
    let key_count = short_vec(transaction, &mut cursor)?;
    let end = cursor + key_count * 32;
    if transaction.len() < end + 32 {
        return Err("transaction is truncated before its recent blockhash".to_string());
    }
    Ok(end)
}

/// Sign a serialized transaction in place with the supplied keypairs.
///
/// # Errors
/// Returns an error when a required signer has no supplied keypair, or when
/// the transaction framing does not parse.
pub fn sign(transaction: &mut [u8], keypairs: &[&Keypair]) -> Result<(), String> {
    let (signatures_start, message_start, signer_keys) = layout(transaction)?;
    let (signature_prefix, message) = transaction.split_at_mut(message_start);
    for (index, signer_key) in signer_keys.iter().enumerate() {
        let keypair = keypairs
            .iter()
            .find(|candidate| candidate.pubkey().to_bytes() == *signer_key)
            .ok_or_else(|| format!("no keypair for required signer {}", base58(signer_key)))?;
        let signature = keypair.sign_message(message);
        let start = signatures_start + index * 64;
        signature_prefix[start..start + 64].copy_from_slice(signature.as_ref());
    }
    Ok(())
}

/// Parse a serialized legacy transaction's signature region and signer keys.
fn layout(transaction: &[u8]) -> Result<(usize, usize, Vec<[u8; 32]>), String> {
    let mut cursor = 0;
    let signature_count = short_vec(transaction, &mut cursor)?;
    let signatures_start = cursor;
    cursor += signature_count * 64;
    let message_start = cursor;
    let required = usize::from(
        *transaction
            .get(cursor)
            .ok_or("transaction has no message header")?,
    );
    if required != signature_count {
        return Err(format!(
            "signature count {signature_count} disagrees with message header {required}"
        ));
    }
    cursor += 3;
    let key_count = short_vec(transaction, &mut cursor)?;
    let keys_start = cursor;
    if transaction.len() < keys_start + key_count * 32 {
        return Err("transaction is truncated inside its account keys".to_string());
    }
    let mut signers = Vec::with_capacity(required);
    for index in 0..required {
        let start = keys_start + index * 32;
        signers.push(
            transaction[start..start + 32]
                .try_into()
                .map_err(|_| "a message key is not 32 bytes".to_string())?,
        );
    }
    Ok((signatures_start, message_start, signers))
}

fn short_vec(bytes: &[u8], offset: &mut usize) -> Result<usize, String> {
    let mut value = 0_usize;
    let mut shift = 0_u32;
    loop {
        let byte = *bytes
            .get(*offset)
            .ok_or("truncated short-vec while parsing a transaction")?;
        *offset += 1;
        value |= usize::from(byte & 0x7f)
            .checked_shl(shift)
            .ok_or("short-vec value overflow")?;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift > 21 {
            return Err("short-vec is wider than a Solana packet permits".to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base58_round_trips_the_zero_and_a_dense_key() {
        assert_eq!(base58(&[0_u8; 32]), "11111111111111111111111111111111");
        let dense = [0x7f_u8; 32];
        assert_eq!(
            base58_decode_32(&base58(&dense)).expect("dense key decodes"),
            dense
        );
    }

    #[test]
    fn the_frozen_program_addresses_decode() {
        for address in [
            COMPUTE_BUDGET,
            SYSTEM_PROGRAM,
            CLOCK_SYSVAR,
            RENT_SYSVAR,
            INCINERATOR,
        ] {
            let bytes = base58_decode_32(address).expect("a frozen address decodes");
            assert_eq!(base58(&bytes), address, "and re-encodes to itself");
        }
    }

    #[test]
    fn compact_u16_matches_the_runtime_framing() {
        let mut out = Vec::new();
        compact_u16(0, &mut out);
        compact_u16(127, &mut out);
        compact_u16(128, &mut out);
        compact_u16(1232, &mut out);
        assert_eq!(out, vec![0x00, 0x7f, 0x80, 0x01, 0xd0, 0x09]);
    }

    #[test]
    fn signing_fills_every_signature_slot_and_refuses_a_missing_signer() {
        let payer = Keypair::new();
        let other = Keypair::new();
        let message = Message::new(
            &[payer.pubkey().to_bytes()],
            &[],
            &[],
            &[[9_u8; 32], [8_u8; 32]],
        );
        let instruction = Instruction {
            program_index: message.index(&[9_u8; 32]),
            accounts: Vec::new(),
            data: vec![1, 2, 3],
        };
        let mut bytes = serialize(&message, &[instruction], &[7_u8; 32]);
        assert!(sign(&mut bytes, &[&other]).is_err());
        sign(&mut bytes, &[&payer, &other]).expect("the payer signs");
        assert!(bytes[1..65].iter().any(|byte| *byte != 0));
    }

    #[test]
    fn the_request_envelope_round_trips_through_the_reference_decoder() {
        use clutch_solana_layout::Hash32;
        let bytes = request(
            0,
            &Intent::FreezeEpoch {
                market: Hash32::from_bytes([3; 32]),
                epoch: Hash32::from_bytes([4; 32]),
            },
        );
        assert_eq!(bytes[0], REQUEST_TAG);
        assert_eq!(bytes.len(), 13 + 66);
    }
}
