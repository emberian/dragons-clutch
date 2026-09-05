//! Fixed-layout request and receipt for permissionless Direct retirement start.
//!
//! This route is intentionally distinct from Core `CloseCapability`. It moves
//! only the existing Direct root tail from `Open` to `Retiring` after the
//! Trading adapter authenticates an already-`Retiring` Core Market. It neither
//! closes funding nor changes Core's outstanding-capability count.

use dclutch_sha256_adapter::digestv;

/// High selector reserved for beginning Direct root retirement.
pub const DIRECT_BEGIN_RETIRING_SELECTOR_V1: u32 = 0xffff_ff00;
/// Exact permissionless begin-retiring request width.
pub const DIRECT_BEGIN_RETIRING_REQUEST_BYTES_V1: usize = 320;
/// Exact begin-retiring receipt width.
pub const DIRECT_BEGIN_RETIRING_RECEIPT_BYTES_V1: usize = 320;
/// Begin-retiring request magic.
pub const DIRECT_BEGIN_RETIRING_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTDBR1";
/// Begin-retiring receipt magic.
pub const DIRECT_BEGIN_RETIRING_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTDRR1";
/// Implemented begin-retiring wire version.
pub const DIRECT_BEGIN_RETIRING_VERSION_V1: u16 = 1;
/// Canonical `u32` selector byte offset shared with the Direct ProgramSet.
pub const DIRECT_BEGIN_RETIRING_SELECTOR_OFFSET_V1: usize = 12;
/// Canonical context derivation domain.
pub const DIRECT_BEGIN_RETIRING_CONTEXT_DOMAIN_V1: &[u8] =
    b"dclutch/direct/begin-retiring-context/v1";
/// Finalized schema label for the begin-retiring request.
pub const DIRECT_BEGIN_RETIRING_REQUEST_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/direct-begin-retiring-request-v1";
/// SHA-256 of [`DIRECT_BEGIN_RETIRING_REQUEST_SCHEMA_PREIMAGE_V1`].
pub const DIRECT_BEGIN_RETIRING_REQUEST_SCHEMA_ID_V1: [u8; 32] = [
    0x00, 0xdc, 0x55, 0xf6, 0x85, 0xbd, 0x24, 0x85, 0x82, 0x72, 0xfc, 0xd6, 0xde, 0x37, 0x7b, 0x3b,
    0x9f, 0x2b, 0x98, 0xf6, 0x88, 0x6a, 0x23, 0x9a, 0x76, 0x28, 0x43, 0x70, 0x69, 0xd7, 0x29, 0x94,
];

/// Exact number of scalar registers in the authenticated retirement artifacts.
pub const DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1: u16 = 10;
/// Exact number of identity registers in the authenticated retirement artifacts.
pub const DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1: u16 = 2;
/// Sole runtime account: the existing composite Direct root.
pub const DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1: u16 = 0;
/// Caller-supplied selector scalar.
pub const DIRECT_BEGIN_RETIRING_SELECTOR_SCALAR_V1: u16 = 0;
/// Profile-projected Direct root magic scalar.
pub const DIRECT_BEGIN_RETIRING_ROOT_MAGIC_SCALAR_V1: u16 = 1;
/// Profile-projected Direct root version/phase/reserved word scalar.
pub const DIRECT_BEGIN_RETIRING_ROOT_HEADER_SCALAR_V1: u16 = 2;
/// Profile-projected open-maker count scalar.
pub const DIRECT_BEGIN_RETIRING_MAKER_COUNT_SCALAR_V1: u16 = 3;
/// Profile-projected root lamport balance scalar.
pub const DIRECT_BEGIN_RETIRING_ROOT_LAMPORTS_SCALAR_V1: u16 = 4;
/// Transition constant containing the one reserved lifecycle selector.
pub const DIRECT_BEGIN_RETIRING_EXPECTED_SELECTOR_SCALAR_V1: u16 = 5;
/// Transition constant containing the canonical Direct root magic word.
pub const DIRECT_BEGIN_RETIRING_EXPECTED_MAGIC_SCALAR_V1: u16 = 6;
/// Transition constant containing the canonical Open root header word.
pub const DIRECT_BEGIN_RETIRING_EXPECTED_OPEN_HEADER_SCALAR_V1: u16 = 7;
/// Transition constant containing canonical zero.
pub const DIRECT_BEGIN_RETIRING_EXPECTED_ZERO_SCALAR_V1: u16 = 8;
/// Transition output containing the canonical Retiring root header word.
pub const DIRECT_BEGIN_RETIRING_RETIRING_HEADER_SCALAR_V1: u16 = 9;
/// Caller-supplied current Trading program identity register.
pub const DIRECT_BEGIN_RETIRING_TRADING_IDENTITY_V1: u16 = 0;
/// Caller-supplied exact composite Direct root identity register.
pub const DIRECT_BEGIN_RETIRING_ROOT_IDENTITY_V1: u16 = 1;

/// Exact number of accounts in the permissionless top-level instruction.
pub const DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1: usize = 20;
/// Existing writable composite Direct root.
pub const DIRECT_BEGIN_RETIRING_ROOT_TOP_ACCOUNT_V1: usize = 0;
/// Canonical readonly Core Market state.
pub const DIRECT_BEGIN_RETIRING_MARKET_ACCOUNT_V1: usize = 1;
/// Root-selected persistent manifest raw record.
pub const DIRECT_BEGIN_RETIRING_MANIFEST_RAW_ACCOUNT_V1: usize = 2;
/// Finalized ProgramSet raw record.
pub const DIRECT_BEGIN_RETIRING_PROGRAM_SET_RAW_ACCOUNT_V1: usize = 3;
/// Vacant ProgramSet staging coordinate.
pub const DIRECT_BEGIN_RETIRING_PROGRAM_SET_STAGING_ACCOUNT_V1: usize = 4;
/// Finalized selected descriptor raw record.
pub const DIRECT_BEGIN_RETIRING_DESCRIPTOR_RAW_ACCOUNT_V1: usize = 5;
/// Vacant selected-descriptor staging coordinate.
pub const DIRECT_BEGIN_RETIRING_DESCRIPTOR_STAGING_ACCOUNT_V1: usize = 6;
/// Finalized Direct config raw record.
pub const DIRECT_BEGIN_RETIRING_CONFIG_RAW_ACCOUNT_V1: usize = 7;
/// Vacant Direct-config staging coordinate.
pub const DIRECT_BEGIN_RETIRING_CONFIG_STAGING_ACCOUNT_V1: usize = 8;
/// Finalized begin-retiring AccountProfile raw record.
pub const DIRECT_BEGIN_RETIRING_PROFILE_RAW_ACCOUNT_V1: usize = 9;
/// Vacant AccountProfile staging coordinate.
pub const DIRECT_BEGIN_RETIRING_PROFILE_STAGING_ACCOUNT_V1: usize = 10;
/// Finalized begin-retiring EffectProgram raw record.
pub const DIRECT_BEGIN_RETIRING_EFFECT_RAW_ACCOUNT_V1: usize = 11;
/// Vacant EffectProgram staging coordinate.
pub const DIRECT_BEGIN_RETIRING_EFFECT_STAGING_ACCOUNT_V1: usize = 12;
/// Registry activation cache for the request release set.
pub const DIRECT_BEGIN_RETIRING_ACTIVATION_CACHE_ACCOUNT_V1: usize = 13;
/// Current executable Core program.
pub const DIRECT_BEGIN_RETIRING_CORE_PROGRAM_ACCOUNT_V1: usize = 14;
/// Current Core upgradeable-loader ProgramData.
pub const DIRECT_BEGIN_RETIRING_CORE_PROGRAMDATA_ACCOUNT_V1: usize = 15;
/// Current executable Trading program.
pub const DIRECT_BEGIN_RETIRING_TRADING_PROGRAM_ACCOUNT_V1: usize = 16;
/// Current Trading upgradeable-loader ProgramData.
pub const DIRECT_BEGIN_RETIRING_TRADING_PROGRAMDATA_ACCOUNT_V1: usize = 17;
/// Executable Registry program.
pub const DIRECT_BEGIN_RETIRING_REGISTRY_ACCOUNT_V1: usize = 18;
/// Readonly Rent sysvar.
pub const DIRECT_BEGIN_RETIRING_RENT_ACCOUNT_V1: usize = 19;

/// Return the exact `(writable, executable)` membrane for one account index.
#[must_use]
pub const fn direct_begin_retiring_account_privileges_v1(index: usize) -> Option<(bool, bool)> {
    if index >= DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1 {
        None
    } else {
        Some((
            index == DIRECT_BEGIN_RETIRING_ROOT_TOP_ACCOUNT_V1,
            index == DIRECT_BEGIN_RETIRING_CORE_PROGRAM_ACCOUNT_V1
                || index == DIRECT_BEGIN_RETIRING_TRADING_PROGRAM_ACCOUNT_V1
                || index == DIRECT_BEGIN_RETIRING_REGISTRY_ACCOUNT_V1,
        ))
    }
}

const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const CONTEXT_OFFSET: usize = 80;
const ROOT_OFFSET: usize = 112;
const MANIFEST_OFFSET: usize = 144;
const PROGRAM_SET_OFFSET: usize = 176;
const CONFIG_OFFSET: usize = 208;
const MARKET_DIGEST_OFFSET: usize = 240;
const ROOT_DIGEST_OFFSET: usize = 272;
const GENERATION_OFFSET: usize = 304;
const ENTRY_INDEX_OFFSET: usize = 312;

const RECEIPT_REQUEST_DIGEST_OFFSET: usize = 16;
const RECEIPT_MARKET_OFFSET: usize = 48;
const RECEIPT_ROOT_OFFSET: usize = 80;
const RECEIPT_PRE_ROOT_DIGEST_OFFSET: usize = 112;
const RECEIPT_POST_ROOT_DIGEST_OFFSET: usize = 144;
const RECEIPT_CONTEXT_OFFSET: usize = 176;
const RECEIPT_PROGRAM_SET_OFFSET: usize = 208;
const RECEIPT_TRADING_PROGRAM_OFFSET: usize = 240;
const RECEIPT_RELEASE_SET_OFFSET: usize = 272;
const RECEIPT_GENERATION_OFFSET: usize = 304;
const RECEIPT_ENTRY_INDEX_OFFSET: usize = 312;

/// Stable request/receipt refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectBeginRetiringErrorV1 {
    /// A wire did not have its one exact width.
    InvalidLength,
    /// Magic or version selected another route.
    InvalidHeader,
    /// Reserved bytes or the selector were noncanonical.
    NonCanonical,
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// The supplied context was not the canonical derived context.
    ContextMismatch,
    /// A structurally valid receipt did not equal the one exact request result.
    ReceiptMismatch,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, DirectBeginRetiringErrorV1>;

/// Exact immutable permissionless request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBeginRetiringRequestV1 {
    /// Selected execution release set.
    pub release_set: [u8; 32],
    /// Canonical Core Market PDA.
    pub market: [u8; 32],
    /// Canonical digest-derived route context.
    pub context: [u8; 32],
    /// Canonical composite Direct root PDA.
    pub root: [u8; 32],
    /// Root-selected capability manifest identity.
    pub manifest: [u8; 32],
    /// Root-selected Direct ProgramSet identity.
    pub program_set: [u8; 32],
    /// Root-selected Direct config identity.
    pub config: [u8; 32],
    /// SHA-256 of the exact authenticated pre-Market bytes.
    pub expected_market_digest: [u8; 32],
    /// SHA-256 of the exact authenticated pre-root bytes.
    pub expected_root_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Root-selected manifest entry index.
    pub entry_index: u16,
}

impl DirectBeginRetiringRequestV1 {
    /// Construct one exact request and require its derived context.
    pub fn new(self) -> Result<Self> {
        for identity in [
            self.release_set,
            self.market,
            self.context,
            self.root,
            self.manifest,
            self.program_set,
            self.config,
            self.expected_market_digest,
            self.expected_root_digest,
        ] {
            if identity.iter().all(|byte| *byte == 0) {
                return Err(DirectBeginRetiringErrorV1::ZeroIdentity);
            }
        }
        if self.context
            != direct_begin_retiring_context_v1(
                self.release_set,
                self.market,
                self.root,
                self.manifest,
                self.program_set,
                self.config,
                self.generation,
                self.entry_index,
            )
        {
            return Err(DirectBeginRetiringErrorV1::ContextMismatch);
        }
        Ok(self)
    }

    /// Hostile-decode one exact request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, DIRECT_BEGIN_RETIRING_REQUEST_MAGIC_V1)?;
        if u32_at(input, DIRECT_BEGIN_RETIRING_SELECTOR_OFFSET_V1)?
            != DIRECT_BEGIN_RETIRING_SELECTOR_V1
            || input.get(10..12).is_none_or(|bytes| bytes != [0, 0])
            || input.get(314..320).is_none_or(|bytes| bytes != [0; 6])
        {
            return Err(DirectBeginRetiringErrorV1::NonCanonical);
        }
        Self {
            release_set: array(input, RELEASE_SET_OFFSET)?,
            market: array(input, MARKET_OFFSET)?,
            context: array(input, CONTEXT_OFFSET)?,
            root: array(input, ROOT_OFFSET)?,
            manifest: array(input, MANIFEST_OFFSET)?,
            program_set: array(input, PROGRAM_SET_OFFSET)?,
            config: array(input, CONFIG_OFFSET)?,
            expected_market_digest: array(input, MARKET_DIGEST_OFFSET)?,
            expected_root_digest: array(input, ROOT_DIGEST_OFFSET)?,
            generation: u64_at(input, GENERATION_OFFSET)?,
            entry_index: u16_at(input, ENTRY_INDEX_OFFSET)?,
        }
        .new()
    }

    /// Encode the one canonical request.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_BEGIN_RETIRING_REQUEST_BYTES_V1]> {
        Self::new(self)?;
        let mut output = [0_u8; DIRECT_BEGIN_RETIRING_REQUEST_BYTES_V1];
        put(&mut output, 0, &DIRECT_BEGIN_RETIRING_REQUEST_MAGIC_V1)?;
        put(
            &mut output,
            8,
            &DIRECT_BEGIN_RETIRING_VERSION_V1.to_le_bytes(),
        )?;
        put(
            &mut output,
            DIRECT_BEGIN_RETIRING_SELECTOR_OFFSET_V1,
            &DIRECT_BEGIN_RETIRING_SELECTOR_V1.to_le_bytes(),
        )?;
        for (offset, value) in [
            (RELEASE_SET_OFFSET, self.release_set),
            (MARKET_OFFSET, self.market),
            (CONTEXT_OFFSET, self.context),
            (ROOT_OFFSET, self.root),
            (MANIFEST_OFFSET, self.manifest),
            (PROGRAM_SET_OFFSET, self.program_set),
            (CONFIG_OFFSET, self.config),
            (MARKET_DIGEST_OFFSET, self.expected_market_digest),
            (ROOT_DIGEST_OFFSET, self.expected_root_digest),
        ] {
            put(&mut output, offset, &value)?;
        }
        put(
            &mut output,
            GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            ENTRY_INDEX_OFFSET,
            &self.entry_index.to_le_bytes(),
        )?;
        Ok(output)
    }
}

/// Exact successful poststate receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectBeginRetiringReceiptV1 {
    /// SHA-256 of the exact request bytes.
    pub request_digest: [u8; 32],
    /// Canonical Core Market PDA.
    pub market: [u8; 32],
    /// Canonical Direct root PDA.
    pub root: [u8; 32],
    /// Exact root digest before the transition.
    pub pre_root_digest: [u8; 32],
    /// Exact root digest after the transition.
    pub post_root_digest: [u8; 32],
    /// Canonical request context.
    pub context: [u8; 32],
    /// Root-selected ProgramSet identity.
    pub program_set: [u8; 32],
    /// Current Trading program producing the receipt.
    pub trading_program: [u8; 32],
    /// Selected execution release set.
    pub release_set: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Selected manifest entry index.
    pub entry_index: u16,
}

impl DirectBeginRetiringReceiptV1 {
    /// Construct a receipt bound to one exact request and post-root digest.
    pub fn new(
        request: DirectBeginRetiringRequestV1,
        request_digest: [u8; 32],
        post_root_digest: [u8; 32],
        trading_program: [u8; 32],
    ) -> Result<Self> {
        request.new()?;
        if request_digest.iter().all(|byte| *byte == 0)
            || post_root_digest.iter().all(|byte| *byte == 0)
            || trading_program.iter().all(|byte| *byte == 0)
        {
            return Err(DirectBeginRetiringErrorV1::ZeroIdentity);
        }
        Ok(Self {
            request_digest,
            market: request.market,
            root: request.root,
            pre_root_digest: request.expected_root_digest,
            post_root_digest,
            context: request.context,
            program_set: request.program_set,
            trading_program,
            release_set: request.release_set,
            generation: request.generation,
            entry_index: request.entry_index,
        })
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, DIRECT_BEGIN_RETIRING_RECEIPT_MAGIC_V1)?;
        if u32_at(input, DIRECT_BEGIN_RETIRING_SELECTOR_OFFSET_V1)?
            != DIRECT_BEGIN_RETIRING_SELECTOR_V1
            || input.get(10..12).is_none_or(|bytes| bytes != [0, 0])
            || input.get(314..320).is_none_or(|bytes| bytes != [0; 6])
        {
            return Err(DirectBeginRetiringErrorV1::NonCanonical);
        }
        let value = Self {
            request_digest: array(input, RECEIPT_REQUEST_DIGEST_OFFSET)?,
            market: array(input, RECEIPT_MARKET_OFFSET)?,
            root: array(input, RECEIPT_ROOT_OFFSET)?,
            pre_root_digest: array(input, RECEIPT_PRE_ROOT_DIGEST_OFFSET)?,
            post_root_digest: array(input, RECEIPT_POST_ROOT_DIGEST_OFFSET)?,
            context: array(input, RECEIPT_CONTEXT_OFFSET)?,
            program_set: array(input, RECEIPT_PROGRAM_SET_OFFSET)?,
            trading_program: array(input, RECEIPT_TRADING_PROGRAM_OFFSET)?,
            release_set: array(input, RECEIPT_RELEASE_SET_OFFSET)?,
            generation: u64_at(input, RECEIPT_GENERATION_OFFSET)?,
            entry_index: u16_at(input, RECEIPT_ENTRY_INDEX_OFFSET)?,
        };
        for identity in [
            value.request_digest,
            value.market,
            value.root,
            value.pre_root_digest,
            value.post_root_digest,
            value.context,
            value.program_set,
            value.trading_program,
            value.release_set,
        ] {
            if identity.iter().all(|byte| *byte == 0) {
                return Err(DirectBeginRetiringErrorV1::ZeroIdentity);
            }
        }
        Ok(value)
    }

    /// Authenticate this receipt against exact request bytes and observed poststate.
    ///
    /// Structural receipt decoding is intentionally insufficient: consumers
    /// must join every duplicated coordinate to the request and exact post-root
    /// digest at the same finalized snapshot.
    pub fn authenticate_for_request(
        self,
        request_bytes: &[u8],
        observed_post_root_digest: [u8; 32],
        current_trading_program: [u8; 32],
    ) -> Result<Self> {
        let request = DirectBeginRetiringRequestV1::decode(request_bytes)?;
        let expected = Self::new(
            request,
            dclutch_sha256_adapter::digest(request_bytes),
            observed_post_root_digest,
            current_trading_program,
        )?;
        if self != expected {
            return Err(DirectBeginRetiringErrorV1::ReceiptMismatch);
        }
        Ok(self)
    }

    /// Encode the one canonical receipt.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_BEGIN_RETIRING_RECEIPT_BYTES_V1]> {
        let mut output = [0_u8; DIRECT_BEGIN_RETIRING_RECEIPT_BYTES_V1];
        put(&mut output, 0, &DIRECT_BEGIN_RETIRING_RECEIPT_MAGIC_V1)?;
        put(
            &mut output,
            8,
            &DIRECT_BEGIN_RETIRING_VERSION_V1.to_le_bytes(),
        )?;
        put(
            &mut output,
            DIRECT_BEGIN_RETIRING_SELECTOR_OFFSET_V1,
            &DIRECT_BEGIN_RETIRING_SELECTOR_V1.to_le_bytes(),
        )?;
        for (offset, value) in [
            (RECEIPT_REQUEST_DIGEST_OFFSET, self.request_digest),
            (RECEIPT_MARKET_OFFSET, self.market),
            (RECEIPT_ROOT_OFFSET, self.root),
            (RECEIPT_PRE_ROOT_DIGEST_OFFSET, self.pre_root_digest),
            (RECEIPT_POST_ROOT_DIGEST_OFFSET, self.post_root_digest),
            (RECEIPT_CONTEXT_OFFSET, self.context),
            (RECEIPT_PROGRAM_SET_OFFSET, self.program_set),
            (RECEIPT_TRADING_PROGRAM_OFFSET, self.trading_program),
            (RECEIPT_RELEASE_SET_OFFSET, self.release_set),
        ] {
            put(&mut output, offset, &value)?;
        }
        put(
            &mut output,
            RECEIPT_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            RECEIPT_ENTRY_INDEX_OFFSET,
            &self.entry_index.to_le_bytes(),
        )?;
        Self::decode(&output)?;
        Ok(output)
    }
}

/// Whether bytes select this dedicated route, including malformed payloads.
#[must_use]
pub fn is_direct_begin_retiring_v1(input: &[u8]) -> bool {
    input.get(..8) == Some(DIRECT_BEGIN_RETIRING_REQUEST_MAGIC_V1.as_slice())
}

/// Derive the one canonical lifecycle context from immutable root selection facts.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn direct_begin_retiring_context_v1(
    release_set: [u8; 32],
    market: [u8; 32],
    root: [u8; 32],
    manifest: [u8; 32],
    program_set: [u8; 32],
    config: [u8; 32],
    generation: u64,
    entry_index: u16,
) -> [u8; 32] {
    digestv(&[
        DIRECT_BEGIN_RETIRING_CONTEXT_DOMAIN_V1,
        &release_set,
        &market,
        &root,
        &manifest,
        &program_set,
        &config,
        &generation.to_le_bytes(),
        &entry_index.to_le_bytes(),
    ])
}

fn require_header(input: &[u8], magic: [u8; 8]) -> Result<()> {
    if input.len() != DIRECT_BEGIN_RETIRING_REQUEST_BYTES_V1 {
        return Err(DirectBeginRetiringErrorV1::InvalidLength);
    }
    if input.get(..8) != Some(magic.as_slice())
        || u16_at(input, 8)? != DIRECT_BEGIN_RETIRING_VERSION_V1
    {
        return Err(DirectBeginRetiringErrorV1::InvalidHeader);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(DirectBeginRetiringErrorV1::InvalidLength)?,
        )
        .ok_or(DirectBeginRetiringErrorV1::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn array(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(32)
                    .ok_or(DirectBeginRetiringErrorV1::InvalidLength)?,
        )
        .ok_or(DirectBeginRetiringErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| DirectBeginRetiringErrorV1::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        input
            .get(offset..offset + 2)
            .ok_or(DirectBeginRetiringErrorV1::InvalidLength)?
            .try_into()
            .map_err(|_| DirectBeginRetiringErrorV1::InvalidLength)?,
    ))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        input
            .get(offset..offset + 4)
            .ok_or(DirectBeginRetiringErrorV1::InvalidLength)?
            .try_into()
            .map_err(|_| DirectBeginRetiringErrorV1::InvalidLength)?,
    ))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        input
            .get(offset..offset + 8)
            .ok_or(DirectBeginRetiringErrorV1::InvalidLength)?
            .try_into()
            .map_err(|_| DirectBeginRetiringErrorV1::InvalidLength)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_sha256_adapter::digest;

    fn request() -> DirectBeginRetiringRequestV1 {
        let release_set = [1; 32];
        let market = [2; 32];
        let root = [3; 32];
        let manifest = [4; 32];
        let program_set = [5; 32];
        let config = [6; 32];
        DirectBeginRetiringRequestV1 {
            release_set,
            market,
            context: direct_begin_retiring_context_v1(
                release_set,
                market,
                root,
                manifest,
                program_set,
                config,
                7,
                8,
            ),
            root,
            manifest,
            program_set,
            config,
            expected_market_digest: [9; 32],
            expected_root_digest: [10; 32],
            generation: 7,
            entry_index: 8,
        }
    }

    #[test]
    fn exact_request_and_receipt_round_trip() {
        assert_eq!(
            digest(DIRECT_BEGIN_RETIRING_REQUEST_SCHEMA_PREIMAGE_V1),
            DIRECT_BEGIN_RETIRING_REQUEST_SCHEMA_ID_V1
        );
        let request = request();
        let bytes = request.to_bytes().expect("request");
        assert!(is_direct_begin_retiring_v1(&bytes));
        assert_eq!(DirectBeginRetiringRequestV1::decode(&bytes), Ok(request));
        let receipt =
            DirectBeginRetiringReceiptV1::new(request, digest(&bytes), [11; 32], [12; 32])
                .expect("receipt");
        let receipt_bytes = receipt.to_bytes().expect("receipt bytes");
        assert_eq!(
            DirectBeginRetiringReceiptV1::decode(&receipt_bytes),
            Ok(receipt)
        );
    }

    #[test]
    fn selector_context_reserved_and_digest_substitution_refuse() {
        let request = request();
        let bytes = request.to_bytes().expect("request");
        for offset in [
            10_usize,
            DIRECT_BEGIN_RETIRING_SELECTOR_OFFSET_V1,
            CONTEXT_OFFSET,
            319,
        ] {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("byte") ^= 1;
            assert!(DirectBeginRetiringRequestV1::decode(&hostile).is_err());
        }
        let mut zero = request;
        zero.expected_root_digest = [0; 32];
        assert_eq!(
            zero.to_bytes(),
            Err(DirectBeginRetiringErrorV1::ZeroIdentity)
        );
    }

    #[test]
    fn receipt_requires_exact_request_poststate_and_trading_producer() {
        let request = request();
        let request_bytes = request.to_bytes().expect("request");
        let receipt =
            DirectBeginRetiringReceiptV1::new(request, digest(&request_bytes), [11; 32], [12; 32])
                .expect("receipt");
        assert_eq!(
            receipt.authenticate_for_request(&request_bytes, [11; 32], [12; 32]),
            Ok(receipt)
        );
        for (post, trading) in [([13; 32], [12; 32]), ([11; 32], [13; 32])] {
            assert_eq!(
                receipt.authenticate_for_request(&request_bytes, post, trading),
                Err(DirectBeginRetiringErrorV1::ReceiptMismatch)
            );
        }
        let mut hostile = request_bytes;
        hostile[MARKET_DIGEST_OFFSET] ^= 1;
        assert_eq!(
            receipt.authenticate_for_request(&hostile, [11; 32], [12; 32]),
            Err(DirectBeginRetiringErrorV1::ReceiptMismatch)
        );
    }

    #[test]
    fn top_level_membrane_has_one_writable_and_three_executables() {
        let mut writable_count = 0;
        let mut executable_count = 0;
        for index in 0..DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1 {
            let (writable, executable) =
                direct_begin_retiring_account_privileges_v1(index).expect("privileges");
            writable_count += usize::from(writable);
            executable_count += usize::from(executable);
        }
        assert_eq!(writable_count, 1);
        assert_eq!(executable_count, 3);
        assert_eq!(direct_begin_retiring_account_privileges_v1(20), None);
    }
}
