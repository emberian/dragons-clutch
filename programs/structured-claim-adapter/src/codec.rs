//! Exact persisted-account and instruction codecs.

use crate::{is_zero, Error, Key, Result, MAX_OUTCOMES};

const DESCRIPTOR_TAG: u8 = 0xd1;
const REPLAY_TAG: u8 = 0xd2;
const REQUEST_TAG: u8 = 0xd3;
const VERSION: u8 = 1;

/// Exact bytes in [`StructuredClaimDescriptorV1`].
pub const DESCRIPTOR_BYTES: usize = 384;
/// Exact bytes in [`WrapperReplayV1`].
pub const REPLAY_BYTES: usize = 80;
/// Exact bytes in [`RequestV1`].
pub const REQUEST_BYTES: usize = 72;

/// Live descriptor lifecycle.
pub const DESCRIPTOR_LIVE: u8 = 0;
/// Permanent descriptor tombstone.
pub const DESCRIPTOR_RETIRED: u8 = 1;

/// The one persisted wrapper product description.
///
/// It intentionally stores no wrapper supply, backing totals, residual vector,
/// price, payout, label, certificate, or analytic program. Those are derived
/// from authenticated Token-2022/base truth and the primitive vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredClaimDescriptorV1 {
    /// Exact base executable program.
    pub base_program: Key,
    /// Exact base ProgramData account.
    pub base_program_data: Key,
    /// Authenticated base deployment slot.
    pub base_deployment_slot: u64,
    /// Exact wrapper ProgramData account; the wrapper program is the owner.
    pub wrapper_program_data: Key,
    /// Authenticated wrapper deployment slot.
    pub wrapper_deployment_slot: u64,
    /// Exact Token-2022 executable program.
    pub token_2022_program: Key,
    /// Exact Token-2022 ProgramData account.
    pub token_2022_program_data: Key,
    /// Authenticated Token-2022 deployment slot.
    pub token_2022_deployment_slot: u64,
    /// Canonical base Market identity.
    pub market: Key,
    /// Complete immutable Terms digest.
    pub terms: Key,
    /// Primitive GCD-one native Egg vector; padding is checked against Terms.
    pub primitive: [u64; MAX_OUTCOMES],
    /// Live or permanently retired.
    pub state: u8,
    /// Descriptor PDA bump.
    pub descriptor_bump: u8,
    /// Wrapper mint PDA bump.
    pub mint_bump: u8,
    /// Shared mint-authority/vault-owner PDA bump.
    pub vault_owner_bump: u8,
}

impl StructuredClaimDescriptorV1 {
    /// Validate fields decidable without the authenticated Terms/program owner.
    pub fn validate_shape(&self) -> Result<()> {
        let keys = [
            self.base_program,
            self.base_program_data,
            self.wrapper_program_data,
            self.token_2022_program,
            self.token_2022_program_data,
            self.market,
            self.terms,
        ];
        let mut i = 0;
        while i < keys.len() {
            if is_zero(&keys[i]) {
                return Err(Error::InvalidIdentity);
            }
            i += 1;
        }
        if self.state > DESCRIPTOR_RETIRED {
            return Err(Error::NonCanonical);
        }
        let mut divisor = 0_u64;
        i = 0;
        while i < MAX_OUTCOMES {
            divisor = gcd(divisor, self.primitive[i]);
            i += 1;
        }
        if divisor != 1 {
            return Err(Error::NonCanonical);
        }
        Ok(())
    }

    /// Encode exactly [`DESCRIPTOR_BYTES`] bytes.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        exact_output(output, DESCRIPTOR_BYTES)?;
        let mut writer = Writer::new(output);
        writer.header(DESCRIPTOR_TAG)?;
        writer.key(&self.base_program)?;
        writer.key(&self.base_program_data)?;
        writer.u64(self.base_deployment_slot)?;
        writer.key(&self.wrapper_program_data)?;
        writer.u64(self.wrapper_deployment_slot)?;
        writer.key(&self.token_2022_program)?;
        writer.key(&self.token_2022_program_data)?;
        writer.u64(self.token_2022_deployment_slot)?;
        writer.key(&self.market)?;
        writer.key(&self.terms)?;
        writer.amounts(&self.primitive)?;
        writer.u8(self.state)?;
        writer.u8(self.descriptor_bump)?;
        writer.u8(self.mint_bump)?;
        writer.u8(self.vault_owner_bump)?;
        writer.done()
    }

    /// Decode exactly [`DESCRIPTOR_BYTES`] hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_input(input, DESCRIPTOR_BYTES)?;
        let mut reader = Reader::new(input);
        reader.header(DESCRIPTOR_TAG)?;
        let value = Self {
            base_program: reader.key()?,
            base_program_data: reader.key()?,
            base_deployment_slot: reader.u64()?,
            wrapper_program_data: reader.key()?,
            wrapper_deployment_slot: reader.u64()?,
            token_2022_program: reader.key()?,
            token_2022_program_data: reader.key()?,
            token_2022_deployment_slot: reader.u64()?,
            market: reader.key()?,
            terms: reader.key()?,
            primitive: reader.amounts()?,
            state: reader.u8()?,
            descriptor_bump: reader.u8()?,
            mint_bump: reader.u8()?,
            vault_owner_bump: reader.u8()?,
        };
        reader.done()?;
        value.validate_shape()?;
        Ok(value)
    }
}

/// Per-actor replay anchor for wrapper instructions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WrapperReplayV1 {
    /// Canonical descriptor account address.
    pub descriptor: Key,
    /// Signer or permissionless caller namespace.
    pub actor: Key,
    /// Exact next wrapper request sequence.
    pub sequence: u64,
    /// Replay PDA bump.
    pub stored_bump: u8,
}

impl WrapperReplayV1 {
    /// Validate its nonzero identity shape.
    pub fn validate(&self) -> Result<()> {
        if is_zero(&self.descriptor) || is_zero(&self.actor) || self.descriptor == self.actor {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
    }

    /// Encode exactly [`REPLAY_BYTES`] bytes.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        exact_output(output, REPLAY_BYTES)?;
        let mut writer = Writer::new(output);
        writer.header(REPLAY_TAG)?;
        writer.key(&self.descriptor)?;
        writer.key(&self.actor)?;
        writer.u64(self.sequence)?;
        writer.u8(self.stored_bump)?;
        writer.zeroes(3)?;
        writer.done()
    }

    /// Decode exactly [`REPLAY_BYTES`] hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_input(input, REPLAY_BYTES)?;
        let mut reader = Reader::new(input);
        reader.header(REPLAY_TAG)?;
        let value = Self {
            descriptor: reader.key()?,
            actor: reader.key()?,
            sequence: reader.u64()?,
            stored_bump: reader.u8()?,
        };
        reader.zeroes(3)?;
        reader.done()?;
        value.validate()?;
        Ok(value)
    }
}

/// Wrapper adapter operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Action {
    /// Mint from cash plus residual native Eggs.
    WrapCanonical = 1,
    /// Mint from a full native Egg vector, merging its complete-set floor.
    WrapFull = 2,
    /// Burn and return cash plus residual native Eggs.
    UnwindCanonical = 3,
    /// Burn, split the cash floor, and return the full native Egg vector.
    UnwindFull = 4,
    /// Donate every surplus created by direct holder burns.
    CompactDonation = 5,
    /// Burn an exact terminal lot and redeem its aggregate vector value.
    RedeemVector = 6,
    /// Permanently tombstone a fully empty descriptor.
    Retire = 7,
}

impl Action {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::WrapCanonical),
            2 => Ok(Self::WrapFull),
            3 => Ok(Self::UnwindCanonical),
            4 => Ok(Self::UnwindFull),
            5 => Ok(Self::CompactDonation),
            6 => Ok(Self::RedeemVector),
            7 => Ok(Self::Retire),
            _ => Err(Error::NonCanonical),
        }
    }
}

/// Exact replay and optimistic-concurrency envelope for one route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestV1 {
    /// Requested route.
    pub action: Action,
    /// Exact current wrapper replay sequence.
    pub wrapper_sequence: u64,
    /// Exact current source/beneficiary base replay sequence.
    pub source_base_sequence: u64,
    /// Exact current vault base replay sequence.
    pub vault_base_sequence: u64,
    /// Wrapper atoms; zero only for compaction and retirement.
    pub quantity: u64,
    /// Exact pre-instruction Token-2022 mint supply.
    pub expected_mint_supply: u64,
    /// Exact pre-instruction holder token balance.
    pub expected_holder_amount: u64,
    /// Exact source/beneficiary Position generation.
    pub source_generation: u64,
    /// Exact wrapper-vault Position generation.
    pub vault_generation: u64,
}

impl RequestV1 {
    /// Validate operation-specific zero/positive quantity rules.
    pub fn validate(&self) -> Result<()> {
        match self.action {
            Action::CompactDonation | Action::Retire if self.quantity != 0 => {
                Err(Error::NonCanonical)
            }
            Action::CompactDonation | Action::Retire => Ok(()),
            _ if self.quantity == 0 => Err(Error::NonCanonical),
            _ => Ok(()),
        }
    }

    /// Encode exactly [`REQUEST_BYTES`] bytes.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        exact_output(output, REQUEST_BYTES)?;
        let mut writer = Writer::new(output);
        writer.header(REQUEST_TAG)?;
        writer.u8(self.action as u8)?;
        writer.zeroes(3)?;
        writer.u64(self.wrapper_sequence)?;
        writer.u64(self.source_base_sequence)?;
        writer.u64(self.vault_base_sequence)?;
        writer.u64(self.quantity)?;
        writer.u64(self.expected_mint_supply)?;
        writer.u64(self.expected_holder_amount)?;
        writer.u64(self.source_generation)?;
        writer.u64(self.vault_generation)?;
        writer.done()
    }

    /// Decode exactly [`REQUEST_BYTES`] hostile bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact_input(input, REQUEST_BYTES)?;
        let mut reader = Reader::new(input);
        reader.header(REQUEST_TAG)?;
        let action = Action::decode(reader.u8()?)?;
        reader.zeroes(3)?;
        let value = Self {
            action,
            wrapper_sequence: reader.u64()?,
            source_base_sequence: reader.u64()?,
            vault_base_sequence: reader.u64()?,
            quantity: reader.u64()?,
            expected_mint_supply: reader.u64()?,
            expected_holder_amount: reader.u64()?,
            source_generation: reader.u64()?,
            vault_generation: reader.u64()?,
        };
        reader.done()?;
        value.validate()?;
        Ok(value)
    }
}

const fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn exact_input(input: &[u8], expected: usize) -> Result<()> {
    if input.len() < expected {
        Err(Error::Truncated)
    } else if input.len() > expected {
        Err(Error::TrailingBytes)
    } else {
        Ok(())
    }
}

fn exact_output(output: &[u8], expected: usize) -> Result<()> {
    exact_input(output, expected)
}

struct Reader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, at: 0 }
    }

    fn header(&mut self, tag: u8) -> Result<()> {
        if self.u8()? != tag {
            return Err(Error::WrongTag);
        }
        if self.u8()? != VERSION {
            return Err(Error::WrongVersion);
        }
        if self.u16()? != 0 {
            return Err(Error::NonCanonical);
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8> {
        let value = *self.input.get(self.at).ok_or(Error::Truncated)?;
        self.at += 1;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16> {
        let bytes = self.take::<2>()?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64> {
        let bytes = self.take::<8>()?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn key(&mut self) -> Result<Key> {
        self.take::<32>()
    }

    fn amounts(&mut self) -> Result<[u64; MAX_OUTCOMES]> {
        let mut values = [0; MAX_OUTCOMES];
        let mut i = 0;
        while i < MAX_OUTCOMES {
            values[i] = self.u64()?;
            i += 1;
        }
        Ok(values)
    }

    fn zeroes(&mut self, count: usize) -> Result<()> {
        let end = self.at.checked_add(count).ok_or(Error::Arithmetic)?;
        let bytes = self.input.get(self.at..end).ok_or(Error::Truncated)?;
        if bytes.iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonical);
        }
        self.at = end;
        Ok(())
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).ok_or(Error::Arithmetic)?;
        let bytes = self.input.get(self.at..end).ok_or(Error::Truncated)?;
        let mut value = [0; N];
        value.copy_from_slice(bytes);
        self.at = end;
        Ok(value)
    }

    fn done(&self) -> Result<()> {
        if self.at == self.input.len() {
            Ok(())
        } else {
            Err(Error::TrailingBytes)
        }
    }
}

struct Writer<'a> {
    output: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self { output, at: 0 }
    }

    fn header(&mut self, tag: u8) -> Result<()> {
        self.u8(tag)?;
        self.u8(VERSION)?;
        self.put(&0_u16.to_le_bytes())
    }

    fn u8(&mut self, value: u8) -> Result<()> {
        self.put(&[value])
    }

    fn u64(&mut self, value: u64) -> Result<()> {
        self.put(&value.to_le_bytes())
    }

    fn key(&mut self, value: &Key) -> Result<()> {
        self.put(value)
    }

    fn amounts(&mut self, values: &[u64; MAX_OUTCOMES]) -> Result<()> {
        let mut i = 0;
        while i < MAX_OUTCOMES {
            self.u64(values[i])?;
            i += 1;
        }
        Ok(())
    }

    fn zeroes(&mut self, count: usize) -> Result<()> {
        let mut i = 0;
        while i < count {
            self.u8(0)?;
            i += 1;
        }
        Ok(())
    }

    fn put(&mut self, bytes: &[u8]) -> Result<()> {
        let end = self.at.checked_add(bytes.len()).ok_or(Error::Arithmetic)?;
        let destination = self.output.get_mut(self.at..end).ok_or(Error::Truncated)?;
        destination.copy_from_slice(bytes);
        self.at = end;
        Ok(())
    }

    fn done(&self) -> Result<()> {
        if self.at == self.output.len() {
            Ok(())
        } else {
            Err(Error::TrailingBytes)
        }
    }
}
