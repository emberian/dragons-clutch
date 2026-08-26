#![no_std]
#![forbid(unsafe_code)]
//! Tiny transactional interpreter for Lean-owned dClutch Effect IR.
//!
//! This crate does not decide whether a Direct fill is economically admissible.
//! Lean owns that meaning. This crate is the bounded physical refinement target:
//! it hostile-decodes one canonical plan and applies supported effects to a
//! caller-owned state projection, committing only after every effect succeeds.

/// Runtime-width, account-profile-constrained physical effect projection.
pub mod v2;
/// Runtime-tail local effects and typed fixed-role request projection.
pub mod v3;
/// Finite scalar-selected fixed-account route spans over canonical V3 effects.
pub mod v4;

/// Canonical wire magic (`DCEF`).
pub const MAGIC: [u8; 4] = *b"DCEF";
/// Canonical Effect IR wire version.
pub const VERSION: u8 = 1;
/// Bytes in the plan header.
pub const HEADER_BYTES: usize = 8;
/// Bytes in one fixed effect record.
pub const EFFECT_BYTES: usize = 16;
/// Effects admitted by the first physical execution profile.
pub const MAX_EFFECTS: usize = 7;
/// Largest V1 plan admitted by this profile.
pub const MAX_PLAN_BYTES: usize = HEADER_BYTES + MAX_EFFECTS * EFFECT_BYTES;

/// Stable parser or execution refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input length was not the exact canonical length.
    InvalidLength,
    /// Header magic differed from `DCEF`.
    InvalidMagic,
    /// Wire version is not implemented by this kernel.
    UnsupportedVersion,
    /// Effect count exceeded the measured execution profile.
    InvalidCount,
    /// A reserved byte was nonzero.
    NonzeroReserved,
    /// Effect opcode was unknown.
    UnknownOpcode,
    /// Party tag was unknown.
    UnknownParty,
    /// Resource tag was unknown.
    UnknownResource,
    /// A non-claim resource carried a nonzero outcome coordinate.
    NoncanonicalCoordinate,
    /// An effect was well-formed but outside this kernel's admitted vocabulary.
    UnsupportedEffect,
    /// A claim effect addressed a different outcome projection.
    OutcomeMismatch,
    /// A debit exceeded the source balance.
    InsufficientBalance,
    /// Checked `u64` arithmetic overflowed.
    ArithmeticOverflow,
}

/// Effect operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Opcode {
    /// Replace one non-resource fact.
    Set = 0,
    /// Subtract a resource amount.
    Debit = 1,
    /// Add a resource amount.
    Credit = 2,
}

impl Opcode {
    fn decode(value: u8) -> Result<Self, Error> {
        match value {
            0 => Ok(Self::Set),
            1 => Ok(Self::Debit),
            2 => Ok(Self::Credit),
            _ => Err(Error::UnknownOpcode),
        }
    }
}

/// Semantic party tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Party {
    /// Direct seller.
    Seller = 0,
    /// Direct buyer.
    Buyer = 1,
    /// Authenticated venue fee recipient.
    Venue = 2,
}

impl Party {
    fn decode(value: u8) -> Result<Self, Error> {
        match value {
            0 => Ok(Self::Seller),
            1 => Ok(Self::Buyer),
            2 => Ok(Self::Venue),
            _ => Err(Error::UnknownParty),
        }
    }
}

/// Typed state resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Resource {
    /// Gap-free replay nonce.
    ReplayNonce = 0,
    /// One Product-owned outcome claim coordinate.
    OutcomeClaim = 1,
    /// Realm-selected collateral.
    Collateral = 2,
}

impl Resource {
    fn decode(value: u8) -> Result<Self, Error> {
        match value {
            0 => Ok(Self::ReplayNonce),
            1 => Ok(Self::OutcomeClaim),
            2 => Ok(Self::Collateral),
            _ => Err(Error::UnknownResource),
        }
    }
}

/// One decoded first-order effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Effect {
    /// Operation.
    pub opcode: Opcode,
    /// Semantic party.
    pub party: Party,
    /// Typed resource.
    pub resource: Resource,
    /// Outcome coordinate, zero for non-claim resources.
    pub outcome: u32,
    /// Set value or resource amount.
    pub value: u64,
}

const EMPTY_EFFECT: Effect = Effect {
    opcode: Opcode::Set,
    party: Party::Seller,
    resource: Resource::ReplayNonce,
    outcome: 0,
    value: 0,
};

/// Fixed-capacity decoded effect plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Plan {
    count: u8,
    effects: [Effect; MAX_EFFECTS],
}

impl Plan {
    /// Hostile-decode one exact canonical plan.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() < HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if input.get(0..MAGIC.len()) != Some(MAGIC.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_byte(input, 4)? != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let count = read_byte(input, 5)?;
        if usize::from(count) > MAX_EFFECTS {
            return Err(Error::InvalidCount);
        }
        if read_byte(input, 6)? != 0 || read_byte(input, 7)? != 0 {
            return Err(Error::NonzeroReserved);
        }
        let records = usize::from(count)
            .checked_mul(EFFECT_BYTES)
            .ok_or(Error::InvalidLength)?;
        let expected = HEADER_BYTES
            .checked_add(records)
            .ok_or(Error::InvalidLength)?;
        if input.len() != expected {
            return Err(Error::InvalidLength);
        }

        let mut effects = [EMPTY_EFFECT; MAX_EFFECTS];
        let mut index = 0_usize;
        while index < usize::from(count) {
            let offset = HEADER_BYTES
                .checked_add(
                    index
                        .checked_mul(EFFECT_BYTES)
                        .ok_or(Error::InvalidLength)?,
                )
                .ok_or(Error::InvalidLength)?;
            let effect = decode_effect(input, offset)?;
            let destination = effects.get_mut(index).ok_or(Error::InvalidCount)?;
            *destination = effect;
            index = index.checked_add(1).ok_or(Error::InvalidCount)?;
        }
        Ok(Self { count, effects })
    }

    /// Number of active effects.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count as usize
    }

    /// Whether the plan contains no effects.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Return one active effect.
    #[must_use]
    pub fn effect(&self, index: usize) -> Option<&Effect> {
        if index < self.len() {
            self.effects.get(index)
        } else {
            None
        }
    }

    /// Exact encoded length.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        HEADER_BYTES + self.len() * EFFECT_BYTES
    }

    /// Re-encode into caller-owned fixed memory.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<usize, Error> {
        let length = self.encoded_len();
        if output.len() != length {
            return Err(Error::InvalidLength);
        }
        write_slice(output, 0, &MAGIC)?;
        write_byte(output, 4, VERSION)?;
        write_byte(output, 5, self.count)?;
        write_byte(output, 6, 0)?;
        write_byte(output, 7, 0)?;

        let mut index = 0_usize;
        while index < self.len() {
            let offset = HEADER_BYTES
                .checked_add(
                    index
                        .checked_mul(EFFECT_BYTES)
                        .ok_or(Error::InvalidLength)?,
                )
                .ok_or(Error::InvalidLength)?;
            encode_effect(
                self.effect(index).ok_or(Error::InvalidCount)?,
                output,
                offset,
            )?;
            index = index.checked_add(1).ok_or(Error::InvalidCount)?;
        }
        Ok(length)
    }
}

/// Mutable state projection for the first Direct effect vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    /// Product-owned outcome coordinate represented by the two claim fields.
    pub outcome: u32,
    /// Seller gap-free next nonce.
    pub seller_next_nonce: u64,
    /// Buyer gap-free next nonce.
    pub buyer_next_nonce: u64,
    /// Seller claims at `outcome`.
    pub seller_claims: u64,
    /// Buyer claims at `outcome`.
    pub buyer_claims: u64,
    /// Buyer collateral balance.
    pub buyer_collateral: u64,
    /// Seller collateral balance.
    pub seller_collateral: u64,
    /// Venue collateral balance.
    pub venue_collateral: u64,
}

/// Apply a complete plan transactionally.
///
/// The caller's state is replaced only after all effects succeed.
pub fn execute(plan: &Plan, state: &mut State) -> Result<(), Error> {
    let mut next = *state;
    let mut index = 0_usize;
    while index < plan.len() {
        apply_effect(&mut next, plan.effect(index).ok_or(Error::InvalidCount)?)?;
        index = index.checked_add(1).ok_or(Error::InvalidCount)?;
    }
    *state = next;
    Ok(())
}

fn apply_effect(state: &mut State, effect: &Effect) -> Result<(), Error> {
    match (effect.opcode, effect.party, effect.resource) {
        (Opcode::Set, Party::Seller, Resource::ReplayNonce) => {
            state.seller_next_nonce = effect.value;
        }
        (Opcode::Set, Party::Buyer, Resource::ReplayNonce) => {
            state.buyer_next_nonce = effect.value;
        }
        (Opcode::Debit, Party::Seller, Resource::OutcomeClaim) => {
            require_outcome(state, effect)?;
            state.seller_claims = state
                .seller_claims
                .checked_sub(effect.value)
                .ok_or(Error::InsufficientBalance)?;
        }
        (Opcode::Credit, Party::Buyer, Resource::OutcomeClaim) => {
            require_outcome(state, effect)?;
            state.buyer_claims = state
                .buyer_claims
                .checked_add(effect.value)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        (Opcode::Debit, Party::Buyer, Resource::Collateral) => {
            state.buyer_collateral = state
                .buyer_collateral
                .checked_sub(effect.value)
                .ok_or(Error::InsufficientBalance)?;
        }
        (Opcode::Credit, Party::Seller, Resource::Collateral) => {
            state.seller_collateral = state
                .seller_collateral
                .checked_add(effect.value)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        (Opcode::Credit, Party::Venue, Resource::Collateral) => {
            state.venue_collateral = state
                .venue_collateral
                .checked_add(effect.value)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        _ => return Err(Error::UnsupportedEffect),
    }
    Ok(())
}

fn require_outcome(state: &State, effect: &Effect) -> Result<(), Error> {
    if state.outcome == effect.outcome {
        Ok(())
    } else {
        Err(Error::OutcomeMismatch)
    }
}

fn decode_effect(input: &[u8], offset: usize) -> Result<Effect, Error> {
    let opcode = Opcode::decode(read_byte(input, offset)?)?;
    let party = Party::decode(read_byte(input, checked_offset(offset, 1)?)?)?;
    let resource = Resource::decode(read_byte(input, checked_offset(offset, 2)?)?)?;
    if read_byte(input, checked_offset(offset, 3)?)? != 0 {
        return Err(Error::NonzeroReserved);
    }
    let outcome = read_u32(input, checked_offset(offset, 4)?)?;
    if resource != Resource::OutcomeClaim && outcome != 0 {
        return Err(Error::NoncanonicalCoordinate);
    }
    let value = read_u64(input, checked_offset(offset, 8)?)?;
    Ok(Effect {
        opcode,
        party,
        resource,
        outcome,
        value,
    })
}

fn encode_effect(effect: &Effect, output: &mut [u8], offset: usize) -> Result<(), Error> {
    if effect.resource != Resource::OutcomeClaim && effect.outcome != 0 {
        return Err(Error::NoncanonicalCoordinate);
    }
    write_byte(output, offset, effect.opcode as u8)?;
    write_byte(output, checked_offset(offset, 1)?, effect.party as u8)?;
    write_byte(output, checked_offset(offset, 2)?, effect.resource as u8)?;
    write_byte(output, checked_offset(offset, 3)?, 0)?;
    write_slice(
        output,
        checked_offset(offset, 4)?,
        &effect.outcome.to_le_bytes(),
    )?;
    write_slice(
        output,
        checked_offset(offset, 8)?,
        &effect.value.to_le_bytes(),
    )
}

fn checked_offset(base: usize, delta: usize) -> Result<usize, Error> {
    base.checked_add(delta).ok_or(Error::InvalidLength)
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32, Error> {
    let end = checked_offset(offset, 4)?;
    let bytes: &[u8; 4] = input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)?;
    Ok(u32::from_le_bytes(*bytes))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    let end = checked_offset(offset, 8)?;
    let bytes: &[u8; 8] = input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)?;
    Ok(u64::from_le_bytes(*bytes))
}

fn write_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    let destination = output.get_mut(offset).ok_or(Error::InvalidLength)?;
    *destination = value;
    Ok(())
}

fn write_slice(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = checked_offset(offset, value.len())?;
    let destination = output.get_mut(offset..end).ok_or(Error::InvalidLength)?;
    destination.copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    const VECTOR_HEX: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../formal/dclutch-semantics/vectors/direct-inline-ordinary-v1.hex"
    ));

    fn decode_hex(value: &str) -> Vec<u8> {
        value
            .trim()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let pair = core::str::from_utf8(pair).expect("fixture is UTF-8");
                u8::from_str_radix(pair, 16).expect("fixture is hexadecimal")
            })
            .collect()
    }

    fn pre_state() -> State {
        State {
            outcome: 1,
            seller_next_nonce: 0,
            buyer_next_nonce: 0,
            seller_claims: 5000,
            buyer_claims: 200,
            buyer_collateral: 2000,
            seller_collateral: 100,
            venue_collateral: 20,
        }
    }

    #[test]
    fn lean_vector_round_trips_and_executes() {
        let bytes = decode_hex(VECTOR_HEX);
        assert_eq!(bytes.len(), MAX_PLAN_BYTES);
        let plan = Plan::decode(&bytes).expect("Lean vector decodes");
        assert_eq!(plan.len(), MAX_EFFECTS);
        let mut encoded = [0_u8; MAX_PLAN_BYTES];
        assert_eq!(plan.encode_into(&mut encoded), Ok(MAX_PLAN_BYTES));
        assert_eq!(encoded.as_slice(), bytes.as_slice());

        let mut state = pre_state();
        assert_eq!(execute(&plan, &mut state), Ok(()));
        assert_eq!(
            state,
            State {
                outcome: 1,
                seller_next_nonce: 1,
                buyer_next_nonce: 1,
                seller_claims: 3000,
                buyer_claims: 2200,
                buyer_collateral: 998,
                seller_collateral: 1100,
                venue_collateral: 22,
            }
        );
    }

    #[test]
    fn parser_refuses_noncanonical_frames() {
        let bytes = decode_hex(VECTOR_HEX);
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(Plan::decode(&trailing), Err(Error::InvalidLength));

        let mut reserved = bytes.clone();
        *reserved.get_mut(6).expect("header reserved byte exists") = 1;
        assert_eq!(Plan::decode(&reserved), Err(Error::NonzeroReserved));

        let mut coordinate = bytes;
        *coordinate
            .get_mut(12)
            .expect("first record outcome byte exists") = 1;
        assert_eq!(
            Plan::decode(&coordinate),
            Err(Error::NoncanonicalCoordinate)
        );
    }

    #[test]
    fn late_failure_rolls_back_every_prior_effect() {
        let mut bytes = decode_hex(VECTOR_HEX);
        let last_value = HEADER_BYTES + 6 * EFFECT_BYTES + 8;
        bytes
            .get_mut(last_value..last_value + 8)
            .expect("last amount exists")
            .copy_from_slice(&u64::MAX.to_le_bytes());
        let plan = Plan::decode(&bytes).expect("syntactically valid late overflow");
        let mut state = pre_state();
        let original = state;
        assert_eq!(execute(&plan, &mut state), Err(Error::ArithmeticOverflow));
        assert_eq!(state, original);
    }

    #[test]
    fn wrong_outcome_and_unsupported_effect_roll_back() {
        let bytes = decode_hex(VECTOR_HEX);
        let plan = Plan::decode(&bytes).expect("Lean vector decodes");
        let mut wrong_outcome = pre_state();
        wrong_outcome.outcome = 0;
        let original = wrong_outcome;
        assert_eq!(
            execute(&plan, &mut wrong_outcome),
            Err(Error::OutcomeMismatch)
        );
        assert_eq!(wrong_outcome, original);

        let mut unsupported_bytes = bytes;
        *unsupported_bytes
            .get_mut(HEADER_BYTES + 1)
            .expect("first record party byte exists") = Party::Venue as u8;
        let unsupported = Plan::decode(&unsupported_bytes).expect("syntax remains canonical");
        let mut state = pre_state();
        let original = state;
        assert_eq!(
            execute(&unsupported, &mut state),
            Err(Error::UnsupportedEffect)
        );
        assert_eq!(state, original);
    }

    #[test]
    fn insufficient_claims_roll_back() {
        let bytes = decode_hex(VECTOR_HEX);
        let plan = Plan::decode(&bytes).expect("Lean vector decodes");
        let mut state = pre_state();
        state.seller_claims = 1999;
        let original = state;
        assert_eq!(execute(&plan, &mut state), Err(Error::InsufficientBalance));
        assert_eq!(state, original);
    }
}
