#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Provider-neutral same-PDA terminal Market representation.
//!
//! The active profile-specific Market account may be compacted to this exact
//! record only after every liability and direct child is retired. The compact
//! record retains the universal Market root, Product result, replay authority,
//! and immutable rent-refund owner without retaining a Pyth policy, venue, or
//! active supply vector. A composing adapter owns hashing, same-PDA
//! reallocation, exact rent-delta refund, and rollback evidence.

use dclutch_core_contract::{MARKET_ROOT_BYTES, MarketRoot, Phase};
use dclutch_product_contract::terminal::{TERMINAL_RESULT_BYTES, TerminalResultV1};

/// Canonical compact terminal-Market magic.
pub const TERMINAL_MARKET_MAGIC: [u8; 8] = *b"DCLTTMN1";
/// Implemented compact terminal-Market schema.
pub const TERMINAL_MARKET_SCHEMA_VERSION: u16 = 1;
/// Exact compact terminal-Market byte width.
pub const TERMINAL_MARKET_BYTES: usize = 16 + MARKET_ROOT_BYTES + TERMINAL_RESULT_BYTES;

const RESERVED_OFFSET: usize = 10;
const RESERVED_BYTES: usize = 6;
const ROOT_OFFSET: usize = 16;
const RESULT_OFFSET: usize = ROOT_OFFSET + MARKET_ROOT_BYTES;

/// Explicit refusal returned by the compact terminal contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have the one exact canonical width.
    InvalidLength,
    /// Magic bytes did not name this contract.
    InvalidMagic,
    /// The schema version is not implemented.
    UnsupportedSchema,
    /// Reserved bytes were not zero.
    NonCanonicalReservedBytes,
    /// The embedded Market root was invalid.
    InvalidMarketRoot {
        /// Exact owning-contract refusal.
        error: dclutch_core_contract::Error,
    },
    /// The embedded Product terminal result was invalid.
    InvalidTerminalResult {
        /// Exact owning-contract refusal.
        error: dclutch_product_contract::Error,
    },
    /// The Market had not reached its final retired phase.
    MarketNotRetired,
    /// Terminal generation did not match immutable Market generation.
    GenerationMismatch,
    /// Terminal Product identity did not match immutable Market identity.
    ProductInstanceMismatch,
    /// The adapter-computed Market-identity content ID did not match the result.
    MarketIdentityMismatch,
    /// Output did not have the one exact canonical width.
    OutputLength,
}

/// Result alias for this contract.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact persistent state after profile-specific active state is reclaimed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalMarketV1 {
    root: MarketRoot,
    result: TerminalResultV1,
}

impl TerminalMarketV1 {
    /// Join a retired root to its provider-neutral Product result.
    pub fn new(root: MarketRoot, result: TerminalResultV1) -> Result<Self> {
        root.validate()
            .map_err(|error| Error::InvalidMarketRoot { error })?;
        if root.phase() != Phase::Retired || root.outstanding_children() != 0 {
            return Err(Error::MarketNotRetired);
        }
        if root.identity().generation() != result.settled_generation() {
            return Err(Error::GenerationMismatch);
        }
        if root.identity().product_instance_id().as_bytes()
            != result.product_instance_id().as_bytes()
        {
            return Err(Error::ProductInstanceMismatch);
        }
        Ok(Self { root, result })
    }

    /// Decode one exact terminal Market and recheck all locally owned links.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != TERMINAL_MARKET_BYTES {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != TERMINAL_MARKET_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(read_array(bytes, 8)?) != TERMINAL_MARKET_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if bytes
            .get(RESERVED_OFFSET..RESERVED_OFFSET + RESERVED_BYTES)
            .ok_or(Error::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let root = MarketRoot::decode(
            bytes
                .get(ROOT_OFFSET..RESULT_OFFSET)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|error| Error::InvalidMarketRoot { error })?;
        let result = TerminalResultV1::decode(
            bytes
                .get(RESULT_OFFSET..TERMINAL_MARKET_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .map_err(|error| Error::InvalidTerminalResult { error })?;
        Self::new(root, result)
    }

    /// Return exact canonical terminal bytes.
    pub fn to_bytes(self) -> [u8; TERMINAL_MARKET_BYTES] {
        let mut output = [0; TERMINAL_MARKET_BYTES];
        put(&mut output, 0, &TERMINAL_MARKET_MAGIC);
        put(
            &mut output,
            8,
            &TERMINAL_MARKET_SCHEMA_VERSION.to_le_bytes(),
        );
        put(&mut output, ROOT_OFFSET, &self.root.to_bytes());
        put(&mut output, RESULT_OFFSET, &self.result.to_bytes());
        output
    }

    /// Encode atomically into an exact caller-owned output buffer.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != TERMINAL_MARKET_BYTES {
            return Err(Error::OutputLength);
        }
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Recheck the Market-identity content ID computed by the adapter.
    pub fn validate_market_identity_id(&self, computed: [u8; 32]) -> Result<()> {
        if self.result.market_identity_id().as_bytes() != &computed {
            return Err(Error::MarketIdentityMismatch);
        }
        Ok(())
    }

    /// Return the retained universal root.
    pub const fn root(self) -> MarketRoot {
        self.root
    }

    /// Return the retained provider-neutral terminal result.
    pub const fn result(self) -> TerminalResultV1 {
        self.result
    }
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity};
    use dclutch_product_contract::{
        ContentId as ProductContentId,
        terminal::{ResolutionKind, TerminalPayoff, TerminalResultV1Input},
    };

    use super::*;

    fn core_id(value: u8) -> CoreContentId {
        CoreContentId::new([value; 32]).expect("nonzero core ID")
    }

    fn product_id(value: u8) -> ProductContentId {
        ProductContentId::new([value; 32]).expect("nonzero Product ID")
    }

    fn identity() -> MarketIdentity {
        MarketIdentity::new(
            core_id(1),
            core_id(2),
            core_id(3),
            core_id(4),
            core_id(5),
            9,
        )
    }

    fn retired_root() -> MarketRoot {
        let mut root = MarketRoot::founding(identity(), [6; 32]).expect("founding root");
        root.transition_phase(9, Phase::Open).expect("open");
        root.transition_phase(9, Phase::Resolved).expect("resolved");
        root.transition_phase(9, Phase::Retiring).expect("retiring");
        root.transition_phase(9, Phase::Retired).expect("retired");
        root
    }

    fn result() -> TerminalResultV1 {
        TerminalResultV1::new(TerminalResultV1Input {
            market_identity_id: product_id(7),
            product_instance_id: product_id(2),
            resolution_evidence_id: product_id(8),
            resolution_kind: ResolutionKind::Occurrence,
            payoff: TerminalPayoff::FiniteOutcome {
                selector: 1,
                outcome_count: 3,
            },
            settled_generation: 9,
            terminal_sequence: 1,
        })
        .expect("terminal result")
    }

    #[test]
    fn exact_compact_terminal_record_round_trips() {
        let terminal = TerminalMarketV1::new(retired_root(), result()).expect("terminal Market");
        let bytes = terminal.to_bytes();
        assert_eq!(TERMINAL_MARKET_BYTES, 416);
        assert_eq!(bytes.get(0..8), Some(&TERMINAL_MARKET_MAGIC[..]));
        assert_eq!(TerminalMarketV1::decode(&bytes), Ok(terminal));
        assert_eq!(terminal.root().rent_refund(), [6; 32]);
        assert_eq!(terminal.validate_market_identity_id([7; 32]), Ok(()));
        assert_eq!(
            terminal.validate_market_identity_id([9; 32]),
            Err(Error::MarketIdentityMismatch)
        );
    }

    #[test]
    fn hostile_phase_generation_product_and_envelope_refuse() {
        let founding = MarketRoot::founding(identity(), [6; 32]).expect("founding root");
        assert_eq!(
            TerminalMarketV1::new(founding, result()),
            Err(Error::MarketNotRetired)
        );

        let mut wrong_generation = result().to_bytes();
        wrong_generation
            .get_mut(144..152)
            .expect("generation field")
            .copy_from_slice(&10u64.to_le_bytes());
        let wrong_generation = TerminalResultV1::decode(&wrong_generation).expect("result");
        assert_eq!(
            TerminalMarketV1::new(retired_root(), wrong_generation),
            Err(Error::GenerationMismatch)
        );

        let mut wrong_product = result().to_bytes();
        wrong_product.get_mut(48..80).expect("Product ID").fill(9);
        let wrong_product = TerminalResultV1::decode(&wrong_product).expect("result");
        assert_eq!(
            TerminalMarketV1::new(retired_root(), wrong_product),
            Err(Error::ProductInstanceMismatch)
        );

        let canonical = TerminalMarketV1::new(retired_root(), result())
            .expect("terminal Market")
            .to_bytes();
        for length in 0..TERMINAL_MARKET_BYTES {
            assert_eq!(
                TerminalMarketV1::decode(canonical.get(..length).expect("prefix")),
                Err(Error::InvalidLength)
            );
        }
        for (offset, expected) in [
            (0, Error::InvalidMagic),
            (8, Error::UnsupportedSchema),
            (RESERVED_OFFSET, Error::NonCanonicalReservedBytes),
        ] {
            let mut changed = canonical;
            *changed.get_mut(offset).expect("field") ^= 1;
            assert_eq!(TerminalMarketV1::decode(&changed), Err(expected));
        }
    }
}
