//! Market-free Fractional selection config: the record a capability manifest
//! entry names.
//!
//! # Why this record exists
//!
//! A founded Market's PDA derives from `MarketIdentity` seeds that include the
//! capability manifest digest, so every identity a manifest entry carries must
//! be derivable **before** the Market address exists. Fractional's descriptor
//! previously named [`FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2`] as its config
//! schema, which made the entry's config identity `SHA-256(terms)` — and the
//! terms bind the Core Market at header offset 16. That is a SHA-256 fixed
//! point (`manifest ⊃ config_id = SHA-256(bytes containing Market)` with
//! `Market = f(SHA-256(manifest))`) and no author can construct it. The
//! Fractional capability was therefore unselectable by any founded Market.
//!
//! This record is the market-free half of the terms. It carries exactly the
//! coordinates a *selection* decision needs — the exact denominator, the two
//! widths, the Token program, and the stable source graph — and no Market, no
//! Product record, no release set, and no shard Mint. It is constructible
//! from nothing but a family's own release intent, so a manifest entry naming
//! it is acyclic.
//!
//! # Why the N→K exposure is NOT here
//!
//! The obvious field list for this record includes `exposure_id`, and the
//! first version of it did. That is wrong, and wrong in the subtle way this
//! whole record exists to prevent: `exposure_id` is the content id of a
//! `CompositionExposureBundleV3`, and that record carries the **Market** at
//! byte 16 (`COMPOSITION_EXPOSURE_MARKET_OFFSET_V3`), pinned equal to
//! `terms.market()` by `check_fractional_exposure_bundle_v2`. So an
//! `exposure_id` is a digest over bytes containing the Market — a config
//! naming it is market-derived, and the fixed point survives one hop deeper
//! while every surface-level check still passes.
//!
//! The invariant is not "does the config contain the Market" but "does any
//! record the config NAMES contain it", applied through the full closure.
//!
//! Nothing is left unpinned by the omission, because the exposure kernel
//! already draws exactly this line for its own reasons:
//! `CompositionExposureBundleV3` verifies its market-bearing identities
//! through `verify_execution_for` and its `graph_id` through a separate
//! check. Selection pins the **graph**; the graph pins the admitted
//! source-DAG correspondence of the matrix (that kernel "never accepts a
//! caller-authored matrix"); and the exposure record is content-addressed and
//! verified against both the graph and the market-bearing terms at execution.
//! The authority does not vanish — it moves to the side of the seam where it
//! can exist before the Market does.
//!
//! # The split, and where each half is authoritative
//!
//! The terms record does not go away and does not lose a field. It remains the
//! **execution** record: the durable owner of the shard Mints, the Market
//! binding, the release set, and the bases. What changes is that it is no
//! longer *named by the manifest*; it is joined to the manifest-named config
//! **at runtime**, by [`join_fractional_selection_config_v1`], which requires
//! every market-free field to agree. The market-bearing fields are bound
//! separately and were always bound separately — the root and the Claims route
//! already pin `terms.market` against the request and the Core Market.
//!
//! So the two records are not two authors of one fact. Each field has exactly
//! one authority, and the join is the single place that says so:
//!
//! | fact | authority | how the other side is bound |
//! |---|---|---|
//! | denominator, widths, token program, graph | **this record** | terms must equal it (the join) |
//! | Market, exposure, release set, product record, bases, shard Mints | **the terms** | not in this record at all |
//!
//! A disagreement on any market-free field is [`Error::SelectionConfigMismatch`]
//! — a distinct code from [`Error::AdmissionMismatch`], because the two have
//! different causes and different fixes: an admission mismatch means a record
//! was substituted, while this means the selection a Market committed to and
//! the terms it executes against describe different instruments.

use crate::{Error, FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsV2, Result};

/// Exact encoded width of one selection config record.
pub const FRACTIONAL_SELECTION_CONFIG_BYTES_V1: usize = 128;
/// Exact selection-config magic.
pub const FRACTIONAL_SELECTION_CONFIG_MAGIC_V1: [u8; 8] = *b"DCFRSC01";
/// Selection-config schema preimage.
///
/// Named beside the digest it hashes to so a control can recompute the
/// identity rather than trust a pasted constant.
pub const FRACTIONAL_SELECTION_CONFIG_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/fractional-selection-config-v1|header128|market-free|exact-denominator|productN1..512|claimsK1..256|token-program|graph";
/// SHA-256 of [`FRACTIONAL_SELECTION_CONFIG_SCHEMA_PREIMAGE_V1`].
pub const FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1: [u8; 32] = [
    0x1b, 0x9c, 0xed, 0x09, 0xad, 0xdd, 0xc1, 0x22, 0xd7, 0xfc, 0x7d, 0xec, 0xa0, 0x74, 0x2f, 0xa0,
    0x4f, 0x2a, 0x84, 0xd3, 0x4c, 0xf6, 0xc8, 0xac, 0xf7, 0x4c, 0xb2, 0x6f, 0x7f, 0x9e, 0xda, 0x1a,
];

const VERSION_V1: u16 = 1;
const RESERVED_HEAD_OFFSET: usize = 10;
const RESERVED_HEAD_BYTES: usize = 6;
const TOKEN_PROGRAM_OFFSET: usize = 16;
const GRAPH_ID_OFFSET: usize = 48;
const PRODUCT_WIDTH_OFFSET: usize = 80;
const REPRESENTATION_WIDTH_OFFSET: usize = 84;
const DENOMINATOR_OFFSET: usize = 88;

/// Atomic encoder input for one market-free selection config.
///
/// There is deliberately no Market field, and no field from which a Market
/// could be recovered. That absence is the record's entire purpose: a
/// parameter through which a Market could arrive would reopen the fixed point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalSelectionConfigInputV1 {
    /// Selected Token program.
    pub token_program: [u8; 32],
    /// Stable source graph identity.
    ///
    /// The graph, NOT the exposure it admits: an `exposure_id` is a digest
    /// over bytes carrying the Market, so naming one here would reopen the
    /// fixed point. See the module header.
    pub graph_id: [u8; 32],
    /// Product terminal-result width `N`.
    pub product_width: u32,
    /// Claims representation width `K`.
    pub representation_width: u32,
    /// Exact shard atoms per whole Claims coordinate.
    pub denominator: u64,
}

/// One decoded, canonical, market-free selection config.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalSelectionConfigV1<'a> {
    bytes: &'a [u8],
}

impl<'a> FractionalSelectionConfigV1<'a> {
    /// Hostile-decode exact canonical selection-config bytes.
    ///
    /// Refuses a wrong width, a foreign magic, an unsupported version,
    /// noncanonical reserved bytes, a zero identity, and every scalar the
    /// encoder itself refuses — so a decoded config is exactly a config the
    /// encoder could have produced.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        if input.len() != FRACTIONAL_SELECTION_CONFIG_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(input, 0)? != FRACTIONAL_SELECTION_CONFIG_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, 8)? != VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, RESERVED_HEAD_OFFSET, RESERVED_HEAD_BYTES)?;
        require_zero(
            input,
            DENOMINATOR_OFFSET
                .checked_add(8)
                .ok_or(Error::InvalidLength)?,
            FRACTIONAL_SELECTION_CONFIG_BYTES_V1
                .checked_sub(
                    DENOMINATOR_OFFSET
                        .checked_add(8)
                        .ok_or(Error::InvalidLength)?,
                )
                .ok_or(Error::InvalidLength)?,
        )?;
        for offset in [TOKEN_PROGRAM_OFFSET, GRAPH_ID_OFFSET] {
            let _ = nonzero(input, offset)?;
        }
        let config = Self { bytes: input };
        check_scalars(
            config.product_width(),
            config.representation_width(),
            config.denominator(),
        )?;
        Ok(config)
    }

    /// Exact canonical bytes this config decoded from.
    #[must_use]
    pub const fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Selected Token program.
    pub fn token_program(&self) -> Result<[u8; 32]> {
        array(self.bytes, TOKEN_PROGRAM_OFFSET)
    }

    /// Stable source graph identity.
    pub fn graph_id(&self) -> Result<[u8; 32]> {
        array(self.bytes, GRAPH_ID_OFFSET)
    }

    /// Product terminal-result width `N`.
    #[must_use]
    pub fn product_width(&self) -> u32 {
        u32_at(self.bytes, PRODUCT_WIDTH_OFFSET).unwrap_or(0)
    }

    /// Claims representation width `K`.
    #[must_use]
    pub fn representation_width(&self) -> u32 {
        u32_at(self.bytes, REPRESENTATION_WIDTH_OFFSET).unwrap_or(0)
    }

    /// Exact shard atoms per whole Claims coordinate.
    #[must_use]
    pub fn denominator(&self) -> u64 {
        u64_at(self.bytes, DENOMINATOR_OFFSET).unwrap_or(0)
    }
}

/// Project the market-free half of authenticated terms.
///
/// This is the **sole** author of the correspondence between a terms record
/// and its selection config. The release compiler derives the config it
/// publishes through this function and the runtime join checks against this
/// same projection, so "which fields are market-free" is stated once. Adding a
/// field to the config without adding it here cannot silently pass: the join
/// reads the projection, not the caller's opinion of it.
pub fn fractional_selection_config_from_terms_v1(
    terms: FractionalExposureTermsV2<'_>,
) -> FractionalSelectionConfigInputV1 {
    FractionalSelectionConfigInputV1 {
        token_program: terms.token_program(),
        graph_id: terms.graph_id(),
        product_width: terms.product_width(),
        representation_width: terms.representation_width(),
        denominator: terms.denominator(),
    }
}

/// Encode one market-free selection config into an exact caller buffer.
pub fn encode_fractional_selection_config_v1(
    input: FractionalSelectionConfigInputV1,
    output: &mut [u8],
) -> Result<()> {
    if output.len() != FRACTIONAL_SELECTION_CONFIG_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    for value in [input.token_program, input.graph_id] {
        if value == [0; 32] {
            return Err(Error::ZeroIdentity);
        }
    }
    check_scalars(
        input.product_width,
        input.representation_width,
        input.denominator,
    )?;
    output.fill(0);
    put(output, 0, &FRACTIONAL_SELECTION_CONFIG_MAGIC_V1)?;
    put(output, 8, &VERSION_V1.to_le_bytes())?;
    put(output, TOKEN_PROGRAM_OFFSET, &input.token_program)?;
    put(output, GRAPH_ID_OFFSET, &input.graph_id)?;
    put(
        output,
        PRODUCT_WIDTH_OFFSET,
        &input.product_width.to_le_bytes(),
    )?;
    put(
        output,
        REPRESENTATION_WIDTH_OFFSET,
        &input.representation_width.to_le_bytes(),
    )?;
    put(output, DENOMINATOR_OFFSET, &input.denominator.to_le_bytes())?;
    Ok(())
}

/// Join one manifest-named selection config to the terms that execute under it.
///
/// This is the runtime half of the config split, and the only thing standing
/// between a Market's selection and the instrument it actually settles. Every
/// market-free field must agree exactly. The market-bearing fields are not
/// examined here — they have their own binds, against the request and the Core
/// Market, and duplicating them here would make this a second author of facts
/// that already have one.
///
/// Refuses with [`Error::SelectionConfigMismatch`] on any disagreement.
pub fn join_fractional_selection_config_v1(
    config: FractionalSelectionConfigV1<'_>,
    terms: FractionalExposureTermsV2<'_>,
) -> Result<()> {
    let expected = fractional_selection_config_from_terms_v1(terms);
    if config.token_program()? != expected.token_program
        || config.graph_id()? != expected.graph_id
        || config.product_width() != expected.product_width
        || config.representation_width() != expected.representation_width
        || config.denominator() != expected.denominator
    {
        return Err(Error::SelectionConfigMismatch);
    }
    Ok(())
}

/// Admission for a selection config observed as a finalized Record.
///
/// The schema a descriptor selects must be the selection-config schema and not
/// the terms schema. Naming both explicitly is what makes an attempt to serve
/// the old market-bearing terms as the manifest config a refusal rather than a
/// silent success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalSelectionConfigAdmissionV1 {
    /// Selection-config schema selected by the immutable capability descriptor.
    pub selected_schema_id: [u8; 32],
    /// Selection-config schema observed in the finalized Record coordinates.
    pub finalized_schema_id: [u8; 32],
    /// SHA-256 recomputed over the exact config bytes by the outer adapter.
    pub recomputed_config_digest: [u8; 32],
    /// Digest committed by the finalized Record identity.
    pub finalized_config_digest: [u8; 32],
    /// Config identity the capability manifest entry named.
    pub selected_config_id: [u8; 32],
}

/// Admit one finalized selection-config Record and decode it.
///
/// Refuses the terms schema explicitly, so the pre-split arrangement cannot be
/// reintroduced by a caller that simply keeps passing the terms.
pub fn admit_fractional_selection_config_v1<'a>(
    input: &'a [u8],
    admission: FractionalSelectionConfigAdmissionV1,
) -> Result<FractionalSelectionConfigV1<'a>> {
    if admission.selected_schema_id == FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2
        || admission.selected_schema_id != FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1
        || admission.finalized_schema_id != FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1
    {
        return Err(Error::AdmissionMismatch);
    }
    if admission.recomputed_config_digest != admission.finalized_config_digest
        || admission.selected_config_id != admission.finalized_config_digest
        || admission.selected_config_id == [0; 32]
    {
        return Err(Error::AdmissionMismatch);
    }
    FractionalSelectionConfigV1::decode(input)
}

fn check_scalars(product_width: u32, representation_width: u32, denominator: u64) -> Result<()> {
    if product_width == 0 || product_width > 512 {
        return Err(Error::InvalidOutcome);
    }
    if representation_width == 0 || representation_width > 256 {
        return Err(Error::InvalidOutcome);
    }
    if denominator <= 1 {
        return Err(Error::NonFractionalDenominator);
    }
    Ok(())
}

fn nonzero(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = array(input, offset)?;
    if value == [0; 32] {
        Err(Error::ZeroIdentity)
    } else {
        Ok(value)
    }
}

fn array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    input
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(input, offset)?))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(input, offset)?))
}

fn require_zero(input: &[u8], offset: usize, len: usize) -> Result<()> {
    if input
        .get(offset..offset.checked_add(len).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(Error::NonCanonical)
    } else {
        Ok(())
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}
