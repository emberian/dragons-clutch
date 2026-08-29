//! Claims-owned atomic execution contract for one selected Fractional coordinate.
//!
//! The first executable rung deliberately admits only open-market Wrap and
//! WholeUnwrap. Both mutate the canonical native Claims positions and the
//! terms-selected Token-2022 Mint in one Claims instruction. Terminal actions
//! require the wider terminal/Custody frame and therefore cannot silently use
//! this account layout.

use core::convert::TryInto;

use crate::{FractionalExposureActionV2, FractionalExposureRequestV2};

/// Existing SignedDeltaV3 frame for the two ordered Positions.
pub const FRACTIONAL_ATOMIC_SIGNED_DELTA_ACCOUNT_COUNT_V3: usize = 22;
/// Finalized Fractional terms raw record.
pub const FRACTIONAL_ATOMIC_TERMS_RAW_V3: usize = 22;
/// Vacant finalized Fractional terms staging cursor.
pub const FRACTIONAL_ATOMIC_TERMS_STAGING_V3: usize = 23;
/// Finalized TokenBehavior selection raw record.
pub const FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3: usize = 24;
/// Vacant finalized TokenBehavior selection staging cursor.
pub const FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3: usize = 25;
/// Trading-owned Fractional root and Token controller PDA.
pub const FRACTIONAL_ATOMIC_ROOT_V3: usize = 26;
/// Holder identity signing the economic action.
pub const FRACTIONAL_ATOMIC_ACTOR_V3: usize = 27;
/// Terms-selected shard Mint.
pub const FRACTIONAL_ATOMIC_SHARD_MINT_V3: usize = 28;
/// Holder Token account minted into or burned from.
pub const FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3: usize = 29;
/// Terms-selected Token-2022 program.
pub const FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3: usize = 30;
/// Exact open-action Claims child frame width.
pub const FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3: usize = 31;

/// Existing family-neutral terminal Claims+Custody frame width.
pub const FRACTIONAL_TERMINAL_BASE_ACCOUNT_COUNT_V3: usize = 36;
/// Finalized Fractional terms raw record in the terminal frame.
pub const FRACTIONAL_TERMINAL_TERMS_RAW_V3: usize = 36;
/// Vacant Fractional terms staging cursor in the terminal frame.
pub const FRACTIONAL_TERMINAL_TERMS_STAGING_V3: usize = 37;
/// Finalized TokenBehavior raw record in the terminal frame.
pub const FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3: usize = 38;
/// Vacant TokenBehavior staging cursor in the terminal frame.
pub const FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3: usize = 39;
/// Trading-owned Fractional root and shard-Mint controller.
pub const FRACTIONAL_TERMINAL_ROOT_V3: usize = 40;
/// Holder signing the terminal shard burn.
pub const FRACTIONAL_TERMINAL_ACTOR_V3: usize = 41;
/// Terms-selected shard Mint burned after terminal settlement.
pub const FRACTIONAL_TERMINAL_SHARD_MINT_V3: usize = 42;
/// Holder shard Token account burned after terminal settlement.
pub const FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3: usize = 43;
/// Exact Fractional terminal Claims+Custody+Token frame width.
pub const FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3: usize = 44;

/// Stable Fractional root PDA domain under the activated Trading program.
pub const FRACTIONAL_ROOT_PDA_SEED_V1: &[u8] = b"dclutch/fractional-root-v1";

/// Exact fixed atomic receipt width.
pub const FRACTIONAL_ATOMIC_RECEIPT_BYTES_V3: usize = 256;
/// Atomic receipt magic.
pub const FRACTIONAL_ATOMIC_RECEIPT_MAGIC_V3: [u8; 8] = *b"DCFRAA03";
/// Atomic receipt schema preimage.
pub const FRACTIONAL_ATOMIC_RECEIPT_SCHEMA_PREIMAGE_V3: &[u8] = b"dclutch/schema/fractional-atomic-receipt-v3|bytes256|claims-signed-delta+token2022|request+postresources+revisions+exact-supply";
/// Exact terminal atomic receipt width.
pub const FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_BYTES_V3: usize = 256;
/// Terminal atomic receipt magic.
pub const FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_MAGIC_V3: [u8; 8] = *b"DCFRTM03";
/// Terminal atomic receipt schema preimage.
pub const FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_SCHEMA_PREIMAGE_V3: &[u8] = b"dclutch/schema/fractional-terminal-atomic-receipt-v3|bytes256|claims+custody+token2022|chain-derived-recipient+replay|exact-denominator-burn";

const VERSION: u16 = 3;
const REQUEST_DIGEST: usize = 16;
const SIGNED_PACKET_DIGEST: usize = 48;
const SIGNED_RECEIPT_DIGEST: usize = 80;
const TOKEN_POST_DIGEST: usize = 112;
const CLAIMS_POST_DIGEST: usize = 144;
const ROOT: usize = 176;
const POST_MARKET_REVISION: usize = 208;
const POST_ACTOR_REVISION: usize = 216;
const POST_RESERVE_REVISION: usize = 224;
const POST_MINT_SUPPLY: usize = 232;
const POST_HOLDER_AMOUNT: usize = 240;
const CONSUMED_SHARDS: usize = 248;

/// Exact evidence emitted only after both Claims and Token postconditions hold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalAtomicReceiptV3 {
    action: FractionalExposureActionV2,
    request_digest: [u8; 32],
    signed_packet_digest: [u8; 32],
    signed_receipt_digest: [u8; 32],
    token_post_digest: [u8; 32],
    claims_post_digest: [u8; 32],
    root: [u8; 32],
    post_market_revision: u64,
    post_actor_revision: u64,
    post_reserve_revision: u64,
    post_mint_supply: u64,
    post_holder_amount: u64,
    consumed_shards: u64,
}

impl FractionalAtomicReceiptV3 {
    /// Construct exact nonzero atomic completion evidence.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: FractionalExposureActionV2,
        request_digest: [u8; 32],
        signed_packet_digest: [u8; 32],
        signed_receipt_digest: [u8; 32],
        token_post_digest: [u8; 32],
        claims_post_digest: [u8; 32],
        root: [u8; 32],
        post_market_revision: u64,
        post_actor_revision: u64,
        post_reserve_revision: u64,
        post_mint_supply: u64,
        post_holder_amount: u64,
        consumed_shards: u64,
    ) -> Result<Self> {
        if !matches!(
            action,
            FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap
        ) || [
            request_digest,
            signed_packet_digest,
            signed_receipt_digest,
            token_post_digest,
            claims_post_digest,
            root,
        ]
        .contains(&[0; 32])
            || post_market_revision == 0
            || post_actor_revision == 0
            || post_reserve_revision == 0
            || consumed_shards == 0
        {
            return Err(FractionalAtomicReceiptErrorV3::InvalidFields);
        }
        Ok(Self {
            action,
            request_digest,
            signed_packet_digest,
            signed_receipt_digest,
            token_post_digest,
            claims_post_digest,
            root,
            post_market_revision,
            post_actor_revision,
            post_reserve_revision,
            post_mint_supply,
            post_holder_amount,
            consumed_shards,
        })
    }

    /// Hostile-decode exact canonical receipt bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FRACTIONAL_ATOMIC_RECEIPT_BYTES_V3 {
            return Err(FractionalAtomicReceiptErrorV3::InvalidLength);
        }
        if bytes.get(..8) != Some(FRACTIONAL_ATOMIC_RECEIPT_MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != VERSION
            || bytes
                .get(11..16)
                .is_none_or(|value| value.iter().any(|byte| *byte != 0))
        {
            return Err(FractionalAtomicReceiptErrorV3::InvalidHeader);
        }
        Self::new(
            decode_action(
                *bytes
                    .get(10)
                    .ok_or(FractionalAtomicReceiptErrorV3::InvalidLength)?,
            )?,
            array(bytes, REQUEST_DIGEST)?,
            array(bytes, SIGNED_PACKET_DIGEST)?,
            array(bytes, SIGNED_RECEIPT_DIGEST)?,
            array(bytes, TOKEN_POST_DIGEST)?,
            array(bytes, CLAIMS_POST_DIGEST)?,
            array(bytes, ROOT)?,
            read_u64(bytes, POST_MARKET_REVISION)?,
            read_u64(bytes, POST_ACTOR_REVISION)?,
            read_u64(bytes, POST_RESERVE_REVISION)?,
            read_u64(bytes, POST_MINT_SUPPLY)?,
            read_u64(bytes, POST_HOLDER_AMOUNT)?,
            read_u64(bytes, CONSUMED_SHARDS)?,
        )
    }

    /// Encode exact canonical receipt bytes.
    pub fn to_bytes(self) -> [u8; FRACTIONAL_ATOMIC_RECEIPT_BYTES_V3] {
        let mut output = [0; FRACTIONAL_ATOMIC_RECEIPT_BYTES_V3];
        output[..8].copy_from_slice(&FRACTIONAL_ATOMIC_RECEIPT_MAGIC_V3);
        output[8..10].copy_from_slice(&VERSION.to_le_bytes());
        output[10] = self.action.byte();
        for (offset, value) in [
            (REQUEST_DIGEST, self.request_digest),
            (SIGNED_PACKET_DIGEST, self.signed_packet_digest),
            (SIGNED_RECEIPT_DIGEST, self.signed_receipt_digest),
            (TOKEN_POST_DIGEST, self.token_post_digest),
            (CLAIMS_POST_DIGEST, self.claims_post_digest),
            (ROOT, self.root),
        ] {
            output[offset..offset + 32].copy_from_slice(&value);
        }
        for (offset, value) in [
            (POST_MARKET_REVISION, self.post_market_revision),
            (POST_ACTOR_REVISION, self.post_actor_revision),
            (POST_RESERVE_REVISION, self.post_reserve_revision),
            (POST_MINT_SUPPLY, self.post_mint_supply),
            (POST_HOLDER_AMOUNT, self.post_holder_amount),
            (CONSUMED_SHARDS, self.consumed_shards),
        ] {
            output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        output
    }

    /// Validate the receipt against the exact family request and request digest.
    pub fn verify_for(
        self,
        request: FractionalExposureRequestV2,
        request_digest: [u8; 32],
    ) -> Result<()> {
        if self.action != request.action()
            || self.request_digest != request_digest
            || self.post_market_revision == 0
            || self.consumed_shards == 0
        {
            Err(FractionalAtomicReceiptErrorV3::Mismatch)
        } else {
            Ok(())
        }
    }

    /// Selected action.
    pub const fn action(self) -> FractionalExposureActionV2 {
        self.action
    }
    /// Family request digest.
    pub const fn request_digest(self) -> [u8; 32] {
        self.request_digest
    }
    /// Generated SignedDelta packet digest.
    pub const fn signed_packet_digest(self) -> [u8; 32] {
        self.signed_packet_digest
    }
    /// Immediate SignedDelta receipt digest.
    pub const fn signed_receipt_digest(self) -> [u8; 32] {
        self.signed_receipt_digest
    }
    /// Immediate Token poststate digest.
    pub const fn token_post_digest(self) -> [u8; 32] {
        self.token_post_digest
    }
    /// Immediate Claims postresource digest.
    pub const fn claims_post_digest(self) -> [u8; 32] {
        self.claims_post_digest
    }
    /// Trading-owned Fractional root identity.
    pub const fn root(self) -> [u8; 32] {
        self.root
    }
    /// Post Claims Market revision.
    pub const fn post_market_revision(self) -> u64 {
        self.post_market_revision
    }
    /// Post actor Position revision.
    pub const fn post_actor_revision(self) -> u64 {
        self.post_actor_revision
    }
    /// Post reserve Position revision.
    pub const fn post_reserve_revision(self) -> u64 {
        self.post_reserve_revision
    }
    /// Exact raw shard Mint supply after the Token CPI.
    pub const fn post_mint_supply(self) -> u64 {
        self.post_mint_supply
    }
    /// Exact raw holder Token balance after the Token CPI.
    pub const fn post_holder_amount(self) -> u64 {
        self.post_holder_amount
    }
    /// Exact denominator-scaled shard atoms minted or burned.
    pub const fn consumed_shards(self) -> u64 {
        self.consumed_shards
    }
}

/// Stable receipt refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalAtomicReceiptErrorV3 {
    /// Receipt length differed.
    InvalidLength,
    /// Magic, version, action, or reserved bytes differed.
    InvalidHeader,
    /// A required field was zero or selected an unsupported action.
    InvalidFields,
    /// Receipt did not bind the exact request.
    Mismatch,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, FractionalAtomicReceiptErrorV3>;

/// Evidence emitted after terminal Claims, optional Custody, and shard burn all commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalTerminalAtomicReceiptV3 {
    action: FractionalExposureActionV2,
    request_digest: [u8; 32],
    terminal_request_digest: [u8; 32],
    terminal_receipt_digest: [u8; 32],
    terminal_post_resource_digest: [u8; 32],
    token_post_digest: [u8; 32],
    root: [u8; 32],
    payout: u64,
    post_mint_supply: u64,
    post_holder_amount: u64,
    consumed_shards: u64,
}

impl FractionalTerminalAtomicReceiptV3 {
    /// Construct exact terminal completion evidence.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: FractionalExposureActionV2,
        request_digest: [u8; 32],
        terminal_request_digest: [u8; 32],
        terminal_receipt_digest: [u8; 32],
        terminal_post_resource_digest: [u8; 32],
        token_post_digest: [u8; 32],
        root: [u8; 32],
        payout: u64,
        post_mint_supply: u64,
        post_holder_amount: u64,
        consumed_shards: u64,
    ) -> Result<Self> {
        if !matches!(
            action,
            FractionalExposureActionV2::TerminalRedeem
                | FractionalExposureActionV2::TerminalZeroBurn
        ) || [
            request_digest,
            terminal_request_digest,
            terminal_receipt_digest,
            terminal_post_resource_digest,
            token_post_digest,
            root,
        ]
        .contains(&[0; 32])
            || consumed_shards == 0
            || (action == FractionalExposureActionV2::TerminalRedeem) != (payout != 0)
        {
            return Err(FractionalAtomicReceiptErrorV3::InvalidFields);
        }
        Ok(Self {
            action,
            request_digest,
            terminal_request_digest,
            terminal_receipt_digest,
            terminal_post_resource_digest,
            token_post_digest,
            root,
            payout,
            post_mint_supply,
            post_holder_amount,
            consumed_shards,
        })
    }

    /// Decode exact canonical terminal receipt bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_BYTES_V3 {
            return Err(FractionalAtomicReceiptErrorV3::InvalidLength);
        }
        if bytes.get(..8) != Some(FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != VERSION
            || bytes
                .get(11..16)
                .is_none_or(|value| value.iter().any(|byte| *byte != 0))
            || bytes
                .get(240..256)
                .is_none_or(|value| value.iter().any(|byte| *byte != 0))
        {
            return Err(FractionalAtomicReceiptErrorV3::InvalidHeader);
        }
        Self::new(
            match *bytes
                .get(10)
                .ok_or(FractionalAtomicReceiptErrorV3::InvalidLength)?
            {
                3 => FractionalExposureActionV2::TerminalRedeem,
                4 => FractionalExposureActionV2::TerminalZeroBurn,
                _ => return Err(FractionalAtomicReceiptErrorV3::InvalidHeader),
            },
            array(bytes, 16)?,
            array(bytes, 48)?,
            array(bytes, 80)?,
            array(bytes, 112)?,
            array(bytes, 144)?,
            array(bytes, 176)?,
            read_u64(bytes, 208)?,
            read_u64(bytes, 216)?,
            read_u64(bytes, 224)?,
            read_u64(bytes, 232)?,
        )
    }

    /// Encode exact canonical terminal receipt bytes.
    pub fn to_bytes(self) -> [u8; FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_BYTES_V3] {
        let mut output = [0; FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_BYTES_V3];
        output[..8].copy_from_slice(&FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_MAGIC_V3);
        output[8..10].copy_from_slice(&VERSION.to_le_bytes());
        output[10] = self.action.byte();
        for (offset, value) in [
            (16, self.request_digest),
            (48, self.terminal_request_digest),
            (80, self.terminal_receipt_digest),
            (112, self.terminal_post_resource_digest),
            (144, self.token_post_digest),
            (176, self.root),
        ] {
            output[offset..offset + 32].copy_from_slice(&value);
        }
        for (offset, value) in [
            (208, self.payout),
            (216, self.post_mint_supply),
            (224, self.post_holder_amount),
            (232, self.consumed_shards),
        ] {
            output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        output
    }

    /// Verify exact family request binding and terminal action/payout shape.
    pub fn verify_for(
        self,
        request: FractionalExposureRequestV2,
        request_digest: [u8; 32],
    ) -> Result<()> {
        if self.action != request.action()
            || self.request_digest != request_digest
            || (self.action == FractionalExposureActionV2::TerminalRedeem) != (self.payout != 0)
        {
            Err(FractionalAtomicReceiptErrorV3::Mismatch)
        } else {
            Ok(())
        }
    }

    /// Selected terminal action.
    pub const fn action(self) -> FractionalExposureActionV2 {
        self.action
    }
    /// Family request digest.
    pub const fn request_digest(self) -> [u8; 32] {
        self.request_digest
    }
    /// Derived family-neutral terminal request digest.
    pub const fn terminal_request_digest(self) -> [u8; 32] {
        self.terminal_request_digest
    }
    /// Immediate terminal receipt digest.
    pub const fn terminal_receipt_digest(self) -> [u8; 32] {
        self.terminal_receipt_digest
    }
    /// Terminal Claims/Custody postresource digest.
    pub const fn terminal_post_resource_digest(self) -> [u8; 32] {
        self.terminal_post_resource_digest
    }
    /// Shard Token poststate digest.
    pub const fn token_post_digest(self) -> [u8; 32] {
        self.token_post_digest
    }
    /// Fractional root identity.
    pub const fn root(self) -> [u8; 32] {
        self.root
    }
    /// Chain-derived collateral payout.
    pub const fn payout(self) -> u64 {
        self.payout
    }
    /// Post shard-Mint supply.
    pub const fn post_mint_supply(self) -> u64 {
        self.post_mint_supply
    }
    /// Post holder shard balance.
    pub const fn post_holder_amount(self) -> u64 {
        self.post_holder_amount
    }
    /// Exact denominator multiple burned.
    pub const fn consumed_shards(self) -> u64 {
        self.consumed_shards
    }
}

fn decode_action(value: u8) -> Result<FractionalExposureActionV2> {
    match value {
        0 => Ok(FractionalExposureActionV2::Wrap),
        2 => Ok(FractionalExposureActionV2::WholeUnwrap),
        _ => Err(FractionalAtomicReceiptErrorV3::InvalidHeader),
    }
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(N)
                    .ok_or(FractionalAtomicReceiptErrorV3::InvalidLength)?,
        )
        .ok_or(FractionalAtomicReceiptErrorV3::InvalidLength)?
        .try_into()
        .map_err(|_| FractionalAtomicReceiptErrorV3::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> FractionalAtomicReceiptV3 {
        FractionalAtomicReceiptV3::new(
            FractionalExposureActionV2::Wrap,
            [1; 32],
            [2; 32],
            [3; 32],
            [4; 32],
            [5; 32],
            [6; 32],
            7,
            8,
            9,
            10,
            11,
            12,
        )
        .expect("receipt")
    }

    #[test]
    fn exact_receipt_round_trips_and_reserved_substitution_refuses() {
        let receipt = receipt();
        assert_eq!(
            FractionalAtomicReceiptV3::decode(&receipt.to_bytes()),
            Ok(receipt)
        );
        let mut hostile = receipt.to_bytes();
        hostile[15] = 1;
        assert_eq!(
            FractionalAtomicReceiptV3::decode(&hostile),
            Err(FractionalAtomicReceiptErrorV3::InvalidHeader)
        );
        assert!(FractionalAtomicReceiptV3::decode(&hostile[..255]).is_err());
    }

    #[test]
    fn terminal_action_cannot_cross_the_open_atomic_frame() {
        assert!(
            FractionalAtomicReceiptV3::new(
                FractionalExposureActionV2::TerminalRedeem,
                [1; 32],
                [2; 32],
                [3; 32],
                [4; 32],
                [5; 32],
                [6; 32],
                7,
                8,
                9,
                10,
                11,
                12,
            )
            .is_err()
        );
        assert_eq!(FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, 31);
        assert!(FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3 < 64);
    }

    #[test]
    fn terminal_receipt_enforces_positive_and_zero_payout_partition() {
        for (action, payout) in [
            (FractionalExposureActionV2::TerminalRedeem, 9),
            (FractionalExposureActionV2::TerminalZeroBurn, 0),
        ] {
            let receipt = FractionalTerminalAtomicReceiptV3::new(
                action, [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], payout, 7, 8, 9,
            )
            .expect("terminal receipt");
            assert_eq!(
                FractionalTerminalAtomicReceiptV3::decode(&receipt.to_bytes()),
                Ok(receipt)
            );
        }
        assert!(
            FractionalTerminalAtomicReceiptV3::new(
                FractionalExposureActionV2::TerminalRedeem,
                [1; 32],
                [2; 32],
                [3; 32],
                [4; 32],
                [5; 32],
                [6; 32],
                0,
                7,
                8,
                9,
            )
            .is_err()
        );
        assert!(FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3 < 64);
    }
}
