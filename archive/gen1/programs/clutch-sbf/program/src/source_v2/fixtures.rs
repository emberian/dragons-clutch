//! Byte-accurate fabricated account images for the pull profile.
//!
//! Every builder here writes the **real** layout of the account it names, at
//! the offsets the corresponding decoder reads, so a fixture exercises the
//! decoder rather than a convenient parallel encoding:
//!
//! | builder | real layout it reproduces |
//! | --- | --- |
//! | [`receiver_program_body`] | `UpgradeableLoaderState::Program`, `bincode` fixint LE |
//! | [`programdata_body`] | `UpgradeableLoaderState::ProgramData` plus the ELF tail |
//! | [`config_body`] | an Anchor-framed receiver `Config`: discriminator, borsh fields |
//! | [`price_update_body`] | the 134-byte `PriceUpdateV2` account |
//! | [`instructions_sysvar_body`] | the serialized Instructions sysvar, offset table and trailer included |
//!
//! Compiled only under `cfg(test)`: these are laboratory constructors and have
//! no business inside a deployable ELF. When the bank campaign lands, the
//! builders move behind a dev-only surface rather than becoming reachable code
//! — an ELF that can *write* a provider account is a different artifact from
//! one that can only read one.
//!
//! The `Config` builder deserves a note. The runtime deliberately has no
//! `Config` codec, because the profile authenticates that account by SHA-256
//! over its complete body and a codec would create a field-level exception
//! where the design permits none. The builder therefore exists purely so the
//! fixture is *shaped* like a real governance account rather than an opaque
//! blob — nothing reads its fields back, and the test that matters is that any
//! single-byte change is a different generation.

use crate::instructions_sysvar::{ACCOUNT_META_LEN, META_FLAG_IS_SIGNER, META_FLAG_IS_WRITABLE};
use crate::loader_state::{
    LOADER_TAG_PROGRAM, LOADER_TAG_PROGRAMDATA, OPTION_NONE, OPTION_SOME, PROGRAMDATA_METADATA_LEN,
    PROGRAM_ACCOUNT_METADATA_LEN,
};
use crate::pyth_receiver::{
    PRICE_UPDATE_V2_ACCOUNT_LEN, PRICE_UPDATE_V2_DISCRIMINATOR, VERIFICATION_LEVEL_FULL,
};

/// One account meta as the Instructions sysvar serializes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetaFixture {
    /// Account address.
    pub address: [u8; 32],
    /// Whether the transaction presented it as a signer.
    pub is_signer: bool,
    /// Whether the transaction presented it as writable.
    pub is_writable: bool,
}

impl MetaFixture {
    /// A read-only, non-signing meta.
    pub const fn plain(address: [u8; 32]) -> Self {
        Self {
            address,
            is_signer: false,
            is_writable: false,
        }
    }
}

/// One instruction as the Instructions sysvar serializes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionFixture {
    /// Program the instruction invokes.
    pub program: [u8; 32],
    /// Account metas, in ABI position order.
    pub metas: Vec<MetaFixture>,
    /// Instruction data.
    pub data: Vec<u8>,
}

/// `UpgradeableLoaderState::Program { programdata_address }`.
pub fn receiver_program_body(programdata: [u8; 32]) -> Vec<u8> {
    let mut out = LOADER_TAG_PROGRAM.to_le_bytes().to_vec();
    out.extend_from_slice(&programdata);
    debug_assert_eq!(out.len(), PROGRAM_ACCOUNT_METADATA_LEN);
    out
}

/// `UpgradeableLoaderState::ProgramData { slot, upgrade_authority }` plus the
/// deployed bytes the loader stores after the fixed 45-byte metadata region.
///
/// Passing `None` writes the **revoked** image: the discriminant is zero and
/// the thirty-two bytes that follow keep whatever `stale_authority` supplies,
/// which is exactly the hazard `loader_state`'s finding 1 describes. A decoder
/// that reads them would report a live authority on an immutable program.
pub fn programdata_body(
    slot: u64,
    upgrade_authority: Option<[u8; 32]>,
    stale_authority: [u8; 32],
    elf: &[u8],
) -> Vec<u8> {
    let mut out = LOADER_TAG_PROGRAMDATA.to_le_bytes().to_vec();
    out.extend_from_slice(&slot.to_le_bytes());
    match upgrade_authority {
        Some(authority) => {
            out.push(OPTION_SOME);
            out.extend_from_slice(&authority);
        }
        None => {
            out.push(OPTION_NONE);
            out.extend_from_slice(&stale_authority);
        }
    }
    debug_assert_eq!(out.len(), PROGRAMDATA_METADATA_LEN);
    out.extend_from_slice(elf);
    out
}

/// An Anchor-framed receiver `Config` body.
///
/// Shape only: an eight-byte account discriminator, a governance authority, an
/// optional pending authority, a wormhole address, a length-prefixed vector of
/// `(chain, emitter)` data sources, a fee, and a signature threshold. Nothing
/// reads these fields back — see the module note.
pub fn config_body(
    discriminator: [u8; 8],
    governance_authority: [u8; 32],
    pending_authority: Option<[u8; 32]>,
    wormhole: [u8; 32],
    data_sources: &[(u16, [u8; 32])],
    single_update_fee_lamports: u64,
    minimum_signatures: u8,
) -> Vec<u8> {
    let mut out = discriminator.to_vec();
    out.extend_from_slice(&governance_authority);
    match pending_authority {
        Some(authority) => {
            out.push(1);
            out.extend_from_slice(&authority);
        }
        None => out.push(0),
    }
    out.extend_from_slice(&wormhole);
    out.extend_from_slice(&(data_sources.len() as u32).to_le_bytes());
    for (chain, emitter) in data_sources {
        out.extend_from_slice(&chain.to_le_bytes());
        out.extend_from_slice(emitter);
    }
    out.extend_from_slice(&single_update_fee_lamports.to_le_bytes());
    out.push(minimum_signatures);
    out
}

/// Every field of a fabricated `PriceUpdateV2` account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceUpdateFixture {
    /// Recorded write authority.
    pub write_authority: [u8; 32],
    /// Verification-level discriminant; `1` is `Full`.
    pub verification_level: u8,
    /// Provider feed id.
    pub feed_id: [u8; 32],
    /// Signed price.
    pub price: i64,
    /// Confidence half-width.
    pub confidence: u64,
    /// Decimal exponent.
    pub exponent: i32,
    /// Aggregate publish time.
    pub publish_time: i64,
    /// Previous successful aggregate's publish time.
    pub prev_publish_time: i64,
    /// EMA price.
    pub ema_price: i64,
    /// EMA confidence.
    pub ema_confidence: u64,
    /// Receiver-write slot.
    pub posted_slot: u64,
    /// Trailing byte; zero for a fully verified message.
    pub trailing_pad: u8,
}

impl PriceUpdateFixture {
    /// A well-formed fully verified update for one feed.
    pub const fn new(
        write_authority: [u8; 32],
        feed_id: [u8; 32],
        publish_time: i64,
        prev_publish_time: i64,
        posted_slot: u64,
    ) -> Self {
        Self {
            write_authority,
            verification_level: VERIFICATION_LEVEL_FULL,
            feed_id,
            price: 123_456_789,
            confidence: 12_345,
            exponent: -8,
            publish_time,
            prev_publish_time,
            ema_price: 123_450_000,
            ema_confidence: 20_000,
            posted_slot,
            trailing_pad: 0,
        }
    }
}

/// Serialize one `PriceUpdateV2` account body.
pub fn price_update_body(fixture: PriceUpdateFixture) -> Vec<u8> {
    price_update_body_with_discriminator(fixture, PRICE_UPDATE_V2_DISCRIMINATOR)
}

/// Serialize one `PriceUpdateV2` account body under a chosen discriminator.
///
/// The discriminator is a parameter so the hostile battery can present an
/// otherwise perfect account under the wrong magic.
pub fn price_update_body_with_discriminator(
    fixture: PriceUpdateFixture,
    discriminator: [u8; 8],
) -> Vec<u8> {
    let mut out = vec![0_u8; PRICE_UPDATE_V2_ACCOUNT_LEN];
    out[..8].copy_from_slice(&discriminator);
    out[8..40].copy_from_slice(&fixture.write_authority);
    out[40] = fixture.verification_level;
    out[41..73].copy_from_slice(&fixture.feed_id);
    out[73..81].copy_from_slice(&fixture.price.to_le_bytes());
    out[81..89].copy_from_slice(&fixture.confidence.to_le_bytes());
    out[89..93].copy_from_slice(&fixture.exponent.to_le_bytes());
    out[93..101].copy_from_slice(&fixture.publish_time.to_le_bytes());
    out[101..109].copy_from_slice(&fixture.prev_publish_time.to_le_bytes());
    out[109..117].copy_from_slice(&fixture.ema_price.to_le_bytes());
    out[117..125].copy_from_slice(&fixture.ema_confidence.to_le_bytes());
    out[125..133].copy_from_slice(&fixture.posted_slot.to_le_bytes());
    out[133] = fixture.trailing_pad;
    out
}

/// Serialize an Instructions-sysvar account body.
///
/// The runtime writes a `u16` count, a `u16` offset per instruction, each
/// instruction's `(account_count, metas, program_id, data_len, data)` body,
/// and a two-byte current-index trailer outside the documented layout.
pub fn instructions_sysvar_body(instructions: &[InstructionFixture], current: u16) -> Vec<u8> {
    let count = instructions.len();
    let table_bytes = 2 + 2 * count;
    let mut bodies: Vec<Vec<u8>> = Vec::with_capacity(count);
    for instruction in instructions {
        let mut body = Vec::new();
        body.extend_from_slice(&(instruction.metas.len() as u16).to_le_bytes());
        for meta in &instruction.metas {
            let mut flags = 0_u8;
            if meta.is_signer {
                flags |= META_FLAG_IS_SIGNER;
            }
            if meta.is_writable {
                flags |= META_FLAG_IS_WRITABLE;
            }
            body.push(flags);
            body.extend_from_slice(&meta.address);
            debug_assert_eq!(1 + 32, ACCOUNT_META_LEN);
        }
        body.extend_from_slice(&instruction.program);
        body.extend_from_slice(&(instruction.data.len() as u16).to_le_bytes());
        body.extend_from_slice(&instruction.data);
        bodies.push(body);
    }

    let mut out = (count as u16).to_le_bytes().to_vec();
    let mut at = table_bytes;
    for body in &bodies {
        out.extend_from_slice(&(at as u16).to_le_bytes());
        at += body.len();
    }
    for body in &bodies {
        out.extend_from_slice(body);
    }
    out.extend_from_slice(&current.to_le_bytes());
    out
}

/// The seven-account post instruction the fixture receiver ABI addresses.
///
/// Positions match the shape of the reviewed Pyth `PostUpdate` context: payer,
/// encoded VAA / guardian set, config, treasury, price-update account, system
/// program, write authority.
pub fn post_instruction(
    receiver_program: [u8; 32],
    config: [u8; 32],
    update_account: [u8; 32],
    write_authority: [u8; 32],
) -> InstructionFixture {
    InstructionFixture {
        program: receiver_program,
        metas: vec![
            MetaFixture {
                address: [0x01; 32],
                is_signer: true,
                is_writable: true,
            },
            MetaFixture::plain([0x02; 32]),
            MetaFixture::plain(config),
            MetaFixture {
                address: [0x04; 32],
                is_signer: false,
                is_writable: true,
            },
            MetaFixture {
                address: update_account,
                is_signer: true,
                is_writable: true,
            },
            MetaFixture::plain([0x06; 32]),
            MetaFixture {
                address: write_authority,
                is_signer: true,
                is_writable: false,
            },
        ],
        data: crate::source_identity::fixture::POST_UPDATE_DISCRIMINATOR.to_vec(),
    }
}

/// The consuming instruction: this program's own archive append.
pub fn consuming_instruction(clutch_program: [u8; 32]) -> InstructionFixture {
    InstructionFixture {
        program: clutch_program,
        metas: vec![MetaFixture::plain([0x21; 32])],
        data: vec![0x19],
    }
}

/// A canonical Clock sysvar body carrying one slot and one signed timestamp.
pub fn clock_body(slot: u64, unix_timestamp: i64) -> Vec<u8> {
    let mut out = vec![0_u8; super::auth::CLOCK_SYSVAR_LEN];
    out[..8].copy_from_slice(&slot.to_le_bytes());
    out[super::auth::CLOCK_UNIX_TIMESTAMP_OFFSET..].copy_from_slice(&unix_timestamp.to_le_bytes());
    out
}
