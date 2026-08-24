//! Provider-neutral compact terminal Product result.

use crate::{ContentId, Error, Result, array, byte, content_id, put, require_zero};

/// Exact byte width of [`TerminalResultV1`].
pub const TERMINAL_RESULT_BYTES: usize = 168;
/// Canonical terminal-result magic.
pub const TERMINAL_RESULT_MAGIC: [u8; 8] = *b"DCLTEND1";
/// Implemented terminal-result schema version.
pub const TERMINAL_RESULT_SCHEMA_VERSION: u16 = 1;

/// Provider-neutral reason a Product reached a terminal result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ResolutionKind {
    /// Normal occurrence resolution under committed Product semantics.
    Occurrence = 0,
    /// Committed failure semantics selected the terminal result.
    Failure = 1,
    /// Committed recovery semantics superseded a prior nonterminal attempt.
    Recovery = 2,
}

impl ResolutionKind {
    /// Decode the canonical provider-neutral resolution-route byte.
    pub const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Occurrence),
            1 => Ok(Self::Failure),
            2 => Ok(Self::Recovery),
            _ => Err(Error::UnknownResolutionKind),
        }
    }

    /// Return the canonical provider-neutral resolution-route byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::Occurrence => 0,
            Self::Failure => 1,
            Self::Recovery => 2,
        }
    }
}

/// Final payoff-state selector retained for redemption and replay checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalPayoff {
    /// One selected cell in a finite ordered Product partition.
    FiniteOutcome {
        /// Zero-based selected cell.
        selector: u32,
        /// Exact partition width against which the selector is checked.
        outcome_count: u32,
    },
    /// Content identity of a richer finite payoff-state artifact.
    PayoffState {
        /// Exact verified payoff-state content identity.
        payoff_state_id: ContentId,
    },
}

/// Inputs to a provider-neutral compact terminal result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalResultV1Input {
    /// Content identity of the immutable Market identity preimage.
    pub market_identity_id: ContentId,
    /// Content identity of the settled Product instance.
    pub product_instance_id: ContentId,
    /// Content identity of the accepted resolution receipt/evidence.
    pub resolution_evidence_id: ContentId,
    /// Provider-neutral terminal route.
    pub resolution_kind: ResolutionKind,
    /// Exact payoff state required for redemption.
    pub payoff: TerminalPayoff,
    /// Immutable Market generation which settled.
    pub settled_generation: u64,
    /// Monotone sequence under the Market's committed resolution policy.
    pub terminal_sequence: u64,
}

/// Same-PDA terminal summary retaining redemption, replay, and audit truth.
///
/// The receipt/evidence ID may commit provider-specific evidence elsewhere,
/// but no oracle, source account, transport, or incentive policy is inlined.
/// The Market identity and Product instance remain directly bound after any
/// larger live-state representation is compacted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalResultV1 {
    market_identity_id: ContentId,
    product_instance_id: ContentId,
    resolution_evidence_id: ContentId,
    resolution_kind: ResolutionKind,
    payoff: TerminalPayoff,
    settled_generation: u64,
    terminal_sequence: u64,
}

impl TerminalResultV1 {
    /// Construct one canonical terminal result.
    pub fn new(input: TerminalResultV1Input) -> Result<Self> {
        if let TerminalPayoff::FiniteOutcome {
            selector,
            outcome_count,
        } = input.payoff
            && (outcome_count < 2 || selector >= outcome_count)
        {
            return Err(Error::InvalidFiniteSelector);
        }
        Ok(Self {
            market_identity_id: input.market_identity_id,
            product_instance_id: input.product_instance_id,
            resolution_evidence_id: input.resolution_evidence_id,
            resolution_kind: input.resolution_kind,
            payoff: input.payoff,
            settled_generation: input.settled_generation,
            terminal_sequence: input.terminal_sequence,
        })
    }

    /// Decode one exact terminal-result record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != TERMINAL_RESULT_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != TERMINAL_RESULT_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array(bytes, 8)?) != TERMINAL_RESULT_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        let result_kind = byte(bytes, 10)?;
        let resolution_kind = ResolutionKind::decode(byte(bytes, 11)?)?;
        require_zero(bytes, 12, 4)?;
        let payoff_state_bytes = array::<32>(bytes, 80)?;
        let selector = u32::from_le_bytes(array(bytes, 160)?);
        let outcome_count = u32::from_le_bytes(array(bytes, 164)?);
        let payoff = match result_kind {
            1 => {
                if payoff_state_bytes.iter().any(|value| *value != 0) {
                    return Err(Error::NonCanonicalTerminalResult);
                }
                TerminalPayoff::FiniteOutcome {
                    selector,
                    outcome_count,
                }
            }
            2 => {
                if selector != 0 || outcome_count != 0 {
                    return Err(Error::NonCanonicalTerminalResult);
                }
                TerminalPayoff::PayoffState {
                    payoff_state_id: ContentId::new(payoff_state_bytes)?,
                }
            }
            _ => return Err(Error::UnknownTerminalResultKind),
        };
        Self::new(TerminalResultV1Input {
            market_identity_id: content_id(bytes, 16)?,
            product_instance_id: content_id(bytes, 48)?,
            resolution_evidence_id: content_id(bytes, 112)?,
            resolution_kind,
            payoff,
            settled_generation: u64::from_le_bytes(array(bytes, 144)?),
            terminal_sequence: u64::from_le_bytes(array(bytes, 152)?),
        })
    }

    /// Encode the exact terminal result.
    pub fn to_bytes(self) -> [u8; TERMINAL_RESULT_BYTES] {
        let mut output = [0; TERMINAL_RESULT_BYTES];
        put(&mut output, 0, &TERMINAL_RESULT_MAGIC);
        put(
            &mut output,
            8,
            &TERMINAL_RESULT_SCHEMA_VERSION.to_le_bytes(),
        );
        put(&mut output, 11, &[self.resolution_kind.byte()]);
        put(&mut output, 16, self.market_identity_id.as_bytes());
        put(&mut output, 48, self.product_instance_id.as_bytes());
        put(&mut output, 112, self.resolution_evidence_id.as_bytes());
        put(&mut output, 144, &self.settled_generation.to_le_bytes());
        put(&mut output, 152, &self.terminal_sequence.to_le_bytes());
        match self.payoff {
            TerminalPayoff::FiniteOutcome {
                selector,
                outcome_count,
            } => {
                put(&mut output, 10, &[1]);
                put(&mut output, 160, &selector.to_le_bytes());
                put(&mut output, 164, &outcome_count.to_le_bytes());
            }
            TerminalPayoff::PayoffState { payoff_state_id } => {
                put(&mut output, 10, &[2]);
                put(&mut output, 80, payoff_state_id.as_bytes());
            }
        }
        output
    }

    /// Return the immutable Market identity content ID.
    pub const fn market_identity_id(self) -> ContentId {
        self.market_identity_id
    }

    /// Return the settled Product-instance identity.
    pub const fn product_instance_id(self) -> ContentId {
        self.product_instance_id
    }

    /// Return the accepted resolution evidence identity.
    pub const fn resolution_evidence_id(self) -> ContentId {
        self.resolution_evidence_id
    }

    /// Return the provider-neutral terminal route.
    pub const fn resolution_kind(self) -> ResolutionKind {
        self.resolution_kind
    }

    /// Return the exact terminal payoff selector.
    pub const fn payoff(self) -> TerminalPayoff {
        self.payoff
    }

    /// Return the settled Market generation.
    pub const fn settled_generation(self) -> u64 {
        self.settled_generation
    }

    /// Return the monotone terminal sequence.
    pub const fn terminal_sequence(self) -> u64 {
        self.terminal_sequence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id;

    fn finite() -> TerminalResultV1 {
        TerminalResultV1::new(TerminalResultV1Input {
            market_identity_id: id(1),
            product_instance_id: id(2),
            resolution_evidence_id: id(3),
            resolution_kind: ResolutionKind::Occurrence,
            payoff: TerminalPayoff::FiniteOutcome {
                selector: 2,
                outcome_count: 4,
            },
            settled_generation: 9,
            terminal_sequence: 7,
        })
        .expect("valid result")
    }

    #[test]
    fn finite_result_has_exact_provider_neutral_encoding() {
        let value = finite();
        let bytes = value.to_bytes();
        assert_eq!(bytes.len(), TERMINAL_RESULT_BYTES);
        assert_eq!(bytes.get(10), Some(&1));
        assert_eq!(bytes.get(11), Some(&0));
        assert_eq!(bytes.get(80..112), Some([0u8; 32].as_slice()));
        assert_eq!(TerminalResultV1::decode(&bytes), Ok(value));
    }

    #[test]
    fn payoff_state_has_unique_canonical_form() {
        let value = TerminalResultV1::new(TerminalResultV1Input {
            market_identity_id: id(1),
            product_instance_id: id(2),
            resolution_evidence_id: id(3),
            resolution_kind: ResolutionKind::Recovery,
            payoff: TerminalPayoff::PayoffState {
                payoff_state_id: id(4),
            },
            settled_generation: 5,
            terminal_sequence: 2,
        })
        .expect("valid payoff-state result");
        let bytes = value.to_bytes();
        assert_eq!(TerminalResultV1::decode(&bytes), Ok(value));

        let mut noncanonical = bytes;
        noncanonical[160..164].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(
            TerminalResultV1::decode(&noncanonical),
            Err(Error::NonCanonicalTerminalResult)
        );
    }

    #[test]
    fn refuses_invalid_selector_unknown_kind_and_zero_evidence() {
        assert_eq!(
            TerminalResultV1::new(TerminalResultV1Input {
                market_identity_id: id(1),
                product_instance_id: id(2),
                resolution_evidence_id: id(3),
                resolution_kind: ResolutionKind::Failure,
                payoff: TerminalPayoff::FiniteOutcome {
                    selector: 4,
                    outcome_count: 4,
                },
                settled_generation: 0,
                terminal_sequence: 0,
            }),
            Err(Error::InvalidFiniteSelector)
        );

        let mut bytes = finite().to_bytes();
        bytes[10] = 9;
        assert_eq!(
            TerminalResultV1::decode(&bytes),
            Err(Error::UnknownTerminalResultKind)
        );

        let mut bytes = finite().to_bytes();
        bytes[112..144].fill(0);
        assert_eq!(TerminalResultV1::decode(&bytes), Err(Error::ZeroIdentifier));
    }
}
