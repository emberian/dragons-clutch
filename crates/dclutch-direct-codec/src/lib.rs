#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout codecs for Lean-owned compiled Direct data.

mod generated_layout;
#[rustfmt::skip]
mod generated_lifecycle;
#[rustfmt::skip]
mod generated_registered_controller;
#[rustfmt::skip]
#[allow(missing_docs)]
mod generated_intent_v2;
pub mod artifacts_v4;
pub mod execution_v3;
pub mod intent_v2;
#[cfg(not(target_os = "solana"))]
pub mod ordinary_account_artifacts_v3;
pub mod ordinary_artifacts_v3;
#[cfg(not(target_os = "solana"))]
pub mod ordinary_bundle_v4;
#[cfg(not(target_os = "solana"))]
pub mod ordinary_effect_artifacts_v3;
pub mod ordinary_v3;
pub mod state_artifacts_v3;
pub mod successor;

/// Bytes in one independently signed compact intent.
pub const COMPACT_INTENT_BYTES: usize = generated_layout::COMPACT_INTENT_BYTES_VALUE;
/// Bytes in one controller instruction containing two compact intents.
pub const CONTROLLER_INSTRUCTION_BYTES: usize =
    generated_layout::CONTROLLER_INSTRUCTION_BYTES_VALUE;
/// Bytes in one canonical registered-intent state account.
pub const REGISTERED_INTENT_STATE_BYTES: usize = generated_lifecycle::REGISTERED_STATE_BYTES_VALUE;
/// Bytes in the Lean-owned registered residual-fill program.
pub const REGISTERED_FILL_PROGRAM_BYTES: usize = generated_lifecycle::REGISTERED_FILL_PROGRAM.len();
/// Lean-owned bytecode deriving remaining quantity, replay sequence, and phase.
pub const REGISTERED_FILL_PROGRAM: [u8; REGISTERED_FILL_PROGRAM_BYTES] =
    generated_lifecycle::REGISTERED_FILL_PROGRAM;
/// Bytes in one controller request to fill two registered intents.
pub const REGISTERED_CONTROLLER_INSTRUCTION_BYTES: usize =
    generated_registered_controller::REGISTERED_CONTROLLER_BYTES_VALUE;
/// Bytes in the claim-owner request derived from a registered controller fill.
pub const REGISTERED_CLAIM_FILL_BYTES: usize =
    generated_registered_controller::REGISTERED_CLAIM_FILL_BYTES_VALUE;
/// Bytes in one signed, prepaid registered-intent creation request.
pub const REGISTERED_CREATE_INSTRUCTION_BYTES: usize =
    generated_registered_controller::REGISTERED_CREATE_BYTES_VALUE;
/// Bytes in one controller cancellation or permissionless expiry request.
pub const REGISTERED_TERMINAL_INSTRUCTION_BYTES: usize =
    generated_registered_controller::REGISTERED_TERMINAL_BYTES_VALUE;
/// Bytes in one claim-owner cancellation or expiry request.
pub const REGISTERED_CLAIM_TERMINAL_BYTES: usize =
    generated_registered_controller::REGISTERED_CLAIM_CANCEL_TEMPLATE.len();
/// Bytes in one terminal registered-intent account-retirement request.
pub const REGISTERED_RETIRE_INSTRUCTION_BYTES: usize =
    generated_registered_controller::REGISTERED_RETIRE_BYTES_VALUE;
/// Scalar inputs consumed by the Lean-owned registered residual-fill program.
pub const REGISTERED_FILL_INPUT_COUNT: usize = generated_lifecycle::REGISTERED_INPUT_COUNT;
/// Registered lifecycle input register containing the signed lifecycle tag.
pub const REGISTERED_LIFECYCLE_REGISTER: usize = generated_lifecycle::REGISTERED_LIFECYCLE;
/// Registered lifecycle input register containing the sole remaining quantity.
pub const REGISTERED_REMAINING_REGISTER: usize = generated_lifecycle::REGISTERED_REMAINING;
/// Registered lifecycle input register containing the proposed fill.
pub const REGISTERED_FILL_REGISTER: usize = generated_lifecycle::REGISTERED_FILL;
/// Registered lifecycle input register containing the local replay sequence.
pub const REGISTERED_SEQUENCE_REGISTER: usize = generated_lifecycle::REGISTERED_SEQUENCE;
/// Registered lifecycle output register containing the successor remaining quantity.
pub const REGISTERED_REMAINING_OUTPUT_REGISTER: usize =
    generated_lifecycle::REGISTERED_REMAINING_OUTPUT;
/// Registered lifecycle output register containing the successor replay sequence.
pub const REGISTERED_SEQUENCE_OUTPUT_REGISTER: usize =
    generated_lifecycle::REGISTERED_SEQUENCE_OUTPUT;
/// Registered lifecycle output register containing the successor phase tag.
pub const REGISTERED_PHASE_OUTPUT_REGISTER: usize = generated_lifecycle::REGISTERED_PHASE_OUTPUT;
/// Current compiled Direct ABI version.
pub const VERSION: u16 = generated_layout::ABI_VERSION;
/// Semantic release selected by a Market for this compiled inline controller.
///
/// SHA-256 of `dclutch/release/direct-compiled-controller-v1`. A checked
/// release manifest separately binds this semantic coordinate to exact ELF and
/// Loader evidence; it is not itself an artifact digest.
pub const COMPILED_DIRECT_RELEASE_ID_V1: [u8; 32] = [
    0x79, 0xfa, 0xd2, 0xf0, 0x4f, 0x8d, 0x9c, 0xe0, 0x7d, 0x76, 0xc8, 0x09, 0xfe, 0x11, 0x6d, 0xb8,
    0xef, 0x93, 0x74, 0xad, 0xbe, 0xb1, 0x5e, 0x62, 0xf6, 0x03, 0x23, 0x5c, 0x3a, 0x2b, 0x96, 0xb9,
];
/// Measured compiled inline capacity coordinate for `N = 2..=16`.
pub const COMPILED_DIRECT_CAPACITY_ID_V1: [u8; 32] = [
    0x2e, 0xaf, 0xb1, 0x44, 0x84, 0x0a, 0x9d, 0xc3, 0x1c, 0xed, 0x73, 0xac, 0x19, 0x9b, 0xa1, 0xcf,
    0x49, 0x16, 0x15, 0x28, 0x47, 0x02, 0x05, 0x14, 0x37, 0xb1, 0xa5, 0x9d, 0xf5, 0xd2, 0x93, 0x7d,
];
/// Compiled replay/Position child-state schema coordinate.
pub const COMPILED_DIRECT_CHILD_SCHEMA_ID_V1: [u8; 32] = [
    0x97, 0x67, 0xa1, 0x54, 0x54, 0x9c, 0x1f, 0x06, 0xa1, 0xe0, 0xc7, 0x41, 0xc1, 0x84, 0xac, 0xc0,
    0xd9, 0xbd, 0x18, 0xa4, 0x21, 0x19, 0xfa, 0xac, 0x14, 0x0c, 0x4d, 0xd7, 0xf1, 0x55, 0xfc, 0xe7,
];
/// Compiled replay/Position PDA-derivation coordinate.
pub const COMPILED_DIRECT_DERIVATION_ID_V1: [u8; 32] = [
    0x2d, 0x00, 0xc7, 0x72, 0x68, 0xf9, 0x0c, 0x56, 0xc2, 0xf4, 0x3d, 0xb5, 0x43, 0x74, 0x54, 0x92,
    0xd0, 0x5e, 0x36, 0x9a, 0xef, 0xd5, 0xd3, 0x86, 0x9a, 0x6a, 0xa6, 0xae, 0xe7, 0xc9, 0xc5, 0x8d,
];

const INTENT_MAGIC: &[u8; 8] = &generated_layout::INTENT_MAGIC_BYTES;
const CONTROLLER_MAGIC: &[u8; 8] = &generated_layout::CONTROLLER_MAGIC_BYTES;
const REGISTERED_STATE_MAGIC: &[u8; 8] = &generated_lifecycle::REGISTERED_STATE_MAGIC_BYTES;

/// Strict codec refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input width differed from the exact schema width.
    InvalidLength,
    /// Domain-separating magic was not exact.
    InvalidMagic,
    /// Schema version is not implemented.
    UnsupportedVersion,
    /// A reserved byte was nonzero.
    NonzeroReserved,
    /// A persisted registered-intent phase was not open, filled, cancelled, or expired.
    InvalidPhase,
    /// A registered terminal action was not cancellation or expiry.
    InvalidAction,
    /// A fixed output field could not be written.
    Output,
}

/// One independently signed reusable Direct limit intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactIntentV1 {
    /// Seller `0` or buyer `1`; transition admission rejects other tags.
    pub side: u8,
    /// Product-owned outcome coordinate.
    pub outcome: u8,
    /// Fill-or-kill `0` or immediate-or-cancel `1`.
    pub lifecycle: u8,
    /// Canonical Market account identity.
    pub market: [u8; 32],
    /// Replay generation selected by the immutable Market identity.
    pub generation: u64,
    /// Exact next maker replay nonce.
    pub nonce: u64,
    /// First valid Clock slot.
    pub valid_from: u64,
    /// Last valid Clock slot.
    pub valid_through: u64,
    /// Maximum admitted fill.
    pub maximum_fill: u64,
    /// Seller minimum or buyer maximum price at the profile scale.
    pub limit_price: u64,
    /// Exact maker-accepted cumulative floor-fee rate.
    pub fee_basis_points: u16,
    /// Seller destination or buyer source token account.
    pub collateral_account: [u8; 32],
}

impl CompactIntentV1 {
    /// Strictly decode one canonical compact intent.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_width(input, COMPACT_INTENT_BYTES)?;
        exact_magic(input, INTENT_MAGIC)?;
        exact_version(input)?;
        reserved(
            input,
            generated_layout::INTENT_RESERVED_A_OFFSET,
            generated_layout::INTENT_RESERVED_A_WIDTH,
        )?;
        reserved(
            input,
            generated_layout::INTENT_RESERVED_B_OFFSET,
            generated_layout::INTENT_RESERVED_B_WIDTH,
        )?;
        Ok(generated_layout::decode_compact_intent_fields!(input))
    }

    /// Encode one canonical compact intent.
    pub fn encode(self) -> Result<[u8; COMPACT_INTENT_BYTES], Error> {
        let mut output = [0_u8; COMPACT_INTENT_BYTES];
        put(&mut output, generated_layout::MAGIC_OFFSET, INTENT_MAGIC)?;
        put(
            &mut output,
            generated_layout::VERSION_OFFSET,
            &VERSION.to_le_bytes(),
        )?;
        generated_layout::encode_compact_intent_fields!(output, self);
        Ok(output)
    }
}

/// Matcher coordinates and two independently signed compact intents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerInstructionV1 {
    /// Global controller PDA bump.
    pub controller_bump: u8,
    /// Seller replay-root PDA bump.
    pub seller_replay_bump: u8,
    /// Buyer replay-root PDA bump.
    pub buyer_replay_bump: u8,
    /// Seller maker/outcome Position PDA bump.
    pub seller_position_bump: u8,
    /// Buyer maker/outcome Position PDA bump.
    pub buyer_position_bump: u8,
    /// Matcher-selected fill checked against both intents.
    pub fill: u64,
    /// Matcher-selected execution price checked against both limits.
    pub execution_price: u64,
    /// Seller's independently signed intent.
    pub seller: CompactIntentV1,
    /// Buyer's independently signed intent.
    pub buyer: CompactIntentV1,
}

/// Canonical persisted authority for one registered Direct intent.
///
/// The claims/replay program owns the physical account. `controller` names the
/// only controller PDA allowed to advance it; `intent` is the exact signed
/// message admitted at registration. Only `phase`, `remaining`, and `sequence`
/// evolve after creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredIntentStateV1 {
    /// Open `0`, filled `1`, cancelled `2`, or expired `3`.
    pub phase: u8,
    /// Exact controller PDA authorized by the selected semantic release.
    pub controller: [u8; 32],
    /// Native Ed25519 maker authenticated during registration.
    pub maker: [u8; 32],
    /// Exact signed compact intent; no selected term is copied beside it.
    pub intent: CompactIntentV1,
    /// Sole residual-fill authority.
    pub remaining: u64,
    /// Registration-local replay sequence.
    pub sequence: u64,
}

/// Matcher-selected fill over two already authenticated registration states.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredFillInstructionV1 {
    /// Global controller PDA bump.
    pub controller_bump: u8,
    /// Seller registered-intent PDA bump.
    pub seller_registration_bump: u8,
    /// Buyer registered-intent PDA bump.
    pub buyer_registration_bump: u8,
    /// Seller maker/outcome Position PDA bump.
    pub seller_position_bump: u8,
    /// Buyer maker/outcome Position PDA bump.
    pub buyer_position_bump: u8,
    /// Matcher-selected positive fill.
    pub fill: u64,
    /// Matcher-selected execution price at the Market profile scale.
    pub execution_price: u64,
}

/// One maker-signed request to create a reusable registered intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredCreateInstructionV1 {
    /// Global controller PDA bump.
    pub controller_bump: u8,
    /// Maker/Market/generation replay-root PDA bump.
    pub replay_bump: u8,
    /// Exact registered-intent PDA bump.
    pub registration_bump: u8,
    /// Canonical terms authorized by the maker's transaction signature.
    pub intent: CompactIntentV1,
}

/// Terminal mutation selected for one registered intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisteredTerminalAction {
    /// Maker-authorized cancellation.
    Cancel,
    /// Permissionless expiry after the signed validity window.
    Expire,
}

/// One sequence-pinned registered cancellation or expiry request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredTerminalInstructionV1 {
    /// Cancellation or expiry.
    pub action: RegisteredTerminalAction,
    /// Global controller PDA bump.
    pub controller_bump: u8,
    /// Registered-intent PDA bump.
    pub registration_bump: u8,
    /// Exact current registration-local sequence observed by the caller.
    pub expected_sequence: u64,
}

/// One request to retire a terminal registered-intent account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredRetireInstructionV1 {
    /// Global controller PDA bump.
    pub controller_bump: u8,
    /// Registered-intent PDA bump.
    pub registration_bump: u8,
}

impl RegisteredIntentStateV1 {
    /// Project the persisted semantic authority into the generated VM input prefix.
    #[must_use]
    pub fn fill_input_scalars(self, fill: u64) -> [u64; REGISTERED_FILL_INPUT_COUNT] {
        generated_lifecycle::registered_fill_inputs! {
            lifecycle: self.intent.lifecycle as u64,
            remaining: self.remaining,
            fill: fill,
            sequence: self.sequence,
        }
    }

    /// Strictly decode one canonical registered-intent state.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_width(input, REGISTERED_INTENT_STATE_BYTES)?;
        exact_magic(input, REGISTERED_STATE_MAGIC)?;
        if u16_at(input, generated_lifecycle::REGISTERED_STATE_VERSION_OFFSET)?
            != generated_lifecycle::REGISTERED_STATE_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        reserved(
            input,
            generated_lifecycle::REGISTERED_STATE_RESERVED_OFFSET,
            generated_lifecycle::REGISTERED_STATE_CONTROLLER_OFFSET
                .checked_sub(generated_lifecycle::REGISTERED_STATE_RESERVED_OFFSET)
                .ok_or(Error::InvalidLength)?,
        )?;
        let state = generated_lifecycle::decode_registered_state_fields!(input);
        if state.phase > 3 {
            return Err(Error::InvalidPhase);
        }
        Ok(state)
    }

    /// Encode one canonical registered-intent state.
    pub fn encode(self) -> Result<[u8; REGISTERED_INTENT_STATE_BYTES], Error> {
        if self.phase > 3 {
            return Err(Error::InvalidPhase);
        }
        let mut output = [0_u8; REGISTERED_INTENT_STATE_BYTES];
        put(
            &mut output,
            generated_lifecycle::REGISTERED_STATE_MAGIC_OFFSET,
            REGISTERED_STATE_MAGIC,
        )?;
        put(
            &mut output,
            generated_lifecycle::REGISTERED_STATE_VERSION_OFFSET,
            &generated_lifecycle::REGISTERED_STATE_ABI_VERSION.to_le_bytes(),
        )?;
        generated_lifecycle::encode_registered_state_fields!(output, self);
        Ok(output)
    }
}

impl RegisteredFillInstructionV1 {
    /// Strictly decode one canonical registered-fill controller request.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_width(input, REGISTERED_CONTROLLER_INSTRUCTION_BYTES)?;
        exact_magic(
            input,
            &generated_registered_controller::REGISTERED_CONTROLLER_MAGIC_BYTES,
        )?;
        if u16_at(
            input,
            generated_registered_controller::REGISTERED_CONTROLLER_VERSION_OFFSET,
        )? != generated_registered_controller::REGISTERED_CONTROLLER_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        reserved(
            input,
            generated_registered_controller::REGISTERED_CONTROLLER_RESERVED_OFFSET,
            1,
        )?;
        Ok(Self {
            controller_bump: byte(
                input,
                generated_registered_controller::REGISTERED_CONTROLLER_BUMP_OFFSET,
            )?,
            seller_registration_bump: byte(
                input,
                generated_registered_controller::REGISTERED_SELLER_REGISTRATION_BUMP_OFFSET,
            )?,
            buyer_registration_bump: byte(
                input,
                generated_registered_controller::REGISTERED_BUYER_REGISTRATION_BUMP_OFFSET,
            )?,
            seller_position_bump: byte(
                input,
                generated_registered_controller::REGISTERED_SELLER_POSITION_BUMP_OFFSET,
            )?,
            buyer_position_bump: byte(
                input,
                generated_registered_controller::REGISTERED_BUYER_POSITION_BUMP_OFFSET,
            )?,
            fill: u64_at(
                input,
                generated_registered_controller::REGISTERED_CONTROLLER_FILL_OFFSET,
            )?,
            execution_price: u64_at(
                input,
                generated_registered_controller::REGISTERED_CONTROLLER_EXECUTION_PRICE_OFFSET,
            )?,
        })
    }

    /// Encode one canonical registered-fill controller request.
    pub fn encode(self) -> Result<[u8; REGISTERED_CONTROLLER_INSTRUCTION_BYTES], Error> {
        let mut output = [0_u8; REGISTERED_CONTROLLER_INSTRUCTION_BYTES];
        put(
            &mut output,
            generated_registered_controller::REGISTERED_CONTROLLER_MAGIC_OFFSET,
            &generated_registered_controller::REGISTERED_CONTROLLER_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_registered_controller::REGISTERED_CONTROLLER_VERSION_OFFSET,
            &generated_registered_controller::REGISTERED_CONTROLLER_ABI_VERSION.to_le_bytes(),
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_CONTROLLER_BUMP_OFFSET,
            self.controller_bump,
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_SELLER_REGISTRATION_BUMP_OFFSET,
            self.seller_registration_bump,
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_BUYER_REGISTRATION_BUMP_OFFSET,
            self.buyer_registration_bump,
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_SELLER_POSITION_BUMP_OFFSET,
            self.seller_position_bump,
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_BUYER_POSITION_BUMP_OFFSET,
            self.buyer_position_bump,
        )?;
        put(
            &mut output,
            generated_registered_controller::REGISTERED_CONTROLLER_FILL_OFFSET,
            &self.fill.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_registered_controller::REGISTERED_CONTROLLER_EXECUTION_PRICE_OFFSET,
            &self.execution_price.to_le_bytes(),
        )?;
        Ok(output)
    }
}

impl RegisteredCreateInstructionV1 {
    /// Strictly decode one canonical registration-creation request.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_width(input, REGISTERED_CREATE_INSTRUCTION_BYTES)?;
        exact_magic(
            input,
            &generated_registered_controller::REGISTERED_CREATE_MAGIC_BYTES,
        )?;
        if u16_at(
            input,
            generated_registered_controller::REGISTERED_CREATE_VERSION_OFFSET,
        )? != generated_registered_controller::REGISTERED_CREATE_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        reserved(
            input,
            generated_registered_controller::REGISTERED_CREATE_RESERVED_OFFSET,
            generated_registered_controller::REGISTERED_CREATE_INTENT_OFFSET
                .checked_sub(generated_registered_controller::REGISTERED_CREATE_RESERVED_OFFSET)
                .ok_or(Error::InvalidLength)?,
        )?;
        let intent_end = generated_registered_controller::REGISTERED_CREATE_INTENT_OFFSET
            .checked_add(COMPACT_INTENT_BYTES)
            .ok_or(Error::InvalidLength)?;
        Ok(Self {
            controller_bump: byte(
                input,
                generated_registered_controller::REGISTERED_CREATE_CONTROLLER_BUMP_OFFSET,
            )?,
            replay_bump: byte(
                input,
                generated_registered_controller::REGISTERED_CREATE_REPLAY_BUMP_OFFSET,
            )?,
            registration_bump: byte(
                input,
                generated_registered_controller::REGISTERED_CREATE_REGISTRATION_BUMP_OFFSET,
            )?,
            intent: CompactIntentV1::decode(
                input
                    .get(
                        generated_registered_controller::REGISTERED_CREATE_INTENT_OFFSET
                            ..intent_end,
                    )
                    .ok_or(Error::InvalidLength)?,
            )?,
        })
    }

    /// Encode one canonical registration-creation request.
    pub fn encode(self) -> Result<[u8; REGISTERED_CREATE_INSTRUCTION_BYTES], Error> {
        let mut output = [0_u8; REGISTERED_CREATE_INSTRUCTION_BYTES];
        put(
            &mut output,
            generated_registered_controller::REGISTERED_CREATE_MAGIC_OFFSET,
            &generated_registered_controller::REGISTERED_CREATE_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_registered_controller::REGISTERED_CREATE_VERSION_OFFSET,
            &generated_registered_controller::REGISTERED_CREATE_ABI_VERSION.to_le_bytes(),
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_CREATE_CONTROLLER_BUMP_OFFSET,
            self.controller_bump,
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_CREATE_REPLAY_BUMP_OFFSET,
            self.replay_bump,
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_CREATE_REGISTRATION_BUMP_OFFSET,
            self.registration_bump,
        )?;
        put(
            &mut output,
            generated_registered_controller::REGISTERED_CREATE_INTENT_OFFSET,
            &self.intent.encode()?,
        )?;
        Ok(output)
    }
}

impl RegisteredTerminalInstructionV1 {
    /// Strictly decode one canonical registered terminal request.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_width(input, REGISTERED_TERMINAL_INSTRUCTION_BYTES)?;
        exact_magic(
            input,
            &generated_registered_controller::REGISTERED_TERMINAL_MAGIC_BYTES,
        )?;
        if u16_at(
            input,
            generated_registered_controller::REGISTERED_TERMINAL_VERSION_OFFSET,
        )? != generated_registered_controller::REGISTERED_TERMINAL_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        reserved(
            input,
            generated_registered_controller::REGISTERED_TERMINAL_RESERVED_OFFSET,
            generated_registered_controller::REGISTERED_TERMINAL_EXPECTED_SEQUENCE_OFFSET
                .checked_sub(generated_registered_controller::REGISTERED_TERMINAL_RESERVED_OFFSET)
                .ok_or(Error::InvalidLength)?,
        )?;
        let action = match byte(
            input,
            generated_registered_controller::REGISTERED_TERMINAL_ACTION_OFFSET,
        )? {
            generated_registered_controller::REGISTERED_TERMINAL_CANCEL => {
                RegisteredTerminalAction::Cancel
            }
            generated_registered_controller::REGISTERED_TERMINAL_EXPIRE => {
                RegisteredTerminalAction::Expire
            }
            _ => return Err(Error::InvalidAction),
        };
        Ok(Self {
            action,
            controller_bump: byte(
                input,
                generated_registered_controller::REGISTERED_TERMINAL_CONTROLLER_BUMP_OFFSET,
            )?,
            registration_bump: byte(
                input,
                generated_registered_controller::REGISTERED_TERMINAL_REGISTRATION_BUMP_OFFSET,
            )?,
            expected_sequence: u64_at(
                input,
                generated_registered_controller::REGISTERED_TERMINAL_EXPECTED_SEQUENCE_OFFSET,
            )?,
        })
    }

    /// Encode one canonical registered terminal request.
    pub fn encode(self) -> Result<[u8; REGISTERED_TERMINAL_INSTRUCTION_BYTES], Error> {
        let mut output = [0_u8; REGISTERED_TERMINAL_INSTRUCTION_BYTES];
        put(
            &mut output,
            generated_registered_controller::REGISTERED_TERMINAL_MAGIC_OFFSET,
            &generated_registered_controller::REGISTERED_TERMINAL_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_registered_controller::REGISTERED_TERMINAL_VERSION_OFFSET,
            &generated_registered_controller::REGISTERED_TERMINAL_ABI_VERSION.to_le_bytes(),
        )?;
        let action = match self.action {
            RegisteredTerminalAction::Cancel => {
                generated_registered_controller::REGISTERED_TERMINAL_CANCEL
            }
            RegisteredTerminalAction::Expire => {
                generated_registered_controller::REGISTERED_TERMINAL_EXPIRE
            }
        };
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_TERMINAL_ACTION_OFFSET,
            action,
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_TERMINAL_CONTROLLER_BUMP_OFFSET,
            self.controller_bump,
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_TERMINAL_REGISTRATION_BUMP_OFFSET,
            self.registration_bump,
        )?;
        put(
            &mut output,
            generated_registered_controller::REGISTERED_TERMINAL_EXPECTED_SEQUENCE_OFFSET,
            &self.expected_sequence.to_le_bytes(),
        )?;
        Ok(output)
    }
}

impl RegisteredRetireInstructionV1 {
    /// Strictly decode one canonical registered retirement request.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_width(input, REGISTERED_RETIRE_INSTRUCTION_BYTES)?;
        exact_magic(
            input,
            &generated_registered_controller::REGISTERED_RETIRE_MAGIC_BYTES,
        )?;
        if u16_at(
            input,
            generated_registered_controller::REGISTERED_RETIRE_VERSION_OFFSET,
        )? != generated_registered_controller::REGISTERED_RETIRE_ABI_VERSION
        {
            return Err(Error::UnsupportedVersion);
        }
        reserved(
            input,
            generated_registered_controller::REGISTERED_RETIRE_RESERVED_OFFSET,
            REGISTERED_RETIRE_INSTRUCTION_BYTES
                .checked_sub(generated_registered_controller::REGISTERED_RETIRE_RESERVED_OFFSET)
                .ok_or(Error::InvalidLength)?,
        )?;
        Ok(Self {
            controller_bump: byte(
                input,
                generated_registered_controller::REGISTERED_RETIRE_CONTROLLER_BUMP_OFFSET,
            )?,
            registration_bump: byte(
                input,
                generated_registered_controller::REGISTERED_RETIRE_REGISTRATION_BUMP_OFFSET,
            )?,
        })
    }

    /// Encode one canonical registered retirement request.
    pub fn encode(self) -> Result<[u8; REGISTERED_RETIRE_INSTRUCTION_BYTES], Error> {
        let mut output = [0_u8; REGISTERED_RETIRE_INSTRUCTION_BYTES];
        put(
            &mut output,
            generated_registered_controller::REGISTERED_RETIRE_MAGIC_OFFSET,
            &generated_registered_controller::REGISTERED_RETIRE_MAGIC_BYTES,
        )?;
        put(
            &mut output,
            generated_registered_controller::REGISTERED_RETIRE_VERSION_OFFSET,
            &generated_registered_controller::REGISTERED_RETIRE_ABI_VERSION.to_le_bytes(),
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_RETIRE_CONTROLLER_BUMP_OFFSET,
            self.controller_bump,
        )?;
        put_byte(
            &mut output,
            generated_registered_controller::REGISTERED_RETIRE_REGISTRATION_BUMP_OFFSET,
            self.registration_bump,
        )?;
        Ok(output)
    }
}

/// Derive the only registered-fill request accepted by the claim owner.
pub fn registered_claim_fill_instruction(
    fill: u64,
) -> Result<[u8; REGISTERED_CLAIM_FILL_BYTES], Error> {
    let mut output = generated_registered_controller::REGISTERED_CLAIM_FILL_TEMPLATE;
    put(
        &mut output,
        generated_registered_controller::REGISTERED_CLAIM_FILL_OFFSET,
        &fill.to_le_bytes(),
    )?;
    Ok(output)
}

/// Derive the only sequence-pinned terminal request accepted by the claim owner.
pub fn registered_claim_terminal_instruction(
    action: RegisteredTerminalAction,
    expected_sequence: u64,
) -> Result<[u8; REGISTERED_CLAIM_TERMINAL_BYTES], Error> {
    let mut output = match action {
        RegisteredTerminalAction::Cancel => {
            generated_registered_controller::REGISTERED_CLAIM_CANCEL_TEMPLATE
        }
        RegisteredTerminalAction::Expire => {
            generated_registered_controller::REGISTERED_CLAIM_EXPIRE_TEMPLATE
        }
    };
    put(
        &mut output,
        generated_registered_controller::REGISTERED_CLAIM_TERMINAL_SEQUENCE_OFFSET,
        &expected_sequence.to_le_bytes(),
    )?;
    Ok(output)
}

impl ControllerInstructionV1 {
    /// Strictly decode one canonical controller instruction.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_width(input, CONTROLLER_INSTRUCTION_BYTES)?;
        exact_magic(input, CONTROLLER_MAGIC)?;
        exact_version(input)?;
        reserved(
            input,
            generated_layout::CONTROLLER_RESERVED_OFFSET,
            generated_layout::CONTROLLER_RESERVED_WIDTH,
        )?;
        Ok(generated_layout::decode_controller_fields!(input))
    }

    /// Encode one canonical controller instruction.
    pub fn encode(self) -> Result<[u8; CONTROLLER_INSTRUCTION_BYTES], Error> {
        let mut output = [0_u8; CONTROLLER_INSTRUCTION_BYTES];
        put(
            &mut output,
            generated_layout::MAGIC_OFFSET,
            CONTROLLER_MAGIC,
        )?;
        put(
            &mut output,
            generated_layout::VERSION_OFFSET,
            &VERSION.to_le_bytes(),
        )?;
        generated_layout::encode_controller_fields!(output, self);
        Ok(output)
    }
}

fn exact_width(input: &[u8], expected: usize) -> Result<(), Error> {
    if input.len() == expected {
        Ok(())
    } else {
        Err(Error::InvalidLength)
    }
}

fn exact_magic(input: &[u8], expected: &[u8; 8]) -> Result<(), Error> {
    if slice(input, generated_layout::MAGIC_OFFSET, expected.len())? == expected {
        Ok(())
    } else {
        Err(Error::InvalidMagic)
    }
}

fn exact_version(input: &[u8]) -> Result<(), Error> {
    if u16_at(input, generated_layout::VERSION_OFFSET)? == VERSION {
        Ok(())
    } else {
        Err(Error::UnsupportedVersion)
    }
}

fn reserved(input: &[u8], offset: usize, width: usize) -> Result<(), Error> {
    if slice(input, offset, width)?.iter().all(|byte| *byte == 0) {
        Ok(())
    } else {
        Err(Error::NonzeroReserved)
    }
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8], Error> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    input.get(offset..end).ok_or(Error::InvalidLength)
}

fn array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn byte(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(array(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(array(input, offset)?))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = offset.checked_add(value.len()).ok_or(Error::Output)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Output)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    *output.get_mut(offset).ok_or(Error::Output)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::unwrap_used)]

    extern crate std;

    use super::*;
    use dclutch_transition_vm::{execute, Registers};
    use std::{string::String, vec::Vec};

    const LEAN_VECTORS: &str =
        include_str!("../../../formal/dclutch-semantics/vectors/direct-controller-v1.txt");

    fn fixture_intent(side: u8) -> CompactIntentV1 {
        CompactIntentV1 {
            side,
            outcome: 1,
            lifecycle: 0,
            market: [4; 32],
            generation: 3,
            nonce: 0,
            valid_from: 0,
            valid_through: u64::MAX,
            maximum_fill: 2_000,
            limit_price: if side == 0 { 400_000 } else { 600_000 },
            fee_basis_points: 25,
            collateral_account: [if side == 0 { 5 } else { 6 }; 32],
        }
    }

    fn fixture_registered_state(lifecycle: u8) -> RegisteredIntentStateV1 {
        let mut intent = fixture_intent(0);
        intent.lifecycle = lifecycle;
        RegisteredIntentStateV1 {
            phase: 0,
            controller: [7; 32],
            maker: [8; 32],
            intent,
            remaining: 2_000,
            sequence: 0,
        }
    }

    fn registered_transition(
        state: RegisteredIntentStateV1,
        fill: u64,
    ) -> Result<(u64, u64, u64), dclutch_transition_vm::Error> {
        let inputs = state.fill_input_scalars(fill);
        let mut registers = Registers::zeroed();
        for (index, value) in inputs.into_iter().enumerate() {
            registers.set_scalar(index, value)?;
        }
        execute(&REGISTERED_FILL_PROGRAM, &mut registers)?;
        Ok((
            registers.scalar(REGISTERED_REMAINING_OUTPUT_REGISTER)?,
            registers.scalar(REGISTERED_SEQUENCE_OUTPUT_REGISTER)?,
            registers.scalar(REGISTERED_PHASE_OUTPUT_REGISTER)?,
        ))
    }

    fn hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(char::from(DIGITS[usize::from(*byte >> 4)]));
            output.push(char::from(DIGITS[usize::from(*byte & 0x0f)]));
        }
        output
    }

    fn vector(name: &str) -> String {
        let prefix = String::from(name) + "=";
        LEAN_VECTORS
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .map(String::from)
            .unwrap_or_default()
    }

    #[test]
    fn encoders_exactly_match_lean_vectors_and_round_trip() {
        let seller = fixture_intent(0);
        let buyer = fixture_intent(1);
        let controller = ControllerInstructionV1 {
            controller_bump: 1,
            seller_replay_bump: 2,
            buyer_replay_bump: 3,
            seller_position_bump: 4,
            buyer_position_bump: 5,
            fill: 2_000,
            execution_price: 500_000,
            seller,
            buyer,
        };
        for (name, encoded) in [
            ("seller_intent", seller.encode().map(Vec::from)),
            ("buyer_intent", buyer.encode().map(Vec::from)),
            ("controller", controller.encode().map(Vec::from)),
        ] {
            let encoded = encoded.expect("fixed encoder");
            assert_eq!(hex(&encoded), vector(name));
        }
        assert_eq!(
            CompactIntentV1::decode(&seller.encode().expect("seller encoding")),
            Ok(seller)
        );
        assert_eq!(
            ControllerInstructionV1::decode(&controller.encode().expect("controller encoding")),
            Ok(controller)
        );
    }

    #[test]
    fn hostile_width_magic_version_and_reserved_bytes_refuse() {
        let mut encoded = fixture_intent(0).encode().expect("intent encoding");
        assert_eq!(
            CompactIntentV1::decode(&encoded[..135]),
            Err(Error::InvalidLength)
        );
        encoded[0] ^= 1;
        assert_eq!(CompactIntentV1::decode(&encoded), Err(Error::InvalidMagic));
        encoded = fixture_intent(0).encode().expect("intent encoding");
        encoded[8] = 2;
        assert_eq!(
            CompactIntentV1::decode(&encoded),
            Err(Error::UnsupportedVersion)
        );
        encoded = fixture_intent(0).encode().expect("intent encoding");
        encoded[generated_layout::INTENT_RESERVED_A_OFFSET] = 1;
        assert_eq!(
            CompactIntentV1::decode(&encoded),
            Err(Error::NonzeroReserved)
        );
    }

    #[test]
    fn registered_state_matches_lean_and_round_trips() {
        let state = fixture_registered_state(0);
        let encoded = state.encode().expect("registered state encoding");
        assert_eq!(encoded, generated_lifecycle::REGISTERED_STATE_EXAMPLE);
        assert_eq!(RegisteredIntentStateV1::decode(&encoded), Ok(state));
    }

    #[test]
    fn registered_controller_matches_lean_and_round_trips() {
        let instruction = RegisteredFillInstructionV1 {
            controller_bump: 1,
            seller_registration_bump: 2,
            buyer_registration_bump: 3,
            seller_position_bump: 4,
            buyer_position_bump: 5,
            fill: 2_000,
            execution_price: 500_000,
        };
        let encoded = instruction
            .encode()
            .expect("registered controller encoding");
        assert_eq!(
            encoded,
            generated_registered_controller::REGISTERED_CONTROLLER_EXAMPLE
        );
        assert_eq!(
            RegisteredFillInstructionV1::decode(&encoded),
            Ok(instruction)
        );
        assert_eq!(
            registered_claim_fill_instruction(instruction.fill),
            Ok([b'D', b'C', b'R', b'F', 1, 0, 0, 0, 0xd0, 0x07, 0, 0, 0, 0, 0, 0,])
        );
    }

    #[test]
    fn registered_creation_matches_lean_and_round_trips() {
        let instruction = RegisteredCreateInstructionV1 {
            controller_bump: 1,
            replay_bump: 2,
            registration_bump: 3,
            intent: fixture_registered_state(0).intent,
        };
        let bytes = instruction.encode().expect("registration encoding");
        assert_eq!(
            bytes,
            generated_registered_controller::REGISTERED_CREATE_EXAMPLE
        );
        assert_eq!(
            RegisteredCreateInstructionV1::decode(&bytes),
            Ok(instruction)
        );

        let mut hostile = bytes;
        hostile[generated_registered_controller::REGISTERED_CREATE_RESERVED_OFFSET] = 1;
        assert_eq!(
            RegisteredCreateInstructionV1::decode(&hostile),
            Err(Error::NonzeroReserved)
        );
        assert_eq!(
            RegisteredCreateInstructionV1::decode(&bytes[..bytes.len() - 1]),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn hostile_registered_controller_refuses() {
        let instruction = RegisteredFillInstructionV1 {
            controller_bump: 1,
            seller_registration_bump: 2,
            buyer_registration_bump: 3,
            seller_position_bump: 4,
            buyer_position_bump: 5,
            fill: 2_000,
            execution_price: 500_000,
        };
        let encoded = instruction
            .encode()
            .expect("registered controller encoding");
        assert_eq!(
            RegisteredFillInstructionV1::decode(&encoded[..encoded.len() - 1]),
            Err(Error::InvalidLength)
        );
        let mut hostile = encoded;
        hostile[generated_registered_controller::REGISTERED_CONTROLLER_MAGIC_OFFSET] ^= 1;
        assert_eq!(
            RegisteredFillInstructionV1::decode(&hostile),
            Err(Error::InvalidMagic)
        );
        let mut hostile = encoded;
        hostile[generated_registered_controller::REGISTERED_CONTROLLER_VERSION_OFFSET] = 2;
        assert_eq!(
            RegisteredFillInstructionV1::decode(&hostile),
            Err(Error::UnsupportedVersion)
        );
        let mut hostile = encoded;
        hostile[generated_registered_controller::REGISTERED_CONTROLLER_RESERVED_OFFSET] = 1;
        assert_eq!(
            RegisteredFillInstructionV1::decode(&hostile),
            Err(Error::NonzeroReserved)
        );
    }

    #[test]
    fn registered_terminal_requests_match_lean() {
        let cancel = RegisteredTerminalInstructionV1 {
            action: RegisteredTerminalAction::Cancel,
            controller_bump: 2,
            registration_bump: 3,
            expected_sequence: 7,
        };
        let expire = RegisteredTerminalInstructionV1 {
            action: RegisteredTerminalAction::Expire,
            ..cancel
        };
        let cancel_bytes = cancel.encode().expect("cancel encoding");
        let expire_bytes = expire.encode().expect("expire encoding");
        assert_eq!(
            cancel_bytes,
            generated_registered_controller::REGISTERED_TERMINAL_CANCEL_EXAMPLE
        );
        assert_eq!(
            expire_bytes,
            generated_registered_controller::REGISTERED_TERMINAL_EXPIRE_EXAMPLE
        );
        assert_eq!(
            RegisteredTerminalInstructionV1::decode(&cancel_bytes),
            Ok(cancel)
        );
        assert_eq!(
            RegisteredTerminalInstructionV1::decode(&expire_bytes),
            Ok(expire)
        );
        assert_eq!(
            registered_claim_terminal_instruction(RegisteredTerminalAction::Cancel, 7),
            Ok([b'D', b'C', b'R', b'C', 1, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0,])
        );
        assert_eq!(
            registered_claim_terminal_instruction(RegisteredTerminalAction::Expire, 7),
            Ok([b'D', b'C', b'R', b'E', 1, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0,])
        );
    }

    #[test]
    fn hostile_registered_terminal_requests_refuse() {
        let request = RegisteredTerminalInstructionV1 {
            action: RegisteredTerminalAction::Cancel,
            controller_bump: 2,
            registration_bump: 3,
            expected_sequence: 7,
        };
        let bytes = request.encode().expect("terminal encoding");
        let mut hostile = bytes;
        hostile[generated_registered_controller::REGISTERED_TERMINAL_ACTION_OFFSET] = 2;
        assert_eq!(
            RegisteredTerminalInstructionV1::decode(&hostile),
            Err(Error::InvalidAction)
        );
        let mut hostile = bytes;
        hostile[generated_registered_controller::REGISTERED_TERMINAL_RESERVED_OFFSET] = 1;
        assert_eq!(
            RegisteredTerminalInstructionV1::decode(&hostile),
            Err(Error::NonzeroReserved)
        );
        assert_eq!(
            RegisteredTerminalInstructionV1::decode(&bytes[..bytes.len() - 1]),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn registered_retirement_matches_lean_and_refuses_hostile_bytes() {
        let request = RegisteredRetireInstructionV1 {
            controller_bump: 2,
            registration_bump: 3,
        };
        let bytes = request.encode().expect("retirement encoding");
        assert_eq!(
            bytes,
            generated_registered_controller::REGISTERED_RETIRE_EXAMPLE
        );
        assert_eq!(RegisteredRetireInstructionV1::decode(&bytes), Ok(request));

        let mut hostile = bytes;
        hostile[generated_registered_controller::REGISTERED_RETIRE_RESERVED_OFFSET] = 1;
        assert_eq!(
            RegisteredRetireInstructionV1::decode(&hostile),
            Err(Error::NonzeroReserved)
        );
        assert_eq!(
            RegisteredRetireInstructionV1::decode(&bytes[..bytes.len() - 1]),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn hostile_registered_state_refuses() {
        let state = fixture_registered_state(2);
        let mut encoded = state.encode().expect("registered state encoding");
        assert_eq!(
            RegisteredIntentStateV1::decode(&encoded[..encoded.len() - 1]),
            Err(Error::InvalidLength)
        );
        encoded[0] ^= 1;
        assert_eq!(
            RegisteredIntentStateV1::decode(&encoded),
            Err(Error::InvalidMagic)
        );
        encoded = state.encode().expect("registered state encoding");
        encoded[generated_lifecycle::REGISTERED_STATE_VERSION_OFFSET] = 2;
        assert_eq!(
            RegisteredIntentStateV1::decode(&encoded),
            Err(Error::UnsupportedVersion)
        );
        encoded = state.encode().expect("registered state encoding");
        encoded[generated_lifecycle::REGISTERED_STATE_RESERVED_OFFSET] = 1;
        assert_eq!(
            RegisteredIntentStateV1::decode(&encoded),
            Err(Error::NonzeroReserved)
        );
        encoded = state.encode().expect("registered state encoding");
        encoded[generated_lifecycle::REGISTERED_STATE_PHASE_OFFSET] = 4;
        assert_eq!(
            RegisteredIntentStateV1::decode(&encoded),
            Err(Error::InvalidPhase)
        );
        encoded = state.encode().expect("registered state encoding");
        encoded[generated_lifecycle::REGISTERED_STATE_INTENT_OFFSET] ^= 1;
        assert_eq!(
            RegisteredIntentStateV1::decode(&encoded),
            Err(Error::InvalidMagic)
        );
    }

    #[test]
    fn generated_registered_program_derives_residual_state() {
        let gtc = fixture_registered_state(2);
        assert_eq!(registered_transition(gtc, 500), Ok((1_500, 1, 0)));
        assert_eq!(registered_transition(gtc, 2_000), Ok((0, 1, 1)));

        let ioc = fixture_registered_state(1);
        assert_eq!(registered_transition(ioc, 500), Ok((1_500, 1, 2)));

        let mut registers = Registers::zeroed();
        for (index, value) in [2, 2_000, 0, 0].into_iter().enumerate() {
            registers.set_scalar(index, value).expect("input register");
        }
        let before = registers;
        assert!(execute(&REGISTERED_FILL_PROGRAM, &mut registers).is_err());
        assert_eq!(registers, before, "refusal must preserve the full frame");
    }
}
