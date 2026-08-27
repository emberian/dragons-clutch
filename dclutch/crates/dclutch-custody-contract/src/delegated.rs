//! Successor Custody transfer contract for exact delegated-allowance exhaustion.

use crate::{
    CUSTODY_RECEIPT_BYTES_V1, CUSTODY_REQUEST_BYTES_V1, CompartmentV1, CustodyReceiptV1,
    CustodyRequestV1, Error, OperationV1, ReceiptEvidenceV1,
};

/// Distinct delegated-transfer request magic.
pub const DELEGATED_CUSTODY_REQUEST_MAGIC_V2: [u8; 8] = *b"DCLCUDQ2";
/// Distinct delegated-transfer receipt magic.
pub const DELEGATED_CUSTODY_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLCUDC2";
/// Exact successor request width.
pub const DELEGATED_CUSTODY_REQUEST_BYTES_V2: usize = 776;
/// Exact successor receipt width.
pub const DELEGATED_CUSTODY_RECEIPT_BYTES_V2: usize = 488;

const VERSION_V2: u16 = 2;
const BASE_OFFSET: usize = 16;
const REQUEST_DELEGATE_BEFORE_OFFSET: usize = BASE_OFFSET + CUSTODY_REQUEST_BYTES_V1;
const REQUEST_DELEGATE_AFTER_OFFSET: usize = REQUEST_DELEGATE_BEFORE_OFFSET + 32;
const REQUEST_TOTAL_OFFSET: usize = REQUEST_DELEGATE_AFTER_OFFSET + 32;
const REQUEST_ALLOWANCE_BEFORE_OFFSET: usize = REQUEST_TOTAL_OFFSET + 8;
const REQUEST_ALLOWANCE_AFTER_OFFSET: usize = REQUEST_ALLOWANCE_BEFORE_OFFSET + 8;
const RECEIPT_DELEGATE_BEFORE_OFFSET: usize = BASE_OFFSET + CUSTODY_RECEIPT_BYTES_V1;
const RECEIPT_DELEGATE_AFTER_OFFSET: usize = RECEIPT_DELEGATE_BEFORE_OFFSET + 32;
const RECEIPT_TOTAL_OFFSET: usize = RECEIPT_DELEGATE_AFTER_OFFSET + 32;
const RECEIPT_ALLOWANCE_BEFORE_OFFSET: usize = RECEIPT_TOTAL_OFFSET + 8;
const RECEIPT_ALLOWANCE_AFTER_OFFSET: usize = RECEIPT_ALLOWANCE_BEFORE_OFFSET + 8;

/// Canonical patchable coordinates of a [`DelegatedCustodyRequestV2`] wire.
///
/// Effect encoders use these coordinates together with
/// [`crate::CustodyRequestLayoutV1`] for fields inside [`Self::BASE`]. This
/// keeps the successor wrapper's byte geometry owned by this contract rather
/// than repeated in each composing venue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DelegatedCustodyRequestLayoutV2;

impl DelegatedCustodyRequestLayoutV2 {
    /// Start of the nested exact [`CustodyRequestV1`] wire.
    pub const BASE: usize = BASE_OFFSET;
    /// `starts_atomic_debit` as a canonical `0` or `1` byte.
    pub const STARTS_ATOMIC_DEBIT: usize = 10;
    /// `terminal` as a canonical `0` or `1` byte.
    pub const TERMINAL: usize = 11;
    /// Exact pre-transfer delegate identity.
    pub const DELEGATE_BEFORE: usize = REQUEST_DELEGATE_BEFORE_OFFSET;
    /// Exact post-transfer delegate identity; zero means revoked.
    pub const DELEGATE_AFTER: usize = REQUEST_DELEGATE_AFTER_OFFSET;
    /// Exact total atomic debit as little-endian `u64`.
    pub const TOTAL_DEBIT: usize = REQUEST_TOTAL_OFFSET;
    /// Exact pre-transfer delegated allowance as little-endian `u64`.
    pub const ALLOWANCE_BEFORE: usize = REQUEST_ALLOWANCE_BEFORE_OFFSET;
    /// Exact post-transfer delegated allowance as little-endian `u64`.
    pub const ALLOWANCE_AFTER: usize = REQUEST_ALLOWANCE_AFTER_OFFSET;
}

/// Stable refusal from the delegated-allowance successor contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DelegatedCustodyErrorV2 {
    /// The nested canonical Custody request or receipt refused.
    Base(Error),
    /// Width, magic, version, flags, or reserved bytes refused.
    Wire,
    /// The route was not an external-source positive transfer.
    Route,
    /// Delegate identity or exact allowance arithmetic refused.
    Allowance,
    /// Base receipt or exact observed token facts differed.
    Receipt,
}

impl From<Error> for DelegatedCustodyErrorV2 {
    fn from(value: Error) -> Self {
        Self::Base(value)
    }
}

/// One external-source transfer with an exact share of one atomic allowance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DelegatedCustodyRequestV2 {
    /// Existing canonical Custody transfer coordinates.
    pub custody: CustodyRequestV1,
    /// True only for the first positive transfer of the atomic debit.
    pub starts_atomic_debit: bool,
    /// True only for the last positive transfer of the atomic debit.
    pub terminal: bool,
    /// Exact nonzero Custody delegate before transfer.
    pub delegate_before: [u8; 32],
    /// Same delegate for intermediate transfers; zero for terminal revocation.
    pub delegate_after: [u8; 32],
    /// Exact total allowance authorized at atomic-debit start.
    pub total_debit: u64,
    /// Exact delegated allowance before this transfer.
    pub allowance_before: u64,
    /// Exact delegated allowance after this transfer.
    pub allowance_after: u64,
}

impl DelegatedCustodyRequestV2 {
    /// Validate route shape, start binding, decrement, and terminal exhaustion.
    pub fn validate(self) -> Result<(), DelegatedCustodyErrorV2> {
        self.custody.validate()?;
        if self.custody.operation != OperationV1::Transfer
            || self.custody.source_compartment != CompartmentV1::External
        {
            return Err(DelegatedCustodyErrorV2::Route);
        }
        if is_zero(self.delegate_before)
            || self.total_debit == 0
            || self.allowance_before == 0
            || self.total_debit < self.allowance_before
            || self.allowance_before.checked_sub(self.custody.amount) != Some(self.allowance_after)
            || self.starts_atomic_debit != (self.allowance_before == self.total_debit)
            || self.terminal != (self.allowance_after == 0)
            || (self.terminal && !is_zero(self.delegate_after))
            || (!self.terminal && self.delegate_after != self.delegate_before)
        {
            return Err(DelegatedCustodyErrorV2::Allowance);
        }
        Ok(())
    }

    /// Encode one exact successor request without reinterpreting V1 bytes.
    pub fn encode(
        self,
    ) -> Result<[u8; DELEGATED_CUSTODY_REQUEST_BYTES_V2], DelegatedCustodyErrorV2> {
        self.validate()?;
        let mut output = [0; DELEGATED_CUSTODY_REQUEST_BYTES_V2];
        output[..8].copy_from_slice(&DELEGATED_CUSTODY_REQUEST_MAGIC_V2);
        output[8..10].copy_from_slice(&VERSION_V2.to_le_bytes());
        output[10] = u8::from(self.starts_atomic_debit);
        output[11] = u8::from(self.terminal);
        put(&mut output, BASE_OFFSET, &self.custody.to_bytes()?)?;
        put(
            &mut output,
            REQUEST_DELEGATE_BEFORE_OFFSET,
            &self.delegate_before,
        )?;
        put(
            &mut output,
            REQUEST_DELEGATE_AFTER_OFFSET,
            &self.delegate_after,
        )?;
        put_u64(&mut output, REQUEST_TOTAL_OFFSET, self.total_debit)?;
        put_u64(
            &mut output,
            REQUEST_ALLOWANCE_BEFORE_OFFSET,
            self.allowance_before,
        )?;
        put_u64(
            &mut output,
            REQUEST_ALLOWANCE_AFTER_OFFSET,
            self.allowance_after,
        )?;
        Ok(output)
    }

    /// Hostile-decode one exact successor request.
    pub fn decode(input: &[u8]) -> Result<Self, DelegatedCustodyErrorV2> {
        header(
            input,
            &DELEGATED_CUSTODY_REQUEST_MAGIC_V2,
            DELEGATED_CUSTODY_REQUEST_BYTES_V2,
        )?;
        let starts_atomic_debit = read_bool(input, 10)?;
        let terminal = read_bool(input, 11)?;
        require_zero(input, 12, 4)?;
        let value = Self {
            custody: CustodyRequestV1::decode(slice(
                input,
                BASE_OFFSET,
                CUSTODY_REQUEST_BYTES_V1,
            )?)?,
            starts_atomic_debit,
            terminal,
            delegate_before: read_array(input, REQUEST_DELEGATE_BEFORE_OFFSET)?,
            delegate_after: read_array(input, REQUEST_DELEGATE_AFTER_OFFSET)?,
            total_debit: read_u64(input, REQUEST_TOTAL_OFFSET)?,
            allowance_before: read_u64(input, REQUEST_ALLOWANCE_BEFORE_OFFSET)?,
            allowance_after: read_u64(input, REQUEST_ALLOWANCE_AFTER_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Exact pre/post delegate facts parsed from the source token account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DelegatedAllowanceObservationV2 {
    /// Parsed pre-transfer delegate; zero represents `None`.
    pub delegate_before: [u8; 32],
    /// Parsed pre-transfer delegated amount.
    pub allowance_before: u64,
    /// Parsed post-transfer delegate; terminal canonical state is zero/`None`.
    pub delegate_after: [u8; 32],
    /// Parsed post-transfer delegated amount.
    pub allowance_after: u64,
}

/// Immediate acknowledgement binding balance and delegated-authority postconditions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DelegatedCustodyReceiptV2 {
    /// Canonical balance/replay receipt for the same exact request digest.
    pub custody: CustodyReceiptV1,
    /// True only for the first transfer of the exact total debit.
    pub starts_atomic_debit: bool,
    /// True only when allowance is zero and delegation is revoked.
    pub terminal: bool,
    /// Exact pre-transfer delegate.
    pub delegate_before: [u8; 32],
    /// Exact post-transfer delegate, zero only at terminal.
    pub delegate_after: [u8; 32],
    /// Exact original total atomic debit.
    pub total_debit: u64,
    /// Exact pre-transfer delegated amount.
    pub allowance_before: u64,
    /// Exact post-transfer delegated amount.
    pub allowance_after: u64,
}

impl DelegatedCustodyReceiptV2 {
    /// Construct exact successor evidence from canonical Custody and token facts.
    pub fn new(
        request: DelegatedCustodyRequestV2,
        request_digest: [u8; 32],
        evidence: ReceiptEvidenceV1,
        observed: DelegatedAllowanceObservationV2,
    ) -> Result<Self, DelegatedCustodyErrorV2> {
        request.validate()?;
        if observed.delegate_before != request.delegate_before
            || observed.allowance_before != request.allowance_before
            || observed.delegate_after != request.delegate_after
            || observed.allowance_after != request.allowance_after
        {
            return Err(DelegatedCustodyErrorV2::Receipt);
        }
        let custody = CustodyReceiptV1::new(request.custody, request_digest, evidence)?;
        Ok(Self {
            custody,
            starts_atomic_debit: request.starts_atomic_debit,
            terminal: request.terminal,
            delegate_before: observed.delegate_before,
            delegate_after: observed.delegate_after,
            total_debit: request.total_debit,
            allowance_before: observed.allowance_before,
            allowance_after: observed.allowance_after,
        })
    }

    /// Encode the distinct successor receipt.
    pub fn encode(
        self,
    ) -> Result<[u8; DELEGATED_CUSTODY_RECEIPT_BYTES_V2], DelegatedCustodyErrorV2> {
        if self.custody.operation != OperationV1::Transfer
            || self.custody.source_compartment != CompartmentV1::External
            || self.custody.amount == 0
            || self
                .custody
                .evidence
                .source_before
                .checked_sub(self.custody.amount)
                != Some(self.custody.evidence.source_after)
            || self
                .custody
                .evidence
                .destination_before
                .checked_add(self.custody.amount)
                != Some(self.custody.evidence.destination_after)
            || self.total_debit == 0
            || self.allowance_before == 0
            || is_zero(self.delegate_before)
            || self.total_debit < self.allowance_before
            || self.allowance_before.checked_sub(self.custody.amount) != Some(self.allowance_after)
            || self.starts_atomic_debit != (self.allowance_before == self.total_debit)
            || self.terminal != (self.allowance_after == 0)
            || (self.terminal && !is_zero(self.delegate_after))
            || (!self.terminal && self.delegate_after != self.delegate_before)
        {
            return Err(DelegatedCustodyErrorV2::Receipt);
        }
        let mut output = [0; DELEGATED_CUSTODY_RECEIPT_BYTES_V2];
        output[..8].copy_from_slice(&DELEGATED_CUSTODY_RECEIPT_MAGIC_V2);
        output[8..10].copy_from_slice(&VERSION_V2.to_le_bytes());
        output[10] = u8::from(self.starts_atomic_debit);
        output[11] = u8::from(self.terminal);
        put(&mut output, BASE_OFFSET, &self.custody.to_bytes()?)?;
        put(
            &mut output,
            RECEIPT_DELEGATE_BEFORE_OFFSET,
            &self.delegate_before,
        )?;
        put(
            &mut output,
            RECEIPT_DELEGATE_AFTER_OFFSET,
            &self.delegate_after,
        )?;
        put_u64(&mut output, RECEIPT_TOTAL_OFFSET, self.total_debit)?;
        put_u64(
            &mut output,
            RECEIPT_ALLOWANCE_BEFORE_OFFSET,
            self.allowance_before,
        )?;
        put_u64(
            &mut output,
            RECEIPT_ALLOWANCE_AFTER_OFFSET,
            self.allowance_after,
        )?;
        Ok(output)
    }

    /// Hostile-decode one exact successor receipt.
    pub fn decode(input: &[u8]) -> Result<Self, DelegatedCustodyErrorV2> {
        header(
            input,
            &DELEGATED_CUSTODY_RECEIPT_MAGIC_V2,
            DELEGATED_CUSTODY_RECEIPT_BYTES_V2,
        )?;
        let starts_atomic_debit = read_bool(input, 10)?;
        let terminal = read_bool(input, 11)?;
        require_zero(input, 12, 4)?;
        let value = Self {
            custody: CustodyReceiptV1::decode(slice(
                input,
                BASE_OFFSET,
                CUSTODY_RECEIPT_BYTES_V1,
            )?)?,
            starts_atomic_debit,
            terminal,
            delegate_before: read_array(input, RECEIPT_DELEGATE_BEFORE_OFFSET)?,
            delegate_after: read_array(input, RECEIPT_DELEGATE_AFTER_OFFSET)?,
            total_debit: read_u64(input, RECEIPT_TOTAL_OFFSET)?,
            allowance_before: read_u64(input, RECEIPT_ALLOWANCE_BEFORE_OFFSET)?,
            allowance_after: read_u64(input, RECEIPT_ALLOWANCE_AFTER_OFFSET)?,
        };
        value.encode()?;
        Ok(value)
    }
}

fn is_zero(value: [u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn header(input: &[u8], magic: &[u8; 8], width: usize) -> Result<(), DelegatedCustodyErrorV2> {
    if input.len() != width
        || input.get(..8) != Some(magic.as_slice())
        || read_u16(input, 8)? != VERSION_V2
    {
        return Err(DelegatedCustodyErrorV2::Wire);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<(), DelegatedCustodyErrorV2> {
    if slice(input, offset, width)?.iter().any(|byte| *byte != 0) {
        return Err(DelegatedCustodyErrorV2::Wire);
    }
    Ok(())
}

fn read_bool(input: &[u8], offset: usize) -> Result<bool, DelegatedCustodyErrorV2> {
    match *input.get(offset).ok_or(DelegatedCustodyErrorV2::Wire)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(DelegatedCustodyErrorV2::Wire),
    }
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, DelegatedCustodyErrorV2> {
    Ok(u16::from_le_bytes(
        slice(input, offset, 2)?
            .try_into()
            .map_err(|_| DelegatedCustodyErrorV2::Wire)?,
    ))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, DelegatedCustodyErrorV2> {
    Ok(u64::from_le_bytes(
        slice(input, offset, 8)?
            .try_into()
            .map_err(|_| DelegatedCustodyErrorV2::Wire)?,
    ))
}

fn read_array(input: &[u8], offset: usize) -> Result<[u8; 32], DelegatedCustodyErrorV2> {
    slice(input, offset, 32)?
        .try_into()
        .map_err(|_| DelegatedCustodyErrorV2::Wire)
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8], DelegatedCustodyErrorV2> {
    input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(DelegatedCustodyErrorV2::Wire)?,
        )
        .ok_or(DelegatedCustodyErrorV2::Wire)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), DelegatedCustodyErrorV2> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(DelegatedCustodyErrorV2::Wire)?,
        )
        .ok_or(DelegatedCustodyErrorV2::Wire)?
        .copy_from_slice(value);
    Ok(())
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) -> Result<(), DelegatedCustodyErrorV2> {
    put(output, offset, &value.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use dclutch_release_set_contract::ExecutionRoleV1;

    use super::*;
    use crate::ContextV1;

    fn id(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn custody(amount: u64) -> CustodyRequestV1 {
        CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: ExecutionRoleV1::Trading,
            source_compartment: CompartmentV1::External,
            destination_compartment: CompartmentV1::HoardPrincipal,
            release_set: id(1),
            market: id(2),
            realm: id(3),
            context: id(4),
            caller_program: id(5),
            semantic: ContextV1 {
                candidate: id(6),
                source_owner: id(7),
                destination_owner: [0; 32],
                order: id(8),
                parent_request_digest: id(9),
                order_nonce: 10,
                generation: 11,
                page_index: 12,
                execution_index: 13,
                transfer_index: 0,
            },
            source: id(14),
            destination: id(15),
            source_vault_context: [0; 32],
            destination_vault_context: id(4),
            mint: id(16),
            token_program: id(17),
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 3,
            resulting_revision: 4,
            amount,
            rent_lamports: 0,
        }
    }

    fn first() -> DelegatedCustodyRequestV2 {
        DelegatedCustodyRequestV2 {
            custody: custody(40),
            starts_atomic_debit: true,
            terminal: false,
            delegate_before: id(18),
            delegate_after: id(18),
            total_debit: 100,
            allowance_before: 100,
            allowance_after: 60,
        }
    }

    #[test]
    fn successor_roundtrips_without_reinterpreting_v1() {
        let request = first();
        let bytes = request.encode().expect("request bytes");
        assert_eq!(bytes.len(), DELEGATED_CUSTODY_REQUEST_BYTES_V2);
        assert_eq!(DelegatedCustodyRequestV2::decode(&bytes), Ok(request));
        assert_eq!(CustodyRequestV1::decode(&bytes), Err(Error::InvalidLength));

        let evidence = ReceiptEvidenceV1 {
            source_before: 500,
            source_after: 460,
            destination_before: 10,
            destination_after: 50,
            poststate_commitment: id(19),
            replay_state_digest: id(20),
        };
        let receipt = DelegatedCustodyReceiptV2::new(
            request,
            id(21),
            evidence,
            DelegatedAllowanceObservationV2 {
                delegate_before: id(18),
                allowance_before: 100,
                delegate_after: id(18),
                allowance_after: 60,
            },
        )
        .expect("receipt");
        let receipt_bytes = receipt.encode().expect("receipt bytes");
        assert_eq!(
            DelegatedCustodyReceiptV2::decode(&receipt_bytes),
            Ok(receipt)
        );
    }

    #[test]
    fn public_request_layout_tracks_successor_encoder() {
        let request = first();
        let bytes = request.encode().expect("request bytes");
        let base = request.custody.to_bytes().expect("base request bytes");

        assert_eq!(
            slice(
                &bytes,
                DelegatedCustodyRequestLayoutV2::BASE,
                CUSTODY_REQUEST_BYTES_V1,
            )
            .expect("base request"),
            base.as_slice()
        );
        assert_eq!(
            bytes[DelegatedCustodyRequestLayoutV2::STARTS_ATOMIC_DEBIT],
            1
        );
        assert_eq!(bytes[DelegatedCustodyRequestLayoutV2::TERMINAL], 0);
        assert_eq!(
            slice(&bytes, DelegatedCustodyRequestLayoutV2::DELEGATE_BEFORE, 32,)
                .expect("delegate before"),
            request.delegate_before.as_slice()
        );
        assert_eq!(
            slice(&bytes, DelegatedCustodyRequestLayoutV2::DELEGATE_AFTER, 32,)
                .expect("delegate after"),
            request.delegate_after.as_slice()
        );
        for (offset, expected) in [
            (
                DelegatedCustodyRequestLayoutV2::TOTAL_DEBIT,
                request.total_debit,
            ),
            (
                DelegatedCustodyRequestLayoutV2::ALLOWANCE_BEFORE,
                request.allowance_before,
            ),
            (
                DelegatedCustodyRequestLayoutV2::ALLOWANCE_AFTER,
                request.allowance_after,
            ),
        ] {
            assert_eq!(
                slice(&bytes, offset, 8).expect("integer field"),
                expected.to_le_bytes().as_slice()
            );
        }
        assert_eq!(
            DelegatedCustodyRequestLayoutV2::ALLOWANCE_AFTER + 8,
            DELEGATED_CUSTODY_REQUEST_BYTES_V2
        );
        assert_eq!(DelegatedCustodyRequestV2::decode(&bytes), Ok(request));
    }

    #[test]
    fn residual_terminal_substitution_and_noncanonical_bool_refuse() {
        let mut terminal = first();
        terminal.custody.amount = 100;
        terminal.terminal = true;
        terminal.allowance_after = 0;
        terminal.delegate_after = [0; 32];
        terminal.validate().expect("terminal");

        let mut residual = terminal;
        residual.allowance_after = 1;
        assert_eq!(residual.validate(), Err(DelegatedCustodyErrorV2::Allowance));
        let mut retained = terminal;
        retained.delegate_after = id(18);
        assert_eq!(retained.validate(), Err(DelegatedCustodyErrorV2::Allowance));
        let mut substituted = first();
        substituted.delegate_after = id(22);
        assert_eq!(
            substituted.validate(),
            Err(DelegatedCustodyErrorV2::Allowance)
        );

        let mut bytes = first().encode().expect("bytes");
        bytes[10] = 2;
        assert_eq!(
            DelegatedCustodyRequestV2::decode(&bytes),
            Err(DelegatedCustodyErrorV2::Wire)
        );
    }
}
